//! What a modpack version contains: the mods, resource packs and shader packs its `modrinth.index.json` lists,
//! named with one bulk `projects` call (and one bulk `versions` call for their version numbers).

use std::collections::HashMap;
use std::path::Path;

use cubby_common::packs::{sort_contents, ContentSite, PackContentItem, PackContentKind};
use cubby_common::Result;

use crate::api;
use crate::client::Modrinth;
use crate::mrpack::MrpackIndex;

const CDN_DATA: &str = "https://cdn.modrinth.com/data/";

/// The project and version ids in a Modrinth CDN address
/// (`https://cdn.modrinth.com/data/<project>/versions/<version>/<file>`).
pub fn ids_from_cdn_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix(CDN_DATA)?;
    let mut parts = rest.split('/');
    let project = parts.next()?;
    if parts.next()? != "versions" {
        return None;
    }
    let version = parts.next()?;
    parts.next()?; // the file name
    let ok = |id: &str| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric());
    (ok(project) && ok(version)).then(|| (project.to_string(), version.to_string()))
}

/// The content files of a pack index, before the projects are looked up: named after their files, with the
/// Modrinth project (and version, in `version`) of the files hosted on Modrinth. Files the client doesn't
/// use, configs and other files outside `mods/`, `resourcepacks/` and `shaderpacks/` are left out.
pub fn index_contents(index: &MrpackIndex) -> Vec<PackContentItem> {
    let mut out = Vec::new();
    for file in &index.files {
        let client = file.env.as_ref().and_then(|e| e.client.as_deref());
        if client == Some("unsupported") {
            continue;
        }
        let Some(kind) = PackContentKind::of_path(&file.path) else { continue };
        let file_name = file.path.rsplit(['/', '\\']).next().unwrap_or(&file.path);
        let mut item = PackContentItem::from_file(file_name, kind);
        item.optional = client == Some("optional");
        if let Some((project, version)) = file.downloads.iter().find_map(|u| ids_from_cdn_url(u)) {
            item.site = Some(ContentSite::Modrinth);
            item.project_id = Some(project);
            item.version = Some(version);
        }
        out.push(item);
    }
    out
}

/// The owner of each team (else its first member), by team id.
pub(crate) fn team_owners(teams: &[Vec<api::TeamMember>]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for members in teams {
        let owner = members.iter().find(|m| m.is_owner == Some(true) || m.role.eq_ignore_ascii_case("owner")).or_else(|| members.iter().min_by_key(|m| m.ordering));
        if let Some(m) = owner.filter(|m| !m.team_id.is_empty() && !m.user.username.is_empty()) {
            out.insert(m.team_id.clone(), m.user.username.clone());
        }
    }
    out
}

/// Fills in the names, icons, pages, descriptions, owners, categories and version numbers of `items` (from
/// [`index_contents`]) from the projects, versions and team owners Modrinth knows. An item whose version isn't
/// known keeps no version number.
pub(crate) fn name_contents(items: &mut [PackContentItem], projects: &[api::Project], versions: &[api::Version], owners: &HashMap<String, String>) {
    let projects: HashMap<&str, &api::Project> = projects.iter().map(|p| (p.id.as_str(), p)).collect();
    let versions: HashMap<&str, &api::Version> = versions.iter().map(|v| (v.id.as_str(), v)).collect();
    for item in items.iter_mut().filter(|i| i.site == Some(ContentSite::Modrinth)) {
        let version_id = item.version.take();
        item.version = version_id.as_deref().and_then(|id| versions.get(id)).map(|v| v.version_number.trim().to_string()).filter(|v| !v.is_empty());
        let Some(project) = item.project_id.as_deref().and_then(|id| projects.get(id)) else { continue };
        if !project.title.trim().is_empty() {
            item.name = project.title.trim().to_string();
        }
        item.icon_url = project.icon_url.clone().filter(|u| !u.is_empty());
        item.description = Some(project.description.trim().to_string()).filter(|d| !d.is_empty());
        item.authors = owners.get(&project.team).cloned().into_iter().collect();
        item.categories = project.categories.clone();
        if !project.slug.is_empty() {
            let section = if project.project_type.is_empty() { "mod" } else { project.project_type.as_str() };
            item.page_url = Some(format!("https://modrinth.com/{section}/{}", project.slug));
        }
    }
}

impl Modrinth {
    /// The mods, resource packs and shader packs of the modpack version `version_id`. The `.mrpack` is
    /// downloaded into `cache_dir` to read its index, then deleted.
    pub async fn pack_contents(&self, version_id: &str, cache_dir: &Path) -> Result<Vec<PackContentItem>> {
        let pack = self.fetch_modpack(version_id, cache_dir).await?;
        if let Err(e) = std::fs::remove_file(&pack.archive) {
            tracing::debug!("could not remove {}: {e}", pack.archive.display());
        }
        let mut items = index_contents(&pack.index);
        let project_ids: Vec<String> = items.iter().filter_map(|i| i.project_id.clone()).collect();
        let version_ids: Vec<String> = items.iter().filter(|i| i.project_id.is_some()).filter_map(|i| i.version.clone()).collect();
        if !project_ids.is_empty() {
            let (projects, versions) = futures::try_join!(self.projects_raw(&project_ids), self.versions_raw(&version_ids))?;
            // the owners are cosmetic: a failed call leaves the items without
            let team_ids: Vec<String> = projects.iter().map(|p| p.team.clone()).filter(|t| !t.is_empty()).collect();
            let owners = match self.teams_raw(&team_ids).await {
                Ok(teams) => team_owners(&teams),
                Err(e) => {
                    tracing::debug!("no team owners for the pack contents: {e}");
                    HashMap::new()
                }
            };
            name_contents(&mut items, &projects, &versions, &owners);
        }
        sort_contents(&mut items);
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap()
    }

    #[test]
    fn cdn_urls() {
        assert_eq!(
            ids_from_cdn_url("https://cdn.modrinth.com/data/AANobbMI/versions/IZskON6d/sodium-fabric-0.5.8%2Bmc1.20.1.jar"),
            Some(("AANobbMI".to_string(), "IZskON6d".to_string()))
        );
        assert_eq!(ids_from_cdn_url("https://cdn.modrinth.com/data/AANobbMI/versions/IZskON6d/"), Some(("AANobbMI".into(), "IZskON6d".into())));
        assert_eq!(ids_from_cdn_url("https://cdn.modrinth.com/data/AANobbMI/images/x.png"), None);
        assert_eq!(ids_from_cdn_url("https://github.com/a/b/releases/download/1/x.jar"), None);
        assert_eq!(ids_from_cdn_url("https://cdn.modrinth.com/data/../versions/x/y.jar"), None);
    }

    #[test]
    fn index_lists_content_files() {
        let index = MrpackIndex::parse(&fixture("modrinth.index.json")).unwrap();
        let items = index_contents(&index);
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        // the server-only mod and the config file are left out
        assert_eq!(names, ["sodium-fabric-0.5.8+mc1.20.1", "iris-mc1.20.1-1.7.0", "custom-mod", "FreshAnimations_v1.9", "ComplementaryReimagined_r5.2"]);
        assert_eq!(items[0].site, Some(ContentSite::Modrinth));
        assert_eq!(items[0].project_id.as_deref(), Some("AANobbMI"));
        assert_eq!(items[0].version.as_deref(), Some("IZskON6d"));
        assert_eq!(items[0].kind, PackContentKind::Mod);
        assert!(items[1].optional);
        assert_eq!(items[2].site, None, "a file hosted elsewhere has no project");
        assert_eq!(items[3].kind, PackContentKind::ResourcePack);
        assert_eq!(items[4].kind, PackContentKind::Shader);
    }

    #[test]
    fn projects_name_the_files() {
        let index = MrpackIndex::parse(&fixture("modrinth.index.json")).unwrap();
        let mut items = index_contents(&index);
        let project = api::Project {
            id: "AANobbMI".into(),
            slug: "sodium".into(),
            project_type: "mod".into(),
            title: "Sodium".into(),
            icon_url: Some("https://cdn.modrinth.com/data/AANobbMI/icon.png".into()),
            team: "4reLOAKe".into(),
            description: "The fastest rendering optimization mod.".into(),
            categories: vec!["optimization".into()],
            ..Default::default()
        };
        let version = api::Version { id: "IZskON6d".into(), project_id: "AANobbMI".into(), version_number: "mc1.20.1-0.5.8".into(), ..Default::default() };
        let member = |name: &str, owner: bool, ordering: i64| api::TeamMember {
            team_id: "4reLOAKe".into(),
            user: api::User { id: name.into(), username: name.into() },
            role: if owner { "Owner".into() } else { "Member".into() },
            is_owner: Some(owner),
            ordering,
        };
        let owners = team_owners(&[vec![member("helper", false, 0), member("jellysquid3", true, 1)]]);
        name_contents(&mut items, &[project], &[version], &owners);
        assert_eq!(items[0].authors, ["jellysquid3"]);
        assert_eq!(items[0].description.as_deref(), Some("The fastest rendering optimization mod."));
        assert_eq!(items[0].categories, ["optimization"]);
        assert_eq!(items[0].name, "Sodium");
        assert_eq!(items[0].version.as_deref(), Some("mc1.20.1-0.5.8"));
        assert_eq!(items[0].page_url.as_deref(), Some("https://modrinth.com/mod/sodium"));
        assert!(items[0].icon_url.is_some());
        // unknown to Modrinth: keeps its file name, loses the version id
        assert_eq!(items[1].name, "iris-mc1.20.1-1.7.0");
        assert_eq!(items[1].version, None);
        sort_contents(&mut items);
        assert_eq!(items[0].name, "custom-mod");
    }
}

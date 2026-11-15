//! What a CurseForge modpack file contains. CurseForge has no endpoint for a pack's manifest, so the pack's
//! zip is downloaded to read `manifest.json` (then deleted); one bulk `/v1/mods/files` and one bulk `/v1/mods`
//! call name the files.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use cubby_common::net::{ensure_file, Download};
use cubby_common::packs::{sort_contents, ContentSite, PackContentItem, PackContentKind};
use cubby_common::{Error, Result};

use crate::api;
use crate::client::CurseForgeClient;
use crate::content::brief_from_mod;
use crate::manifest::{read_manifest, CfManifest};
use crate::plan::{unique, FileIds, ModIds, IDS_PER_REQUEST};

/// The kind of content a project class installs; `None` for classes that aren't mods, resource packs or
/// shader packs (worlds, datapacks…).
fn kind_of_class(class_id: Option<u64>) -> Option<PackContentKind> {
    match class_id {
        Some(12) => Some(PackContentKind::ResourcePack),
        Some(6552) => Some(PackContentKind::Shader),
        Some(17) | Some(6945) => None,
        _ => Some(PackContentKind::Mod),
    }
}

/// Modrinth-style keys for the CurseForge categories that say what kind of mod a project is.
fn category_keys(categories: &[api::Category]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in categories {
        let slug = c.slug.to_ascii_lowercase();
        let key = if slug.contains("library") || slug == "api" || slug.ends_with("-api") {
            "library".to_string()
        } else if slug.contains("performance") {
            "optimization".to_string()
        } else {
            slug
        };
        if !key.is_empty() && !out.contains(&key) {
            out.push(key);
        }
    }
    out
}

/// What the project says about itself: name (when `rename`), icon, page, summary, authors and categories.
fn describe(item: &mut PackContentItem, project: &api::Mod, rename: bool) {
    let brief = brief_from_mod(project);
    if rename {
        item.name = brief.name;
    }
    item.icon_url = brief.icon_url.or(item.icon_url.take());
    item.page_url = Some(brief.page_url);
    item.description = Some(project.summary.trim().to_string()).filter(|s| !s.is_empty());
    item.authors = project.authors.iter().map(|a| a.name.trim().to_string()).filter(|a| !a.is_empty()).collect();
    item.categories = category_keys(&project.categories);
}

/// The mods, resource packs and shader packs of a manifest, named from the API answers (`files` and
/// `projects`). A file the API no longer knows is still listed, by its project id.
pub fn manifest_contents(manifest: &CfManifest, files: &[api::File], projects: &[api::Mod]) -> Vec<PackContentItem> {
    let files: HashMap<u64, &api::File> = files.iter().map(|f| (f.id, f)).collect();
    let projects: HashMap<u64, &api::Mod> = projects.iter().map(|p| (p.id, p)).collect();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for entry in &manifest.files {
        if !seen.insert(entry.file_id) {
            continue;
        }
        let project = projects.get(&entry.project_id).copied();
        let Some(kind) = kind_of_class(project.and_then(|p| p.class_id)) else { continue };
        let file = files.get(&entry.file_id).copied();
        let mut item = match file.map(|f| f.file_name.trim()).filter(|n| !n.is_empty()) {
            Some(name) => PackContentItem::from_file(name, kind),
            None => PackContentItem { file_name: None, ..PackContentItem::from_file(&format!("Project {}", entry.project_id), kind) },
        };
        if let Some(display) = file.map(|f| f.display_name.trim()).filter(|n| !n.is_empty()) {
            item.name = display.to_string();
        }
        item.site = Some(ContentSite::Curseforge);
        item.project_id = Some(entry.project_id.to_string());
        item.optional = !entry.required;
        if let Some(project) = project {
            describe(&mut item, project, true);
            // the file's display name, when it says more than the project's name or the file name
            let stem = item.file_name.as_deref().map(|f| f.rsplit_once('.').map_or(f, |(s, _)| s)).unwrap_or_default();
            item.version = file
                .map(|f| f.display_name.trim().to_string())
                .filter(|d| !d.is_empty() && *d != item.name && Some(d.as_str()) != item.file_name.as_deref() && d != stem);
        } else {
            item.page_url = Some(format!("https://www.curseforge.com/projects/{}", entry.project_id));
        }
        out.push(item);
    }
    sort_contents(&mut out);
    out
}

impl CurseForgeClient {
    /// Describes the CurseForge projects in `items` (packs of other sources that name them by id) with one
    /// bulk call. `rename`: the items are named after their files, the project's name is better.
    pub async fn describe_contents(&self, items: &mut [PackContentItem], rename: bool) -> Result<()> {
        let ids: Vec<u64> = unique(items.iter().filter(|i| i.site == Some(ContentSite::Curseforge)).filter_map(|i| i.project_id.as_deref()?.parse().ok()));
        let mut projects = Vec::new();
        for chunk in ids.chunks(IDS_PER_REQUEST) {
            let response: api::Envelope<Vec<api::Mod>> = self.post("/v1/mods", &ModIds { mod_ids: chunk }, "the projects of the pack").await?;
            projects.extend(response.data);
        }
        describe_items(items, &projects, rename);
        Ok(())
    }

    /// The mods, resource packs and shader packs of the file `file_id` of the modpack `pack_id`. The pack's zip
    /// is downloaded into `cache_dir` to read its manifest, then deleted.
    pub async fn pack_contents(&self, pack_id: u64, file_id: u64, cache_dir: &Path) -> Result<Vec<PackContentItem>> {
        let file = self.file_raw(pack_id, file_id).await?;
        let url = file
            .download_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .ok_or_else(|| Error::Other("The author of this modpack doesn't let launchers download it, so Cubby can't read which mods it has.".into()))?
            .to_string();
        let archive = cache_dir.join(format!("contents-{pack_id}-{file_id}.zip"));
        let mut job = Download::new(url, &archive);
        if let Some(sha1) = file.sha1() {
            job = job.with_sha1(sha1);
        }
        if let Some(size) = file.file_length {
            job = job.with_size(size);
        }
        ensure_file(&self.http, &job).await?;
        let path = archive.clone();
        let manifest = tokio::task::spawn_blocking(move || read_manifest(&path)).await.map_err(|e| Error::Other(e.to_string()));
        if let Err(e) = std::fs::remove_file(&archive) {
            tracing::debug!("could not remove {}: {e}", archive.display());
        }
        let manifest = manifest??;

        let file_ids = unique(manifest.files.iter().map(|f| f.file_id));
        let mod_ids = unique(manifest.files.iter().map(|f| f.project_id));
        let mut files = Vec::new();
        for chunk in file_ids.chunks(IDS_PER_REQUEST) {
            let response: api::Envelope<Vec<api::File>> = self.post("/v1/mods/files", &FileIds { file_ids: chunk }, "the files of the pack").await?;
            files.extend(response.data);
        }
        let mut projects = Vec::new();
        for chunk in mod_ids.chunks(IDS_PER_REQUEST) {
            let response: api::Envelope<Vec<api::Mod>> = self.post("/v1/mods", &ModIds { mod_ids: chunk }, "the projects of the pack").await?;
            projects.extend(response.data);
        }
        Ok(manifest_contents(&manifest, &files, &projects))
    }
}

/// [`CurseForgeClient::describe_contents`] once the projects are fetched.
pub(crate) fn describe_items(items: &mut [PackContentItem], projects: &[api::Mod], rename: bool) {
    let by_id: HashMap<String, &api::Mod> = projects.iter().map(|p| (p.id.to_string(), p)).collect();
    for item in items.iter_mut().filter(|i| i.site == Some(ContentSite::Curseforge)) {
        if let Some(project) = item.project_id.as_ref().and_then(|id| by_id.get(id)) {
            describe(item, project, rename);
        }
    }
    sort_contents(items);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::parse_manifest;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap()
    }

    fn resolved() -> (CfManifest, Vec<api::File>, Vec<api::Mod>) {
        let manifest = parse_manifest(&fixture("manifest_forge.json")).unwrap();
        let files: api::Envelope<Vec<api::File>> = serde_json::from_slice(&fixture("files_resolve.json")).unwrap();
        let mods: api::Envelope<Vec<api::Mod>> = serde_json::from_slice(&fixture("mods_resolve.json")).unwrap();
        (manifest, files.data, mods.data)
    }

    #[test]
    fn lists_the_manifest_with_project_names() {
        let (manifest, files, mods) = resolved();
        let items = manifest_contents(&manifest, &files, &mods);
        let names: Vec<(&str, PackContentKind)> = items.iter().map(|i| (i.name.as_str(), i.kind)).collect();
        assert_eq!(
            names,
            [
                ("Blocked Mod", PackContentKind::Mod),
                ("CDN-able", PackContentKind::Mod),
                ("Just Enough Items", PackContentKind::Mod),
                ("Optional extra", PackContentKind::Mod),
                ("Faithful", PackContentKind::ResourcePack),
                ("Complementary", PackContentKind::Shader),
            ]
        );
        let jei = &items[2];
        assert_eq!(jei.site, Some(ContentSite::Curseforge));
        assert_eq!(jei.project_id.as_deref(), Some("1001"));
        assert_eq!(jei.file_name.as_deref(), Some("jei-1.20.1-forge-15.3.0.4.jar"));
        assert_eq!(jei.version.as_deref(), Some("JEI 15.3.0.4"));
        assert!(jei.page_url.as_deref().unwrap().contains("jei"));
        assert!(!jei.optional);
        assert_eq!(jei.authors, mods.iter().find(|m| m.id == 1001).unwrap().authors.iter().map(|a| a.name.clone()).collect::<Vec<_>>());
        assert!(items[3].optional, "required: false in the manifest");
        assert_eq!(items[3].version, None, "a display name equal to the project's name says nothing more");
        let mut files = files;
        files[0].display_name = "jei-1.20.1-forge-15.3.0.4.jar".into();
        let items = manifest_contents(&manifest, &files, &mods);
        assert_eq!(items[2].version, None, "nor does one equal to the file name");
    }

    #[test]
    fn describes_projects_named_by_id() {
        let (_, _, mut mods) = resolved();
        mods[0].summary = "Shows recipes.".into();
        mods[0].categories = vec![api::Category { slug: "api-and-library".into(), ..Default::default() }, api::Category { slug: "performance".into(), ..Default::default() }];
        let mut items = vec![
            PackContentItem { site: Some(ContentSite::Curseforge), project_id: Some("1001".into()), ..PackContentItem::from_file("jei-15.jar", PackContentKind::Mod) },
            PackContentItem::from_file("other.jar", PackContentKind::Mod),
        ];
        describe_items(&mut items, &mods, true);
        let jei = items.iter().find(|i| i.project_id.is_some()).unwrap();
        assert_eq!(jei.name, "Just Enough Items");
        assert_eq!(jei.file_name.as_deref(), Some("jei-15.jar"));
        assert_eq!(jei.description.as_deref(), Some("Shows recipes."));
        assert_eq!(jei.categories, ["library", "optimization"]);
        let other = items.iter().find(|i| i.project_id.is_none()).unwrap();
        assert_eq!(other.name, "other");
    }

    #[test]
    fn unknown_files_and_projects_stay_listed() {
        let (manifest, _, _) = resolved();
        let items = manifest_contents(&manifest, &[], &[]);
        assert_eq!(items.len(), manifest.files.len());
        assert!(items.iter().all(|i| i.kind == PackContentKind::Mod && i.page_url.as_deref().unwrap().starts_with("https://www.curseforge.com/projects/")));
        assert!(items.iter().any(|i| i.name == "Project 1001"));
    }
}

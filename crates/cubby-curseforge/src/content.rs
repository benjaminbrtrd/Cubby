//! Mods, resource packs, shader packs and datapacks installed one at a time from CurseForge: the files of a
//! project, the best one for an instance, and the required projects it pulls in.
//!
//! A file without a `downloadUrl` is blocked by its author for third-party launchers: the player downloads it
//! from [`CfProjectBrief::download_page`].

use std::collections::HashSet;

use serde::Serialize;
use cubby_common::packs::{PackChannel, PackLoaderKind};
use cubby_common::{Error, Result};

use crate::api;
use crate::browse::{loader_from_name, sorted_versions};
use crate::client::CurseForgeClient;

const SITE: &str = "https://www.curseforge.com/minecraft";
const PAGE_SIZE: u64 = 50;
/// Pages of `/files` read for one Minecraft version: the best file is among the newest.
const VERSION_PAGES: u64 = 4;
/// Required projects followed, at most (a guard against odd dependency graphs).
const MAX_DEPENDENCIES: usize = 60;
const MAX_DEPTH: usize = 6;
/// `relationType` of a required dependency.
const REQUIRED: u8 = 3;

/// A file of a project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfFile {
    pub id: u64,
    pub project_id: u64,
    /// The display name, else the file name.
    pub name: String,
    pub file_name: String,
    pub channel: PackChannel,
    /// Minecraft releases, newest first.
    pub minecraft: Vec<String>,
    pub loaders: Vec<PackLoaderKind>,
    pub released: Option<String>,
    pub size: Option<u64>,
    pub downloads: u64,
    /// Lowercase hex.
    pub sha1: Option<String>,
    /// `None`: the author doesn't let launchers download it.
    pub download_url: Option<String>,
    /// Projects this file requires.
    pub required: Vec<u64>,
}

impl From<&api::File> for CfFile {
    fn from(f: &api::File) -> Self {
        let mut loaders: Vec<PackLoaderKind> = Vec::new();
        for l in f.game_versions.iter().filter_map(|v| loader_from_name(v)) {
            if !loaders.contains(&l) {
                loaders.push(l);
            }
        }
        let mut required: Vec<u64> = Vec::new();
        for d in f.dependencies.iter().filter(|d| d.relation_type == REQUIRED && d.mod_id != 0 && d.mod_id != f.mod_id) {
            if !required.contains(&d.mod_id) {
                required.push(d.mod_id);
            }
        }
        CfFile {
            id: f.id,
            project_id: f.mod_id,
            name: if f.display_name.trim().is_empty() { f.file_name.clone() } else { f.display_name.trim().to_string() },
            file_name: f.file_name.clone(),
            channel: match f.release_type {
                2 => PackChannel::Beta,
                3 => PackChannel::Alpha,
                _ => PackChannel::Release,
            },
            minecraft: sorted_versions(f.game_versions.iter().cloned()),
            loaders,
            released: f.file_date.clone(),
            size: f.file_length,
            downloads: f.download_count.max(0.0) as u64,
            sha1: f.sha1(),
            download_url: f.download_url.as_deref().map(str::trim).filter(|u| !u.is_empty()).map(str::to_string),
            required,
        }
    }
}

/// What a project is, for an install plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfProjectBrief {
    pub id: u64,
    pub name: String,
    pub slug: String,
    pub class_id: Option<u64>,
    pub icon_url: Option<String>,
    /// The project's page on curseforge.com.
    pub page_url: String,
}

impl CfProjectBrief {
    /// Where the player downloads `file_id` by hand.
    pub fn download_page(&self, file_id: u64) -> String {
        format!("{}/download/{file_id}", self.page_url.trim_end_matches('/'))
    }
}

/// Section of the website for a class of projects.
fn class_section(class_id: Option<u64>) -> &'static str {
    match class_id {
        Some(12) => "texture-packs",
        Some(6552) => "shaders",
        Some(6945) => "data-packs",
        Some(4471) => "modpacks",
        Some(17) => "worlds",
        _ => "mc-mods",
    }
}

pub(crate) fn brief_from_mod(m: &api::Mod) -> CfProjectBrief {
    let page_url = match m.links.website_url.as_deref().map(|u| u.trim_end_matches('/')).filter(|u| !u.is_empty()) {
        Some(url) => url.to_string(),
        None if !m.slug.is_empty() => format!("{SITE}/{}/{}", class_section(m.class_id), m.slug),
        None => format!("https://www.curseforge.com/projects/{}", m.id),
    };
    CfProjectBrief {
        id: m.id,
        name: if m.name.trim().is_empty() { format!("Project {}", m.id) } else { m.name.trim().to_string() },
        slug: m.slug.clone(),
        class_id: m.class_id,
        icon_url: m.logo.as_ref().and_then(|l| l.thumbnail_url.clone().or_else(|| l.url.clone())).filter(|u| !u.is_empty()),
        page_url,
    }
}

/// The Minecraft version and loaders a file must suit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileTarget {
    pub game_version: Option<String>,
    /// Loaders whose mods run on the instance (a Quilt instance also runs Fabric mods); empty = any.
    pub loaders: Vec<PackLoaderKind>,
}

impl FileTarget {
    /// The file lists the Minecraft version, and one of the loaders when it names loaders at all (resource
    /// packs, shader packs and datapacks name none).
    pub fn fits(&self, file: &CfFile) -> bool {
        let version = self.game_version.as_deref().is_none_or(|v| file.minecraft.iter().any(|m| m == v));
        let loader = self.loaders.is_empty() || file.loaders.is_empty() || file.loaders.iter().any(|l| self.loaders.contains(l));
        version && loader
    }
}

fn channel_rank(c: PackChannel) -> u8 {
    match c {
        PackChannel::Release => 0,
        PackChannel::Beta => 1,
        PackChannel::Alpha => 2,
    }
}

/// The file to install for `target`: the newest release that fits, else the newest beta, else the newest alpha.
pub fn best_file<'a>(files: &'a [CfFile], target: &FileTarget) -> Option<&'a CfFile> {
    files
        .iter()
        .filter(|f| target.fits(f))
        .min_by(|a, b| channel_rank(a.channel).cmp(&channel_rank(b.channel)).then_with(|| b.released.cmp(&a.released)).then_with(|| b.id.cmp(&a.id)))
}

/// A project of an install plan and the file chosen for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub project: CfProjectBrief,
    /// `None`: no file suits the target (a missing dependency).
    pub file: Option<CfFile>,
    /// Pulled in by another project.
    pub dependency: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModIds<'a> {
    mod_ids: &'a [u64],
}

fn newest_first(files: &mut [CfFile]) {
    files.sort_by(|a, b| b.released.cmp(&a.released).then(b.id.cmp(&a.id)));
}

impl CurseForgeClient {
    /// Every file of a project, newest first.
    pub async fn project_files(&self, id: u64) -> Result<Vec<CfFile>> {
        let files = self.files_raw(id).await?;
        Ok(files.iter().map(CfFile::from).collect())
    }

    /// The newest files of a project for a Minecraft version (all versions when `None`), newest first.
    pub async fn files_for_version(&self, id: u64, game_version: Option<&str>) -> Result<Vec<CfFile>> {
        let mut out: Vec<CfFile> = Vec::new();
        for page in 0..VERSION_PAGES {
            let mut query = vec![("index", (page * PAGE_SIZE).to_string()), ("pageSize", PAGE_SIZE.to_string())];
            if let Some(v) = game_version.map(str::trim).filter(|v| !v.is_empty()) {
                query.push(("gameVersion", v.to_string()));
            }
            let response: api::Envelope<Vec<api::File>> = self.get(&format!("/v1/mods/{id}/files"), &query, &format!("files of CurseForge project {id}")).await?;
            let got = response.data.len() as u64;
            out.extend(response.data.iter().map(CfFile::from));
            let total = response.pagination.map(|p| p.total_count).unwrap_or(0);
            if got < PAGE_SIZE || out.len() as u64 >= total {
                break;
            }
        }
        newest_first(&mut out);
        Ok(out)
    }

    /// One file of a project.
    pub async fn file(&self, project_id: u64, file_id: u64) -> Result<CfFile> {
        Ok(CfFile::from(&self.file_raw(project_id, file_id).await?))
    }

    /// The projects with these ids that CurseForge knows, in no particular order.
    pub async fn projects(&self, ids: &[u64]) -> Result<Vec<CfProjectBrief>> {
        let mut out = Vec::new();
        for chunk in ids.chunks(500) {
            let response: api::Envelope<Vec<api::Mod>> = self.post("/v1/mods", &ModIds { mod_ids: chunk }, "CurseForge projects").await?;
            out.extend(response.data.iter().map(brief_from_mod));
        }
        Ok(out)
    }

    /// The project with the file to install (`file_id`, else the best one for `target`), then the projects
    /// its file requires, level by level. `skip` leaves out a required project (already in the instance) and
    /// what it would require in turn. The requested project comes back even without a suitable file.
    pub async fn resolve(&self, project_id: u64, file_id: Option<u64>, target: &FileTarget, skip: impl Fn(&CfProjectBrief) -> bool) -> Result<Vec<Resolved>> {
        let project = self.projects(&[project_id]).await?.into_iter().next().ok_or_else(|| Error::NotFound(format!("CurseForge project {project_id}")))?;
        let file = match file_id {
            Some(id) => Some(self.file(project_id, id).await?),
            None => best_file(&self.files_for_version(project_id, target.game_version.as_deref()).await?, target).cloned(),
        };
        let mut next: Vec<u64> = file.as_ref().map(|f| f.required.clone()).unwrap_or_default();
        let mut out = vec![Resolved { project, file, dependency: false }];
        let mut seen: HashSet<u64> = HashSet::from([project_id]);

        for _ in 0..MAX_DEPTH {
            let ids: Vec<u64> = next.drain(..).filter(|id| seen.insert(*id)).collect();
            if ids.is_empty() || out.len() > MAX_DEPENDENCIES {
                break;
            }
            let briefs = self.projects(&ids).await?;
            for id in ids {
                let brief = briefs.iter().find(|b| b.id == id).cloned().unwrap_or_else(|| CfProjectBrief {
                    id,
                    name: format!("Project {id}"),
                    slug: String::new(),
                    class_id: None,
                    icon_url: None,
                    page_url: format!("https://www.curseforge.com/projects/{id}"),
                });
                if skip(&brief) {
                    continue;
                }
                let file = match self.files_for_version(id, target.game_version.as_deref()).await {
                    Ok(files) => best_file(&files, target).cloned(),
                    Err(Error::NotFound(_)) => None,
                    Err(e) => return Err(e),
                };
                if let Some(f) = &file {
                    next.extend(f.required.iter().copied());
                }
                out.push(Resolved { project: brief, file, dependency: true });
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: u64, channel: PackChannel, released: &str, mc: &[&str], loaders: &[PackLoaderKind]) -> CfFile {
        CfFile {
            id,
            project_id: 1,
            name: format!("f{id}"),
            file_name: format!("f{id}.jar"),
            channel,
            minecraft: mc.iter().map(|s| s.to_string()).collect(),
            loaders: loaders.to_vec(),
            released: Some(released.to_string()),
            size: Some(10),
            downloads: 0,
            sha1: None,
            download_url: None,
            required: Vec::new(),
        }
    }

    #[test]
    fn best_file_prefers_newest_release_that_fits() {
        use PackChannel::*;
        use PackLoaderKind::*;
        let files = vec![
            file(5, Beta, "2026-05-01", &["1.21.1"], &[Fabric]),
            file(4, Release, "2026-04-01", &["1.21.1"], &[NeoForge]),
            file(3, Release, "2026-03-01", &["1.21.1"], &[Fabric]),
            file(2, Release, "2026-02-01", &["1.21.1"], &[Fabric]),
            file(1, Release, "2026-06-01", &["1.20.1"], &[Fabric]),
        ];
        let target = FileTarget { game_version: Some("1.21.1".into()), loaders: vec![Fabric] };
        assert_eq!(best_file(&files, &target).map(|f| f.id), Some(3));
        // a Quilt instance runs Fabric mods
        let quilt = FileTarget { game_version: Some("1.21.1".into()), loaders: vec![Quilt, Fabric] };
        assert_eq!(best_file(&files, &quilt).map(|f| f.id), Some(3));
        // only a beta fits
        let only_beta = vec![files[0].clone(), files[1].clone()];
        assert_eq!(best_file(&only_beta, &target).map(|f| f.id), Some(5));
        // nothing for this version
        let old = FileTarget { game_version: Some("1.12.2".into()), loaders: vec![Fabric] };
        assert!(best_file(&files, &old).is_none());
        // packs name no loader: any loader target accepts them
        let pack = vec![file(9, Release, "2026-01-01", &["1.21.1"], &[])];
        assert_eq!(best_file(&pack, &target).map(|f| f.id), Some(9));
    }

    #[test]
    fn file_maps_from_the_api() {
        let raw = api::File {
            id: 77,
            mod_id: 10,
            display_name: " ".into(),
            file_name: "thing-1.0.jar".into(),
            release_type: 2,
            hashes: vec![api::Hash { value: "ABC".into(), algo: 1 }],
            file_date: Some("2026-01-01".into()),
            file_length: Some(42),
            download_url: Some("".into()),
            game_versions: vec!["1.21.1".into(), "Fabric".into(), "Client".into(), "Quilt".into(), "1.21".into()],
            download_count: 5.0,
            dependencies: vec![
                api::FileDependency { mod_id: 306612, relation_type: 3 },
                api::FileDependency { mod_id: 306612, relation_type: 3 },
                api::FileDependency { mod_id: 5, relation_type: 2 },
                api::FileDependency { mod_id: 10, relation_type: 3 },
            ],
        };
        let f = CfFile::from(&raw);
        assert_eq!(f.name, "thing-1.0.jar");
        assert_eq!(f.channel, PackChannel::Beta);
        assert_eq!(f.minecraft, vec!["1.21.1".to_string(), "1.21".to_string()]);
        assert_eq!(f.loaders, vec![PackLoaderKind::Fabric, PackLoaderKind::Quilt]);
        assert_eq!(f.sha1.as_deref(), Some("abc"));
        // an empty URL is a blocked file
        assert_eq!(f.download_url, None);
        // only required projects, once, never itself
        assert_eq!(f.required, vec![306612]);
    }

    #[test]
    fn brief_has_a_page() {
        let m = api::Mod { id: 3, name: "Jade".into(), slug: "jade".into(), class_id: Some(6), ..Default::default() };
        let b = brief_from_mod(&m);
        assert_eq!(b.page_url, "https://www.curseforge.com/minecraft/mc-mods/jade");
        assert_eq!(b.download_page(99), "https://www.curseforge.com/minecraft/mc-mods/jade/download/99");
        let m = api::Mod { id: 4, slug: "x".into(), class_id: Some(6945), ..Default::default() };
        assert_eq!(brief_from_mod(&m).page_url, "https://www.curseforge.com/minecraft/data-packs/x");
    }
}

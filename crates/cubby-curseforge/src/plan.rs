//! Turning a CurseForge modpack into a [`PackPlan`], following Prism Launcher's `FlameInstanceCreationTask`.
//!
//! A file with an empty `downloadUrl` is blocked: the player gets it from `{project website}/download/{fileId}`.
//! Optional files are installed with a `.disabled` suffix. [`CurseForgeClient::with_cdn_fallback`] guesses an
//! `edge.forgecdn.net` URL instead, unless the author forbids distribution.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;
use cubby_common::net::{ensure_file, Download};
use cubby_common::packs::{safe_relative_path, BlockedFile, PackFile, PackOverrides, PackPlan};
use cubby_common::{Error, Result};

use crate::api;
use crate::client::CurseForgeClient;
use crate::manifest::{read_manifest, zip_has_folder, CfManifest};

const IDS_PER_REQUEST: usize = 500;
const SITE: &str = "https://www.curseforge.com/minecraft";

/// Game folder where a project of this class goes, and the class slug used on the website.
fn class_target(class_id: Option<u64>) -> (&'static str, &'static str) {
    match class_id {
        Some(12) => ("resourcepacks", "texture-packs"),
        Some(6552) => ("shaderpacks", "shaders"),
        // Worlds are zips: they land in saves/ and the player opens or extracts them from there.
        Some(17) => ("saves", "worlds"),
        Some(6945) => ("datapacks", "data-packs"),
        _ => ("mods", "mc-mods"),
    }
}

/// `https://edge.forgecdn.net/files/{id / 1000}/{id % 1000}/{fileName}`.
pub(crate) fn cdn_url(file_id: u64, file_name: &str) -> String {
    format!("https://edge.forgecdn.net/files/{}/{}/{}", file_id / 1000, file_id % 1000, percent_encode(file_name))
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Page where the player downloads a blocked file: `{project page}/download/{fileId}`.
pub(crate) fn blocked_page_url(project: Option<&api::Mod>, project_id: u64, file_id: u64) -> String {
    let page = match project {
        Some(p) => match p.links.website_url.as_deref().map(|u| u.trim_end_matches('/')).filter(|u| !u.is_empty()) {
            Some(url) => url.to_string(),
            None if !p.slug.is_empty() => format!("{SITE}/{}/{}", class_target(p.class_id).1, p.slug),
            None => format!("https://www.curseforge.com/projects/{project_id}"),
        },
        None => format!("https://www.curseforge.com/projects/{project_id}"),
    };
    format!("{page}/download/{file_id}")
}

/// A single path segment, checked.
fn safe_file_name(name: &str) -> Option<String> {
    let path = safe_relative_path(name)?;
    let mut parts = path.components();
    let first = parts.next()?;
    parts.next().is_none().then(|| first.as_os_str().to_string_lossy().into_owned())
}

/// Maps the manifest and the API answers to a plan. `files` and `projects` are what the two bulk calls
/// returned; a manifest file the API doesn't know is an error rather than a silently incomplete pack.
pub(crate) fn build_plan(manifest: &CfManifest, files: &[api::File], projects: &[api::Mod], archive: &Path, has_overrides: bool, cdn_fallback: bool) -> Result<PackPlan> {
    let files_by_id: HashMap<u64, &api::File> = files.iter().map(|f| (f.id, f)).collect();
    let projects_by_id: HashMap<u64, &api::Mod> = projects.iter().map(|p| (p.id, p)).collect();

    let missing: Vec<String> =
        manifest.files.iter().filter(|f| !files_by_id.contains_key(&f.file_id)).map(|f| format!("{}/{}", f.project_id, f.file_id)).collect();
    if !missing.is_empty() {
        return Err(Error::Other(format!(
            "{} file(s) of this pack no longer exist on CurseForge (project/file: {}). Ask the pack's author for an update.",
            missing.len(),
            missing.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
        )));
    }

    let mut plan_files = Vec::new();
    let mut blocked = Vec::new();
    let mut seen = HashSet::new();
    for entry in &manifest.files {
        if !seen.insert(entry.file_id) {
            continue;
        }
        let file = files_by_id[&entry.file_id];
        let project = projects_by_id.get(&entry.project_id).or_else(|| projects_by_id.get(&file.mod_id)).copied();
        let name = safe_file_name(&file.file_name).ok_or_else(|| Error::Other(format!("The pack contains a file with an unsafe name: \"{}\".", file.file_name)))?;
        let (folder, _) = class_target(project.and_then(|p| p.class_id));
        let mut dest = name.clone();
        if !entry.required {
            dest.push_str(".disabled");
        }
        let path = PathBuf::from(folder).join(&dest);

        let direct = file.download_url.as_deref().map(str::trim).filter(|u| !u.is_empty()).map(str::to_string);
        let forbidden = project.is_some_and(|p| p.allow_mod_distribution == Some(false));
        let url = direct.or_else(|| (cdn_fallback && !forbidden).then(|| cdn_url(file.id, &file.file_name)));
        match url {
            Some(url) => plan_files.push(PackFile { path, urls: vec![url], sha1: file.sha1(), size: file.file_length }),
            None => blocked.push(BlockedFile {
                file_name: name,
                page_url: blocked_page_url(project, entry.project_id, entry.file_id),
                path,
                sha1: file.sha1(),
                size: file.file_length,
            }),
        }
    }

    let overrides = if has_overrides { vec![PackOverrides { archive: archive.to_path_buf(), prefix: manifest.overrides_prefix() }] } else { Vec::new() };
    let title = if manifest.name.trim().is_empty() { "CurseForge modpack".to_string() } else { manifest.name.trim().to_string() };
    Ok(PackPlan {
        provider: "curseforge".into(),
        pack_id: title.clone(),
        version_id: manifest.version.trim().to_string(),
        title,
        version_name: manifest.version.trim().to_string(),
        minecraft: manifest.minecraft.version.clone(),
        loader: manifest.loader()?,
        icon_url: None,
        files: plan_files,
        overrides,
        blocked,
        recommended_memory_mib: manifest.minecraft.recommended_ram.filter(|m| *m > 0),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileIds<'a> {
    file_ids: &'a [u64],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModIds<'a> {
    mod_ids: &'a [u64],
}

impl CurseForgeClient {
    /// Downloads the file `file_id` of the modpack `pack_id` into `cache_dir` and plans it. The zip stays
    /// in `cache_dir` (the plan's overrides point at it): the app removes it after installing.
    pub async fn plan(&self, pack_id: u64, file_id: u64, cache_dir: &Path) -> Result<PackPlan> {
        let file = self.file_raw(pack_id, file_id).await?;
        let url = match file.download_url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
            Some(url) => url.to_string(),
            None => {
                return Err(Error::Other(format!(
                    "The author of this modpack doesn't allow launchers to download it. Download it from {} and import the .zip instead.",
                    blocked_page_url(None, pack_id, file_id)
                )))
            }
        };
        let archive = cache_dir.join(format!("curseforge-{pack_id}-{file_id}.zip"));
        let mut job = Download::new(url, &archive);
        if let Some(sha1) = file.sha1() {
            job = job.with_sha1(sha1);
        }
        if let Some(size) = file.file_length {
            job = job.with_size(size);
        }
        ensure_file(&self.http, &job).await?;

        let mut plan = self.plan_from_zip(&archive).await?;
        plan.pack_id = pack_id.to_string();
        plan.version_id = file_id.to_string();
        if !file.display_name.trim().is_empty() {
            plan.version_name = file.display_name.trim().to_string();
        }
        // The page (name, icon) is cosmetic: a failure doesn't stop the install.
        match self.mod_raw(pack_id).await {
            Ok(project) => {
                if !project.name.trim().is_empty() {
                    plan.title = project.name.trim().to_string();
                }
                plan.icon_url = project.logo.and_then(|l| l.thumbnail_url.or(l.url)).filter(|u| !u.is_empty());
            }
            Err(error) => tracing::debug!("no project page for CurseForge modpack {pack_id}: {error}"),
        }
        Ok(plan)
    }

    /// Plans a CurseForge modpack `.zip` (the "import" of the app, and what [`Self::plan`] runs on its
    /// download). The archive isn't copied or modified: `PackPlan::overrides[..].archive` is `archive`
    /// itself, so for an imported file the app must not delete it after installing.
    pub async fn plan_from_zip(&self, archive: &Path) -> Result<PackPlan> {
        let path = archive.to_path_buf();
        let (manifest, has_overrides) = tokio::task::spawn_blocking(move || {
            let manifest = read_manifest(&path)?;
            let has_overrides = zip_has_folder(&path, &manifest.overrides_prefix())?;
            Ok::<_, Error>((manifest, has_overrides))
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))??;
        // Fail on an unsupported loader before spending API calls.
        manifest.loader()?;

        let file_ids: Vec<u64> = unique(manifest.files.iter().map(|f| f.file_id));
        let mod_ids: Vec<u64> = unique(manifest.files.iter().map(|f| f.project_id));
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
        build_plan(&manifest, &files, &projects, archive, has_overrides, self.cdn_fallback)
    }
}

fn unique(ids: impl Iterator<Item = u64>) -> Vec<u64> {
    let mut seen = HashSet::new();
    ids.filter(|id| seen.insert(*id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::parse_manifest;
    use cubby_common::packs::PackLoaderKind;

    fn fixture_bytes(name: &str) -> Vec<u8> {
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap()
    }

    fn resolved() -> (CfManifest, Vec<api::File>, Vec<api::Mod>) {
        let manifest = parse_manifest(&fixture_bytes("manifest_forge.json")).unwrap();
        let files: api::Envelope<Vec<api::File>> = serde_json::from_slice(&fixture_bytes("files_resolve.json")).unwrap();
        let mods: api::Envelope<Vec<api::Mod>> = serde_json::from_slice(&fixture_bytes("mods_resolve.json")).unwrap();
        (manifest, files.data, mods.data)
    }

    #[test]
    fn plan_matches_prism_behaviour() {
        let (manifest, files, mods) = resolved();
        let plan = build_plan(&manifest, &files, &mods, Path::new("pack.zip"), true, false).unwrap();
        assert_eq!(plan.provider, "curseforge");
        assert_eq!(plan.title, "Example Forge Pack");
        assert_eq!(plan.version_name, "1.2.0");
        assert_eq!(plan.minecraft, "1.20.1");
        assert_eq!(plan.loader, Some((PackLoaderKind::Forge, "47.2.0".into())));
        assert_eq!(plan.recommended_memory_mib, Some(6144));

        let paths: Vec<PathBuf> = plan.files.iter().map(|f| f.path.clone()).collect();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("mods").join("jei-1.20.1-forge-15.3.0.4.jar"),
                PathBuf::from("resourcepacks").join("Faithful 32x.zip"),
                PathBuf::from("mods").join("optional-extra.jar.disabled"),
                PathBuf::from("shaderpacks").join("Complementary.zip"),
            ]
        );
        let jei = &plan.files[0];
        assert_eq!(jei.urls, vec!["https://edge.forgecdn.net/files/4712/3456/jei-1.20.1-forge-15.3.0.4.jar".to_string()]);
        assert_eq!(jei.sha1.as_deref(), Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert_eq!(jei.size, Some(1_048_576));

        // null downloadUrl: both the "forbidden" and the "allowed" one are blocked without the CDN fallback
        assert_eq!(plan.blocked.len(), 2);
        let b = &plan.blocked[0];
        assert_eq!(b.file_name, "blocked-mod-1.0.jar");
        assert_eq!(b.page_url, "https://www.curseforge.com/minecraft/mc-mods/blocked-mod/download/2002");
        assert_eq!(b.path, PathBuf::from("mods").join("blocked-mod-1.0.jar"));
        assert_eq!(b.sha1.as_deref(), Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        assert_eq!(plan.blocked[1].file_name, "cdn-able.jar");

        assert_eq!(plan.overrides, vec![PackOverrides { archive: PathBuf::from("pack.zip"), prefix: "overrides/".into() }]);
    }

    #[test]
    fn cdn_fallback_skips_forbidden_authors() {
        let (manifest, files, mods) = resolved();
        let plan = build_plan(&manifest, &files, &mods, Path::new("pack.zip"), false, true).unwrap();
        assert!(plan.overrides.is_empty());
        assert_eq!(plan.blocked.len(), 1);
        assert_eq!(plan.blocked[0].file_name, "blocked-mod-1.0.jar");
        let cdn = plan.files.iter().find(|f| f.path.ends_with("cdn-able.jar")).expect("cdn-able in files");
        assert_eq!(cdn.urls, vec!["https://edge.forgecdn.net/files/4500/12/cdn-able.jar".to_string()]);
    }

    #[test]
    fn cdn_url_encodes_names() {
        assert_eq!(cdn_url(1234567, "My Mod+1.jar"), "https://edge.forgecdn.net/files/1234/567/My%20Mod%2B1.jar");
        assert_eq!(cdn_url(5, "a.jar"), "https://edge.forgecdn.net/files/0/5/a.jar");
    }

    #[test]
    fn blocked_page_fallbacks() {
        let mut project = api::Mod { id: 9, slug: "some-pack".into(), class_id: Some(12), ..Default::default() };
        assert_eq!(blocked_page_url(Some(&project), 9, 77), "https://www.curseforge.com/minecraft/texture-packs/some-pack/download/77");
        project.links.website_url = Some("https://www.curseforge.com/minecraft/mc-mods/x/".into());
        assert_eq!(blocked_page_url(Some(&project), 9, 77), "https://www.curseforge.com/minecraft/mc-mods/x/download/77");
        assert_eq!(blocked_page_url(None, 9, 77), "https://www.curseforge.com/projects/9/download/77");
    }

    #[test]
    fn unsafe_file_names_are_refused() {
        let (manifest, mut files, mods) = resolved();
        files[0].file_name = "../../evil.jar".into();
        assert!(build_plan(&manifest, &files, &mods, Path::new("p.zip"), false, false).is_err());
        files[0].file_name = "sub/dir.jar".into();
        assert!(build_plan(&manifest, &files, &mods, Path::new("p.zip"), false, false).is_err());
        assert_eq!(safe_file_name("ok.jar").as_deref(), Some("ok.jar"));
        assert_eq!(safe_file_name("C:evil.jar"), None);
    }

    #[test]
    fn unknown_files_are_an_error() {
        let (manifest, mut files, mods) = resolved();
        files.pop();
        let error = build_plan(&manifest, &files, &mods, Path::new("p.zip"), false, false).unwrap_err().to_string();
        assert!(error.contains("no longer exist"), "{error}");
    }

    #[test]
    fn duplicate_manifest_entries_are_installed_once() {
        let (mut manifest, files, mods) = resolved();
        manifest.files.push(manifest.files[0].clone());
        let plan = build_plan(&manifest, &files, &mods, Path::new("p.zip"), false, false).unwrap();
        assert_eq!(plan.files.iter().filter(|f| f.path.ends_with("jei-1.20.1-forge-15.3.0.4.jar")).count(), 1);
    }

    #[test]
    fn class_folders() {
        assert_eq!(class_target(Some(6)).0, "mods");
        assert_eq!(class_target(Some(12)).0, "resourcepacks");
        assert_eq!(class_target(Some(6552)).0, "shaderpacks");
        assert_eq!(class_target(Some(17)).0, "saves");
        assert_eq!(class_target(Some(6945)).0, "datapacks");
        assert_eq!(class_target(None).0, "mods");
    }
}

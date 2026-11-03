//! Content files already in a game directory, and identifying them on Modrinth by SHA-1.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use futures::StreamExt;
use cubby_common::{Error, Result};

use crate::api;
use crate::client::{CachedHash, Modrinth};
use crate::model::{ContentSource, Folder};
use crate::resolve::Installed;

pub(crate) const DISABLED_SUFFIX: &str = ".disabled";
const HASH_CONCURRENCY: usize = 8;

/// A content file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalFile {
    pub folder: Folder,
    /// Name on disk, with `.disabled` for a disabled mod.
    pub file_name: String,
    pub path: PathBuf,
    pub enabled: bool,
    pub size: u64,
    /// Lowercase hex; empty until hashed.
    pub sha1: String,
}

fn is_content_name(folder: Folder, lower: &str) -> bool {
    match folder {
        Folder::Mods => {
            let base = lower.strip_suffix(DISABLED_SUFFIX).unwrap_or(lower);
            base.ends_with(".jar") || base.ends_with(".zip") || base.ends_with(".litemod")
        }
        Folder::ResourcePacks | Folder::ShaderPacks => lower.ends_with(".zip"),
    }
}

/// Content files of a game directory, not hashed yet: mods (`.jar`, `.zip`, `.litemod`, optionally
/// `.disabled`) and zipped resource and shader packs (folders cannot be identified). Sorted by folder and name.
pub(crate) fn list_files(game_dir: &Path) -> Vec<LocalFile> {
    let mut out = Vec::new();
    for folder in Folder::ALL {
        let Ok(rd) = std::fs::read_dir(game_dir.join(folder.as_str())) else { continue };
        for entry in rd.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let lower = file_name.to_lowercase();
            if !meta.is_file() || file_name.starts_with('.') || !is_content_name(folder, &lower) {
                continue;
            }
            out.push(LocalFile {
                folder,
                enabled: !(folder == Folder::Mods && lower.ends_with(DISABLED_SUFFIX)),
                path: entry.path(),
                size: meta.len(),
                sha1: String::new(),
                file_name,
            });
        }
    }
    out.sort_by_cached_key(|f| (f.folder, f.file_name.to_lowercase()));
    out
}

/// Finds `file_name` in mods, then resourcepacks, then shaderpacks.
pub(crate) fn find_file(game_dir: &Path, file_name: &str) -> Option<LocalFile> {
    list_files(game_dir).into_iter().find(|f| f.file_name == file_name)
}

impl Modrinth {
    /// SHA-1 of a file, cached while its size and modification time do not change.
    pub(crate) async fn sha1_of(&self, path: &Path) -> Result<String> {
        let meta = tokio::fs::metadata(path).await.map_err(|e| Error::io(path, e))?;
        let (size, modified) = (meta.len(), meta.modified().ok());
        let cached = {
            let cache = self.hashes.lock().expect("hash cache");
            cache.get(path).filter(|hit| hit.size == size && hit.modified == modified && modified.is_some()).map(|hit| hit.sha1.clone())
        };
        if let Some(sha1) = cached {
            return Ok(sha1);
        }
        let sha1 = cubby_common::net::sha1_file(path).await?;
        self.hashes.lock().expect("hash cache").insert(path.to_path_buf(), CachedHash { size, modified, sha1: sha1.clone() });
        Ok(sha1)
    }

    /// Hashes the files; unreadable ones are left out (and logged).
    pub(crate) async fn hash_files(&self, files: Vec<LocalFile>) -> Vec<LocalFile> {
        futures::stream::iter(files.into_iter().map(|mut f| async move {
            match self.sha1_of(&f.path).await {
                Ok(sha1) => {
                    f.sha1 = sha1;
                    Some(f)
                }
                Err(e) => {
                    tracing::warn!("could not hash {}: {e}", f.path.display());
                    None
                }
            }
        }))
        .buffered(HASH_CONCURRENCY)
        .filter_map(|f| async move { f })
        .collect()
        .await
    }

    /// Every content file of the game directory, hashed.
    pub(crate) async fn local_files(&self, game_dir: &Path) -> Vec<LocalFile> {
        let dir = game_dir.to_path_buf();
        let files = tokio::task::spawn_blocking(move || list_files(&dir)).await.unwrap_or_default();
        self.hash_files(files).await
    }

    /// The Modrinth version of each file, by SHA-1 (unknown files are left out).
    pub(crate) async fn identify(&self, files: &[LocalFile]) -> Result<HashMap<String, api::Version>> {
        if files.is_empty() {
            return Ok(HashMap::new());
        }
        let hashes: Vec<String> = files.iter().map(|f| f.sha1.clone()).collect();
        self.version_files(&hashes).await
    }

    /// Projects installed in the game directory, by project id.
    pub(crate) async fn installed(&self, game_dir: &Path) -> Result<HashMap<String, Installed>> {
        let files = self.local_files(game_dir).await;
        let identified = self.identify(&files).await?;
        Ok(installed_map(&files, &identified))
    }

    /// The files of the game directory found on Modrinth, by folder then name.
    pub async fn content_sources(&self, game_dir: &Path) -> Result<Vec<ContentSource>> {
        let files = self.local_files(game_dir).await;
        let identified = self.identify(&files).await?;
        if identified.is_empty() {
            return Ok(Vec::new());
        }
        let project_ids: Vec<String> = identified.values().map(|v| v.project_id.clone()).collect();
        let projects: HashMap<String, api::Project> = self.projects_raw(&project_ids).await?.into_iter().map(|p| (p.id.clone(), p)).collect();
        Ok(files
            .into_iter()
            .filter_map(|f| {
                let version = identified.get(&f.sha1)?;
                let project = projects.get(&version.project_id);
                Some(ContentSource {
                    folder: f.folder,
                    project_id: version.project_id.clone(),
                    version_id: version.id.clone(),
                    slug: project.map(|p| p.slug.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| version.project_id.clone()),
                    title: project.map(|p| p.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| f.file_name.clone()),
                    icon_url: project.and_then(|p| p.icon_url.clone()).filter(|u| !u.is_empty()),
                    version_number: version.version_number.clone(),
                    file_name: f.file_name,
                })
            })
            .collect())
    }
}

/// Project id -> installed file. When a project has several files, an enabled one wins.
pub(crate) fn installed_map(files: &[LocalFile], identified: &HashMap<String, api::Version>) -> HashMap<String, Installed> {
    let mut out: HashMap<String, Installed> = HashMap::new();
    for f in files {
        let Some(version) = identified.get(&f.sha1) else { continue };
        let entry = Installed {
            project_id: version.project_id.clone(),
            version_id: version.id.clone(),
            version_number: version.version_number.clone(),
            file_name: f.file_name.clone(),
            folder: f.folder,
            size: f.size,
            enabled: f.enabled,
        };
        match out.get(&version.project_id) {
            Some(existing) if existing.enabled || !entry.enabled => {}
            _ => {
                out.insert(version.project_id.clone(), entry);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_content_files() {
        let game = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/modrinth/testdata").join(format!("local-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&game);
        for (path, body) in [
            ("mods/a.jar", "a"),
            ("mods/b.jar.disabled", "b"),
            ("mods/notes.txt", "x"),
            ("mods/.hidden.jar", "x"),
            ("resourcepacks/Pack.zip", "p"),
            ("resourcepacks/Folder/pack.mcmeta", "x"),
            ("shaderpacks/BSL.zip", "s"),
            ("shaderpacks/BSL.zip.txt", "x"),
        ] {
            let p = game.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        let files = list_files(&game);
        let names: Vec<(Folder, &str, bool)> = files.iter().map(|f| (f.folder, f.file_name.as_str(), f.enabled)).collect();
        assert_eq!(
            names,
            [
                (Folder::Mods, "a.jar", true),
                (Folder::Mods, "b.jar.disabled", false),
                (Folder::ResourcePacks, "Pack.zip", true),
                (Folder::ShaderPacks, "BSL.zip", true)
            ]
        );
        assert_eq!(find_file(&game, "BSL.zip").unwrap().folder, Folder::ShaderPacks);
        assert!(find_file(&game, "notes.txt").is_none());
    }

    #[test]
    fn installed_prefers_enabled_files() {
        let file = |name: &str, sha1: &str, enabled: bool| LocalFile {
            folder: Folder::Mods,
            file_name: name.into(),
            path: PathBuf::from(name),
            enabled,
            size: 1,
            sha1: sha1.into(),
        };
        let version = |id: &str, project: &str| api::Version { id: id.into(), project_id: project.into(), ..Default::default() };
        let files = [file("old.jar.disabled", "h1", false), file("new.jar", "h2", true), file("unknown.jar", "h3", true)];
        let identified = HashMap::from([("h1".to_string(), version("v1", "P")), ("h2".to_string(), version("v2", "P"))]);
        let map = installed_map(&files, &identified);
        assert_eq!(map.len(), 1);
        assert_eq!((map["P"].file_name.as_str(), map["P"].version_id.as_str()), ("new.jar", "v2"));
    }
}

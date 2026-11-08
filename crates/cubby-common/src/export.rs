//! Shared by the modpack exports (`.mrpack`, CurseForge `.zip`): what of the game directory can go into a
//! pack, and writing the archive.

use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use zip::write::SimpleFileOptions;

use crate::{Error, Result};

/// Folders the export handles on its own (content files) or that never belong in a pack.
const NEVER: [&str; 14] = [
    "mods", "resourcepacks", "shaderpacks", "logs", "crash-reports", "screenshots", ".trash", "natives", ".fabric", ".mixin.out",
    "downloads", "cache", ".cache", "debug",
];
/// Ticked by default: what modpack authors usually ship.
const SUGGESTED: [&str; 6] = ["config", "defaultconfigs", "kubejs", "scripts", "options.txt", "global_packs"];

/// A top-level entry of the game directory that can go into the pack's overrides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportCandidate {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    /// Usually shipped with a pack (config, options.txt...).
    pub suggested: bool,
}

fn tree_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else { return 0 };
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    fs::read_dir(path).map(|rd| rd.flatten().map(|e| tree_size(&e.path())).sum()).unwrap_or(0)
}

/// Top-level files and folders of the game directory that an export can include, sorted by name.
pub fn export_candidates(game_dir: &Path) -> Vec<ExportCandidate> {
    let Ok(rd) = fs::read_dir(game_dir) else { return Vec::new() };
    let mut out: Vec<ExportCandidate> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let ty = e.file_type().ok()?;
            if NEVER.contains(&name.as_str()) || ty.is_symlink() || name.starts_with('.') && name != ".minecraft" {
                return None;
            }
            Some(ExportCandidate { suggested: SUGGESTED.contains(&name.as_str()), size: tree_size(&e.path()), is_dir: ty.is_dir(), name })
        })
        .collect();
    out.sort_by_key(|c| c.name.to_lowercase());
    out
}

/// Every file under the chosen top-level entries, as (`/`-separated path relative to the game directory,
/// absolute path). Symbolic links are skipped. An entry that isn't a plain name is an error.
pub fn collect_files(game_dir: &Path, include: &[String]) -> Result<Vec<(String, PathBuf)>> {
    let mut out = Vec::new();
    for name in include {
        let plain = !name.is_empty() && Path::new(name).components().all(|c| matches!(c, Component::Normal(_))) && !name.contains(['/', '\\']);
        if !plain || NEVER.contains(&name.as_str()) {
            return Err(Error::Other(format!("\"{name}\" can't go into a pack.")));
        }
        let mut stack = vec![(name.clone(), game_dir.join(name))];
        while let Some((rel, path)) = stack.pop() {
            let Ok(meta) = fs::symlink_metadata(&path) else { continue };
            if meta.is_file() {
                out.push((rel, path));
            } else if meta.is_dir() {
                for entry in fs::read_dir(&path).map_err(|e| Error::io(&path, e))?.flatten() {
                    stack.push((format!("{rel}/{}", entry.file_name().to_string_lossy()), entry.path()));
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Writes a pack: `index_name` (the manifest) at the root, then each file under `prefix` (`overrides/`).
/// The archive is written next to `dest` and renamed at the end, so a failed export leaves nothing behind.
pub fn write_pack(dest: &Path, index_name: &str, index: &[u8], prefix: &str, files: &[(String, PathBuf)]) -> Result<()> {
    let tmp = dest.with_extension("part");
    let result = (|| {
        let file = fs::File::create(&tmp).map_err(|e| Error::io(&tmp, e))?;
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let zip_err = |e: zip::result::ZipError| Error::Other(format!("Couldn't write {} ({e}).", dest.display()));
        zip.start_file(index_name, options).map_err(zip_err)?;
        zip.write_all(index).map_err(|e| Error::io(&tmp, e))?;
        for (rel, src) in files {
            zip.start_file(format!("{prefix}{rel}"), options).map_err(zip_err)?;
            let mut input = fs::File::open(src).map_err(|e| Error::io(src, e))?;
            std::io::copy(&mut input, &mut zip).map_err(|e| Error::io(src, e))?;
        }
        zip.finish().map_err(zip_err)?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    let _ = fs::remove_file(dest);
    fs::rename(&tmp, dest).map_err(|e| Error::io(dest, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cubby-export-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn candidates_collect_and_write() {
        let game = temp("game");
        for dir in ["config/sub", "mods", "saves/World", "logs"] {
            fs::create_dir_all(game.join(dir)).unwrap();
        }
        fs::write(game.join("config/a.toml"), "a").unwrap();
        fs::write(game.join("config/sub/b.json"), "bb").unwrap();
        fs::write(game.join("options.txt"), "fov:1").unwrap();
        fs::write(game.join("saves/World/level.dat"), "x").unwrap();

        let names: Vec<_> = export_candidates(&game).into_iter().map(|c| (c.name, c.suggested)).collect();
        assert_eq!(names, [("config".to_string(), true), ("options.txt".to_string(), true), ("saves".to_string(), false)]);

        let files = collect_files(&game, &["config".into(), "options.txt".into()]).unwrap();
        let rels: Vec<_> = files.iter().map(|(r, _)| r.as_str()).collect();
        assert_eq!(rels, ["config/a.toml", "config/sub/b.json", "options.txt"]);
        assert!(collect_files(&game, &["../x".into()]).is_err());
        assert!(collect_files(&game, &["mods".into()]).is_err());

        let dest = game.join("pack.mrpack");
        write_pack(&dest, "modrinth.index.json", b"{}", "overrides/", &files).unwrap();
        let mut zip = zip::ZipArchive::new(fs::File::open(&dest).unwrap()).unwrap();
        let mut text = String::new();
        zip.by_name("overrides/config/sub/b.json").unwrap().read_to_string(&mut text).unwrap();
        assert_eq!(text, "bb");
        assert!(zip.by_name("modrinth.index.json").is_ok());
        assert!(!dest.with_extension("part").exists());
    }
}

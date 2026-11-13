//! Exporting an instance as a CurseForge modpack `.zip`: `manifest.json` lists the files CurseForge knows
//! (found by fingerprint), everything else goes into `overrides/`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cubby_common::export::{collect_files, write_pack};
use cubby_common::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::client::CurseForgeClient;

const CONTENT_FOLDERS: [&str; 3] = ["mods", "resourcepacks", "shaderpacks"];

/// CurseForge's file fingerprint: MurmurHash2 (seed 1) of the file without its whitespace bytes.
pub fn fingerprint(bytes: &[u8]) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let data: Vec<u8> = bytes.iter().copied().filter(|b| !matches!(b, 9 | 10 | 13 | 32)).collect();
    let mut h: u32 = 1 ^ data.len() as u32;
    let (chunks, rest) = data.as_chunks::<4>();
    for chunk in chunks {
        let mut k = u32::from_le_bytes(*chunk);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M) ^ k;
    }
    if !rest.is_empty() {
        for (i, b) in rest.iter().enumerate() {
            h ^= (*b as u32) << (8 * i);
        }
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^ (h >> 15)
}

/// What goes into the pack besides the content files.
#[derive(Debug, Clone)]
pub struct CfExportSpec {
    pub name: String,
    pub version: String,
    pub author: String,
    pub minecraft: String,
    /// "fabric-0.16.5", "forge-47.2.0", "neoforge-21.1.5", "quilt-0.27.1"; `None` for Vanilla.
    pub loader_id: Option<String>,
    /// Top-level entries of the game directory to copy into `overrides/`.
    pub include: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest<'a> {
    minecraft: MinecraftOut<'a>,
    manifest_type: &'static str,
    manifest_version: u32,
    name: &'a str,
    version: &'a str,
    author: &'a str,
    files: Vec<FileOut>,
    overrides: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftOut<'a> {
    version: &'a str,
    mod_loaders: Vec<LoaderOut<'a>>,
}

#[derive(Serialize)]
struct LoaderOut<'a> {
    id: &'a str,
    primary: bool,
}

#[derive(Serialize)]
struct FileOut {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    required: bool,
}

#[derive(Deserialize)]
struct MatchResponse {
    data: MatchData,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchData {
    #[serde(default)]
    exact_matches: Vec<Match>,
}

#[derive(Deserialize)]
struct Match {
    file: MatchFile,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchFile {
    id: u64,
    mod_id: u64,
    file_fingerprint: u32,
}

/// Content files of a game directory that the game loads (turned-off mods are left out), as
/// (`/`-separated path, absolute path).
fn content_files(game_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for folder in CONTENT_FOLDERS {
        let Ok(rd) = std::fs::read_dir(game_dir.join(folder)) else { continue };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let lower = name.to_lowercase();
            if !entry.path().is_file() || name.starts_with('.') || lower.ends_with(".disabled") || !(lower.ends_with(".jar") || lower.ends_with(".zip")) {
                continue;
            }
            out.push((format!("{folder}/{name}"), entry.path()));
        }
    }
    out.sort();
    out
}

impl CurseForgeClient {
    /// Project and file ids of the fingerprints CurseForge knows exactly.
    pub async fn match_fingerprints(&self, fingerprints: &[u32]) -> Result<HashMap<u32, (u64, u64)>> {
        #[derive(Serialize)]
        struct Body<'a> {
            fingerprints: &'a [u32],
        }
        let mut out = HashMap::new();
        for chunk in fingerprints.chunks(500) {
            let response: MatchResponse = self.post("/v1/fingerprints/432", &Body { fingerprints: chunk }, "CurseForge fingerprints").await?;
            out.extend(response.data.exact_matches.into_iter().map(|m| (m.file.file_fingerprint, (m.file.mod_id, m.file.id))));
        }
        Ok(out)
    }
}

/// Writes the instance whose game directory is `game_dir` to `dest` as a CurseForge `.zip`. With a client,
/// files CurseForge knows are listed in the manifest; without one (no API key) every file goes into the zip.
/// Returns (files listed, files stored).
pub async fn export_curseforge(client: Option<&CurseForgeClient>, game_dir: &Path, dest: &Path, spec: &CfExportSpec) -> Result<(usize, usize)> {
    if spec.name.trim().is_empty() {
        return Err(Error::Other("Give the pack a name.".into()));
    }
    let dir = game_dir.to_path_buf();
    let include = spec.include.clone();
    let (content, extra) = tokio::task::spawn_blocking(move || -> Result<_> {
        let mut content = Vec::new();
        for (rel, path) in content_files(&dir) {
            let bytes = std::fs::read(&path).map_err(|e| Error::io(&path, e))?;
            content.push((rel, path, fingerprint(&bytes)));
        }
        Ok((content, collect_files(&dir, &include)?))
    })
    .await
    .map_err(|e| Error::Other(format!("The export stopped unexpectedly ({e}).")))??;

    let matches = match client {
        Some(client) if !content.is_empty() => client.match_fingerprints(&content.iter().map(|(_, _, f)| *f).collect::<Vec<_>>()).await?,
        _ => HashMap::new(),
    };
    let mut files = Vec::new();
    let mut overrides = Vec::new();
    for (rel, path, fp) in content {
        match matches.get(&fp) {
            Some(&(project_id, file_id)) => files.push(FileOut { project_id, file_id, required: true }),
            None => overrides.push((rel, path)),
        }
    }
    overrides.extend(extra);
    let listed = files.len();
    let stored = overrides.len();
    let manifest = Manifest {
        minecraft: MinecraftOut {
            version: &spec.minecraft,
            mod_loaders: spec.loader_id.as_deref().map(|id| LoaderOut { id, primary: true }).into_iter().collect(),
        },
        manifest_type: "minecraftModpack",
        manifest_version: 1,
        name: spec.name.trim(),
        version: spec.version.trim(),
        author: spec.author.trim(),
        files,
        overrides: "overrides",
    };
    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| Error::Other(e.to_string()))?;
    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || write_pack(&dest, "manifest.json", &json, "overrides/", &overrides))
        .await
        .map_err(|e| Error::Other(format!("The export stopped unexpectedly ({e}).")))??;
    Ok((listed, stored))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_skips_whitespace() {
        assert_eq!(fingerprint(b"a b\tc\r\n"), fingerprint(b"abc"));
        assert_ne!(fingerprint(b"abc"), fingerprint(b"abd"));
        // Known value: the empty input hashes to the seed mixed alone.
        assert_eq!(fingerprint(b""), {
            let m: u32 = 0x5bd1_e995;
            let mut h: u32 = 1;
            h ^= h >> 13;
            h = h.wrapping_mul(m);
            h ^ (h >> 15)
        });
    }

    #[tokio::test]
    async fn export_without_key_stores_everything() {
        let dir = std::env::temp_dir().join(format!("cubby-cf-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mods")).unwrap();
        std::fs::create_dir_all(dir.join("config")).unwrap();
        std::fs::write(dir.join("mods/a.jar"), "jar").unwrap();
        std::fs::write(dir.join("mods/b.jar.disabled"), "off").unwrap();
        std::fs::write(dir.join("config/a.toml"), "x").unwrap();
        let spec = CfExportSpec {
            name: "Pack".into(),
            version: "1.0".into(),
            author: "Me".into(),
            minecraft: "1.20.1".into(),
            loader_id: Some("forge-47.2.0".into()),
            include: vec!["config".into()],
        };
        let dest = dir.join("pack.zip");
        assert_eq!(export_curseforge(None, &dir, &dest, &spec).await.unwrap(), (0, 2));
        let manifest = crate::read_manifest(&dest).unwrap();
        assert_eq!(manifest.minecraft.version, "1.20.1");
        assert_eq!(manifest.minecraft.mod_loaders[0].id, "forge-47.2.0");
    }
}

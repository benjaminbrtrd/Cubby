//! Exporting an instance as a `.mrpack`: the mods, resource packs and shader packs Modrinth knows become
//! downloads, everything else goes into `overrides/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cubby_common::export::{collect_files, write_pack};
use cubby_common::{Error, Result};
use serde::Serialize;

use crate::client::Modrinth;
use crate::mrpack::{PackLoader, INDEX_FILE};

/// What goes into the pack besides the content files.
#[derive(Debug, Clone)]
pub struct ExportSpec {
    pub name: String,
    /// The pack's own version ("1.0.0").
    pub version: String,
    pub summary: Option<String>,
    pub minecraft: String,
    /// `None` for Vanilla.
    pub loader: Option<(PackLoader, String)>,
    /// Top-level entries of the game directory to copy into `overrides/` (`config`, `options.txt`...).
    pub include: Vec<String>,
}

/// What an export wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    /// Files players download from Modrinth when they install the pack.
    pub downloads: usize,
    /// Files stored inside the pack.
    pub included: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Index<'a> {
    format_version: u32,
    game: &'static str,
    version_id: &'a str,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<&'a str>,
    files: Vec<IndexFile>,
    dependencies: BTreeMap<&'static str, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexFile {
    path: String,
    hashes: BTreeMap<&'static str, String>,
    downloads: Vec<String>,
    file_size: u64,
}

/// The `dependencies` of the index: the game and the loader.
pub fn pack_dependencies(minecraft: &str, loader: Option<&(PackLoader, String)>) -> BTreeMap<&'static str, String> {
    let mut deps = BTreeMap::from([("minecraft", minecraft.to_string())]);
    if let Some((kind, version)) = loader {
        let key = match kind {
            PackLoader::Fabric => "fabric-loader",
            PackLoader::Quilt => "quilt-loader",
            PackLoader::Forge => "forge",
            PackLoader::NeoForge => "neoforge",
        };
        deps.insert(key, version.clone());
    }
    deps
}

impl Modrinth {
    /// Writes the instance whose game directory is `game_dir` to `dest` as a `.mrpack`. Turned-off mods are
    /// left out.
    pub async fn export_mrpack(&self, game_dir: &Path, dest: &Path, spec: &ExportSpec) -> Result<ExportSummary> {
        if spec.name.trim().is_empty() {
            return Err(Error::Other("Give the pack a name.".into()));
        }
        let files: Vec<_> = self.local_files(game_dir).await.into_iter().filter(|f| f.enabled && !f.sha1.is_empty()).collect();
        let known = self.identify(&files).await?;
        let mut index_files = Vec::new();
        let mut overrides: Vec<(String, PathBuf)> = Vec::new();
        for f in &files {
            let rel = format!("{}/{}", f.folder.as_str(), f.file_name);
            let remote = known.get(&f.sha1).and_then(|v| v.files.iter().find(|vf| vf.hashes.get("sha1").is_some_and(|h| h.eq_ignore_ascii_case(&f.sha1))));
            match remote {
                Some(vf) if vf.url.starts_with("https://") && vf.hashes.contains_key("sha512") => index_files.push(IndexFile {
                    path: rel,
                    hashes: BTreeMap::from([("sha1", f.sha1.clone()), ("sha512", vf.hashes["sha512"].to_lowercase())]),
                    downloads: vec![vf.url.clone()],
                    file_size: f.size,
                }),
                _ => overrides.push((rel, f.path.clone())),
            }
        }
        let dir = game_dir.to_path_buf();
        let include = spec.include.clone();
        let extra = tokio::task::spawn_blocking(move || collect_files(&dir, &include))
            .await
            .map_err(|e| Error::Other(format!("The export stopped unexpectedly ({e}).")))??;
        overrides.extend(extra);

        let summary = ExportSummary { downloads: index_files.len(), included: overrides.len() };
        let index = Index {
            format_version: 1,
            game: "minecraft",
            version_id: spec.version.trim(),
            name: spec.name.trim(),
            summary: spec.summary.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            files: index_files,
            dependencies: pack_dependencies(&spec.minecraft, spec.loader.as_ref()),
        };
        let json = serde_json::to_vec_pretty(&index).map_err(|e| Error::Other(e.to_string()))?;
        let dest = dest.to_path_buf();
        tokio::task::spawn_blocking(move || write_pack(&dest, INDEX_FILE, &json, "overrides/", &overrides))
            .await
            .map_err(|e| Error::Other(format!("The export stopped unexpectedly ({e}).")))??;
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mrpack::MrpackIndex;

    #[test]
    fn dependencies_use_mrpack_keys() {
        let deps = pack_dependencies("1.21.1", Some(&(PackLoader::Fabric, "0.16.5".into())));
        assert_eq!(deps.get("fabric-loader").map(String::as_str), Some("0.16.5"));
        assert_eq!(pack_dependencies("1.20.1", None).len(), 1);
        // What we write, Cubby (and the Modrinth format) reads back.
        let index = Index {
            format_version: 1,
            game: "minecraft",
            version_id: "1.0.0",
            name: "Pack",
            summary: None,
            files: vec![IndexFile {
                path: "mods/a.jar".into(),
                hashes: BTreeMap::from([("sha1", "a".repeat(40)), ("sha512", "b".repeat(128))]),
                downloads: vec!["https://cdn.modrinth.com/data/x/a.jar".into()],
                file_size: 3,
            }],
            dependencies: pack_dependencies("1.21.1", Some(&(PackLoader::NeoForge, "21.1.5".into()))),
        };
        let parsed = MrpackIndex::parse(&serde_json::to_vec(&index).unwrap()).unwrap();
        assert_eq!(parsed.minecraft(), Some("1.21.1"));
        assert_eq!(parsed.loader(), Some((PackLoader::NeoForge, "21.1.5".into())));
        assert_eq!(parsed.client_downloads().unwrap().len(), 1);
    }
}

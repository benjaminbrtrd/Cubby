//! Mojang version manifest and per-version JSON, cached under `<root>/meta`.

use std::path::Path;
use std::time::Duration;

use cubby_common::net::{ensure_file, Download, Http};
use cubby_common::paths::Paths;
use cubby_common::version::VersionJson;
use cubby_common::{Error, Result};
use serde::{Deserialize, Serialize};

/// URL of the version manifest (v2, which carries SHA-1 hashes).
pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// A cached manifest younger than this is used without touching the network.
const MANIFEST_TTL: Duration = Duration::from_secs(10 * 60);

/// The Mojang version manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    /// Latest release and snapshot ids.
    pub latest: Latest,
    /// Every known version, newest first.
    pub versions: Vec<ManifestEntry>,
}

/// Ids of the latest release and snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

/// One version of the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub id: String,
    /// "release", "snapshot", "old_beta" or "old_alpha".
    #[serde(rename = "type")]
    pub kind: String,
    /// URL of the version JSON.
    pub url: String,
    pub time: String,
    pub release_time: String,
    /// SHA-1 of the version JSON.
    pub sha1: String,
}

impl VersionManifest {
    /// Finds a version by id.
    pub fn find(&self, id: &str) -> Option<&ManifestEntry> {
        self.versions.iter().find(|v| v.id == id)
    }
}

fn manifest_cache(paths: &Paths) -> std::path::PathBuf {
    paths.meta().join("version_manifest_v2.json")
}

async fn read_manifest(path: &Path) -> Result<VersionManifest> {
    let bytes = tokio::fs::read(path).await.map_err(|e| Error::io(path, e))?;
    serde_json::from_slice(&bytes).map_err(|e| Error::json(path.display().to_string(), e))
}

fn is_fresh(path: &Path, ttl: Duration) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < ttl)
}

pub(crate) async fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| Error::io(parent, e))?;
    }
    let part = path.with_extension("part");
    tokio::fs::write(&part, bytes).await.map_err(|e| Error::io(&part, e))?;
    tokio::fs::rename(&part, path).await.map_err(|e| Error::io(path, e))
}

/// Returns the version manifest. The cache (`meta/version_manifest_v2.json`) is reused when younger
/// than 10 minutes, and used as a fallback when the network fails.
pub async fn fetch_manifest(http: &Http, paths: &Paths) -> Result<VersionManifest> {
    let cache = manifest_cache(paths);
    if is_fresh(&cache, MANIFEST_TTL) {
        if let Ok(manifest) = read_manifest(&cache).await {
            return Ok(manifest);
        }
    }
    let fetched = async {
        let bytes = http.get_bytes(MANIFEST_URL).await?;
        let manifest: VersionManifest = serde_json::from_slice(&bytes).map_err(|e| Error::json(MANIFEST_URL, e))?;
        Ok::<_, Error>((manifest, bytes))
    }
    .await;
    match fetched {
        Ok((manifest, bytes)) => {
            if let Err(e) = write_atomic(&cache, &bytes).await {
                tracing::warn!("could not cache the version manifest: {e}");
            }
            Ok(manifest)
        }
        Err(error) => {
            tracing::warn!("version manifest unavailable ({error}), trying the cache");
            read_manifest(&cache).await.map_err(|_| error)
        }
    }
}

/// Returns the JSON of a vanilla version, cached in `meta/versions/<id>.json` and verified against the
/// manifest SHA-1. When the manifest cannot be obtained at all, an existing cached file is used as is.
pub async fn fetch_version(http: &Http, paths: &Paths, id: &str) -> Result<VersionJson> {
    let cache = paths.meta().join("versions").join(format!("{id}.json"));
    match fetch_manifest(http, paths).await {
        Ok(manifest) => {
            let entry = manifest.find(id).ok_or_else(|| Error::NotFound(format!("Minecraft version {id}")))?;
            let job = Download::new(&entry.url, &cache).with_sha1(&entry.sha1);
            ensure_file(http, &job).await?;
        }
        Err(error) => {
            if !cache.exists() {
                return Err(error);
            }
            tracing::warn!("using the cached JSON of {id} without verification");
        }
    }
    let bytes = tokio::fs::read(&cache).await.map_err(|e| Error::io(&cache, e))?;
    serde_json::from_slice(&bytes).map_err(|e| Error::json(format!("version {id}"), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_entry() {
        let json = r#"{"latest":{"release":"1.21.1","snapshot":"24w40a"},"versions":[
            {"id":"1.21.1","type":"release","url":"https://x/1.json","time":"t","releaseTime":"rt","sha1":"abc","complianceLevel":1}]}"#;
        let m: VersionManifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.latest.release, "1.21.1");
        let e = m.find("1.21.1").unwrap();
        assert_eq!(e.kind, "release");
        assert_eq!(e.release_time, "rt");
        assert!(m.find("nope").is_none());
    }
}

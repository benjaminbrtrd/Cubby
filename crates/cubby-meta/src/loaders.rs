//! Fabric and Quilt metadata: loader versions and launcher profiles.

use cubby_common::net::Http;
use cubby_common::paths::Paths;
use cubby_common::version::VersionJson;
use cubby_common::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::mojang::write_atomic;

/// A mod loader. Fabric and Quilt come from a Fabric-style meta API, Forge and NeoForge from their Maven
/// repositories and installers (see [`crate::forge`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoaderKind {
    Fabric,
    Quilt,
    Forge,
    #[serde(rename = "neoforge")]
    NeoForge,
}

impl LoaderKind {
    /// Lowercase name, also used as cache folder.
    pub fn as_str(self) -> &'static str {
        match self {
            LoaderKind::Fabric => "fabric",
            LoaderKind::Quilt => "quilt",
            LoaderKind::Forge => "forge",
            LoaderKind::NeoForge => "neoforge",
        }
    }

    /// Base URL of the Fabric-style meta API; `None` for the loaders that use an installer.
    fn api(self) -> Option<&'static str> {
        match self {
            LoaderKind::Fabric => Some("https://meta.fabricmc.net/v2/versions"),
            LoaderKind::Quilt => Some("https://meta.quiltmc.org/v3/versions"),
            LoaderKind::Forge | LoaderKind::NeoForge => None,
        }
    }
}

/// A loader version available for a game version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderVersion {
    pub version: String,
    /// False for betas and other pre-releases.
    pub stable: bool,
}

#[derive(Deserialize)]
struct LoaderEntry {
    loader: LoaderInfo,
}

#[derive(Deserialize)]
struct LoaderInfo {
    version: String,
    #[serde(default)]
    stable: Option<bool>,
}

fn parse_loader_versions(bytes: &[u8], what: &str) -> Result<Vec<LoaderVersion>> {
    let entries: Vec<LoaderEntry> = serde_json::from_slice(bytes).map_err(|e| Error::json(what, e))?;
    Ok(entries
        .into_iter()
        .map(|e| {
            // Quilt does not flag stability: pre-releases carry a `-beta.N` style suffix.
            let stable = e.loader.stable.unwrap_or_else(|| !e.loader.version.contains('-'));
            LoaderVersion { version: e.loader.version, stable }
        })
        .collect())
}

/// Loader versions available for `game_version`, newest first.
///
/// For Forge, `stable` means the recommended build or older; for NeoForge, not a beta. The Forge and
/// NeoForge lists are cached in memory for ten minutes.
pub async fn loader_versions(http: &Http, kind: LoaderKind, game_version: &str) -> Result<Vec<LoaderVersion>> {
    let Some(api) = kind.api() else { return crate::forge::versions(http, kind, game_version).await };
    let url = format!("{api}/loader/{game_version}");
    let bytes = http.get_bytes(&url).await?;
    parse_loader_versions(&bytes, &url)
}

/// Game versions a loader supports (all versions, snapshots included), newest first.
pub async fn game_versions_supported(http: &Http, kind: LoaderKind) -> Result<Vec<String>> {
    let Some(api) = kind.api() else { return crate::forge::game_versions(http, kind).await };
    #[derive(Deserialize)]
    struct Game {
        version: String,
    }
    let url = format!("{api}/game");
    let games: Vec<Game> = http.get_json(&url).await?;
    Ok(games.into_iter().map(|g| g.version).collect())
}

/// The launcher profile (a partial version JSON with `inheritsFrom`) of a loader for a game version,
/// cached in `meta/<kind>/<game>-<loader>.json` (profiles are immutable, the cache is reused).
///
/// For Forge and NeoForge this downloads the installer (cached in `meta/<kind>/<version>/`) and reads the
/// profile out of it.
pub async fn loader_profile(http: &Http, paths: &Paths, kind: LoaderKind, game_version: &str, loader_version: &str) -> Result<VersionJson> {
    let Some(api) = kind.api() else {
        return crate::forge::Forge::open(http, paths, kind, game_version, loader_version).await?.version_profile().await;
    };
    let cache = paths.meta().join(kind.as_str()).join(format!("{game_version}-{loader_version}.json"));
    if let Ok(bytes) = tokio::fs::read(&cache).await {
        if let Ok(profile) = serde_json::from_slice(&bytes) {
            return Ok(profile);
        }
    }
    let url = format!("{api}/loader/{game_version}/{loader_version}/profile/json");
    let bytes = http.get_bytes(&url).await?;
    let profile: VersionJson = serde_json::from_slice(&bytes).map_err(|e| Error::json(&url, e))?;
    if let Err(e) = write_atomic(&cache, &bytes).await {
        tracing::warn!("could not cache {}: {e}", cache.display());
    }
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fabric_stability_is_read() {
        let json = br#"[{"loader":{"version":"0.20.1","stable":true}},{"loader":{"version":"0.21.0-beta.1","stable":false}}]"#;
        let v = parse_loader_versions(json, "t").unwrap();
        assert_eq!(v, vec![LoaderVersion { version: "0.20.1".into(), stable: true }, LoaderVersion { version: "0.21.0-beta.1".into(), stable: false }]);
    }

    #[test]
    fn quilt_stability_is_inferred() {
        let json = br#"[{"loader":{"version":"0.26.0"}},{"loader":{"version":"0.20.0-beta.9"}}]"#;
        let v = parse_loader_versions(json, "t").unwrap();
        assert!(v[0].stable);
        assert!(!v[1].stable);
    }
}

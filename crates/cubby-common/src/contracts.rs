//! Types exchanged between the Cubby crates. Changing one is a breaking change for several crates:
//! keep them small and stable.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::version::VersionJson;

/// Produced by `cubby-meta` once every file of a version is on disk; consumed by `cubby-launch`.
#[derive(Debug, Clone)]
pub struct InstalledVersion {
    /// Fully merged version (loader profile applied on top of the vanilla version).
    pub version: VersionJson,
    /// Client jar, `<root>/versions/<mc>/<mc>.jar` (for Forge and NeoForge a copy named after the merged
    /// version id, `<root>/versions/<id>/<id>.jar`, which their `-DignoreList` expects).
    pub client_jar: PathBuf,
    /// Library jars for the classpath, in launch order, without natives jars and without the client jar.
    pub libraries: Vec<PathBuf>,
    /// Legacy natives jars (pre-1.19 style) to extract into the natives directory before launching.
    pub natives: Vec<NativeJar>,
    /// `<root>/assets`, the value of `${assets_root}`.
    pub assets_root: PathBuf,
    /// Asset index id, the value of `${assets_index_name}`.
    pub assets_index_id: String,
    /// Directory passed as `${game_assets}` (legacy virtual assets), `assets_root` for modern versions.
    pub game_assets_dir: PathBuf,
    /// Log4j configuration argument with `${path}` already replaced, if the version has one.
    pub logging_argument: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NativeJar {
    pub path: PathBuf,
    /// Path prefixes not to extract, e.g. `META-INF/`.
    pub exclude: Vec<String>,
}

/// Identity given to the game. Produced by `cubby-auth`, consumed by `cubby-launch`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchCredentials {
    pub username: String,
    /// Dashed or undashed UUID of the profile.
    pub uuid: String,
    /// Minecraft access token, or any placeholder for offline accounts.
    pub access_token: String,
    /// "msa" for Microsoft accounts, "legacy" for offline ones.
    pub user_type: String,
    /// Xbox user id, empty when unknown.
    pub xuid: String,
}

/// A Java installation. Produced by `cubby-java`, consumed by `cubby-launch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaInstall {
    /// Path to `javaw.exe` on Windows, `java` elsewhere.
    pub executable: PathBuf,
    /// Major version: 8, 17, 21...
    pub major: u32,
    /// Full version string, e.g. "21.0.5".
    pub version: String,
    /// "x86_64", "x86" or "aarch64".
    pub arch: String,
    pub vendor: Option<String>,
    /// True when Cubby downloaded it into its own `java/` folder.
    pub managed: bool,
}

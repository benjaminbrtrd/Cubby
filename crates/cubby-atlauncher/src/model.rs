//! The JSON documents of the ATLauncher CDN, read as leniently as Prism Launcher reads them: unknown
//! fields are ignored and optional ones default, so a pack with an odd manifest still lists.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer};

/// A field that is `null` in the JSON reads as its default (the pack list has packs with a null description).
fn nullable<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// One entry of `packsnew.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IndexPack {
    #[serde(default, deserialize_with = "nullable")]
    pub position: i64,
    pub name: String,
    /// "public" or "private" (ATLauncher's own docs also mention "semi-public").
    #[serde(default, deserialize_with = "nullable", rename = "type")]
    pub kind: String,
    #[serde(default, deserialize_with = "nullable")]
    pub system: bool,
    #[serde(default, deserialize_with = "nullable")]
    pub description: String,
    #[serde(default, rename = "websiteURL")]
    pub website_url: Option<String>,
    #[serde(default, deserialize_with = "nullable")]
    pub versions: Vec<IndexVersion>,
}

/// One released version in `packsnew.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IndexVersion {
    pub version: String,
    #[serde(default, deserialize_with = "nullable")]
    pub minecraft: String,
    /// `false` for pre-releases and betas.
    #[serde(default, deserialize_with = "nullable")]
    pub is_recommended: bool,
}

/// `Configs.json`: what one version of a pack installs.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtlManifest {
    #[serde(default, deserialize_with = "nullable")]
    pub version: String,
    #[serde(default, deserialize_with = "nullable")]
    pub minecraft: String,
    /// Absent for packs that bring Forge as a `forge` mod.
    #[serde(default)]
    pub loader: Option<ManifestLoader>,
    #[serde(default, deserialize_with = "nullable")]
    pub mods: Vec<AtlMod>,
    /// Extra libraries (old packs); the plan can't express them.
    #[serde(default, deserialize_with = "nullable")]
    pub libraries: Vec<AtlLibrary>,
    /// Archive of config files, extracted into the game directory.
    #[serde(default)]
    pub configs: Option<ManifestConfigs>,
    /// The pack has no `Configs.zip`.
    #[serde(default, deserialize_with = "nullable")]
    pub no_configs: bool,
    /// Recommended memory, MiB.
    #[serde(default)]
    pub memory: Option<u32>,
    /// Named colours (`colour` of a mod refers to one): hex RGB without `#`.
    #[serde(default, deserialize_with = "nullable")]
    pub colours: HashMap<String, String>,
    /// Warnings shown before a mod is enabled (`warning` of a mod refers to one).
    #[serde(default, deserialize_with = "nullable")]
    pub warnings: HashMap<String, String>,
    #[serde(default, deserialize_with = "nullable")]
    pub messages: ManifestMessages,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ManifestMessages {
    /// HTML shown to the player at install time.
    #[serde(default, deserialize_with = "nullable")]
    pub install: String,
    #[serde(default, deserialize_with = "nullable")]
    pub update: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ManifestConfigs {
    #[serde(default)]
    pub filesize: Option<u64>,
    #[serde(default)]
    pub sha1: Option<String>,
}

/// The `loader` block.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ManifestLoader {
    /// "forge", "neoforge", "fabric", "quilt"; empty for none.
    #[serde(default, deserialize_with = "nullable", rename = "type")]
    pub kind: String,
    /// The player picks the loader version at install time (not supported).
    #[serde(default, deserialize_with = "nullable")]
    pub choose: bool,
    #[serde(default, deserialize_with = "nullable")]
    pub metadata: LoaderMetadata,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderMetadata {
    #[serde(default, deserialize_with = "nullable")]
    pub minecraft: String,
    /// Forge and NeoForge version.
    #[serde(default, deserialize_with = "nullable")]
    pub version: String,
    /// Fabric and Quilt loader version.
    #[serde(default, deserialize_with = "nullable")]
    pub loader: String,
    /// Newest / recommended build wanted, version resolved by the launcher (not supported).
    #[serde(default, deserialize_with = "nullable")]
    pub latest: bool,
    #[serde(default, deserialize_with = "nullable")]
    pub recommended: bool,
}

/// One entry of `mods`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtlMod {
    pub name: String,
    #[serde(default, deserialize_with = "nullable")]
    pub version: String,
    /// Relative to the ATL server for "server", the full URL for "direct", a web page for "browser".
    #[serde(default, deserialize_with = "nullable")]
    pub url: String,
    /// File name.
    #[serde(default, deserialize_with = "nullable")]
    pub file: String,
    /// "server", "direct" or "browser".
    #[serde(default, deserialize_with = "nullable")]
    pub download: String,
    #[serde(default)]
    pub md5: Option<String>,
    #[serde(default)]
    pub filesize: Option<u64>,
    /// "mods", "resourcepack", "extract"...; see [`crate::ModKind`].
    #[serde(default, deserialize_with = "nullable", rename = "type")]
    pub kind: String,
    /// `extract`: where the archive goes (a mod type, usually "root" or "mods").
    #[serde(default)]
    pub extract_to: Option<String>,
    /// `extract`: folder of the archive to copy ("%s%" stands for "/").
    #[serde(default)]
    pub extract_folder: Option<String>,
    /// `decomp`: where the file goes (a mod type).
    #[serde(default)]
    pub decomp_type: Option<String>,
    /// `decomp`: file to take out of the archive.
    #[serde(default)]
    pub decomp_file: Option<String>,
    #[serde(default, deserialize_with = "nullable")]
    pub description: String,
    /// The player may leave it out.
    #[serde(default, deserialize_with = "nullable")]
    pub optional: bool,
    #[serde(default, deserialize_with = "nullable")]
    pub recommended: bool,
    /// Selected by default when optional.
    #[serde(default, deserialize_with = "nullable")]
    pub selected: bool,
    #[serde(default, deserialize_with = "nullable")]
    pub hidden: bool,
    #[serde(default, deserialize_with = "nullable")]
    pub library: bool,
    /// Optional mods of one group exclude each other.
    #[serde(default, deserialize_with = "nullable")]
    pub group: String,
    /// Names of the mods this one needs.
    #[serde(default, deserialize_with = "nullable")]
    pub depends: Vec<String>,
    #[serde(default, deserialize_with = "nullable")]
    pub colour: String,
    #[serde(default, deserialize_with = "nullable")]
    pub warning: String,
    /// Installed on the client. Like Prism, a missing flag means "no".
    #[serde(default, deserialize_with = "nullable")]
    pub client: bool,
}

/// One entry of `libraries`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtlLibrary {
    #[serde(default, deserialize_with = "nullable")]
    pub url: String,
    #[serde(default, deserialize_with = "nullable")]
    pub file: String,
    #[serde(default, deserialize_with = "nullable")]
    pub server: String,
    #[serde(default, deserialize_with = "nullable")]
    pub md5: String,
    #[serde(default, deserialize_with = "nullable")]
    pub download: String,
}

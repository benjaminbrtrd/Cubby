//! Pack and Solder data, and the raw JSON they are read from.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A pack page, from `GET /modpack/{slug}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TechnicPack {
    /// The pack's slug (`name` in the API), as given back to `versions` and `plan`.
    pub name: String,
    pub display_name: String,
    /// Author.
    pub user: String,
    /// Plain text.
    pub description: String,
    /// Minecraft version of the current version of the pack, when the page says.
    pub minecraft: Option<String>,
    /// Current version of a zip pack ("1.6.6"); on a Solder pack, the version the page lists.
    pub version: Option<String>,
    /// The pack zip, for a pack that isn't hosted on Solder.
    pub pack_url: Option<String>,
    /// Root of the Solder API (no trailing slash), for a Solder pack.
    pub solder_url: Option<String>,
    pub icon_url: Option<String>,
    pub logo_url: Option<String>,
    pub background_url: Option<String>,
    pub ratings: Option<u64>,
    pub installs: Option<u64>,
    pub runs: Option<u64>,
    /// Page of the pack on technicpack.net.
    pub platform_url: Option<String>,
    pub server_pack_url: Option<String>,
    pub official: bool,
    pub tags: Vec<String>,
}

impl TechnicPack {
    /// Whether the pack is a list of builds on a Solder server rather than a single zip.
    pub fn is_solder(&self) -> bool {
        self.pack_url.is_none() && self.solder_url.is_some()
    }
}

/// One mod zip of a Solder build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolderMod {
    pub name: String,
    #[serde(default)]
    pub version: String,
    /// MD5 of the zip, when Solder says.
    #[serde(default)]
    pub md5: String,
    pub url: String,
    #[serde(default)]
    pub filesize: Option<u64>,
}

/// A Solder build: `GET {solder}/modpack/{slug}/{build}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolderBuild {
    #[serde(default)]
    pub minecraft: Option<String>,
    /// Memory the build asks for, in MiB; 0 or absent when it doesn't say.
    #[serde(default, deserialize_with = "lenient_u64")]
    pub memory: u64,
    pub mods: Vec<SolderMod>,
}

/// Solder serves numbers as numbers, strings or null depending on the server.
fn lenient_u64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Number(n) => n.as_u64().unwrap_or(0),
        Value::String(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
    })
}

// raw API

#[derive(Debug, Deserialize)]
pub(crate) struct RawSearch {
    #[serde(default)]
    pub modpacks: Vec<RawHit>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawHit {
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub icon_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawPack {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    /// A string for a zip pack, `null` for a Solder pack.
    #[serde(default)]
    pub url: Value,
    /// A string for a Solder pack, `null` otherwise.
    #[serde(default)]
    pub solder: Value,
    #[serde(default)]
    pub platform_url: Option<String>,
    #[serde(default)]
    pub minecraft: Option<String>,
    #[serde(default)]
    pub ratings: Value,
    #[serde(default)]
    pub installs: Value,
    #[serde(default)]
    pub runs: Value,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default)]
    pub is_official: bool,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub icon: Option<RawImage>,
    #[serde(default)]
    pub logo: Option<RawImage>,
    #[serde(default)]
    pub background: Option<RawImage>,
    #[serde(default)]
    pub server_pack_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawImage {
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawSolderPack {
    #[serde(default)]
    pub recommended: Option<String>,
    #[serde(default)]
    pub latest: Option<String>,
    #[serde(default)]
    pub builds: Vec<String>,
}

fn text(value: &Value) -> Option<String> {
    value.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn count(value: &Value) -> Option<u64> {
    match value {
        Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f.max(0.0) as u64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn image(image: Option<RawImage>) -> Option<String> {
    image.and_then(|i| i.url).map(|u| u.trim().to_string()).filter(|u| !u.is_empty())
}

fn nonempty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

impl RawPack {
    pub(crate) fn into_pack(self) -> TechnicPack {
        // Prism's rule: a string `url` makes a zip pack, otherwise the pack needs a string `solder`.
        let pack_url = text(&self.url);
        let solder_url = if pack_url.is_some() { None } else { text(&self.solder).map(|s| s.trim_end_matches('/').to_string()) };
        TechnicPack {
            display_name: nonempty(self.display_name).unwrap_or_else(|| self.name.clone()),
            name: self.name,
            user: nonempty(self.user).unwrap_or_default(),
            description: self.description.unwrap_or_default().trim().to_string(),
            minecraft: nonempty(self.minecraft),
            version: nonempty(self.version),
            pack_url,
            solder_url,
            icon_url: image(self.icon),
            logo_url: image(self.logo),
            background_url: image(self.background),
            ratings: count(&self.ratings),
            installs: count(&self.installs),
            runs: count(&self.runs),
            platform_url: nonempty(self.platform_url),
            server_pack_url: nonempty(self.server_pack_url),
            official: self.is_official,
            tags: self.tags.unwrap_or_default().split(',').map(str::trim).filter(|t| !t.is_empty()).map(str::to_string).collect(),
        }
    }
}

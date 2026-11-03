//! JSON shapes of the FTB modpack API. Every field the launcher doesn't need is left out, and nearly
//! every field is optional: the API has several generations of packs in it.

use serde::{Deserialize, Deserializer};

/// Ids are numbers in some places and strings in others.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum StringOrNumber {
    Number(u64),
    Text(String),
}

impl StringOrNumber {
    pub fn into_string(self) -> String {
        match self {
            Self::Number(n) => n.to_string(),
            Self::Text(s) => s,
        }
    }
}

fn null_default<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(d: D) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// `GET /{id}`
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Pack {
    #[serde(default)]
    pub id: u64,
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default, deserialize_with = "null_default")]
    pub synopsis: String,
    #[serde(default, deserialize_with = "null_default")]
    pub description: String,
    #[serde(default, deserialize_with = "null_default")]
    pub art: Vec<Art>,
    #[serde(default, deserialize_with = "null_default")]
    pub links: Vec<Link>,
    #[serde(default, deserialize_with = "null_default")]
    pub authors: Vec<Author>,
    #[serde(default, deserialize_with = "null_default")]
    pub versions: Vec<VersionStub>,
    #[serde(default)]
    pub installs: Option<u64>,
    #[serde(default)]
    pub plays: Option<u64>,
    #[serde(default, deserialize_with = "null_default")]
    pub tags: Vec<Tag>,
    /// Unix seconds.
    #[serde(default)]
    pub released: Option<i64>,
    /// Unix seconds.
    #[serde(default)]
    pub updated: Option<i64>,
    #[serde(default)]
    pub private: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Art {
    #[serde(default)]
    pub url: String,
    /// "square", "splash", "logo"…
    #[serde(default, rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Link {
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
    #[serde(default, deserialize_with = "null_default")]
    pub link: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Author {
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Tag {
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
}

/// A version as listed in `GET /{id}`; `GET /{id}/{version}` adds the files.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct VersionStub {
    #[serde(default)]
    pub id: u64,
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
    /// "release", "beta", "alpha" or "archived".
    #[serde(default, rename = "type", deserialize_with = "null_default")]
    pub kind: String,
    #[serde(default, deserialize_with = "null_default")]
    pub targets: Vec<Target>,
    #[serde(default)]
    pub specs: Option<Specs>,
    #[serde(default)]
    pub private: bool,
    /// Unix seconds.
    #[serde(default)]
    pub updated: Option<i64>,
}

/// What a version runs on: `{ "name": "forge", "type": "modloader", "version": "47.2.0" }`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Target {
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
    #[serde(default, rename = "type", deserialize_with = "null_default")]
    pub kind: String,
    #[serde(default, deserialize_with = "null_default")]
    pub version: String,
}

/// Memory in MiB.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Specs {
    #[serde(default)]
    pub minimum: Option<u32>,
    #[serde(default)]
    pub recommended: Option<u32>,
}

/// `GET /{id}/{version}`
#[derive(Debug, Clone, Default, Deserialize)]
pub struct VersionDetail {
    #[serde(flatten)]
    pub stub: VersionStub,
    #[serde(default, deserialize_with = "null_default")]
    pub files: Vec<File>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct File {
    /// Folder, relative to the game directory, usually "./mods".
    #[serde(default, deserialize_with = "null_default")]
    pub path: String,
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
    /// Empty when the file can't be downloaded from FTB's servers.
    #[serde(default, deserialize_with = "null_default")]
    pub url: String,
    #[serde(default, deserialize_with = "null_default")]
    pub mirrors: Vec<String>,
    #[serde(default, deserialize_with = "null_default")]
    pub sha1: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub serveronly: bool,
    #[serde(default)]
    pub curseforge: Option<CurseForgeRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CurseForgeRef {
    pub project: StringOrNumber,
    pub file: StringOrNumber,
}

/// `GET /popular/installs/{n}`, `/featured/{n}`, `/all`, `/search/{n}?term=`: ids of packs. A search
/// also lists `curseforge` project ids, which belong to the CurseForge provider and are ignored here.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PackIds {
    #[serde(default, deserialize_with = "null_default")]
    pub packs: Vec<u64>,
}

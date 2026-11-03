//! Raw CurseForge API shapes (<https://docs.curseforge.com/rest-api/>), only the fields used.
//! Everything is optional or defaulted: the API is lenient about nulls and we must not fail on them.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Envelope<T> {
    pub data: T,
    #[serde(default)]
    pub pagination: Option<Pagination>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Pagination {
    pub index: u64,
    pub page_size: u64,
    pub result_count: u64,
    pub total_count: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Mod {
    pub id: u64,
    pub name: String,
    pub slug: String,
    pub links: Links,
    pub summary: String,
    pub download_count: f64,
    pub class_id: Option<u64>,
    pub authors: Vec<Author>,
    pub logo: Option<Asset>,
    pub screenshots: Vec<Asset>,
    pub latest_files_indexes: Vec<FileIndex>,
    pub date_modified: Option<String>,
    /// `Some(false)`: the author doesn't let third-party launchers download the files.
    pub allow_mod_distribution: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Links {
    pub website_url: Option<String>,
    pub wiki_url: Option<String>,
    pub issues_url: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Author {
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Asset {
    pub title: Option<String>,
    pub thumbnail_url: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileIndex {
    pub game_version: String,
    /// 0 any, 1 Forge, 2 Cauldron, 3 LiteLoader, 4 Fabric, 5 Quilt, 6 NeoForge.
    pub mod_loader: Option<u8>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct File {
    pub id: u64,
    pub mod_id: u64,
    pub display_name: String,
    pub file_name: String,
    /// 1 release, 2 beta, 3 alpha.
    pub release_type: u8,
    pub hashes: Vec<Hash>,
    pub file_date: Option<String>,
    pub file_length: Option<u64>,
    pub download_url: Option<String>,
    pub game_versions: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Hash {
    pub value: String,
    /// 1 SHA-1, 2 MD5.
    pub algo: u8,
}

impl File {
    pub fn sha1(&self) -> Option<String> {
        self.hashes.iter().find(|h| h.algo == 1 && !h.value.is_empty()).map(|h| h.value.to_ascii_lowercase())
    }
}

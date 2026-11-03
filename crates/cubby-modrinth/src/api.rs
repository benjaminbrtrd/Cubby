//! Raw response types of the Modrinth API v2 (<https://docs.modrinth.com/api/>).
//!
//! Every field is defaulted and `null` reads as the default, so a new or missing field never breaks a
//! whole listing.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer};

/// `null` becomes `T::default()`.
fn nullable<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(d: D) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// `GET /search`
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SearchResponse {
    #[serde(deserialize_with = "nullable")]
    pub hits: Vec<SearchHit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SearchHit {
    #[serde(deserialize_with = "nullable")]
    pub project_id: String,
    #[serde(deserialize_with = "nullable")]
    pub project_type: String,
    #[serde(deserialize_with = "nullable")]
    pub slug: String,
    #[serde(deserialize_with = "nullable")]
    pub author: String,
    #[serde(deserialize_with = "nullable")]
    pub title: String,
    #[serde(deserialize_with = "nullable")]
    pub description: String,
    /// Categories and loaders mixed together.
    #[serde(deserialize_with = "nullable")]
    pub categories: Vec<String>,
    /// Game versions.
    #[serde(deserialize_with = "nullable")]
    pub versions: Vec<String>,
    pub downloads: u64,
    pub follows: u64,
    pub icon_url: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub date_created: String,
    #[serde(deserialize_with = "nullable")]
    pub date_modified: String,
    /// SPDX id.
    pub license: Option<String>,
    pub client_side: Option<String>,
    pub server_side: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub gallery: Vec<String>,
    pub featured_gallery: Option<String>,
}

/// `GET /project/{id|slug}`, `GET /projects?ids=[...]`
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Project {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub slug: String,
    #[serde(deserialize_with = "nullable")]
    pub project_type: String,
    #[serde(deserialize_with = "nullable")]
    pub title: String,
    #[serde(deserialize_with = "nullable")]
    pub description: String,
    #[serde(deserialize_with = "nullable")]
    pub body: String,
    #[serde(deserialize_with = "nullable")]
    pub categories: Vec<String>,
    #[serde(deserialize_with = "nullable")]
    pub additional_categories: Vec<String>,
    pub client_side: Option<String>,
    pub server_side: Option<String>,
    pub issues_url: Option<String>,
    pub source_url: Option<String>,
    pub wiki_url: Option<String>,
    pub discord_url: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub donation_urls: Vec<DonationUrl>,
    pub downloads: u64,
    pub followers: u64,
    pub icon_url: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub published: String,
    #[serde(deserialize_with = "nullable")]
    pub updated: String,
    pub license: Option<License>,
    /// Version ids.
    #[serde(deserialize_with = "nullable")]
    pub versions: Vec<String>,
    #[serde(deserialize_with = "nullable")]
    pub game_versions: Vec<String>,
    #[serde(deserialize_with = "nullable")]
    pub loaders: Vec<String>,
    #[serde(deserialize_with = "nullable")]
    pub gallery: Vec<GalleryItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DonationUrl {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub platform: String,
    #[serde(deserialize_with = "nullable")]
    pub url: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct License {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GalleryItem {
    #[serde(deserialize_with = "nullable")]
    pub url: String,
    pub featured: bool,
    pub title: Option<String>,
    pub description: Option<String>,
    pub ordering: i64,
}

/// `GET /project/{id}/members`
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TeamMember {
    pub user: User,
    #[serde(deserialize_with = "nullable")]
    pub role: String,
    pub is_owner: Option<bool>,
    pub ordering: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct User {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub username: String,
}

/// `GET /version/{id}`, `GET /versions?ids=[...]`, `GET /project/{id}/version`, `POST /version_files`
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Version {
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    #[serde(deserialize_with = "nullable")]
    pub project_id: String,
    #[serde(deserialize_with = "nullable")]
    pub name: String,
    #[serde(deserialize_with = "nullable")]
    pub version_number: String,
    #[serde(deserialize_with = "nullable")]
    pub changelog: String,
    #[serde(deserialize_with = "nullable")]
    pub dependencies: Vec<Dependency>,
    #[serde(deserialize_with = "nullable")]
    pub game_versions: Vec<String>,
    /// "release", "beta" or "alpha".
    #[serde(deserialize_with = "nullable")]
    pub version_type: String,
    #[serde(deserialize_with = "nullable")]
    pub loaders: Vec<String>,
    #[serde(deserialize_with = "nullable")]
    pub date_published: String,
    pub downloads: u64,
    #[serde(deserialize_with = "nullable")]
    pub files: Vec<VersionFile>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Dependency {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub file_name: Option<String>,
    /// "required", "optional", "incompatible" or "embedded".
    #[serde(deserialize_with = "nullable")]
    pub dependency_type: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct VersionFile {
    /// "sha1", "sha512" -> lowercase hex.
    #[serde(deserialize_with = "nullable")]
    pub hashes: HashMap<String, String>,
    #[serde(deserialize_with = "nullable")]
    pub url: String,
    #[serde(deserialize_with = "nullable")]
    pub filename: String,
    pub primary: bool,
    pub size: u64,
}

impl Version {
    /// The primary file, else the first one.
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }

    /// Milliseconds since the epoch of `date_published` (0 when unreadable), for sorting.
    pub fn published_millis(&self) -> i64 {
        chrono::DateTime::parse_from_rfc3339(&self.date_published).map(|d| d.timestamp_millis()).unwrap_or(0)
    }

    pub fn has_sha1(&self, sha1: &str) -> bool {
        self.files.iter().any(|f| f.sha1().is_some_and(|h| h.eq_ignore_ascii_case(sha1)))
    }
}

impl VersionFile {
    pub fn sha1(&self) -> Option<&str> {
        self.hashes.get("sha1").map(String::as_str)
    }
}

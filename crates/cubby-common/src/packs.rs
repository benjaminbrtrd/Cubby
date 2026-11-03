//! What every modpack provider crate (FTB, Technic, ATLauncher, CurseForge…) hands to the app, so the
//! app browses and installs packs the same way whatever their source. Providers only describe; the app
//! downloads `files`, extracts `overrides` and asks the player for `blocked` files.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The mod loader a pack runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackLoaderKind {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

/// How a search is ordered. A provider that can't sort one way falls back to its default order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackSort {
    #[default]
    Relevance,
    Downloads,
    Updated,
    Newest,
    Name,
}

/// One pack in a search result or a featured list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackSummary {
    /// Provider id, as given back to `versions` and `plan`.
    pub id: String,
    pub name: String,
    /// One-line description, plain text.
    pub summary: String,
    pub authors: Vec<String>,
    pub icon_url: Option<String>,
    /// Page of the pack on the provider's website.
    pub website_url: Option<String>,
    pub downloads: Option<u64>,
    /// RFC 3339.
    pub updated: Option<String>,
    /// Minecraft versions of the pack, newest first, when the provider lists them.
    pub minecraft: Vec<String>,
    pub loaders: Vec<PackLoaderKind>,
}

/// A page of search results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackPage {
    pub hits: Vec<PackSummary>,
    /// Total number of results when the provider knows it.
    pub total: Option<u64>,
}

/// Release channel of a pack version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackChannel {
    Release,
    Beta,
    Alpha,
}

/// One installable version of a pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackVersion {
    /// Provider id of the version, as given back to `plan`.
    pub id: String,
    /// Shown to the player, e.g. "1.12.0".
    pub name: String,
    pub channel: PackChannel,
    /// Empty when only known once the version is planned.
    pub minecraft: Option<String>,
    pub loader: Option<PackLoaderKind>,
    /// RFC 3339.
    pub released: Option<String>,
}

/// One file to download into the game directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackFile {
    /// Relative to the game directory, already checked: no `..`, no absolute path.
    pub path: PathBuf,
    /// Tried in order.
    pub urls: Vec<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// Part of a downloaded archive to copy into the game directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackOverrides {
    /// Archive on disk (a zip), owned by the plan: the app removes it after installing.
    pub archive: PathBuf,
    /// Folder inside the archive whose content goes to the game directory, e.g. "overrides/";
    /// empty for the whole archive.
    pub prefix: String,
}

/// A file the provider doesn't let launchers download; the player gets it from the website.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedFile {
    /// Name as shown to the player and as found in the Downloads folder.
    pub file_name: String,
    /// Page where the player downloads it.
    pub page_url: String,
    /// Where it goes, relative to the game directory.
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// Everything needed to install one version of a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackPlan {
    /// Provider name: "ftb", "technic", "atlauncher", "curseforge", "modrinth".
    pub provider: String,
    pub pack_id: String,
    pub version_id: String,
    pub title: String,
    pub version_name: String,
    pub minecraft: String,
    /// `None` for vanilla packs; the string is the loader version, e.g. "47.2.0".
    pub loader: Option<(PackLoaderKind, String)>,
    pub icon_url: Option<String>,
    pub files: Vec<PackFile>,
    /// Applied in order, after `files`: later ones overwrite earlier ones.
    pub overrides: Vec<PackOverrides>,
    pub blocked: Vec<BlockedFile>,
    /// Memory the pack asks for, in MiB, when it says.
    pub recommended_memory_mib: Option<u32>,
}

/// Rejects absolute paths, `..`, drive letters and empty segments; returns the cleaned relative path.
pub fn safe_relative_path(path: &str) -> Option<PathBuf> {
    let path = path.replace('\\', "/");
    if path.starts_with('/') || path.contains(':') {
        return None;
    }
    let mut out = PathBuf::new();
    for part in path.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            part => out.push(part),
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_paths() {
        assert_eq!(safe_relative_path("mods/a.jar"), Some(PathBuf::from("mods").join("a.jar")));
        assert_eq!(safe_relative_path("./config//x.toml"), Some(PathBuf::from("config").join("x.toml")));
        assert_eq!(safe_relative_path("..\\evil"), None);
        assert_eq!(safe_relative_path("/etc/passwd"), None);
        assert_eq!(safe_relative_path("C:/x"), None);
        assert_eq!(safe_relative_path(""), None);
    }
}

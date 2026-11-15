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

/// What a file listed in a pack is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PackContentKind {
    #[serde(rename = "mod")]
    Mod,
    #[serde(rename = "resourcepack")]
    ResourcePack,
    #[serde(rename = "shader")]
    Shader,
}

impl PackContentKind {
    /// The kind of a file by the folder it goes to (`mods/…`, `resourcepacks/…`, `shaderpacks/…`); `None` for
    /// anything else (configs, scripts…).
    pub fn of_path(path: &str) -> Option<Self> {
        let path = path.replace('\\', "/");
        let path = path.trim_start_matches("./");
        let (folder, rest) = path.split_once('/')?;
        if rest.is_empty() {
            return None;
        }
        match folder.to_ascii_lowercase().as_str() {
            "mods" | "coremods" => Some(Self::Mod),
            "resourcepacks" | "texturepacks" => Some(Self::ResourcePack),
            "shaderpacks" => Some(Self::Shader),
            _ => None,
        }
    }
}

/// Where the project of a listed file lives, when it is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentSite {
    Modrinth,
    Curseforge,
}

/// A mod, resource pack or shader pack a version of a pack installs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackContentItem {
    /// The project's name when known, else what the pack calls the file.
    pub name: String,
    pub file_name: Option<String>,
    /// The file's version number, when the provider gives one.
    pub version: Option<String>,
    pub kind: PackContentKind,
    pub icon_url: Option<String>,
    /// The site `project_id` belongs to.
    pub site: Option<ContentSite>,
    pub project_id: Option<String>,
    /// Web page of the project.
    pub page_url: Option<String>,
    /// The player may leave it out (ATLauncher optional mods, CurseForge `required: false`, Modrinth
    /// `env.client: optional`).
    pub optional: bool,
    /// One line about the project, when its source says.
    pub description: Option<String>,
    pub authors: Vec<String>,
    /// Modrinth-style category keys ("library", "optimization"…), for the kind of mod it is.
    pub categories: Vec<String>,
}

impl PackContentItem {
    /// An item known only by its file name: the name is the file name without its extension.
    pub fn from_file(file_name: &str, kind: PackContentKind) -> Self {
        let stem = file_name.rsplit_once('.').map(|(s, _)| s).filter(|s| !s.is_empty()).unwrap_or(file_name);
        Self {
            name: stem.to_string(),
            file_name: Some(file_name.to_string()).filter(|f| !f.is_empty()),
            version: None,
            kind,
            icon_url: None,
            site: None,
            project_id: None,
            page_url: None,
            optional: false,
            description: None,
            authors: Vec::new(),
            categories: Vec::new(),
        }
    }
}

/// Mods first, then resource packs, then shader packs; by name in each.
pub fn sort_contents(items: &mut [PackContentItem]) {
    let rank = |k: PackContentKind| match k {
        PackContentKind::Mod => 0,
        PackContentKind::ResourcePack => 1,
        PackContentKind::Shader => 2,
    };
    items.sort_by(|a, b| rank(a.kind).cmp(&rank(b.kind)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
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

    #[test]
    fn content_kinds_by_folder() {
        assert_eq!(PackContentKind::of_path("mods/a.jar"), Some(PackContentKind::Mod));
        assert_eq!(PackContentKind::of_path("./mods/a.jar"), Some(PackContentKind::Mod));
        assert_eq!(PackContentKind::of_path("resourcepacks\\Faithful.zip"), Some(PackContentKind::ResourcePack));
        assert_eq!(PackContentKind::of_path("shaderpacks/BSL.zip"), Some(PackContentKind::Shader));
        assert_eq!(PackContentKind::of_path("config/a.toml"), None);
        assert_eq!(PackContentKind::of_path("mods/"), None);
        assert_eq!(PackContentKind::of_path("options.txt"), None);
    }

    #[test]
    fn items_from_files_and_order() {
        let a = PackContentItem::from_file("sodium-0.5.8.jar", PackContentKind::Mod);
        assert_eq!(a.name, "sodium-0.5.8");
        assert_eq!(a.file_name.as_deref(), Some("sodium-0.5.8.jar"));
        let mut items = vec![
            PackContentItem::from_file("Zed.zip", PackContentKind::Shader),
            PackContentItem::from_file("b.zip", PackContentKind::ResourcePack),
            PackContentItem::from_file("Beta.jar", PackContentKind::Mod),
            PackContentItem::from_file("alpha.jar", PackContentKind::Mod),
        ];
        sort_contents(&mut items);
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["alpha", "Beta", "b", "Zed"]);
        let json = serde_json::to_value(&items[0]).unwrap();
        assert_eq!(json["kind"], "mod");
        assert_eq!(json["fileName"], "alpha.jar");
    }
}

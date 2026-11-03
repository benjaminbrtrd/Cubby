//! What the client returns: Modrinth data reshaped for the launcher.

use std::path::PathBuf;

/// Kind of project. Modrinth also has data packs and plugins, which are reported as mods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectKind {
    Mod,
    Modpack,
    Shader,
    ResourcePack,
}

impl ProjectKind {
    /// Modrinth's `project_type` value.
    pub fn as_str(self) -> &'static str {
        match self {
            ProjectKind::Mod => "mod",
            ProjectKind::Modpack => "modpack",
            ProjectKind::Shader => "shader",
            ProjectKind::ResourcePack => "resourcepack",
        }
    }

    pub fn from_api(project_type: &str) -> Self {
        match project_type {
            "modpack" => ProjectKind::Modpack,
            "shader" => ProjectKind::Shader,
            "resourcepack" => ProjectKind::ResourcePack,
            _ => ProjectKind::Mod,
        }
    }

    /// Instance folder its files go to (modpacks have none).
    pub fn folder(self) -> Option<Folder> {
        match self {
            ProjectKind::Mod => Some(Folder::Mods),
            ProjectKind::ResourcePack => Some(Folder::ResourcePacks),
            ProjectKind::Shader => Some(Folder::ShaderPacks),
            ProjectKind::Modpack => None,
        }
    }
}

/// A content folder of the game directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Folder {
    Mods,
    ResourcePacks,
    ShaderPacks,
}

impl Folder {
    pub const ALL: [Folder; 3] = [Folder::Mods, Folder::ResourcePacks, Folder::ShaderPacks];

    pub fn as_str(self) -> &'static str {
        match self {
            Folder::Mods => "mods",
            Folder::ResourcePacks => "resourcepacks",
            Folder::ShaderPacks => "shaderpacks",
        }
    }
}

/// Client / server support, from `client_side` / `server_side`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Required,
    Optional,
    Unsupported,
    Unknown,
}

impl Side {
    pub fn from_api(value: Option<&str>) -> Self {
        match value {
            Some("required") => Side::Required,
            Some("optional") => Side::Optional,
            Some("unsupported") => Side::Unsupported,
            _ => Side::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Side::Required => "required",
            Side::Optional => "optional",
            Side::Unsupported => "unsupported",
            Side::Unknown => "unknown",
        }
    }
}

/// Search sort order, Modrinth's `index` parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

impl Sort {
    pub fn as_str(self) -> &'static str {
        match self {
            Sort::Relevance => "relevance",
            Sort::Downloads => "downloads",
            Sort::Follows => "follows",
            Sort::Newest => "newest",
            Sort::Updated => "updated",
        }
    }

    /// Unknown values sort by relevance.
    pub fn parse(value: &str) -> Self {
        match value {
            "downloads" => Sort::Downloads,
            "follows" => Sort::Follows,
            "newest" => Sort::Newest,
            "updated" => Sort::Updated,
            _ => Sort::Relevance,
        }
    }
}

/// Where the game must run for a project to be listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    /// Client support (required or optional).
    Client,
    /// Server support (required or optional).
    Server,
}

/// A search request.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub query: String,
    pub kind: ProjectKind,
    pub game_version: Option<String>,
    /// Launcher loader key: "vanilla", "fabric", "quilt", "forge", "neoforge".
    pub loader: Option<String>,
    /// Category slugs, all required.
    pub categories: Vec<String>,
    pub environment: Option<Environment>,
    pub sort: Sort,
    pub offset: u32,
    /// Clamped to 1..=100.
    pub limit: u32,
}

/// The game version and loader content is installed for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub game_version: String,
    /// Modrinth loader of the instance ("fabric", "quilt", "forge", "neoforge"), `None` for vanilla.
    pub loader: Option<String>,
}

impl Target {
    /// `loader` is the launcher key; "vanilla" (or nothing) means no mod loader.
    pub fn new(game_version: impl Into<String>, loader: Option<&str>) -> Self {
        Self { game_version: game_version.into(), loader: loader.filter(|l| !l.is_empty() && *l != "vanilla").map(str::to_owned) }
    }

    /// Loaders whose mods run on this instance: a Quilt instance also runs Fabric mods.
    pub fn mod_loaders(&self) -> Vec<String> {
        mod_loaders(self.loader.as_deref())
    }
}

/// Loaders whose mods run with `loader` (launcher key); empty for vanilla.
pub fn mod_loaders(loader: Option<&str>) -> Vec<String> {
    match loader {
        None | Some("") | Some("vanilla") => Vec::new(),
        Some("quilt") => vec!["quilt".into(), "fabric".into()],
        Some(other) => vec![other.to_string()],
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectHit {
    pub id: String,
    pub slug: String,
    pub kind: ProjectKind,
    pub title: String,
    pub author: String,
    pub summary: String,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
    pub downloads: u64,
    pub follows: u64,
    /// ISO 8601
    pub updated: String,
    pub published: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub client_side: Side,
    pub server_side: Side,
    pub license: Option<String>,
    pub featured_image: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResults {
    pub hits: Vec<ProjectHit>,
    pub total: u64,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GalleryImage {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub featured: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Donation {
    pub platform: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Links {
    pub source: Option<String>,
    pub issues: Option<String>,
    pub wiki: Option<String>,
    pub discord: Option<String>,
    pub donation: Vec<Donation>,
}

/// A full project page.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub hit: ProjectHit,
    /// Markdown, may contain HTML.
    pub body: String,
    pub gallery: Vec<GalleryImage>,
    pub links: Links,
    pub version_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DependencyInfo {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    /// "required", "optional", "incompatible" or "embedded".
    pub kind: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileInfo {
    pub url: String,
    pub file_name: String,
    pub size: u64,
    pub primary: bool,
    pub sha1: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectVersion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    /// "release", "beta" or "alpha".
    pub channel: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub published: String,
    pub downloads: u64,
    pub changelog: String,
    pub files: Vec<FileInfo>,
    pub dependencies: Vec<DependencyInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Requested,
    Dependency,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Requested => "requested",
            Reason::Dependency => "dependency",
        }
    }
}

/// One project of an install plan.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanItem {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
    /// Name of the file to download, or of the file already there when nothing is downloaded.
    pub file_name: String,
    pub size: u64,
    /// Download URL and SHA-1 (empty when nothing is downloaded).
    pub url: String,
    pub sha1: String,
    pub reason: Reason,
    /// The project is already in the instance (this version or another one).
    pub installed: bool,
    pub folder: Folder,
    /// The file is downloaded when the plan runs: not installed, or installed in another version
    /// (requested project only; dependencies already present are kept as they are).
    pub download: bool,
    /// The file of the other installed version, removed once the new one is in place.
    pub replaces: Option<(Folder, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MissingDependency {
    pub project_id: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    pub project_id: String,
    pub title: Option<String>,
    /// The installed file.
    pub file_name: String,
}

/// What installing a version would do.
#[derive(Debug, Clone, PartialEq)]
pub struct InstallPlan {
    /// The requested project first, then its dependencies.
    pub items: Vec<PlanItem>,
    pub missing: Vec<MissingDependency>,
    pub conflicts: Vec<Conflict>,
    /// Sum of the sizes of the items that are downloaded.
    pub download_bytes: u64,
}

/// A file of the instance identified on Modrinth.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentSource {
    pub file_name: String,
    pub folder: Folder,
    pub project_id: String,
    pub version_id: String,
    pub slug: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub version_number: String,
}

/// A newer compatible version of an installed file.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentUpdate {
    pub file_name: String,
    pub folder: Folder,
    pub project_id: String,
    pub title: String,
    pub current_version: String,
    pub latest_version_id: String,
    pub latest_version: String,
    pub changelog: String,
}

/// A content file replaced by a file with another name (to update the game's pack selection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renamed {
    pub folder: Folder,
    pub old: String,
    pub new: String,
}

/// What running downloads changed. `error` is the first failure: the files that downloaded fine are
/// installed (and listed in `renamed` when they replaced a file with another name) even then.
#[derive(Debug, Default)]
pub struct Applied {
    pub renamed: Vec<Renamed>,
    pub error: Option<cubby_common::Error>,
}

impl Applied {
    pub fn into_result(self) -> cubby_common::Result<Vec<Renamed>> {
        match self.error {
            Some(e) => Err(e),
            None => Ok(self.renamed),
        }
    }
}

/// A downloaded `.mrpack`, ready to be installed into a new instance.
#[derive(Debug, Clone)]
pub struct Modpack {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
    pub icon_url: Option<String>,
    pub minecraft: String,
    /// The pack's loader and its version, `None` for vanilla.
    pub loader: Option<(crate::mrpack::PackLoader, String)>,
    pub index: crate::mrpack::MrpackIndex,
    /// The downloaded `.mrpack` file.
    pub archive: PathBuf,
}

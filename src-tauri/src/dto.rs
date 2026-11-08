//! Types sent to the web UI; keep in sync with `ui/src/lib/types.ts`.

use std::path::Path;

use chrono::{DateTime, Utc};
use cubby_auth::AccountKind;
use cubby_common::Error;
use cubby_instance::{JavaOverrides, Loader, ModpackInfo};
use cubby_modrinth as modrinth;
use serde::{Deserialize, Serialize};

pub fn message(e: Error) -> String {
    match e {
        Error::Http { url, .. } => format!("Can't reach {}. Check your connection.", host(&url)),
        Error::HttpStatus { url, status } => format!(
            "{} returned an error (HTTP {status}). Try again later.",
            host(&url)
        ),
        Error::HashMismatch { path, .. } => {
            format!("{} didn't download correctly. Try again.", path.display())
        }
        Error::Io { path, source } => format!("Couldn't access {}: {source}", path.display()),
        Error::Json { what, .. } => format!("Couldn't read the data from {what}."),
        Error::Toml { path, message } => format!(
            "Couldn't read the settings in {}: {message}",
            path.display()
        ),
        Error::NotFound(what) => format!("Couldn't find {what}."),
        Error::Auth(msg) | Error::Java(msg) | Error::Launch(msg) | Error::Other(msg) => msg,
        Error::Cancelled => "Operation canceled.".into(),
    }
}

fn host(url: &str) -> &str {
    url.split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or(url)
}

/// URL under which the webview can load a local file through Tauri's asset protocol
/// (same result as `convertFileSrc` on the JS side).
pub fn asset_url(path: &Path) -> String {
    let encoded = encode_uri_component(&path.to_string_lossy());
    if cfg!(any(windows, target_os = "android")) {
        format!("http://asset.localhost/{encoded}")
    } else {
        format!("asset://localhost/{encoded}")
    }
}

fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderDto {
    pub kind: String,
    #[serde(default)]
    pub version: Option<String>,
}

impl From<&Loader> for LoaderDto {
    fn from(l: &Loader) -> Self {
        let (kind, version) = match l {
            Loader::Vanilla => ("vanilla", None),
            Loader::Fabric { version } => ("fabric", Some(version)),
            Loader::Quilt { version } => ("quilt", Some(version)),
            Loader::NeoForge { version } => ("neoforge", Some(version)),
            Loader::Forge { version } => ("forge", Some(version)),
        };
        Self {
            kind: kind.into(),
            version: version.cloned(),
        }
    }
}

impl TryFrom<LoaderDto> for Loader {
    type Error = String;
    fn try_from(l: LoaderDto) -> Result<Self, String> {
        let version = || {
            l.version
                .clone()
                .filter(|v| !v.is_empty())
                .ok_or_else(|| "Choose a loader version.".to_string())
        };
        Ok(match l.kind.as_str() {
            "vanilla" => Loader::Vanilla,
            "fabric" => Loader::Fabric {
                version: version()?,
            },
            "quilt" => Loader::Quilt {
                version: version()?,
            },
            "neoforge" => Loader::NeoForge {
                version: version()?,
            },
            "forge" => Loader::Forge {
                version: version()?,
            },
            other => return Err(format!("Unknown loader: {other}.")),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaOverridesDto {
    pub executable: Option<String>,
    pub min_memory_mb: Option<u32>,
    pub max_memory_mb: Option<u32>,
    pub jvm_args: Option<String>,
}

impl From<&JavaOverrides> for JavaOverridesDto {
    fn from(j: &JavaOverrides) -> Self {
        Self {
            executable: j
                .executable
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            min_memory_mb: j.min_memory_mb,
            max_memory_mb: j.max_memory_mb,
            jvm_args: j.jvm_args.clone(),
        }
    }
}

impl From<JavaOverridesDto> for JavaOverrides {
    fn from(j: JavaOverridesDto) -> Self {
        let text = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        Self {
            executable: text(j.executable).map(Into::into),
            min_memory_mb: j.min_memory_mb,
            max_memory_mb: j.max_memory_mb,
            jvm_args: text(j.jvm_args),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceDto {
    pub id: String,
    pub name: String,
    pub group: Option<String>,
    /// Built-in block key, or an asset URL when the instance has its own image.
    pub icon: Option<String>,
    pub minecraft: String,
    pub loader: LoaderDto,
    pub created: DateTime<Utc>,
    pub last_played: Option<DateTime<Utc>>,
    pub total_played_secs: u64,
    #[serde(default)]
    pub installed: bool,
    pub java_overrides: JavaOverridesDto,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub modpack: Option<ModpackDto>,
    /// Files of the modpack the player still has to download by hand (0 normally).
    #[serde(default)]
    pub blocked_files: u32,
    /// The memory Cubby would pick for this instance (MB), from its mods and the computer's memory.
    #[serde(default)]
    pub recommended_memory_mb: u32,
}

impl InstanceDto {
    pub fn new(
        inst: &cubby_instance::Instance,
        instance_dir: &Path,
        installed: bool,
        blocked_files: u32,
    ) -> Self {
        Self {
            id: inst.id.clone(),
            name: inst.name.clone(),
            group: inst.group.clone(),
            icon: inst
                .icon
                .as_ref()
                .map(|icon| icon_value(icon, instance_dir)),
            minecraft: inst.minecraft.clone(),
            loader: (&inst.loader).into(),
            created: inst.created,
            last_played: inst.last_played,
            total_played_secs: inst.total_played_secs,
            installed,
            java_overrides: (&inst.java).into(),
            notes: inst.notes.clone(),
            modpack: inst.modpack.as_ref().map(Into::into),
            blocked_files,
            recommended_memory_mb: 0,
        }
    }
}

/// The Modrinth modpack an instance was installed from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackDto {
    /// "modrinth", "curseforge", "ftb", "atlauncher" or "technic"; absent in old instances (Modrinth).
    #[serde(default)]
    pub provider: Option<String>,
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
}

impl From<&ModpackInfo> for ModpackDto {
    fn from(m: &ModpackInfo) -> Self {
        Self {
            provider: m.provider.clone(),
            project_id: m.project_id.clone(),
            version_id: m.version_id.clone(),
            title: m.title.clone(),
            version_number: m.version_number.clone(),
        }
    }
}

impl From<ModpackDto> for ModpackInfo {
    fn from(m: ModpackDto) -> Self {
        Self {
            provider: m.provider,
            project_id: m.project_id,
            version_id: m.version_id,
            title: m.title,
            version_number: m.version_number,
        }
    }
}

/// An icon stored as a file name inside the instance folder becomes an asset URL; keys pass through.
fn icon_value(icon: &str, instance_dir: &Path) -> String {
    if is_icon_file(icon) {
        let path = instance_dir.join(icon);
        if path.is_file() {
            return asset_url(&path);
        }
    }
    icon.to_string()
}

pub fn is_icon_file(icon: &str) -> bool {
    icon.contains('.') && !icon.contains(['/', '\\'])
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewInstanceDto {
    pub name: String,
    pub group: Option<String>,
    pub icon: Option<String>,
    pub minecraft: String,
    pub loader: LoaderDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    pub id: String,
    pub username: String,
    pub kind: &'static str,
    pub is_default: bool,
    pub skin_url: Option<String>,
}

impl AccountDto {
    pub fn new(a: &cubby_auth::Account, is_default: bool) -> Self {
        let AccountKind::Microsoft { skin_url, .. } = &a.kind;
        Self {
            id: a.id.clone(),
            username: a.username.clone(),
            kind: "microsoft",
            is_default,
            skin_url: skin_url.as_deref().map(https),
        }
    }
}

/// Mojang hands out `http://textures.minecraft.net/...`; the app only lets images in over https.
pub fn https(url: &str) -> String {
    match url.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => url.to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeDto {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

impl From<&cubby_auth::DeviceCode> for DeviceCodeDto {
    fn from(c: &cubby_auth::DeviceCode) -> Self {
        Self {
            user_code: c.user_code.clone(),
            verification_uri: c.verification_uri.clone(),
            expires_in: c.expires_in,
            interval: c.interval,
            message: c.message.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameVersionDto {
    pub id: String,
    pub kind: String,
    pub release_time: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFileDto {
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
}

impl From<cubby_instance::ModFile> for ModFileDto {
    fn from(m: cubby_instance::ModFile) -> Self {
        let meta = m.meta;
        Self {
            file_name: m.file_name,
            enabled: m.enabled,
            size: m.size,
            name: meta.as_ref().map(|x| x.name.clone()),
            version: meta.as_ref().and_then(|x| x.version.clone()),
            description: meta.as_ref().and_then(|x| x.description.clone()),
            authors: meta.map(|x| x.authors).unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldDto {
    pub folder: String,
    pub name: String,
    pub last_played: Option<DateTime<Utc>>,
    pub size_bytes: u64,
    pub icon_url: Option<String>,
}

impl WorldDto {
    pub fn new(w: cubby_instance::WorldInfo, saves_dir: &Path) -> Self {
        let icon_url = w
            .has_icon
            .then(|| asset_url(&saves_dir.join(&w.folder_name).join("icon.png")));
        Self {
            folder: w.folder_name,
            name: w.display_name,
            last_played: w.last_played,
            size_bytes: w.size,
            icon_url,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotDto {
    pub file_name: String,
    pub url: String,
    pub taken: DateTime<Utc>,
}

impl From<cubby_instance::ScreenshotFile> for ScreenshotDto {
    fn from(s: cubby_instance::ScreenshotFile) -> Self {
        Self {
            url: asset_url(&s.path),
            file_name: s.file_name,
            taken: s.modified.unwrap_or_else(Utc::now),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrismInstanceDto {
    pub path: String,
    pub name: String,
    pub minecraft: String,
    pub loader: LoaderDto,
    pub group: Option<String>,
}

impl From<cubby_instance::PrismInstanceInfo> for PrismInstanceDto {
    fn from(p: cubby_instance::PrismInstanceInfo) -> Self {
        Self {
            path: p.path.to_string_lossy().into_owned(),
            name: p.name,
            minecraft: p.minecraft.unwrap_or_else(|| "?".into()),
            loader: (&p.loader).into(),
            group: p.group,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    pub version: &'static str,
    pub data_dir: String,
    pub msa_configured: bool,
    /// Where the Microsoft client id comes from: "settings", "environment", "builtIn", or none.
    pub msa_source: Option<&'static str>,
    /// Where the CurseForge API key comes from: "settings", "environment", "builtIn", or none.
    pub curseforge_key: Option<&'static str>,
    pub total_memory_mb: u64,
    /// Development build: without an account the full game starts offline instead of the demo.
    pub dev_build: bool,
    /// This build can show the game on Discord (it has a Discord application id).
    pub discord: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataFolderDto {
    pub key: &'static str,
    pub label: &'static str,
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatusDto {
    pub online: bool,
    pub motd: String,
    /// The message of the day with its `§` colour and style codes.
    pub motd_formatted: String,
    pub players_online: u32,
    pub players_max: u32,
    pub version: Option<String>,
    pub ping_ms: Option<u32>,
    pub icon_url: Option<String>,
    /// Whether the server takes the Minecraft version of the instance pinged with; `None` when it cannot be known.
    pub compatible: Option<bool>,
}

impl From<cubby_launch::ServerStatus> for ServerStatusDto {
    fn from(s: cubby_launch::ServerStatus) -> Self {
        Self {
            online: s.online,
            motd: s.motd,
            motd_formatted: s.motd_formatted,
            players_online: s.players_online,
            players_max: s.players_max,
            version: s.version,
            ping_ms: s.ping_ms,
            icon_url: s.favicon,
            compatible: None,
        }
    }
}

const PNG_DATA_URL: &str = "data:image/png;base64,";

/// An entry of servers.dat; the game's base64 PNG icon travels as a data: URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerDto {
    pub name: String,
    pub address: String,
    pub icon_url: Option<String>,
    pub accept_textures: Option<bool>,
}

impl From<cubby_instance::ServerEntry> for ServerDto {
    fn from(s: cubby_instance::ServerEntry) -> Self {
        Self {
            name: s.name,
            address: s.address,
            icon_url: s.icon.map(|b64| format!("{PNG_DATA_URL}{b64}")),
            accept_textures: s.accept_textures,
        }
    }
}

impl From<ServerDto> for cubby_instance::ServerEntry {
    fn from(s: ServerDto) -> Self {
        Self {
            name: s.name.trim().to_string(),
            address: s.address.trim().to_string(),
            icon: s
                .icon_url
                .and_then(|u| u.strip_prefix(PNG_DATA_URL).map(str::to_owned)),
            accept_textures: s.accept_textures,
        }
    }
}

// Discover (Modrinth)

fn default_limit() -> u32 {
    20
}

/// `ProjectQuery` in types.ts.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectQueryDto {
    #[serde(default)]
    pub query: String,
    pub kind: String,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub loader: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub sort: String,
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn project_kind(kind: &str) -> Result<modrinth::ProjectKind, String> {
    match kind {
        "mod" => Ok(modrinth::ProjectKind::Mod),
        "modpack" => Ok(modrinth::ProjectKind::Modpack),
        "shader" => Ok(modrinth::ProjectKind::Shader),
        "resourcepack" => Ok(modrinth::ProjectKind::ResourcePack),
        "datapack" => Ok(modrinth::ProjectKind::Datapack),
        other => Err(format!("Unknown project type: {other}.")),
    }
}

impl TryFrom<ProjectQueryDto> for modrinth::SearchQuery {
    type Error = String;
    fn try_from(q: ProjectQueryDto) -> Result<Self, String> {
        let environment = match q.environment.as_deref() {
            None | Some("") => None,
            Some("client") => Some(modrinth::Environment::Client),
            Some("server") => Some(modrinth::Environment::Server),
            Some(other) => return Err(format!("Unknown environment: {other}.")),
        };
        Ok(Self {
            query: q.query,
            kind: project_kind(&q.kind)?,
            game_version: q.game_version.filter(|v| !v.trim().is_empty()),
            loader: q.loader.filter(|l| !l.trim().is_empty()),
            categories: q.categories,
            environment,
            sort: modrinth::Sort::parse(&q.sort),
            offset: q.offset,
            limit: q.limit,
        })
    }
}

/// `ProjectHit` in types.ts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHitDto {
    pub id: String,
    pub slug: String,
    pub kind: &'static str,
    pub title: String,
    pub author: String,
    pub summary: String,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
    pub downloads: u64,
    pub follows: u64,
    pub updated: String,
    pub published: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub client_side: &'static str,
    pub server_side: &'static str,
    pub license: Option<String>,
    pub featured_image: Option<String>,
}

impl From<modrinth::ProjectHit> for ProjectHitDto {
    fn from(h: modrinth::ProjectHit) -> Self {
        Self {
            id: h.id,
            slug: h.slug,
            kind: h.kind.as_str(),
            title: h.title,
            author: h.author,
            summary: h.summary,
            icon_url: h.icon_url,
            categories: h.categories,
            downloads: h.downloads,
            follows: h.follows,
            updated: h.updated,
            published: h.published,
            game_versions: h.game_versions,
            loaders: h.loaders,
            client_side: h.client_side.as_str(),
            server_side: h.server_side.as_str(),
            license: h.license,
            featured_image: h.featured_image,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultsDto {
    pub hits: Vec<ProjectHitDto>,
    pub total: u64,
    pub offset: u32,
    pub limit: u32,
}

impl From<modrinth::SearchResults> for SearchResultsDto {
    fn from(r: modrinth::SearchResults) -> Self {
        Self {
            hits: r.hits.into_iter().map(Into::into).collect(),
            total: r.total,
            offset: r.offset,
            limit: r.limit,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryImageDto {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub featured: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DonationDto {
    pub platform: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLinksDto {
    pub source: Option<String>,
    pub issues: Option<String>,
    pub wiki: Option<String>,
    pub discord: Option<String>,
    pub donation: Vec<DonationDto>,
}

/// `Project` in types.ts: a `ProjectHit` plus the page content.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDto {
    #[serde(flatten)]
    pub hit: ProjectHitDto,
    pub body: String,
    pub gallery: Vec<GalleryImageDto>,
    pub links: ProjectLinksDto,
    pub version_count: usize,
}

impl From<modrinth::Project> for ProjectDto {
    fn from(p: modrinth::Project) -> Self {
        Self {
            hit: p.hit.into(),
            body: p.body,
            gallery: p
                .gallery
                .into_iter()
                .map(|g| GalleryImageDto {
                    url: g.url,
                    title: g.title,
                    description: g.description,
                    featured: g.featured,
                })
                .collect(),
            links: ProjectLinksDto {
                source: p.links.source,
                issues: p.links.issues,
                wiki: p.links.wiki,
                discord: p.links.discord,
                donation: p
                    .links
                    .donation
                    .into_iter()
                    .map(|d| DonationDto {
                        platform: d.platform,
                        url: d.url,
                    })
                    .collect(),
            },
            version_count: p.version_count,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyDto {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub kind: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionFileDto {
    pub url: String,
    pub file_name: String,
    pub size: u64,
    pub primary: bool,
    pub sha1: String,
}

/// `ProjectVersion` in types.ts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVersionDto {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub channel: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub published: String,
    pub downloads: u64,
    pub changelog: String,
    pub files: Vec<VersionFileDto>,
    pub dependencies: Vec<DependencyDto>,
}

impl From<modrinth::ProjectVersion> for ProjectVersionDto {
    fn from(v: modrinth::ProjectVersion) -> Self {
        Self {
            id: v.id,
            project_id: v.project_id,
            name: v.name,
            version_number: v.version_number,
            channel: v.channel,
            game_versions: v.game_versions,
            loaders: v.loaders,
            published: v.published,
            downloads: v.downloads,
            changelog: v.changelog,
            files: v
                .files
                .into_iter()
                .map(|f| VersionFileDto {
                    url: f.url,
                    file_name: f.file_name,
                    size: f.size,
                    primary: f.primary,
                    sha1: f.sha1,
                })
                .collect(),
            dependencies: v
                .dependencies
                .into_iter()
                .map(|d| DependencyDto {
                    project_id: d.project_id,
                    version_id: d.version_id,
                    kind: d.kind,
                    title: d.title,
                    slug: d.slug,
                    icon_url: d.icon_url,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallItemDto {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub version_number: String,
    pub file_name: String,
    pub size: u64,
    pub reason: &'static str,
    pub installed: bool,
    pub folder: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingDependencyDto {
    pub project_id: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictDto {
    pub project_id: String,
    pub title: Option<String>,
    pub file_name: String,
}

/// `InstallPlan` in types.ts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlanDto {
    pub instance_id: String,
    pub items: Vec<InstallItemDto>,
    pub missing: Vec<MissingDependencyDto>,
    pub conflicts: Vec<ConflictDto>,
    pub download_bytes: u64,
}

impl InstallPlanDto {
    pub fn new(instance_id: &str, plan: modrinth::InstallPlan) -> Self {
        Self {
            instance_id: instance_id.to_string(),
            items: plan
                .items
                .into_iter()
                .map(|i| InstallItemDto {
                    project_id: i.project_id,
                    version_id: i.version_id,
                    title: i.title,
                    version_number: i.version_number,
                    file_name: i.file_name,
                    size: i.size,
                    reason: i.reason.as_str(),
                    installed: i.installed,
                    folder: i.folder.as_str(),
                })
                .collect(),
            missing: plan
                .missing
                .into_iter()
                .map(|m| MissingDependencyDto {
                    project_id: m.project_id,
                    title: m.title,
                })
                .collect(),
            conflicts: plan
                .conflicts
                .into_iter()
                .map(|c| ConflictDto {
                    project_id: c.project_id,
                    title: c.title,
                    file_name: c.file_name,
                })
                .collect(),
            download_bytes: plan.download_bytes,
        }
    }
}

/// `ContentSource` in types.ts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSourceDto {
    pub file_name: String,
    pub project_id: String,
    pub version_id: String,
    pub slug: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub version_number: String,
}

impl From<modrinth::ContentSource> for ContentSourceDto {
    fn from(s: modrinth::ContentSource) -> Self {
        Self {
            file_name: s.file_name,
            project_id: s.project_id,
            version_id: s.version_id,
            slug: s.slug,
            title: s.title,
            icon_url: s.icon_url,
            version_number: s.version_number,
        }
    }
}

/// `ContentUpdate` in types.ts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentUpdateDto {
    pub file_name: String,
    pub project_id: String,
    pub title: String,
    pub current_version: String,
    pub latest_version_id: String,
    pub latest_version: String,
    pub changelog: String,
}

impl From<modrinth::ContentUpdate> for ContentUpdateDto {
    fn from(u: modrinth::ContentUpdate) -> Self {
        Self {
            file_name: u.file_name,
            project_id: u.project_id,
            title: u.title,
            current_version: u.current_version,
            latest_version_id: u.latest_version_id,
            latest_version: u.latest_version,
            changelog: u.changelog,
        }
    }
}

// World backups

/// `WorldBackup` in types.ts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldBackupDto {
    pub file_name: String,
    pub world: String,
    pub created: DateTime<Utc>,
    pub size_bytes: u64,
}

impl From<cubby_instance::WorldBackup> for WorldBackupDto {
    fn from(b: cubby_instance::WorldBackup) -> Self {
        Self {
            file_name: b.file_name,
            world: b.world,
            created: b.created,
            size_bytes: b.size,
        }
    }
}

/// `VersionBrief` in types.ts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionBriefDto {
    pub version_id: String,
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub version_number: String,
    pub project_type: String,
    pub size: u64,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
}

impl From<modrinth::VersionBrief> for VersionBriefDto {
    fn from(v: modrinth::VersionBrief) -> Self {
        Self {
            version_id: v.version_id,
            project_id: v.project_id,
            slug: v.slug,
            title: v.title,
            icon_url: v.icon_url,
            version_number: v.version_number,
            project_type: v.project_type,
            size: v.size,
            game_versions: v.game_versions,
            loaders: v.loaders,
        }
    }
}

/// `SkinProfile` in types.ts: the skin an account wears and the capes it owns.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinProfileDto {
    pub skin_url: Option<String>,
    /// "classic" or "slim".
    pub variant: String,
    pub capes: Vec<CapeDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapeDto {
    pub id: String,
    pub name: String,
    pub url: String,
    pub active: bool,
}

impl From<cubby_auth::xbox::Profile> for SkinProfileDto {
    fn from(p: cubby_auth::xbox::Profile) -> Self {
        let active = p.skins.iter().find(|s| s.state.eq_ignore_ascii_case("ACTIVE"));
        Self {
            skin_url: active.map(|s| https(&s.url)),
            variant: if active.is_some_and(|s| s.variant.eq_ignore_ascii_case("SLIM")) { "slim".into() } else { "classic".into() },
            capes: p
                .capes
                .into_iter()
                .map(|c| CapeDto { active: c.state.eq_ignore_ascii_case("ACTIVE"), name: if c.alias.is_empty() { c.id.clone() } else { c.alias }, id: c.id, url: https(&c.url) })
                .collect(),
        }
    }
}

/// `DefaultSkin` in types.ts: a skin of the game, with the PNG of each arm model.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultSkinDto {
    pub id: String,
    pub name: String,
    pub wide: Vec<u8>,
    pub slim: Vec<u8>,
}

impl From<cubby_instance::skins::DefaultSkin> for DefaultSkinDto {
    fn from(s: cubby_instance::skins::DefaultSkin) -> Self {
        Self { id: s.id, name: s.name, wide: s.wide, slim: s.slim }
    }
}

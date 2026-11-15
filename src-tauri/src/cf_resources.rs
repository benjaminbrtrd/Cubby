//! CurseForge mods, resource packs, shader packs and datapacks for Discover: searched with CurseForge's own
//! filters, shaped like the Modrinth results so the same cards show them, and installed straight into the
//! instance (or the world, for a datapack) with the projects they require. Modpacks have their own commands in
//! `packs.rs`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use cubby_common::net::Download;
use cubby_common::packs::{PackChannel, PackLoaderKind, PackSort};
use cubby_curseforge::{best_file, CfFile, CurseForgeClient, FileTarget, ResourceKind, ResourceQuery};
use cubby_instance::Instance;
use cubby_modrinth::{Folder, InstallPlan, PlanItem, Reason};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::commands::{download_concurrency, ensure_stopped, run_task, CmdResult, OrMessage, PackSelection};
use crate::dto::{GalleryImageDto, ProjectDto, ProjectHitDto, ProjectLinksDto, ProjectQueryDto, SearchResultsDto};
use crate::state::Core;

const KEY_MESSAGE: &str = "CurseForge needs an API key. Add yours in Settings.";

fn client(core: &Core) -> CmdResult<CurseForgeClient> {
    core.curseforge().ok_or_else(|| KEY_MESSAGE.to_string())
}

fn kind_of(kind: &str) -> CmdResult<ResourceKind> {
    match kind {
        "mod" => Ok(ResourceKind::Mod),
        "shader" => Ok(ResourceKind::Shader),
        "resourcepack" => Ok(ResourceKind::ResourcePack),
        "datapack" => Ok(ResourceKind::Datapack),
        "modpack" => Ok(ResourceKind::Modpack),
        other => Err(format!("Unknown project type: {other}.")),
    }
}

fn kind_name(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Mod => "mod",
        ResourceKind::Shader => "shader",
        ResourceKind::ResourcePack => "resourcepack",
        ResourceKind::Datapack => "datapack",
        ResourceKind::Modpack => "modpack",
    }
}

pub(crate) fn loader_of(key: &str) -> Option<PackLoaderKind> {
    match key.trim().to_ascii_lowercase().as_str() {
        "fabric" => Some(PackLoaderKind::Fabric),
        "quilt" => Some(PackLoaderKind::Quilt),
        "forge" => Some(PackLoaderKind::Forge),
        "neoforge" => Some(PackLoaderKind::NeoForge),
        _ => None,
    }
}

/// Loaders whose mods run with the launcher loader `key`: a Quilt instance also runs Fabric mods.
fn runs_with(key: &str) -> Vec<PackLoaderKind> {
    match loader_of(key) {
        Some(PackLoaderKind::Quilt) => vec![PackLoaderKind::Quilt, PackLoaderKind::Fabric],
        Some(l) => vec![l],
        None => Vec::new(),
    }
}

fn sort_of(sort: &str) -> PackSort {
    match sort {
        "downloads" | "follows" => PackSort::Downloads,
        "updated" => PackSort::Updated,
        "newest" => PackSort::Newest,
        "name" => PackSort::Name,
        _ => PackSort::Relevance,
    }
}

fn loader_name(kind: PackLoaderKind) -> &'static str {
    match kind {
        PackLoaderKind::Fabric => "fabric",
        PackLoaderKind::Quilt => "quilt",
        PackLoaderKind::Forge => "forge",
        PackLoaderKind::NeoForge => "neoforge",
    }
}

fn channel_name(c: PackChannel) -> &'static str {
    match c {
        PackChannel::Release => "release",
        PackChannel::Beta => "beta",
        PackChannel::Alpha => "alpha",
    }
}

fn parse_id(id: &str, what: &str) -> CmdResult<u64> {
    id.trim().parse().map_err(|_| format!("{id:?} isn't a valid CurseForge {what} ID."))
}

fn instance_loader(inst: &Instance) -> String {
    crate::dto::LoaderDto::from(&inst.loader).kind
}

// Search

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDto {
    pub id: String,
    pub name: String,
}

/// Filters on top of a search query, applied by the source itself (CurseForge or Modrinth): several
/// Minecraft versions and loaders, and (CurseForge) category ids.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchFiltersDto {
    /// Minecraft versions (CurseForge takes 4 at most).
    pub game_versions: Vec<String>,
    /// Launcher loader keys ("fabric", "neoforge"...).
    pub loaders: Vec<String>,
    /// Category ids (CurseForge takes 10 at most).
    pub categories: Vec<String>,
}

impl SearchFiltersDto {
    fn loaders(&self) -> Vec<PackLoaderKind> {
        self.loaders.iter().filter_map(|l| loader_of(l)).collect()
    }

    fn categories(&self) -> Vec<u64> {
        self.categories.iter().filter_map(|c| c.trim().parse().ok()).collect()
    }
}

/// The categories of a kind of project on CurseForge.
#[tauri::command]
pub async fn curseforge_categories(core: State<'_, Core>, kind: String) -> CmdResult<Vec<CategoryDto>> {
    let categories = client(&core)?.resource_categories(kind_of(&kind)?).await.msg()?;
    Ok(categories.into_iter().map(|c| CategoryDto { id: c.id.to_string(), name: c.name }).collect())
}

/// Searches CurseForge with the query the Modrinth search takes, plus CurseForge's own filters. `query.categories`
/// are CurseForge category ids. The instance's version and loader (`query.gameVersion`, `query.loader`) apply
/// when the filters name none.
#[tauri::command]
pub async fn curseforge_search(core: State<'_, Core>, query: ProjectQueryDto, filters: Option<SearchFiltersDto>) -> CmdResult<SearchResultsDto> {
    let filters = filters.unwrap_or_default();
    let kind = kind_of(&query.kind)?;
    let game_versions = if filters.game_versions.is_empty() { query.game_version.clone().into_iter().collect() } else { filters.game_versions.clone() };
    let loaders = if filters.loaders.is_empty() { query.loader.as_deref().map(runs_with).unwrap_or_default() } else { filters.loaders() };
    let mut categories: Vec<u64> = query.categories.iter().filter_map(|c| c.trim().parse().ok()).collect();
    categories.extend(filters.categories());
    let wanted = ResourceQuery { kind, text: query.query.clone(), game_versions, loaders, categories, sort: sort_of(&query.sort), offset: query.offset, limit: query.limit };
    let page = client(&core)?.search_resources(&wanted).await.msg()?;
    let hits: Vec<ProjectHitDto> = page.hits.into_iter().map(|h| hit_dto(h, kind)).collect();
    let total = page.total.unwrap_or(hits.len() as u64);
    Ok(SearchResultsDto { hits, total, offset: query.offset, limit: query.limit })
}

/// A search hit or project summary in the shape of a Modrinth hit.
fn hit_dto(h: cubby_common::packs::PackSummary, kind: ResourceKind) -> ProjectHitDto {
    let updated = h.updated.clone().unwrap_or_default();
    ProjectHitDto {
        id: h.id,
        slug: h.website_url.as_deref().and_then(|u| u.trim_end_matches('/').rsplit('/').next()).unwrap_or_default().to_string(),
        kind: kind_name(kind),
        title: h.name,
        author: h.authors.first().cloned().unwrap_or_default(),
        summary: h.summary,
        icon_url: h.icon_url,
        categories: Vec::new(),
        downloads: h.downloads.unwrap_or(0),
        follows: 0,
        published: updated.clone(),
        updated,
        game_versions: h.minecraft,
        loaders: h.loaders.into_iter().map(|l| loader_name(l).to_string()).collect(),
        client_side: "unknown",
        server_side: "unknown",
        license: None,
        featured_image: None,
    }
}

/// A CurseForge project page, in the shape of a Modrinth one (`Project` in types.ts) plus where it comes from.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfProjectPageDto {
    #[serde(flatten)]
    pub project: ProjectDto,
    /// Always "curseforge".
    pub source: &'static str,
    /// The project's page on curseforge.com.
    pub page_url: String,
}

/// A CurseForge project page: summary, description (HTML, sanitized by the page), screenshots and links.
#[tauri::command]
pub async fn curseforge_project(core: State<'_, Core>, project_id: String, kind: String) -> CmdResult<CfProjectPageDto> {
    let kind = kind_of(&kind)?;
    let p = client(&core)?.pack(parse_id(&project_id, "project")?).await.msg()?;
    let page_url = p.links.website.clone().or_else(|| p.summary.website_url.clone()).unwrap_or_else(|| format!("https://www.curseforge.com/projects/{project_id}"));
    let mut hit = hit_dto(p.summary, kind);
    if hit.slug.is_empty() {
        hit.slug = p.slug.clone();
    }
    hit.categories = p.categories;
    if let Some(created) = p.created.filter(|c| !c.is_empty()) {
        hit.published = created;
    }
    hit.featured_image = p.screenshots.first().map(|s| s.url.clone());
    let project = ProjectDto {
        hit,
        body: p.description_html,
        gallery: p
            .screenshots
            .iter()
            .enumerate()
            .map(|(i, s)| GalleryImageDto { url: s.url.clone(), thumb_url: s.thumbnail_url.clone().filter(|t| !t.is_empty()), title: s.title.clone(), description: None, featured: i == 0 })
            .collect(),
        links: ProjectLinksDto { source: p.links.source, issues: p.links.issues, wiki: p.links.wiki, discord: None, donation: Vec::new() },
        version_count: 0,
    };
    Ok(CfProjectPageDto { project, source: "curseforge", page_url })
}

/// CurseForge modpacks with CurseForge's filters, for `pack_search`.
pub(crate) async fn search_modpacks(core: &Core, text: &str, sort: PackSort, offset: u32, limit: u32, filters: &SearchFiltersDto) -> CmdResult<cubby_common::packs::PackPage> {
    let wanted = ResourceQuery {
        kind: ResourceKind::Modpack,
        text: text.to_string(),
        game_versions: filters.game_versions.clone(),
        loaders: filters.loaders(),
        categories: filters.categories(),
        sort,
        offset,
        limit,
    };
    client(core)?.search_resources(&wanted).await.msg()
}

// What's in an instance

/// A content file of the instance that CurseForge knows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfInstalledDto {
    pub project_id: String,
    pub file_id: String,
    pub file_name: String,
    /// "mods", "resourcepacks" or "shaderpacks".
    pub folder: &'static str,
    /// Cubby downloaded this file from CurseForge (it may be on Modrinth too).
    pub from_curseforge: bool,
}

const FOLDERS: [Folder; 3] = [Folder::Mods, Folder::ResourcePacks, Folder::ShaderPacks];

/// Mods (`.jar`, `.zip`, also disabled), resource packs and shader packs (`.zip`) of the game directory.
fn content_files(game_dir: &Path) -> Vec<(Folder, String, PathBuf)> {
    let mut out = Vec::new();
    for folder in FOLDERS {
        let Ok(rd) = std::fs::read_dir(game_dir.join(folder.as_str())) else { continue };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let lower = name.to_lowercase();
            let base = lower.strip_suffix(".disabled").filter(|_| folder == Folder::Mods).unwrap_or(&lower);
            let wanted = match folder {
                Folder::Mods => base.ends_with(".jar") || base.ends_with(".zip"),
                _ => lower.ends_with(".zip"),
            };
            if wanted && !name.starts_with('.') && entry.path().is_file() {
                out.push((folder, name, entry.path()));
            }
        }
    }
    out
}

/// The CurseForge fingerprint of each file, read again only when its size or modification time changed.
fn fingerprints(core: &Core, files: &[(Folder, String, PathBuf)]) -> Vec<Option<u32>> {
    files
        .iter()
        .map(|(_, _, path)| {
            let meta = std::fs::metadata(path).ok()?;
            let (size, modified) = (meta.len(), meta.modified().ok()?);
            if let Some((s, m, print)) = core.cf_prints.lock().expect("fingerprint cache").get(path).copied()
                && s == size
                && m == modified
            {
                return Some(print);
            }
            let bytes = std::fs::read(path).ok()?;
            let print = cubby_curseforge::fingerprint(&bytes);
            core.cf_prints.lock().expect("fingerprint cache").insert(path.clone(), (size, modified, print));
            Some(print)
        })
        .collect()
}

#[derive(Debug, Clone)]
struct InstalledFile {
    project_id: u64,
    file_id: u64,
    folder: Folder,
    file_name: String,
}

/// The files of the game directory CurseForge knows, by exact fingerprint.
async fn installed_files(app: &AppHandle, cf: &CurseForgeClient, game_dir: &Path) -> CmdResult<Vec<InstalledFile>> {
    let dir = game_dir.to_path_buf();
    let app2 = app.clone();
    let (files, prints) = crate::commands::blocking(move || {
        let core = app2.state::<Core>();
        let files = content_files(&dir);
        let prints = fingerprints(&core, &files);
        Ok((files, prints))
    })
    .await?;
    let known: Vec<u32> = prints.iter().flatten().copied().collect();
    if known.is_empty() {
        return Ok(Vec::new());
    }
    let matches = cf.match_fingerprints(&known).await.msg()?;
    Ok(files
        .into_iter()
        .zip(prints)
        .filter_map(|((folder, file_name, _), print)| {
            let (project_id, file_id) = *matches.get(&print?)?;
            Some(InstalledFile { project_id, file_id, folder, file_name })
        })
        .collect())
}

/// The mods, resource packs and shader packs of the instance that come from CurseForge.
#[tauri::command]
pub async fn curseforge_installed(app: AppHandle, core: State<'_, Core>, instance_id: String) -> CmdResult<Vec<CfInstalledDto>> {
    let inst = core.store.get(&instance_id).msg()?;
    let cf = client(&core)?;
    let files = installed_files(&app, &cf, &core.store.game_dir(&inst)).await?;
    let origins = crate::origins::load(&core.paths.instance(&inst.id));
    Ok(files
        .into_iter()
        .map(|f| CfInstalledDto {
            from_curseforge: origins.get(&crate::origins::key(f.folder.as_str(), &f.file_name)).is_some_and(|s| s == "curseforge"),
            project_id: f.project_id.to_string(),
            file_id: f.file_id.to_string(),
            file_name: f.file_name,
            folder: f.folder.as_str(),
        })
        .collect())
}

// Files

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfFileDto {
    pub id: String,
    pub name: String,
    pub file_name: String,
    pub channel: &'static str,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub released: Option<String>,
    pub size: Option<u64>,
    pub downloads: u64,
    /// False: the author only lets it be downloaded on curseforge.com.
    pub downloadable: bool,
}

impl From<&CfFile> for CfFileDto {
    fn from(f: &CfFile) -> Self {
        Self {
            id: f.id.to_string(),
            name: f.name.clone(),
            file_name: f.file_name.clone(),
            channel: channel_name(f.channel),
            game_versions: f.minecraft.clone(),
            loaders: f.loaders.iter().map(|l| loader_name(*l).to_string()).collect(),
            released: f.released.clone(),
            size: f.size,
            downloads: f.downloads,
            downloadable: f.download_url.is_some(),
        }
    }
}

/// The newest files of a project, for a Minecraft version (every version when `None`), newest first.
#[tauri::command]
pub async fn curseforge_files(core: State<'_, Core>, project_id: String, game_version: Option<String>) -> CmdResult<Vec<CfFileDto>> {
    let id = parse_id(&project_id, "project")?;
    let files = client(&core)?.files_for_version(id, game_version.as_deref()).await.msg()?;
    Ok(files.iter().map(CfFileDto::from).collect())
}

// Datapacks of a world

/// A datapack of a world that Modrinth or CurseForge knows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatapackSourceDto {
    pub file_name: String,
    /// "modrinth" or "curseforge".
    pub source: &'static str,
    pub project_id: String,
    pub title: String,
}

/// The `.zip` datapacks of a world found on Modrinth (by SHA-1) or else on CurseForge (by fingerprint, with a key).
#[tauri::command]
pub async fn datapack_sources(app: AppHandle, core: State<'_, Core>, id: String, world: String) -> CmdResult<Vec<DatapackSourceDto>> {
    let inst = core.store.get(&id).msg()?;
    let game = core.store.game_dir(&inst);
    let files: Vec<(String, PathBuf)> = cubby_instance::list_datapacks(&game, &world)
        .msg()?
        .into_iter()
        .filter(|d| !d.is_dir && d.file_name.to_lowercase().ends_with(".zip"))
        .filter_map(|d| {
            let on = cubby_instance::datapack_path(&game, &world, &d.file_name).ok()?;
            let path = if d.enabled { on } else { on.parent()?.with_file_name("datapacks.disabled").join(&d.file_name) };
            Some((d.file_name, path))
        })
        .collect();
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let paths: Vec<PathBuf> = files.iter().map(|(_, p)| p.clone()).collect();
    let modrinth = core.modrinth.identify_paths(&paths).await.unwrap_or_else(|e| {
        log::warn!("couldn't identify datapacks on Modrinth: {e}");
        vec![None; paths.len()]
    });
    let mut out = Vec::new();
    let mut rest = Vec::new();
    for ((name, path), found) in files.into_iter().zip(modrinth) {
        match found {
            Some(p) => out.push(DatapackSourceDto { file_name: name, source: "modrinth", project_id: p.project_id, title: p.title }),
            None => rest.push((name, path)),
        }
    }
    if let (Some(cf), false) = (core.curseforge(), rest.is_empty()) {
        let app2 = app.clone();
        let paths: Vec<PathBuf> = rest.iter().map(|(_, p)| p.clone()).collect();
        let prints = crate::commands::blocking(move || {
            let core = app2.state::<Core>();
            let files: Vec<(Folder, String, PathBuf)> = paths.into_iter().map(|p| (Folder::Mods, String::new(), p)).collect();
            Ok(fingerprints(&core, &files))
        })
        .await?;
        let known: Vec<u32> = prints.iter().flatten().copied().collect();
        if !known.is_empty() {
            let matches = cf.match_fingerprints(&known).await.unwrap_or_default();
            let ids: Vec<u64> = matches.values().map(|(project, _)| *project).collect();
            let names = cf.projects(&ids).await.unwrap_or_default();
            for ((name, _), print) in rest.into_iter().zip(prints) {
                let Some((project, _)) = print.and_then(|p| matches.get(&p)) else { continue };
                let title = names.iter().find(|b| b.id == *project).map(|b| b.name.clone()).unwrap_or_else(|| name.clone());
                out.push(DatapackSourceDto { file_name: name, source: "curseforge", project_id: project.to_string(), title });
            }
        }
    }
    Ok(out)
}

// Install plans

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfPlanItemDto {
    pub project_id: String,
    pub file_id: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub page_url: String,
    pub file_name: String,
    pub size: u64,
    /// "requested" or "dependency".
    pub reason: &'static str,
    /// This very file is already in the instance.
    pub installed: bool,
    /// The file another version of the project left, replaced by this one.
    pub replaces: Option<String>,
    /// The author only lets it be downloaded on curseforge.com: the page to get it from.
    pub manual_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfMissingDto {
    pub project_id: String,
    pub title: String,
    pub page_url: String,
}

/// What installing a CurseForge project into an instance would do.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfPlanDto {
    pub instance_id: String,
    /// The requested project first, then the projects it requires that the instance doesn't have.
    pub items: Vec<CfPlanItemDto>,
    /// Required projects with no file for this instance.
    pub missing: Vec<CfMissingDto>,
    /// Bytes downloaded by Cubby.
    pub download_bytes: u64,
}

struct Planned {
    dto: CfPlanDto,
    /// What Cubby downloads, as an install plan the Modrinth downloader runs.
    downloads: InstallPlan,
}

fn folder_for(kind: ResourceKind) -> CmdResult<Folder> {
    match kind {
        ResourceKind::Mod => Ok(Folder::Mods),
        ResourceKind::ResourcePack => Ok(Folder::ResourcePacks),
        ResourceKind::Shader => Ok(Folder::ShaderPacks),
        ResourceKind::Datapack => Err("Datapacks go into a world: use curseforge_install_datapack.".into()),
        ResourceKind::Modpack => Err("CurseForge modpacks are installed as new instances.".into()),
    }
}

/// Folder of a required project, by its class (`None` for what doesn't go into the instance's folders).
fn folder_of_class(class_id: Option<u64>) -> Option<Folder> {
    match class_id {
        None | Some(6) => Some(Folder::Mods),
        Some(12) => Some(Folder::ResourcePacks),
        Some(6552) => Some(Folder::ShaderPacks),
        _ => None,
    }
}

async fn plan(app: &AppHandle, core: &Core, inst: &Instance, project_id: u64, file_id: Option<u64>, kind: ResourceKind) -> CmdResult<Planned> {
    let folder = folder_for(kind)?;
    let cf = client(core)?;
    let game_dir = core.store.game_dir(inst);
    let loaders = if kind == ResourceKind::Mod { runs_with(&instance_loader(inst)) } else { Vec::new() };
    if kind == ResourceKind::Mod && loaders.is_empty() {
        return Err(format!("{} is Vanilla: mods need a loader like Fabric, Quilt, Forge or NeoForge. Pick another instance, or create one with a loader.", inst.name));
    }
    let target = FileTarget { game_version: Some(inst.minecraft.clone()), loaders };

    let installed = installed_files(app, &cf, &game_dir).await?;
    let installed_ids: HashSet<u64> = installed.iter().map(|f| f.project_id).collect();
    // Projects found on Modrinth (same slug on both sites): a required library installed from Modrinth stays.
    let modrinth_slugs: HashSet<String> = if kind == ResourceKind::Mod {
        core.modrinth.content_sources(&game_dir).await.map(|s| s.into_iter().map(|c| c.slug).collect()).unwrap_or_default()
    } else {
        HashSet::new()
    };
    let resolved = cf
        .resolve(project_id, file_id, &target, |p| installed_ids.contains(&p.id) || (!p.slug.is_empty() && modrinth_slugs.contains(&p.slug)))
        .await
        .msg()?;

    let mut items = Vec::new();
    let mut missing = Vec::new();
    let mut downloads = Vec::new();
    for (i, r) in resolved.into_iter().enumerate() {
        let requested = i == 0;
        let Some(file) = r.file else {
            if requested {
                let what = if kind == ResourceKind::Mod { format!("Minecraft {} with {}", inst.minecraft, crate::dto::LoaderDto::from(&inst.loader).kind) } else { format!("Minecraft {}", inst.minecraft) };
                return Err(format!("No file of {} is made for {what}. Pick a file from the list, or another instance.", r.project.name));
            }
            missing.push(CfMissingDto { project_id: r.project.id.to_string(), title: r.project.name.clone(), page_url: r.project.page_url.clone() });
            continue;
        };
        let item_folder = if requested { folder } else {
            match folder_of_class(r.project.class_id) {
                Some(f) => f,
                None => continue,
            }
        };
        let same = installed.iter().find(|f| f.project_id == r.project.id);
        let already = same.is_some_and(|f| f.file_id == file.id);
        let replaces = same.filter(|f| f.file_id != file.id).map(|f| (f.folder, f.file_name.clone()));
        let manual_url = match (&file.download_url, &file.sha1) {
            (Some(_), Some(_)) => None,
            _ => Some(r.project.download_page(file.id)),
        };
        if !already
            && manual_url.is_none()
            && let (Some(url), Some(sha1)) = (&file.download_url, &file.sha1)
        {
            downloads.push(PlanItem {
                project_id: r.project.id.to_string(),
                version_id: file.id.to_string(),
                title: r.project.name.clone(),
                version_number: file.name.clone(),
                file_name: file.file_name.clone(),
                size: file.size.unwrap_or(0),
                url: url.clone(),
                sha1: sha1.clone(),
                reason: if requested { Reason::Requested } else { Reason::Dependency },
                installed: same.is_some(),
                folder: item_folder,
                download: true,
                replaces: replaces.clone(),
            });
        }
        items.push(CfPlanItemDto {
            project_id: r.project.id.to_string(),
            file_id: file.id.to_string(),
            title: r.project.name.clone(),
            icon_url: r.project.icon_url.clone(),
            page_url: r.project.page_url.clone(),
            file_name: file.file_name.clone(),
            size: file.size.unwrap_or(0),
            reason: if requested { "requested" } else { "dependency" },
            installed: already,
            replaces: replaces.map(|(_, name)| name),
            manual_url,
        });
    }
    let download_bytes = downloads.iter().map(|d| d.size).sum();
    Ok(Planned {
        dto: CfPlanDto { instance_id: inst.id.clone(), items, missing, download_bytes },
        downloads: InstallPlan { items: downloads, missing: Vec::new(), conflicts: Vec::new(), download_bytes },
    })
}

/// What installing a CurseForge project (`file_id`, else the best file for the instance) would do.
#[tauri::command]
pub async fn curseforge_plan(app: AppHandle, core: State<'_, Core>, instance_id: String, kind: String, project_id: String, file_id: Option<String>) -> CmdResult<CfPlanDto> {
    let inst = core.store.get(&instance_id).msg()?;
    let file = file_id.as_deref().map(|f| parse_id(f, "file")).transpose()?;
    Ok(plan(&app, &core, &inst, parse_id(&project_id, "project")?, file, kind_of(&kind)?).await?.dto)
}

/// Plans again, then downloads the project and the projects it requires into the instance (task
/// `install:<id>`). Files only offered on curseforge.com are left out: the plan lists them. Returns when done.
#[tauri::command]
pub async fn curseforge_install(app: AppHandle, core: State<'_, Core>, instance_id: String, kind: String, project_id: String, file_id: Option<String>) -> CmdResult<CfPlanDto> {
    let inst = core.store.get(&instance_id).msg()?;
    ensure_stopped(&core, &inst.id)?;
    let kind = kind_of(&kind)?;
    let project = parse_id(&project_id, "project")?;
    let file = file_id.as_deref().map(|f| parse_id(f, "file")).transpose()?;
    run_task(&app, format!("install:{}", inst.id), move |app, task| async move {
        let core = app.state::<Core>();
        core.sink.progress(&task, "Checking dependencies", 0, 1);
        install_planned(&app, &core, &inst, project, file, kind, &task).await.map(|(dto, _)| dto)
    })
    .await
}

/// Plans a CurseForge project for the instance and downloads it with what it requires, inside a running task.
/// Returns the plan and the files downloaded as (folder, file name).
pub(crate) async fn install_planned(app: &AppHandle, core: &Core, inst: &Instance, project: u64, file: Option<u64>, kind: ResourceKind, task: &str) -> CmdResult<(CfPlanDto, Vec<(Folder, String)>)> {
    let planned = plan(app, core, inst, project, file, kind).await?;
    let game_dir = core.store.game_dir(inst);
    let mut downloaded = Vec::new();
    if !planned.downloads.items.is_empty() {
        let selection = PackSelection::capture(&game_dir);
        let applied = core.modrinth.install(&planned.downloads, &game_dir, download_concurrency(core), core.sink.as_ref(), task).await.msg()?;
        selection.follow(&game_dir, &applied.renamed);
        let files: Vec<(&str, &str)> = planned.downloads.items.iter().map(|i| (i.folder.as_str(), i.file_name.as_str())).collect();
        crate::origins::record(&core.paths.instance(&inst.id), &files, "curseforge");
        downloaded = planned.downloads.items.iter().map(|i| (i.folder, i.file_name.clone())).collect();
        applied.into_result().msg()?;
    }
    Ok((planned.dto, downloaded))
}

/// The kind of project for a launcher key ("mod", "resourcepack", "shader", "datapack").
pub(crate) fn resource_kind(kind: &str) -> CmdResult<ResourceKind> {
    kind_of(kind)
}

pub(crate) fn parse_project_id(id: &str) -> CmdResult<u64> {
    parse_id(id, "project")
}

/// Downloads a CurseForge datapack (`file_id`, else its newest `.zip` for the instance's Minecraft version) into
/// a world. Returns the file name.
#[tauri::command]
pub async fn curseforge_install_datapack(core: State<'_, Core>, id: String, world: String, project_id: String, file_id: Option<String>) -> CmdResult<String> {
    let inst = core.store.get(&id).msg()?;
    let project_id = parse_id(&project_id, "project")?;
    let file_id = file_id.as_deref().map(|f| parse_id(f, "file")).transpose()?;
    download_datapack(&core, &inst, &world, project_id, file_id).await
}

/// Downloads a CurseForge datapack (`file_id`, else its newest `.zip` for the instance's Minecraft version) into a
/// world of the instance. Returns the file name.
pub(crate) async fn download_datapack(core: &Core, inst: &Instance, world: &str, project_id: u64, file_id: Option<u64>) -> CmdResult<String> {
    let cf = client(core)?;
    let project = cf.projects(&[project_id]).await.msg()?.into_iter().next().ok_or_else(|| "CurseForge doesn't know this datapack.".to_string())?;
    let is_zip = |f: &CfFile| f.file_name.to_lowercase().ends_with(".zip");
    let file = match file_id {
        Some(fid) => cf.file(project_id, fid).await.msg()?,
        None => {
            let files: Vec<CfFile> = cf.files_for_version(project_id, Some(&inst.minecraft)).await.msg()?.into_iter().filter(is_zip).collect();
            let target = FileTarget { game_version: Some(inst.minecraft.clone()), loaders: Vec::new() };
            best_file(&files, &target).cloned().ok_or_else(|| format!("No datapack file of {} is made for Minecraft {}.", project.name, inst.minecraft))?
        }
    };
    if !is_zip(&file) {
        return Err(format!("{} isn't a datapack .zip: it can't go into a world.", file.file_name));
    }
    let Some(url) = file.download_url.clone() else {
        return Err(format!("The author of {} only lets it be downloaded on CurseForge: {}", project.name, project.download_page(file.id)));
    };
    let path = cubby_instance::datapack_path(&core.store.game_dir(inst), world, &file.file_name).msg()?;
    let mut job = Download::new(url, &path);
    if let Some(size) = file.size {
        job = job.with_size(size);
    }
    if let Some(sha1) = file.sha1.clone() {
        job = job.with_sha1(sha1);
    }
    cubby_common::net::ensure_file(&core.http, &job).await.msg()?;
    Ok(file.file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quilt_runs_fabric_mods() {
        assert_eq!(runs_with("quilt"), vec![PackLoaderKind::Quilt, PackLoaderKind::Fabric]);
        assert_eq!(runs_with("neoforge"), vec![PackLoaderKind::NeoForge]);
        assert!(runs_with("vanilla").is_empty());
    }

    #[test]
    fn content_files_lists_what_curseforge_can_know() {
        let dir = std::env::temp_dir().join(format!("cubby-cf-content-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (folder, name) in [("mods", "a.jar"), ("mods", "b.jar.disabled"), ("mods", "notes.txt"), ("resourcepacks", "r.zip"), ("resourcepacks", "folder-pack"), ("shaderpacks", "s.zip"), ("shaderpacks", "s.zip.txt")] {
            std::fs::create_dir_all(dir.join(folder)).unwrap();
            if name == "folder-pack" {
                std::fs::create_dir_all(dir.join(folder).join(name)).unwrap();
            } else {
                std::fs::write(dir.join(folder).join(name), b"x").unwrap();
            }
        }
        let mut names: Vec<String> = content_files(&dir).into_iter().map(|(f, n, _)| format!("{}/{n}", f.as_str())).collect();
        names.sort();
        assert_eq!(names, vec!["mods/a.jar", "mods/b.jar.disabled", "resourcepacks/r.zip", "shaderpacks/s.zip"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! Modpack providers (ATLauncher, CurseForge, FTB, Modrinth, Technic) behind one set of commands.

use std::path::{Path, PathBuf};

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use cubby_common::packs::{PackChannel, PackLoaderKind, PackPage, PackPlan, PackSort, PackSummary, PackVersion};
use cubby_curseforge::CurseForgeClient;
use cubby_instance::{Instance, Loader, ModpackInfo, NewInstance};
use cubby_modrinth::{ProjectKind, SearchQuery, Sort};
use cubby_packinstall::checks::{check_import_url, is_legacy_minecraft};
use cubby_packinstall::install::{self as pack_install, BlockedEntry, PackFormat};
use cubby_packinstall::text::html_to_text;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::commands::{self, blocking, download_concurrency, download_icon, spawn_task, to_dto, CmdResult, OrMessage};
use crate::dto::InstanceDto;
use crate::state::Core;

const PAGE_SIZE: u32 = 20;
/// Largest modpack file `import_pack_url` downloads.
const MAX_IMPORT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Minecraft version an instance shows until the plan of its pack says which one it needs.
const PLACEHOLDER_MINECRAFT: &str = "1.21.1";
/// A pack that fails to install leaves nothing behind: its instance is deleted again, so the player starts
/// over from the same screen instead of finding a broken instance.
const REMOVE_INSTANCE_ON_FAILURE: bool = true;
const CURSEFORGE_KEY_MESSAGE: &str = "CurseForge needs an API key. Add yours in Settings.";
const LEGACY_TECHNIC_MESSAGE: &str = "This Technic pack uses Minecraft 1.5.2 or older, which Cubby can't run yet.";

// Types shared with the UI

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Modrinth,
    Curseforge,
    Ftb,
    Atlauncher,
    Technic,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Modrinth => "modrinth",
            ProviderId::Curseforge => "curseforge",
            ProviderId::Ftb => "ftb",
            ProviderId::Atlauncher => "atlauncher",
            ProviderId::Technic => "technic",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackProviderDto {
    id: ProviderId,
    name: &'static str,
    available: bool,
    reason: Option<String>,
    sorts: Vec<PackSort>,
    paged: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkDto {
    label: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackDetailsDto {
    summary: PackSummary,
    description: String,
    /// "markdown" or "text".
    description_format: &'static str,
    links: Vec<LinkDto>,
    gallery: Vec<String>,
    warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionalModDto {
    name: String,
    description: String,
    selected: bool,
    recommended: bool,
    group: Option<String>,
    depends: Vec<String>,
    warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedFileDto {
    file_name: String,
    page_url: String,
    /// Relative to the game directory, forward slashes.
    path: String,
    found: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackLoaderDto {
    kind: PackLoaderKind,
    version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFileInfoDto {
    /// "mrpack" or "curseforge".
    format: &'static str,
    name: String,
    version: Option<String>,
    minecraft: String,
    loader: Option<PackLoaderDto>,
    file_count: usize,
    blocked_reason: Option<String>,
}

fn link(label: &str, url: Option<String>) -> Option<LinkDto> {
    url.filter(|u| u.starts_with("http")).map(|url| LinkDto { label: label.to_string(), url })
}

fn require_curseforge(core: &Core) -> CmdResult<CurseForgeClient> {
    core.curseforge().ok_or_else(|| CURSEFORGE_KEY_MESSAGE.to_string())
}

fn parse_id(id: &str, what: &str) -> CmdResult<u64> {
    id.trim().parse().map_err(|_| format!("{id:?} isn't a valid {what} ID."))
}

// Browsing

#[tauri::command]
pub fn pack_providers(core: State<'_, Core>) -> Vec<PackProviderDto> {
    use PackSort::*;
    let has_key = core.curseforge_key_with_source().is_some();
    // The order the sources are offered in: Modrinth, CurseForge, then the others.
    vec![
        PackProviderDto { id: ProviderId::Modrinth, name: "Modrinth", available: true, reason: None, sorts: vec![Relevance, Downloads, Updated, Newest], paged: true },
        PackProviderDto {
            id: ProviderId::Curseforge,
            name: "CurseForge",
            available: has_key,
            reason: (!has_key).then(|| CURSEFORGE_KEY_MESSAGE.to_string()),
            sorts: vec![Relevance, Downloads, Updated, Newest, Name],
            paged: true,
        },
        PackProviderDto { id: ProviderId::Atlauncher, name: "ATLauncher", available: true, reason: None, sorts: vec![Relevance, Name], paged: false },
        PackProviderDto { id: ProviderId::Ftb, name: "FTB", available: true, reason: None, sorts: vec![Relevance, Downloads, Updated, Newest, Name], paged: true },
        PackProviderDto { id: ProviderId::Technic, name: "Technic", available: true, reason: None, sorts: vec![Relevance], paged: false },
    ]
}

fn loader_from_key(key: &str) -> Option<PackLoaderKind> {
    match key {
        "fabric" => Some(PackLoaderKind::Fabric),
        "quilt" => Some(PackLoaderKind::Quilt),
        "forge" => Some(PackLoaderKind::Forge),
        "neoforge" => Some(PackLoaderKind::NeoForge),
        _ => None,
    }
}

fn modrinth_summary(hit: cubby_modrinth::ProjectHit) -> PackSummary {
    let mut minecraft = hit.game_versions;
    minecraft.reverse(); // Modrinth lists them oldest first
    let mut loaders: Vec<PackLoaderKind> = hit.loaders.iter().filter_map(|l| loader_from_key(l)).collect();
    loaders.dedup();
    PackSummary {
        website_url: Some(format!("https://modrinth.com/modpack/{}", hit.slug)),
        id: hit.id,
        name: hit.title,
        summary: hit.summary,
        authors: if hit.author.is_empty() { Vec::new() } else { vec![hit.author] },
        icon_url: hit.icon_url,
        downloads: Some(hit.downloads),
        updated: Some(hit.updated).filter(|u| !u.is_empty()),
        minecraft,
        loaders,
    }
}

fn technic_summary(pack: &cubby_technic::TechnicPack) -> PackSummary {
    let text = html_to_text(&pack.description);
    let summary: String = text.split('\n').find(|l| !l.is_empty()).unwrap_or_default().chars().take(200).collect();
    PackSummary {
        id: pack.name.clone(),
        name: if pack.display_name.is_empty() { pack.name.clone() } else { pack.display_name.clone() },
        summary,
        authors: Some(pack.user.clone()).filter(|u| !u.is_empty()).into_iter().collect(),
        icon_url: pack.icon_url.clone().or_else(|| pack.logo_url.clone()),
        website_url: pack.platform_url.clone(),
        downloads: pack.installs.or(pack.runs),
        updated: None,
        minecraft: pack.minecraft.clone().into_iter().collect(),
        loaders: Vec::new(),
    }
}

#[tauri::command]
pub async fn pack_search(core: State<'_, Core>, provider: ProviderId, query: String, sort: PackSort, offset: u32) -> CmdResult<PackPage> {
    let query = query.trim();
    match provider {
        ProviderId::Modrinth => {
            let sort = match sort {
                PackSort::Downloads => Sort::Downloads,
                PackSort::Updated => Sort::Updated,
                PackSort::Newest => Sort::Newest,
                PackSort::Relevance | PackSort::Name => Sort::Relevance,
            };
            let q = SearchQuery {
                query: query.to_string(),
                kind: ProjectKind::Modpack,
                game_version: None,
                loader: None,
                categories: Vec::new(),
                environment: None,
                sort,
                offset,
                limit: PAGE_SIZE,
            };
            let results = core.modrinth.search(&q).await.msg()?;
            Ok(PackPage { total: Some(results.total), hits: results.hits.into_iter().map(modrinth_summary).collect() })
        }
        ProviderId::Curseforge => require_curseforge(&core)?.search(query, sort, offset, PAGE_SIZE).await.msg(),
        ProviderId::Ftb => core.ftb.search(query, sort, offset, PAGE_SIZE).await.msg(),
        ProviderId::Atlauncher => {
            let page = core.atlauncher.search(query, sort).await.msg()?;
            // Not paged: everything comes at once.
            Ok(if offset == 0 { page } else { PackPage { hits: Vec::new(), total: page.total } })
        }
        ProviderId::Technic => {
            if offset > 0 {
                return Ok(PackPage { hits: Vec::new(), total: None });
            }
            if query.is_empty() { core.technic.featured().await.msg() } else { core.technic.search(query).await.msg() }
        }
    }
}

async fn details_inner(core: &Core, provider: ProviderId, id: &str) -> CmdResult<PackDetailsDto> {
    let id = id.trim();
    match provider {
        ProviderId::Modrinth => {
            let project = core.modrinth.project(id).await.msg()?;
            let links = [
                link("Modrinth page", Some(format!("https://modrinth.com/modpack/{}", project.hit.slug))),
                link("Source code", project.links.source.clone()),
                link("Issues", project.links.issues.clone()),
                link("Wiki", project.links.wiki.clone()),
                link("Discord", project.links.discord.clone()),
            ];
            Ok(PackDetailsDto {
                gallery: project.gallery.iter().map(|g| g.url.clone()).collect(),
                links: links.into_iter().flatten().collect(),
                description: project.body.clone(),
                description_format: "markdown",
                warning: None,
                summary: modrinth_summary(project.hit),
            })
        }
        ProviderId::Curseforge => {
            let project = require_curseforge(core)?.pack(parse_id(id, "CurseForge pack")?).await.msg()?;
            let links = [
                link("CurseForge page", project.links.website.clone().or_else(|| project.summary.website_url.clone())),
                link("Source code", project.links.source.clone()),
                link("Issues", project.links.issues.clone()),
                link("Wiki", project.links.wiki.clone()),
            ];
            Ok(PackDetailsDto {
                description: html_to_text(&project.description_html),
                description_format: "text",
                links: links.into_iter().flatten().collect(),
                gallery: project.screenshots.iter().map(|s| s.url.clone()).collect(),
                warning: None,
                summary: project.summary,
            })
        }
        ProviderId::Ftb => {
            let details = core.ftb.details(id).await.msg()?;
            let mut links: Vec<LinkDto> = details.links.into_iter().filter_map(|(label, url)| link(&label, Some(url))).collect();
            if let Some(page) = details.summary.website_url.clone() {
                links.insert(0, LinkDto { label: "FTB page".into(), url: page });
            }
            Ok(PackDetailsDto {
                description: details.description,
                description_format: "markdown",
                links,
                gallery: details.splash_url.into_iter().collect(),
                warning: None,
                summary: details.summary,
            })
        }
        ProviderId::Atlauncher => {
            let summary = core
                .atlauncher
                .packs()
                .await
                .msg()?
                .into_iter()
                .find(|p| p.id == id)
                .ok_or_else(|| format!("Couldn't find the ATLauncher pack {id}."))?;
            let links = link("ATLauncher page", summary.website_url.clone()).into_iter().collect();
            Ok(PackDetailsDto { description: summary.summary.clone(), description_format: "text", links, gallery: Vec::new(), warning: None, summary })
        }
        ProviderId::Technic => {
            let pack = core.technic.pack(id).await.msg()?;
            let links = [link("Technic page", pack.platform_url.clone()), link("Server pack", pack.server_pack_url.clone())];
            // a failed build lookup gives no warning: the install itself still refuses legacy packs
            let minecraft = core.technic.minecraft_of(&pack).await.unwrap_or_else(|e| {
                log::warn!("could not read the Minecraft version of {id}: {e}");
                None
            });
            let warning = minecraft.as_deref().filter(|m| is_legacy_minecraft(m)).map(|_| LEGACY_TECHNIC_MESSAGE.to_string());
            Ok(PackDetailsDto {
                description: html_to_text(&pack.description),
                description_format: "text",
                links: links.into_iter().flatten().collect(),
                gallery: pack.background_url.clone().into_iter().collect(),
                warning,
                summary: technic_summary(&pack),
            })
        }
    }
}

#[tauri::command]
pub async fn pack_details(core: State<'_, Core>, provider: ProviderId, id: String) -> CmdResult<PackDetailsDto> {
    details_inner(&core, provider, &id).await
}

async fn versions_inner(core: &Core, provider: ProviderId, id: &str) -> CmdResult<Vec<PackVersion>> {
    let id = id.trim();
    match provider {
        ProviderId::Modrinth => {
            let versions = core.modrinth.versions(id, None, None).await.msg()?;
            Ok(versions
                .into_iter()
                .map(|v| PackVersion {
                    id: v.id,
                    name: if v.version_number.trim().is_empty() { v.name } else { v.version_number },
                    channel: match v.channel.as_str() {
                        "beta" => PackChannel::Beta,
                        "alpha" => PackChannel::Alpha,
                        _ => PackChannel::Release,
                    },
                    minecraft: v.game_versions.last().cloned(),
                    loader: v.loaders.iter().find_map(|l| loader_from_key(l)),
                    released: Some(v.published).filter(|p| !p.is_empty()),
                })
                .collect())
        }
        ProviderId::Curseforge => require_curseforge(core)?.versions(parse_id(id, "CurseForge pack")?).await.msg(),
        ProviderId::Ftb => core.ftb.versions(id).await.msg(),
        ProviderId::Atlauncher => core.atlauncher.versions(id).await.msg(),
        ProviderId::Technic => core.technic.versions(id).await.msg(),
    }
}

#[tauri::command]
pub async fn pack_versions(core: State<'_, Core>, provider: ProviderId, id: String) -> CmdResult<Vec<PackVersion>> {
    versions_inner(&core, provider, &id).await
}

#[tauri::command]
pub async fn pack_optional_mods(core: State<'_, Core>, provider: ProviderId, id: String, version_id: String) -> CmdResult<Vec<OptionalModDto>> {
    if provider != ProviderId::Atlauncher {
        return Ok(Vec::new());
    }
    let manifest = core.atlauncher.manifest(id.trim(), version_id.trim()).await.msg()?;
    Ok(cubby_atlauncher::optional_mods(&manifest)
        .into_iter()
        .filter(|m| !m.hidden)
        .map(|m| OptionalModDto {
            name: m.name,
            description: m.description,
            selected: m.selected,
            recommended: m.recommended,
            group: Some(m.group).filter(|g| !g.is_empty()),
            depends: m.depends,
            warning: m.warning.filter(|w| !w.is_empty()),
        })
        .collect())
}

// Installing

/// Where the plan of an install comes from.
enum Source {
    /// A version of a pack of a provider.
    Provider { provider: ProviderId, pack_id: String, version_id: String, optional: Option<Vec<String>> },
    /// A file of the player's: never deleted, unless it is a cached download (`remove_after`).
    File { path: PathBuf, remove_after: bool },
    /// A file to download first (into the cache, deleted afterwards).
    Url { url: String },
}

fn loader_of(loader: &Option<(PackLoaderKind, String)>) -> Loader {
    match loader {
        None => Loader::Vanilla,
        Some((kind, version)) => loader_with_version(*kind, version.clone()),
    }
}

fn loader_with_version(kind: PackLoaderKind, version: String) -> Loader {
    match kind {
        PackLoaderKind::Fabric => Loader::Fabric { version },
        PackLoaderKind::Quilt => Loader::Quilt { version },
        PackLoaderKind::Forge => Loader::Forge { version },
        PackLoaderKind::NeoForge => Loader::NeoForge { version },
    }
}

/// Splits what the UI sends as icon: a block key, or an image URL.
fn split_icon(icon: Option<String>) -> (Option<String>, Option<String>) {
    match icon.map(|i| i.trim().to_string()).filter(|i| !i.is_empty()) {
        Some(url) if url.contains("://") => (None, Some(url)),
        key => (key, None),
    }
}

/// Creates the instance of a pack, with what is known so far; the plan fills in the rest in the task.
async fn create_instance(core: &Core, name: String, group: Option<String>, minecraft: String, loader: Loader, icon: Option<String>) -> CmdResult<Instance> {
    let (icon_key, icon_url) = split_icon(icon);
    let mut inst = core
        .store
        .create(NewInstance { name, group: group.map(|g| g.trim().to_string()).filter(|g| !g.is_empty()), minecraft, loader, icon: icon_key })
        .msg()?;
    if let Some(url) = icon_url {
        inst.icon = download_icon(core, &inst.id, &url).await;
        if let Err(e) = core.store.save(&inst) {
            let _ = core.store.delete(&inst.id);
            return Err(crate::dto::message(e));
        }
    }
    Ok(inst)
}

fn cache_dir(core: &Core, sub: &str) -> CmdResult<PathBuf> {
    let dir = core.paths.meta().join("packs").join(sub);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    Ok(dir)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn install_pack(
    app: AppHandle,
    core: State<'_, Core>,
    provider: ProviderId,
    pack_id: String,
    version_id: String,
    name: String,
    group: Option<String>,
    icon: Option<String>,
    optional_mods: Option<Vec<String>>,
) -> CmdResult<InstanceDto> {
    if provider == ProviderId::Modrinth {
        // like the other providers, the pack's own icon when the player picked none
        let icon = match icon {
            Some(icon) => Some(icon),
            None => details_inner(&core, provider, pack_id.trim()).await.ok().and_then(|d| d.summary.icon_url),
        };
        return commands::install_modpack(app.clone(), core, version_id, name, group, icon).await;
    }
    let (pack_id, version_id) = (pack_id.trim().to_string(), version_id.trim().to_string());
    if provider == ProviderId::Curseforge {
        require_curseforge(&core)?;
    }
    // The pack page first: a wrong id or no connection fails here, before an instance exists.
    let summary = details_inner(&core, provider, &pack_id).await?.summary;
    let versions = versions_inner(&core, provider, &pack_id).await.unwrap_or_default();
    let known = versions.iter().find(|v| v.id == version_id);
    let minecraft = known.and_then(|v| v.minecraft.clone()).or_else(|| summary.minecraft.first().cloned()).unwrap_or_else(|| PLACEHOLDER_MINECRAFT.to_string());
    let loader = match known.and_then(|v| v.loader) {
        Some(kind) => loader_with_version(kind, String::new()),
        None => Loader::Vanilla,
    };
    let name = match name.trim() {
        "" => summary.name.clone(),
        name => name.to_string(),
    };
    let icon = icon.or_else(|| summary.icon_url.clone());
    let inst = create_instance(&core, name, group, minecraft, loader, icon).await?;
    start_task(&app, &core, inst, Source::Provider { provider, pack_id, version_id, optional: optional_mods }, false)
}

/// Starts the background install (task `modpack:<id>`) and returns the instance as it is now.
fn start_task(app: &AppHandle, core: &Core, inst: Instance, source: Source, name_from_pack: bool) -> CmdResult<InstanceDto> {
    let id = inst.id.clone();
    let spawned = spawn_task(app, format!("modpack:{id}"), move |app, task| async move { run_install(app, task, id, source, name_from_pack).await });
    if let Err(e) = spawned {
        let _ = core.store.delete(&inst.id);
        return Err(e);
    }
    Ok(to_dto(core, &inst))
}

async fn run_install(app: AppHandle, task: String, id: String, source: Source, name_from_pack: bool) -> CmdResult<()> {
    let core = app.state::<Core>();
    let result = install_source(&core, &task, &id, source, name_from_pack).await;
    if let Err(e) = &result {
        log::warn!("installing the modpack of {id} failed: {e}");
        if REMOVE_INSTANCE_ON_FAILURE
            && let Err(e) = core.store.delete(&id)
        {
            log::warn!("could not remove {id}: {e}");
        }
    }
    result
}

/// Deletes a cached file when dropped.
struct RemoveOnDrop(Option<PathBuf>);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        if let Some(path) = &self.0
            && let Err(e) = std::fs::remove_file(path)
        {
            log::warn!("could not remove {}: {e}", path.display());
        }
    }
}

async fn install_source(core: &Core, task: &str, id: &str, source: Source, name_from_pack: bool) -> CmdResult<()> {
    core.sink.progress(task, "Preparing the modpack", 0, 1);
    let provenance = matches!(source, Source::Provider { .. });
    let mut _cached = RemoveOnDrop(None);
    let (plan, own_archives) = match source {
        Source::Provider { provider, pack_id, version_id, optional } => (plan_provider(core, provider, &pack_id, &version_id, optional).await?, true),
        Source::File { path, remove_after } => {
            if remove_after {
                _cached.0 = Some(path.clone());
            }
            (plan_file(core, &path).await?, false)
        }
        Source::Url { url } => {
            let path = download_pack_url(core, &url, task).await?;
            _cached.0 = Some(path.clone());
            (plan_file(core, &path).await?, false)
        }
    };

    let mut inst = core.store.get(id).msg()?;
    inst.minecraft = plan.minecraft.clone();
    inst.loader = loader_of(&plan.loader);
    if provenance {
        inst.modpack = Some(ModpackInfo {
            provider: Some(plan.provider.clone()),
            project_id: plan.pack_id.clone(),
            version_id: plan.version_id.clone(),
            title: plan.title.clone(),
            version_number: plan.version_name.clone(),
        });
    }
    if name_from_pack && !plan.title.trim().is_empty() {
        inst.name = plan.title.trim().to_string();
    }
    if inst.icon.is_none()
        && let Some(url) = &plan.icon_url
    {
        inst.icon = download_icon(core, id, url).await;
    }
    core.store.save(&inst).msg()?;

    let game_dir = core.store.game_dir(&inst);
    let installed = pack_install::install_pack_plan(&core.http, &plan, &game_dir, download_concurrency(core), own_archives, core.sink.as_ref(), task).await.msg()?;
    let missing = installed.blocked.iter().filter(|b| !b.found(&game_dir)).count();
    pack_install::save_blocked(&core.paths.instance(id), &installed.blocked).map_err(|e| format!("Couldn't save the list of files to download by hand: {e}"))?;
    if missing > 0 {
        core.sink.progress(task, &format!("{missing} file(s) must be downloaded by hand"), 1, 1);
    }
    Ok(())
}

async fn plan_provider(core: &Core, provider: ProviderId, pack_id: &str, version_id: &str, optional: Option<Vec<String>>) -> CmdResult<PackPlan> {
    let cache = cache_dir(core, provider.as_str())?;
    match provider {
        ProviderId::Modrinth => Err("Modrinth packs are installed by install_modpack.".into()),
        ProviderId::Ftb => core.ftb.plan(pack_id, version_id, &cache).await.msg(),
        ProviderId::Curseforge => {
            let client = require_curseforge(core)?;
            client.plan(parse_id(pack_id, "CurseForge pack")?, parse_id(version_id, "CurseForge file")?, &cache).await.msg()
        }
        ProviderId::Technic => {
            let detailed = core.technic.plan_detailed(pack_id, version_id, &cache).await.msg()?;
            for note in &detailed.notes {
                log::info!("Technic pack {pack_id}: {note}");
            }
            if detailed.legacy_jar_mod {
                pack_install::remove_plan_archives(&detailed.plan);
                if let Some(jar) = &detailed.jar_mod {
                    let _ = std::fs::remove_file(jar);
                }
                return Err(LEGACY_TECHNIC_MESSAGE.to_string());
            }
            Ok(detailed.plan)
        }
        ProviderId::Atlauncher => {
            let names = match optional {
                Some(names) => names,
                None => {
                    let manifest = core.atlauncher.manifest(pack_id, version_id).await.msg()?;
                    cubby_atlauncher::OptionalSelection::new(&manifest).names()
                }
            };
            let detailed = core.atlauncher.plan_detailed(pack_id, version_id, &names, &cache).await.msg()?;
            for warning in &detailed.warnings {
                log::warn!("ATLauncher pack {pack_id} {version_id}: {warning}");
            }
            Ok(detailed.plan)
        }
    }
}

/// Plans a modpack file of the player's. The plan's overrides use the file itself.
async fn plan_file(core: &Core, path: &Path) -> CmdResult<PackPlan> {
    let archive = path.to_path_buf();
    let format = {
        let archive = archive.clone();
        blocking(move || pack_install::detect_format(&archive).msg()).await?
    };
    match format {
        PackFormat::Mrpack => blocking(move || pack_install::plan_from_mrpack(&archive).msg()).await,
        PackFormat::Curseforge => {
            let client = core.curseforge().ok_or_else(|| curseforge_import_message().to_string())?;
            client.plan_from_zip(&archive).await.msg()
        }
    }
}

fn curseforge_import_message() -> &'static str {
    "This is a CurseForge modpack, and installing it needs a CurseForge API key. Add yours in Settings."
}

// Importing files

#[tauri::command]
pub async fn pick_pack_file(app: AppHandle) -> CmdResult<Option<String>> {
    let picked = tauri::async_runtime::spawn_blocking(move || app.dialog().file().add_filter("Modpacks", &["mrpack", "zip"]).blocking_pick_file())
        .await
        .map_err(|e| format!("The file dialog stopped unexpectedly ({e})."))?;
    match picked {
        None => Ok(None),
        Some(file) => file.into_path().map(|p| Some(p.to_string_lossy().into_owned())).map_err(|e| format!("Couldn't use the chosen file: {e}")),
    }
}

fn inspect_mrpack(path: &Path) -> CmdResult<PackFileInfoDto> {
    let index = cubby_modrinth::MrpackIndex::read(path).msg()?;
    let minecraft = index.minecraft().ok_or("The modpack doesn't say which Minecraft version it needs.")?.to_string();
    let file_count = index.client_downloads().msg()?.len();
    Ok(PackFileInfoDto {
        format: "mrpack",
        name: Some(index.name.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| file_stem(path)),
        version: Some(index.version_id.clone()).filter(|v| !v.is_empty()),
        minecraft,
        loader: index.loader().map(|(kind, version)| PackLoaderDto { kind: pack_install::loader_kind(kind), version }),
        file_count,
        blocked_reason: None,
    })
}

fn inspect_curseforge(path: &Path, has_key: bool) -> CmdResult<PackFileInfoDto> {
    let manifest = cubby_curseforge::read_manifest(path).msg()?;
    let loader = manifest.loader().msg()?.map(|(kind, version)| PackLoaderDto { kind, version });
    Ok(PackFileInfoDto {
        format: "curseforge",
        name: Some(manifest.name.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| file_stem(path)),
        version: Some(manifest.version.trim().to_string()).filter(|v| !v.is_empty()),
        minecraft: manifest.minecraft.version.clone(),
        loader,
        file_count: manifest.files.len(),
        blocked_reason: (!has_key).then(|| curseforge_import_message().to_string()),
    })
}

/// What a modpack file contains. Without a CurseForge key a CurseForge zip is still described, with the
/// reason it can't be installed.
fn inspect_with_key(path: &Path, has_key: bool) -> CmdResult<PackFileInfoDto> {
    match pack_install::detect_format(path).msg()? {
        PackFormat::Mrpack => inspect_mrpack(path),
        PackFormat::Curseforge => inspect_curseforge(path, has_key),
    }
}

fn file_stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Modpack".to_string())
}

#[tauri::command]
pub async fn inspect_pack_file(core: State<'_, Core>, path: String) -> CmdResult<PackFileInfoDto> {
    let path = PathBuf::from(path);
    let has_key = core.curseforge().is_some();
    blocking(move || inspect_with_key(&path, has_key)).await
}

#[tauri::command]
pub async fn import_pack_file(app: AppHandle, core: State<'_, Core>, path: String, name: String, group: Option<String>) -> CmdResult<InstanceDto> {
    let path = PathBuf::from(path);
    let has_key = core.curseforge().is_some();
    let info = {
        let path = path.clone();
        blocking(move || inspect_with_key(&path, has_key)).await?
    };
    if let Some(reason) = &info.blocked_reason {
        return Err(reason.clone());
    }
    let name = match name.trim() {
        "" => info.name.clone(),
        name => name.to_string(),
    };
    let loader = match &info.loader {
        Some(l) => loader_with_version(l.kind, l.version.clone()),
        None => Loader::Vanilla,
    };
    let inst = create_instance(&core, name, group, info.minecraft.clone(), loader, None).await?;
    start_task(&app, &core, inst, Source::File { path, remove_after: false }, false)
}

#[tauri::command]
pub async fn import_pack_url(app: AppHandle, core: State<'_, Core>, url: String, name: String, group: Option<String>) -> CmdResult<InstanceDto> {
    let url = url.trim().to_string();
    check_import_url(&url)?;
    let name_from_pack = name.trim().is_empty();
    let name = if name_from_pack { "Imported modpack".to_string() } else { name.trim().to_string() };
    // Nothing is known about the pack before it is downloaded: the task fills in the rest.
    let inst = create_instance(&core, name, group, PLACEHOLDER_MINECRAFT.to_string(), Loader::Vanilla, None).await?;
    start_task(&app, &core, inst, Source::Url { url }, name_from_pack)
}

/// Downloads a modpack file into the cache (at most [`MAX_IMPORT_BYTES`]), reporting progress on `task`.
async fn download_pack_url(core: &Core, url: &str, task: &str) -> CmdResult<PathBuf> {
    check_import_url(url)?;
    let dest = cache_dir(core, "import")?.join(format!("{}.zip", uuid::Uuid::new_v4()));
    let guard = RemoveOnDrop(Some(dest.clone()));
    let response = core.http.client().get(url).send().await.map_err(|e| crate::dto::message(cubby_common::Error::http(url, e)))?;
    let status = response.status();
    if !status.is_success() {
        return Err(crate::dto::message(cubby_common::Error::HttpStatus { url: url.to_string(), status: status.as_u16() }));
    }
    let total = response.content_length().unwrap_or(0);
    if total > MAX_IMPORT_BYTES {
        return Err("That file is bigger than 2 GiB, which is more than Cubby imports.".into());
    }
    let mut file = tokio::fs::File::create(&dest).await.map_err(|e| format!("Couldn't write {}: {e}", dest.display()))?;
    let mut stream = response.bytes_stream();
    let mut done = 0u64;
    core.sink.progress(task, "Downloading the modpack", 0, total);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| crate::dto::message(cubby_common::Error::http(url, e)))?;
        done += chunk.len() as u64;
        if done > MAX_IMPORT_BYTES {
            return Err("That file is bigger than 2 GiB, which is more than Cubby imports.".into());
        }
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await.map_err(|e| format!("Couldn't write {}: {e}", dest.display()))?;
        core.sink.progress(task, "Downloading the modpack", done, total.max(done));
    }
    tokio::io::AsyncWriteExt::flush(&mut file).await.map_err(|e| format!("Couldn't write {}: {e}", dest.display()))?;
    drop(file);
    std::mem::forget(guard); // downloaded: the caller owns the file now
    Ok(dest)
}

// Blocked files

fn blocked_dtos(core: &Core, id: &str) -> CmdResult<Vec<BlockedFileDto>> {
    let inst = core.store.get(id).msg()?;
    let game_dir = core.store.game_dir(&inst);
    Ok(pack_install::load_blocked(&core.paths.instance(id))
        .into_iter()
        .map(|e| BlockedFileDto { found: e.found(&game_dir), file_name: e.file_name, page_url: e.page_url, path: e.path })
        .collect())
}

#[tauri::command]
pub fn blocked_files(core: State<'_, Core>, instance_id: String) -> CmdResult<Vec<BlockedFileDto>> {
    blocked_dtos(&core, &instance_id)
}

#[tauri::command]
pub async fn find_blocked_files(core: State<'_, Core>, instance_id: String) -> CmdResult<Vec<BlockedFileDto>> {
    let inst = core.store.get(&instance_id).msg()?;
    let game_dir = core.store.game_dir(&inst);
    let entries: Vec<BlockedEntry> = pack_install::load_blocked(&core.paths.instance(&instance_id));
    let downloads = dirs::download_dir().ok_or("Cubby can't find your Downloads folder.")?;
    let copied = blocking(move || Ok(pack_install::find_in_folder(&downloads, &game_dir, &entries))).await?;
    log::info!("{copied} blocked file(s) of {instance_id} found in Downloads");
    blocked_dtos(&core, &instance_id)
}


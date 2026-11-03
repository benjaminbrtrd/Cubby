//! Tauri commands called by the UI (`ui/src/lib/api.ts`). Errors are user-facing English messages.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cubby_auth::AccountStore;
use cubby_common::contracts::{JavaInstall, LaunchCredentials};
use cubby_common::events::EventSink;
use cubby_instance::{Instance, Loader, ModpackInfo, NewInstance, OnLaunch, PackKind, Settings};
use cubby_launch::{LaunchOptions, LogLine, QuickPlay, RunningGame};
use cubby_meta::{LoaderKind, LoaderSpec};
use cubby_modrinth::{Folder, PackLoader, Renamed, SearchQuery, Target};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::watch;

use crate::dto::{self, *};
use crate::state::{Core, PendingLogin};

pub(crate) type CmdResult<T> = Result<T, String>;

pub(crate) trait OrMessage<T> {
    fn msg(self) -> CmdResult<T>;
}

impl<T> OrMessage<T> for cubby_common::Result<T> {
    fn msg(self) -> CmdResult<T> {
        self.map_err(dto::message)
    }
}

const NO_MSA_CLIENT_ID: &str =
    "This build of Cubby has no Microsoft application ID. Add one in Settings › Accounts, or set CUBBY_MSA_CLIENT_ID.";

pub(crate) async fn blocking<T: Send + 'static>(f: impl FnOnce() -> CmdResult<T> + Send + 'static) -> CmdResult<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| format!("The task stopped unexpectedly ({e})."))?
}

// App

#[tauri::command]
pub fn app_info(core: State<'_, Core>) -> AppInfoDto {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    AppInfoDto {
        version: env!("CARGO_PKG_VERSION"),
        data_dir: core.paths.root().to_string_lossy().into_owned(),
        msa_configured: core.msa_client_id().is_some(),
        msa_source: core.msa_client_id_with_source().map(|(_, source)| source),
        curseforge_key: core.curseforge_key_with_source().map(|(_, source)| source),
        total_memory_mb: sys.total_memory() / (1024 * 1024),
    }
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> CmdResult<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only web links can be opened.".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

// Instances

/// Marker written once the files of an instance's version are downloaded: holds the version key,
/// so changing the version or the loader makes the instance "not installed" again.
const INSTALLED_MARKER: &str = ".cubby-installed";

fn version_key(inst: &Instance) -> String {
    let loader = dto::LoaderDto::from(&inst.loader);
    format!("{}|{}|{}", inst.minecraft, loader.kind, loader.version.unwrap_or_default())
}

pub(crate) fn to_dto(core: &Core, inst: &Instance) -> InstanceDto {
    let dir = core.paths.instance(&inst.id);
    let installed = std::fs::read_to_string(dir.join(INSTALLED_MARKER)).is_ok_and(|k| k.trim() == version_key(inst));
    let blocked = cubby_packinstall::install::count_missing(&dir, &core.store.game_dir(inst));
    InstanceDto::new(inst, &dir, installed, blocked)
}

#[tauri::command]
pub fn list_instances(core: State<'_, Core>) -> CmdResult<Vec<InstanceDto>> {
    let list = core.store.list().msg()?;
    Ok(list.iter().map(|i| to_dto(&core, i)).collect())
}

#[tauri::command]
pub fn create_instance(core: State<'_, Core>, spec: NewInstanceDto) -> CmdResult<InstanceDto> {
    let name = spec.name.trim().to_string();
    if name.is_empty() {
        return Err("Give the instance a name.".into());
    }
    let inst = core
        .store
        .create(NewInstance {
            name,
            group: spec.group.map(|g| g.trim().to_string()).filter(|g| !g.is_empty()),
            minecraft: spec.minecraft,
            loader: spec.loader.try_into()?,
            icon: spec.icon,
        })
        .msg()?;
    Ok(to_dto(&core, &inst))
}

#[tauri::command]
pub fn update_instance(core: State<'_, Core>, instance: InstanceDto) -> CmdResult<InstanceDto> {
    let mut inst = core.store.get(&instance.id).msg()?;
    let name = instance.name.trim();
    if name.is_empty() {
        return Err("Give the instance a name.".into());
    }
    inst.name = name.to_string();
    inst.group = instance.group.map(|g| g.trim().to_string()).filter(|g| !g.is_empty());
    // Image icons come back as asset URLs: only block keys (or clearing) change the stored value.
    match instance.icon {
        Some(icon) if icon.contains("://") => {}
        icon => inst.icon = icon,
    }
    inst.minecraft = instance.minecraft;
    inst.loader = instance.loader.try_into()?;
    inst.java = instance.java_overrides.into();
    inst.notes = instance.notes.filter(|n| !n.trim().is_empty());
    let old_provider = inst.modpack.as_ref().and_then(|m| m.provider.clone());
    inst.modpack = instance.modpack.map(|m| {
        let mut m: ModpackInfo = m.into();
        m.provider = m.provider.or(old_provider);
        m
    });
    core.store.save(&inst).msg()?;
    Ok(to_dto(&core, &inst))
}

#[tauri::command]
pub fn duplicate_instance(core: State<'_, Core>, id: String, name: String) -> CmdResult<InstanceDto> {
    let inst = core.store.duplicate(&id, name.trim()).msg()?;
    Ok(to_dto(&core, &inst))
}

#[tauri::command]
pub fn delete_instance(core: State<'_, Core>, id: String) -> CmdResult<()> {
    if core.games.is_running(&id) {
        return Err("Close the game before deleting this instance.".into());
    }
    core.store.delete(&id).msg()?;
    Ok(())
}

#[tauri::command]
pub fn open_folder(app: AppHandle, core: State<'_, Core>, id: String, sub: Option<String>) -> CmdResult<()> {
    let inst = core.store.get(&id).msg()?;
    let mut dir = core.store.game_dir(&inst);
    if let Some(sub) = sub.filter(|s| !s.is_empty()) {
        let plain = Path::new(&sub).components().all(|c| matches!(c, std::path::Component::Normal(_)));
        if !plain {
            return Err(format!("\"{sub}\" isn't a valid folder."));
        }
        dir = dir.join(sub);
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
}

// Versions

#[tauri::command]
pub async fn list_versions(core: State<'_, Core>) -> CmdResult<Vec<GameVersionDto>> {
    let manifest = cubby_meta::fetch_manifest(&core.http, &core.paths).await.msg()?;
    Ok(manifest
        .versions
        .into_iter()
        .map(|v| GameVersionDto { id: v.id, kind: v.kind, release_time: v.release_time })
        .collect())
}

fn loader_kind(kind: &str) -> CmdResult<LoaderKind> {
    match kind {
        "fabric" => Ok(LoaderKind::Fabric),
        "quilt" => Ok(LoaderKind::Quilt),
        "neoforge" => Ok(LoaderKind::NeoForge),
        "forge" => Ok(LoaderKind::Forge),
        other => Err(format!("Unknown loader: {other}.")),
    }
}

#[tauri::command]
pub async fn list_loader_versions(core: State<'_, Core>, kind: String, game: String) -> CmdResult<Vec<cubby_meta::LoaderVersion>> {
    cubby_meta::loader_versions(&core.http, loader_kind(&kind)?, &game).await.msg()
}

// Instance content

fn game_dir(core: &Core, id: &str) -> CmdResult<PathBuf> {
    let inst = core.store.get(id).msg()?;
    Ok(core.store.game_dir(&inst))
}

#[tauri::command]
pub async fn list_mods(core: State<'_, Core>, id: String) -> CmdResult<Vec<ModFileDto>> {
    let dir = game_dir(&core, &id)?;
    blocking(move || Ok(cubby_instance::list_mods(&dir).into_iter().map(Into::into).collect())).await
}

#[tauri::command]
pub fn set_mod_enabled(core: State<'_, Core>, id: String, file_name: String, enabled: bool) -> CmdResult<()> {
    cubby_instance::set_mod_enabled(&game_dir(&core, &id)?, &file_name, enabled).msg()?;
    Ok(())
}

#[tauri::command]
pub fn remove_mod(core: State<'_, Core>, id: String, file_name: String) -> CmdResult<()> {
    cubby_instance::remove_mod(&game_dir(&core, &id)?, &file_name).msg()?;
    Ok(())
}

#[tauri::command]
pub async fn list_worlds(core: State<'_, Core>, id: String) -> CmdResult<Vec<WorldDto>> {
    let dir = game_dir(&core, &id)?;
    blocking(move || {
        let saves = dir.join("saves");
        Ok(cubby_instance::list_worlds(&dir).into_iter().map(|w| WorldDto::new(w, &saves)).collect())
    })
    .await
}

#[tauri::command]
pub async fn list_screenshots(core: State<'_, Core>, id: String) -> CmdResult<Vec<ScreenshotDto>> {
    let dir = game_dir(&core, &id)?;
    blocking(move || Ok(cubby_instance::list_screenshots(&dir).into_iter().map(Into::into).collect())).await
}

// Running the game

#[tauri::command]
pub fn running_games(core: State<'_, Core>) -> Vec<RunningGame> {
    core.games.running()
}

#[tauri::command]
pub fn game_log(core: State<'_, Core>, id: String) -> Vec<LogLine> {
    core.games.log_lines(&id)
}

#[tauri::command]
pub fn kill_instance(core: State<'_, Core>, id: String) -> CmdResult<()> {
    core.games.kill(&id).msg()
}

fn loader_spec(loader: &Loader) -> CmdResult<LoaderSpec> {
    Ok(match loader {
        Loader::Vanilla => LoaderSpec::Vanilla,
        Loader::Fabric { version } => LoaderSpec::Fabric(version.clone()),
        Loader::Quilt { version } => LoaderSpec::Quilt(version.clone()),
        Loader::NeoForge { version } => LoaderSpec::NeoForge(version.clone()),
        Loader::Forge { version } => LoaderSpec::Forge(version.clone()),
    })
}

/// Downloads and checks every file of the instance's version, picks Java and extracts the natives.
/// Progress goes out on `task`. Writes the "installed" marker on success.
async fn prepare(core: &Core, inst: &Instance, settings: &Settings, task: &str) -> CmdResult<(cubby_common::contracts::InstalledVersion, JavaInstall)> {
    let spec = loader_spec(&inst.loader)?;
    let sink: &dyn EventSink = core.sink.as_ref();
    core.sink.progress(task, "Checking the version", 0, 1);
    let version = cubby_meta::resolve(&core.http, &core.paths, &inst.minecraft, &spec).await.msg()?;
    let game_dir = core.store.game_dir(inst);
    std::fs::create_dir_all(&game_dir).map_err(|e| e.to_string())?;
    // Java first: Forge and NeoForge run their installer steps with it.
    let java = java_for(core, inst, settings, &version, task).await?;
    let installed = cubby_meta::install_loader(&core.http, &core.paths, &version, &spec, Some(&java.executable), &game_dir, download_concurrency(core), sink, task).await.msg()?;
    {
        let (installed, natives_dir) = (installed.clone(), core.paths.natives(&inst.id));
        blocking(move || cubby_launch::extract_natives(&installed, &natives_dir).msg()).await?;
    }
    let marker = core.paths.instance(&inst.id).join(INSTALLED_MARKER);
    if let Err(e) = std::fs::write(&marker, version_key(inst)) {
        log::warn!("could not write {}: {e}", marker.display());
    }
    Ok((installed, java))
}

/// Re-checks every file of the instance (task `repair:<id>`); returns when done.
#[tauri::command]
pub async fn repair_instance(core: State<'_, Core>, id: String) -> CmdResult<()> {
    let task = format!("repair:{id}");
    let inst = core.store.get(&id).msg()?;
    let result = prepare(&core, &inst, &core.settings(), &task).await;
    core.sink.finished(&task, None);
    result.map(|_| ())
}

/// Installs what is missing, then starts the game, joining `server` right away when given.
/// Progress goes out as events on task `launch:<id>`; the command returns once the game process
/// is running (or with the error).
#[tauri::command]
pub async fn launch_instance(app: AppHandle, core: State<'_, Core>, id: String, offline: bool, server: Option<String>) -> CmdResult<()> {
    if core.games.is_running(&id) {
        return Err("This instance is already running.".into());
    }
    let task = format!("launch:{id}");
    let inst = core.store.get(&id).msg()?;
    let settings = core.settings();
    loader_spec(&inst.loader)?;

    // Credentials first: no point downloading a gigabyte to fail on an expired account.
    let (creds, demo) = credentials(&core, offline).await?;
    let (installed, java) = prepare(&core, &inst, &settings, &task).await?;
    let natives_dir = core.paths.natives(&inst.id);
    let game_dir = core.store.game_dir(&inst);

    let jvm_args = inst.java.jvm_args.clone().unwrap_or_else(|| settings.java.jvm_args.clone());
    let window = inst.window.map(|w| (w.width, w.height)).unwrap_or((settings.game.width, settings.game.height));
    let opts = LaunchOptions {
        instance_id: inst.id.clone(),
        game_dir,
        java,
        min_memory_mb: inst.java.min_memory_mb.unwrap_or(settings.java.min_memory_mb),
        max_memory_mb: inst.java.max_memory_mb.unwrap_or(settings.java.max_memory_mb),
        extra_jvm_args: jvm_args.split_whitespace().map(str::to_owned).collect(),
        window: Some(window),
        fullscreen: settings.game.fullscreen,
        // The demo can't join servers.
        quick_play: server.map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && !demo).map(QuickPlay::Multiplayer),
        demo,
    };
    let command = cubby_launch::build_command(&installed, &creds, &opts, &natives_dir).msg()?;

    let handle = core.games.launch(command, &inst.id, core.sink.clone() as Arc<dyn EventSink>).await.msg()?;
    core.sink.finished(&task, None);

    let window = app.get_webview_window("main");
    if let Some(w) = &window {
        let _ = match settings.on_launch {
            OnLaunch::Minimize => w.minimize(),
            OnLaunch::Close => w.hide(),
            OnLaunch::Keep => Ok(()),
        };
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let exit = handle.wait().await;
        let core = app.state::<Core>();
        if core.settings().track_play_time
            && let Err(e) = core.store.record_session(&id, exit.duration)
        {
            log::warn!("could not record the play session of {id}: {e}");
        }
        // Makes the UI reload the instances with the new play time.
        core.sink.finished(&format!("session:{id}"), None);
        match settings.on_launch {
            OnLaunch::Close if core.games.running().is_empty() => app.exit(0),
            OnLaunch::Minimize | OnLaunch::Close => {
                if let Some(w) = window {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }
            OnLaunch::Keep => {}
        }
    });
    Ok(())
}

/// Identity for this launch, and whether it is the demo: without a signed-in account that owns the
/// game, Cubby only starts the Minecraft demo.
async fn credentials(core: &Core, offline: bool) -> CmdResult<(LaunchCredentials, bool)> {
    let mut accounts = core.accounts.lock().await;
    let Some(account) = accounts.default_account().cloned() else {
        return Ok((cubby_auth::demo_credentials(), true));
    };
    if offline {
        return Ok((account.offline_credentials(), false));
    }
    if !account.needs_refresh() {
        return Ok((account.launch_credentials(), false));
    }
    let client_id = core
        .msa_client_id()
        .ok_or("Your Microsoft session expired, and no Azure application ID is set to renew it. Add one in Settings › Accounts.")?;
    let mut account = account;
    cubby_auth::refresh(&core.http, &client_id, &mut account).await.msg()?;
    let creds = account.launch_credentials();
    accounts.add_or_update(account);
    accounts.save().msg()?;
    Ok((creds, false))
}

/// Java for this launch: the instance override, else an automatic pick (downloading Mojang's
/// runtime when needed), else the best installed one.
async fn java_for(core: &Core, inst: &Instance, settings: &Settings, version: &cubby_common::version::VersionJson, task: &str) -> CmdResult<JavaInstall> {
    if let Some(exe) = &inst.java.executable {
        return cubby_java::probe(exe).await.msg();
    }
    if settings.java.auto {
        return cubby_java::find_or_download(&core.http, &core.paths, version, core.sink.as_ref(), task).await.msg();
    }
    let major = cubby_java::required_major(version);
    let installs = cubby_java::detect(&core.paths).await;
    cubby_java::pick(&installs, major).ok_or_else(|| format!("Java {major} is required but wasn't found. Turn on \"Choose Java automatically\" in Settings."))
}

// Accounts

fn account_list(store: &AccountStore) -> Vec<AccountDto> {
    let default = store.default_account().map(|a| a.id.clone());
    store.list().iter().map(|a| AccountDto::new(a, default.as_deref() == Some(a.id.as_str()))).collect()
}

#[tauri::command]
pub async fn list_accounts(core: State<'_, Core>) -> CmdResult<Vec<AccountDto>> {
    Ok(account_list(&*core.accounts.lock().await))
}

#[tauri::command]
pub async fn start_microsoft_login(core: State<'_, Core>) -> CmdResult<DeviceCodeDto> {
    let client_id = core.msa_client_id().ok_or(NO_MSA_CLIENT_ID)?;
    let code = cubby_auth::start_device_login(&core.http, &client_id).await.msg()?;
    let dto = DeviceCodeDto::from(&code);
    let (cancel, _) = watch::channel(false);
    if let Some(previous) = core.login.lock().expect("login lock").replace(PendingLogin { code, cancel }) {
        let _ = previous.cancel.send(true);
    }
    Ok(dto)
}

#[tauri::command]
pub async fn finish_microsoft_login(core: State<'_, Core>) -> CmdResult<AccountDto> {
    let (code, cancel) = {
        let login = core.login.lock().expect("login lock");
        let pending = login.as_ref().ok_or("There's no sign-in in progress.")?;
        (pending.code.clone(), pending.cancel.subscribe())
    };
    let client_id = core.msa_client_id().ok_or(NO_MSA_CLIENT_ID)?;
    let result = cubby_auth::finish_device_login(&core.http, &client_id, &code, cancel).await;
    {
        let mut login = core.login.lock().expect("login lock");
        if login.as_ref().is_some_and(|p| p.code.device_code == code.device_code) {
            *login = None;
        }
    }
    let account = result.msg()?;
    let mut accounts = core.accounts.lock().await;
    accounts.add_or_update(account.clone());
    if accounts.default_account().is_none() {
        accounts.set_default(&account.id).msg()?;
    }
    accounts.save().msg()?;
    let is_default = accounts.default_account().is_some_and(|a| a.id == account.id);
    Ok(AccountDto::new(&account, is_default))
}

#[tauri::command]
pub fn cancel_microsoft_login(core: State<'_, Core>) {
    if let Some(pending) = core.login.lock().expect("login lock").take() {
        let _ = pending.cancel.send(true);
    }
}

#[tauri::command]
pub async fn remove_account(core: State<'_, Core>, id: String) -> CmdResult<()> {
    let mut accounts = core.accounts.lock().await;
    accounts.remove(&id);
    accounts.save().msg()
}

#[tauri::command]
pub async fn set_default_account(core: State<'_, Core>, id: String) -> CmdResult<()> {
    let mut accounts = core.accounts.lock().await;
    accounts.set_default(&id).msg()?;
    accounts.save().msg()
}

// Settings & Java

#[tauri::command]
pub fn get_settings(core: State<'_, Core>) -> Settings {
    core.settings()
}

#[tauri::command]
pub fn save_settings(core: State<'_, Core>, settings: Settings) -> CmdResult<Settings> {
    settings.save(&core.paths).msg()?;
    *core.settings.lock().expect("settings lock") = settings.clone();
    Ok(settings)
}

#[tauri::command]
pub async fn list_java(core: State<'_, Core>) -> CmdResult<Vec<JavaInstall>> {
    Ok(cubby_java::detect(&core.paths).await)
}

// Prism import

fn prism_root(app: &AppHandle) -> CmdResult<PathBuf> {
    Ok(app.path().data_dir().map_err(|e| e.to_string())?.join("PrismLauncher"))
}

#[tauri::command]
pub async fn list_prism_instances(app: AppHandle) -> CmdResult<Vec<PrismInstanceDto>> {
    let root = prism_root(&app)?;
    blocking(move || {
        if !root.is_dir() {
            return Ok(Vec::new());
        }
        Ok(cubby_instance::list_prism_instances(&root).msg()?.into_iter().map(Into::into).collect())
    })
    .await
}

/// Imports Prism instances with their game folder. Prism's files are only read.
#[tauri::command]
pub async fn import_prism(app: AppHandle, paths: Vec<String>) -> CmdResult<Vec<InstanceDto>> {
    let root = prism_root(&app)?;
    let app2 = app.clone();
    let imported = blocking(move || {
        let core = app2.state::<Core>();
        let mut out = Vec::new();
        for path in paths {
            let mut inst = cubby_instance::import_prism(&core.store, Path::new(&path), true).msg()?;
            inst.icon = inst.icon.take().and_then(|key| prism_icon(&core, &inst, &root, &key));
            core.store.save(&inst).msg()?;
            out.push(inst);
        }
        Ok(out)
    })
    .await?;
    let core = app.state::<Core>();
    Ok(imported.iter().map(|i| to_dto(&core, i)).collect())
}

/// Maps a Prism icon key to a Cubby icon: a built-in block for Prism's stock icons, or a copy of
/// the custom image (from Prism's `icons/` folder) inside the instance folder.
fn prism_icon(core: &Core, inst: &Instance, prism_root: &Path, key: &str) -> Option<String> {
    let block = match key {
        "grass" | "dirt" | "steve" | "creeper" | "squarecreeper" => Some("grass"),
        "diamond" => Some("diamond"),
        "gold" => Some("gold"),
        "iron" | "stone" | "gear" | "skeleton" => Some("stone"),
        "planks" | "bookshelf" | "chicken" => Some("planks"),
        "tnt" => Some("tnt"),
        "flame" | "meat" | "herobrine" => Some("netherrack"),
        "netherstar" | "enderpearl" | "enderman" | "infinity" => Some("obsidian"),
        "fox" | "pumpkin" => Some("pumpkin"),
        "bee" => Some("gold"),
        _ => None,
    };
    if let Some(block) = block {
        return Some(block.into());
    }
    let icons = prism_root.join("icons");
    for ext in ["png", "gif", "jpg", "jpeg", "webp", "ico"] {
        let src = icons.join(format!("{key}.{ext}"));
        if src.is_file() {
            let name = format!("icon.{ext}");
            return match std::fs::copy(&src, core.paths.instance(&inst.id).join(&name)) {
                Ok(_) => Some(name),
                Err(e) => {
                    log::warn!("could not copy Prism icon {}: {e}", src.display());
                    None
                }
            };
        }
    }
    None
}

// Data folders

/// Total size of the files under `path` (0 when missing). Symlinks are not followed.
fn dir_size(path: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(path) else { return 0 };
    rd.flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

fn data_folder(core: &Core, key: &str) -> CmdResult<(&'static str, PathBuf)> {
    let p = &core.paths;
    Ok(match key {
        "instances" => ("Instances", p.instances()),
        "libraries" => ("Libraries", p.libraries()),
        "assets" => ("Game assets", p.assets()),
        "java" => ("Java installations", p.java()),
        "meta" => ("Metadata cache", p.meta()),
        "logs" => ("Launcher logs", p.logs()),
        "trash" => ("Trash", p.root().join("trash")),
        other => return Err(format!("Unknown folder: {other}.")),
    })
}

const DATA_FOLDERS: [&str; 7] = ["instances", "libraries", "assets", "java", "meta", "logs", "trash"];

#[tauri::command]
pub async fn data_folders(app: AppHandle) -> CmdResult<Vec<DataFolderDto>> {
    blocking(move || {
        let core = app.state::<Core>();
        DATA_FOLDERS
            .iter()
            .map(|key| {
                let (label, path) = data_folder(&core, key)?;
                Ok(DataFolderDto { key, label, size_bytes: dir_size(&path), path: path.to_string_lossy().into_owned() })
            })
            .collect()
    })
    .await
}

#[tauri::command]
pub fn open_data_folder(app: AppHandle, core: State<'_, Core>, key: String) -> CmdResult<()> {
    let (_, path) = data_folder(&core, &key)?;
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
}

/// Deletes the metadata cache (version manifests, loader profiles); it is fetched again when needed.
#[tauri::command]
pub async fn clear_cache(core: State<'_, Core>) -> CmdResult<()> {
    let meta = core.paths.meta();
    blocking(move || match std::fs::remove_dir_all(&meta) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Couldn't clear the cache: {e}")),
    })
    .await
}

// Servers

#[tauri::command]
pub async fn ping_server(address: String) -> ServerStatusDto {
    cubby_launch::ping(&address, std::time::Duration::from_secs(5)).await.into()
}

#[tauri::command]
pub async fn list_servers(core: State<'_, Core>, id: String) -> CmdResult<Vec<ServerDto>> {
    let dir = game_dir(&core, &id)?;
    blocking(move || Ok(cubby_instance::read_servers(&dir).into_iter().map(Into::into).collect())).await
}

#[tauri::command]
pub async fn save_servers(core: State<'_, Core>, id: String, servers: Vec<ServerDto>) -> CmdResult<()> {
    let dir = game_dir(&core, &id)?;
    let entries: Vec<_> = servers.into_iter().map(Into::into).collect();
    blocking(move || cubby_instance::write_servers(&dir, &entries).msg()).await
}

// Packs and screenshots

fn pack_kind(kind: &str) -> CmdResult<PackKind> {
    match kind {
        "resource" => Ok(PackKind::Resource),
        "shader" => Ok(PackKind::Shader),
        other => Err(format!("Unknown pack type: {other}.")),
    }
}

#[tauri::command]
pub async fn list_packs(core: State<'_, Core>, id: String, kind: String) -> CmdResult<Vec<cubby_instance::PackFile>> {
    let (dir, kind) = (game_dir(&core, &id)?, pack_kind(&kind)?);
    blocking(move || Ok(cubby_instance::list_packs(&dir, kind))).await
}

#[tauri::command]
pub fn set_resource_packs(core: State<'_, Core>, id: String, enabled: Vec<String>) -> CmdResult<()> {
    cubby_instance::set_resource_packs(&game_dir(&core, &id)?, &enabled).msg()
}

#[tauri::command]
pub fn set_shader_pack(core: State<'_, Core>, id: String, file_name: Option<String>) -> CmdResult<()> {
    cubby_instance::set_shader_pack(&game_dir(&core, &id)?, file_name.as_deref()).msg()
}

#[tauri::command]
pub fn delete_screenshot(core: State<'_, Core>, id: String, file_name: String) -> CmdResult<()> {
    cubby_instance::delete_screenshot(&game_dir(&core, &id)?, &file_name).msg()
}

// Background tasks

struct TaskSlot {
    app: AppHandle,
    task: String,
}

impl Drop for TaskSlot {
    fn drop(&mut self) {
        self.app.state::<Core>().tasks.lock().expect("tasks lock").remove(&self.task);
    }
}

/// Runs `work` on the async runtime as task `task`, one task per id at a time. Its `TaskFinished` event
/// carries the error, if any. The returned handle yields the result.
pub(crate) fn spawn_task<T, Fut>(app: &AppHandle, task: String, work: impl FnOnce(AppHandle, String) -> Fut) -> CmdResult<tauri::async_runtime::JoinHandle<CmdResult<T>>>
where
    T: Send + 'static,
    Fut: Future<Output = CmdResult<T>> + Send + 'static,
{
    if !app.state::<Core>().tasks.lock().expect("tasks lock").insert(task.clone()) {
        return Err("Another task is already running on this instance. Wait for it to finish.".into());
    }
    let slot = TaskSlot { app: app.clone(), task: task.clone() };
    let work = work(app.clone(), task.clone());
    let app = app.clone();
    Ok(tauri::async_runtime::spawn(async move {
        let result = work.await;
        drop(slot);
        app.state::<Core>().sink.finished(&task, result.as_ref().err().cloned());
        result
    }))
}

async fn run_task<T, Fut>(app: &AppHandle, task: String, work: impl FnOnce(AppHandle, String) -> Fut) -> CmdResult<T>
where
    T: Send + 'static,
    Fut: Future<Output = CmdResult<T>> + Send + 'static,
{
    match spawn_task(app, task.clone(), work)?.await {
        Ok(result) => result,
        Err(e) => {
            let message = format!("The task stopped unexpectedly ({e}).");
            app.state::<Core>().sink.finished(&task, Some(message.clone()));
            Err(message)
        }
    }
}

fn ensure_stopped(core: &Core, id: &str) -> CmdResult<()> {
    if core.games.is_running(id) {
        return Err("Close the game before changing this instance.".into());
    }
    Ok(())
}

pub(crate) fn download_concurrency(core: &Core) -> usize {
    core.settings().download_concurrency.clamp(1, 64) as usize
}

// Discover (Modrinth)

/// Game version and loader of an instance, as Modrinth sees them.
fn target_of(inst: &Instance) -> Target {
    Target::new(inst.minecraft.clone(), Some(dto::LoaderDto::from(&inst.loader).kind.as_str()))
}

/// Active resource packs and shader pack, carried over when an install or update renames their files.
struct PackSelection {
    resource: Vec<String>,
    shader: Option<String>,
}

impl PackSelection {
    fn capture(game_dir: &Path) -> Self {
        Self {
            resource: cubby_instance::list_packs(game_dir, PackKind::Resource).into_iter().filter(|p| p.enabled).map(|p| p.file_name).collect(),
            shader: cubby_instance::list_packs(game_dir, PackKind::Shader).into_iter().find(|p| p.enabled).map(|p| p.file_name),
        }
    }

    fn follow(&self, game_dir: &Path, renamed: &[Renamed]) {
        let new_name = |folder: Folder, old: &str| renamed.iter().find(|r| r.folder == folder && r.old == old).map(|r| r.new.clone());
        if self.resource.iter().any(|n| new_name(Folder::ResourcePacks, n).is_some()) {
            let list: Vec<String> = self.resource.iter().map(|n| new_name(Folder::ResourcePacks, n).unwrap_or_else(|| n.clone())).collect();
            if let Err(e) = cubby_instance::set_resource_packs(game_dir, &list) {
                log::warn!("could not update the active resource packs: {e}");
            }
        }
        if let Some(new) = self.shader.as_deref().and_then(|s| new_name(Folder::ShaderPacks, s))
            && let Err(e) = cubby_instance::set_shader_pack(game_dir, Some(&new))
        {
            log::warn!("could not update the selected shader pack: {e}");
        }
        // Iris keeps each pack's settings next to it as `<pack file>.txt`: carry them over to the new file name.
        let dir = game_dir.join("shaderpacks");
        for r in renamed.iter().filter(|r| r.folder == Folder::ShaderPacks) {
            let (old, new) = (dir.join(format!("{}.txt", r.old)), dir.join(format!("{}.txt", r.new)));
            if old.is_file()
                && !new.exists()
                && let Err(e) = std::fs::rename(&old, &new)
            {
                log::warn!("could not carry over the settings of {}: {e}", r.old);
            }
        }
    }
}

#[tauri::command]
pub async fn search_projects(core: State<'_, Core>, query: ProjectQueryDto) -> CmdResult<SearchResultsDto> {
    let query = SearchQuery::try_from(query)?;
    Ok(core.modrinth.search(&query).await.msg()?.into())
}

#[tauri::command]
pub async fn get_project(core: State<'_, Core>, id_or_slug: String) -> CmdResult<ProjectDto> {
    Ok(core.modrinth.project(id_or_slug.trim()).await.msg()?.into())
}

/// Newest first; `game_version` / `loader` (launcher key) `None` lists every version.
#[tauri::command]
pub async fn list_project_versions(
    core: State<'_, Core>,
    id_or_slug: String,
    game_version: Option<String>,
    loader: Option<String>,
) -> CmdResult<Vec<ProjectVersionDto>> {
    let versions = core.modrinth.versions(id_or_slug.trim(), game_version.as_deref(), loader.as_deref()).await.msg()?;
    Ok(versions.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn plan_install(core: State<'_, Core>, instance_id: String, version_id: String) -> CmdResult<InstallPlanDto> {
    let inst = core.store.get(&instance_id).msg()?;
    let plan = core.modrinth.plan_install(version_id.trim(), &target_of(&inst), &core.store.game_dir(&inst)).await.msg()?;
    Ok(InstallPlanDto::new(&inst.id, plan))
}

/// Plans again, then downloads what is missing (task `install:<id>`); returns when done.
#[tauri::command]
pub async fn install_version(app: AppHandle, core: State<'_, Core>, instance_id: String, version_id: String) -> CmdResult<()> {
    let inst = core.store.get(&instance_id).msg()?;
    ensure_stopped(&core, &inst.id)?;
    run_task(&app, format!("install:{}", inst.id), move |app, task| async move {
        let core = app.state::<Core>();
        let game_dir = core.store.game_dir(&inst);
        core.sink.progress(&task, "Checking dependencies", 0, 1);
        let plan = core.modrinth.plan_install(version_id.trim(), &target_of(&inst), &game_dir).await.msg()?;
        let selection = PackSelection::capture(&game_dir);
        let applied = core.modrinth.install(&plan, &game_dir, download_concurrency(&core), core.sink.as_ref(), &task).await.msg()?;
        selection.follow(&game_dir, &applied.renamed);
        applied.into_result().msg().map(|_| ())
    })
    .await
}

/// Copies an image (any https address, at most 8 MiB) into the instance folder as `icon.<ext>`; `None` for other URLs or on failure.
pub(crate) async fn download_icon(core: &Core, id: &str, url: &str) -> Option<String> {
    if !url.starts_with("https://") {
        log::warn!("icon {url} ignored: only https images are downloaded");
        return None;
    }
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let ext = path
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .filter(|e| ["png", "jpg", "jpeg", "webp", "gif"].contains(&e.as_str()))
        .unwrap_or_else(|| "png".into());
    let bytes = match core.http.get_bytes(url).await {
        Ok(bytes) => bytes,
        Err(e) => {
            log::warn!("could not download the modpack icon: {e}");
            return None;
        }
    };
    if bytes.len() > 8 * 1024 * 1024 {
        log::warn!("icon {url} ignored: it is larger than 8 MiB");
        return None;
    }
    let name = format!("icon.{ext}");
    match std::fs::write(core.paths.instance(id).join(&name), bytes) {
        Ok(()) => Some(name),
        Err(e) => {
            log::warn!("could not save the modpack icon: {e}");
            None
        }
    }
}

/// Downloads the modpack, creates its instance and returns it; the pack's files are installed in the
/// background (task `modpack:<id>`). `icon`: a block key, or a Modrinth image URL copied into the instance.
#[tauri::command]
pub async fn install_modpack(
    app: AppHandle,
    core: State<'_, Core>,
    version_id: String,
    name: String,
    group: Option<String>,
    icon: Option<String>,
) -> CmdResult<InstanceDto> {
    let pack = core.modrinth.fetch_modpack(version_id.trim(), &core.paths.meta().join("modrinth")).await.msg()?;
    let loader = match &pack.loader {
        None => Loader::Vanilla,
        Some((PackLoader::Fabric, version)) => Loader::Fabric { version: version.clone() },
        Some((PackLoader::Quilt, version)) => Loader::Quilt { version: version.clone() },
        Some((PackLoader::Forge, version)) => Loader::Forge { version: version.clone() },
        Some((PackLoader::NeoForge, version)) => Loader::NeoForge { version: version.clone() },
    };
    let name = match name.trim() {
        "" => pack.title.clone(),
        name => name.to_string(),
    };
    let (icon_key, icon_url) = match icon.map(|i| i.trim().to_string()).filter(|i| !i.is_empty()) {
        Some(url) if url.contains("://") => (None, Some(url)),
        key => (key, None),
    };
    let mut inst = core
        .store
        .create(NewInstance {
            name,
            group: group.map(|g| g.trim().to_string()).filter(|g| !g.is_empty()),
            minecraft: pack.minecraft.clone(),
            loader,
            icon: icon_key,
        })
        .msg()?;
    inst.modpack = Some(ModpackInfo {
        provider: Some("modrinth".into()),
        project_id: pack.project_id.clone(),
        version_id: pack.version_id.clone(),
        title: pack.title.clone(),
        version_number: pack.version_number.clone(),
    });
    if let Some(url) = icon_url {
        inst.icon = download_icon(&core, &inst.id, &url).await;
    }
    if let Err(e) = core.store.save(&inst) {
        let _ = core.store.delete(&inst.id);
        return Err(dto::message(e));
    }
    let game_dir = core.store.game_dir(&inst);
    spawn_task(&app, format!("modpack:{}", inst.id), move |app, task| async move {
        let core = app.state::<Core>();
        let result = core.modrinth.install_modpack(&pack, &game_dir, download_concurrency(&core), core.sink.as_ref(), &task).await.msg();
        if let Err(e) = std::fs::remove_file(&pack.archive) {
            log::warn!("could not remove {}: {e}", pack.archive.display());
        }
        result
    })?;
    Ok(to_dto(&core, &inst))
}

/// Mods, resource packs and shader packs of the instance found on Modrinth.
#[tauri::command]
pub async fn content_sources(core: State<'_, Core>, instance_id: String) -> CmdResult<Vec<ContentSourceDto>> {
    let dir = game_dir(&core, &instance_id)?;
    Ok(core.modrinth.content_sources(&dir).await.msg()?.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn check_updates(core: State<'_, Core>, instance_id: String) -> CmdResult<Vec<ContentUpdateDto>> {
    let inst = core.store.get(&instance_id).msg()?;
    let updates = core.modrinth.check_updates(&target_of(&inst), &core.store.game_dir(&inst)).await.msg()?;
    Ok(updates.into_iter().map(Into::into).collect())
}

/// Updates the given files (task `update:<id>`), after copying them to `<instance>/backups/content-<date>/`;
/// returns when done.
#[tauri::command]
pub async fn apply_updates(app: AppHandle, core: State<'_, Core>, instance_id: String, file_names: Vec<String>) -> CmdResult<()> {
    let inst = core.store.get(&instance_id).msg()?;
    ensure_stopped(&core, &inst.id)?;
    let backup_dir = core.store.backups_dir(&inst.id).msg()?.join(format!("content-{}", cubby_instance::backup_stamp()));
    run_task(&app, format!("update:{}", inst.id), move |app, task| async move {
        let core = app.state::<Core>();
        let game_dir = core.store.game_dir(&inst);
        let selection = PackSelection::capture(&game_dir);
        let applied = core
            .modrinth
            .apply_updates(&target_of(&inst), &game_dir, &backup_dir, &file_names, download_concurrency(&core), core.sink.as_ref(), &task)
            .await
            .msg()?;
        selection.follow(&game_dir, &applied.renamed);
        applied.into_result().msg().map(|_| ())
    })
    .await
}

// Worlds

/// Zips a world into `<instance>/backups/` (task `backup:<id>`); returns the backup when done.
#[tauri::command]
pub async fn backup_world(app: AppHandle, core: State<'_, Core>, instance_id: String, folder: String) -> CmdResult<WorldBackupDto> {
    core.store.get(&instance_id).msg()?;
    run_task(&app, format!("backup:{instance_id}"), move |app, task| async move {
        blocking(move || {
            let core = app.state::<Core>();
            let mut progress = |label: &'static str, done: u64, total: u64| core.sink.progress(&task, label, done, total);
            Ok(core.store.backup_world(&instance_id, &folder, &mut progress).msg()?.into())
        })
        .await
    })
    .await
}

#[tauri::command]
pub async fn list_backups(core: State<'_, Core>, instance_id: String) -> CmdResult<Vec<WorldBackupDto>> {
    let store = core.store.clone();
    blocking(move || Ok(store.list_backups(&instance_id).msg()?.into_iter().map(Into::into).collect())).await
}

/// Replaces the world with the backup, after backing up the current world (task `restore:<id>`).
#[tauri::command]
pub async fn restore_backup(app: AppHandle, core: State<'_, Core>, instance_id: String, file_name: String) -> CmdResult<()> {
    core.store.get(&instance_id).msg()?;
    ensure_stopped(&core, &instance_id)?;
    run_task(&app, format!("restore:{instance_id}"), move |app, task| async move {
        blocking(move || {
            let core = app.state::<Core>();
            let mut progress = |label: &'static str, done: u64, total: u64| core.sink.progress(&task, label, done, total);
            core.store.restore_backup(&instance_id, &file_name, &mut progress).msg()?;
            Ok(())
        })
        .await
    })
    .await
}

/// Moves the world folder to the launcher trash.
#[tauri::command]
pub fn delete_world(core: State<'_, Core>, instance_id: String, folder: String) -> CmdResult<()> {
    ensure_stopped(&core, &instance_id)?;
    core.store.delete_world(&instance_id, &folder).msg()?;
    Ok(())
}

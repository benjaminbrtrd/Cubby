//! Preparing and launching the game, and the running games.

use super::*;

// Running the game

#[tauri::command]
pub fn running_games(core: State<'_, Core>) -> Vec<RunningGame> {
    core.games.running()
}

/// The log of the instance's current or latest run. After Cubby restarts, the log of the last session
/// comes from the copy saved when that game closed.
#[tauri::command]
pub async fn game_log(core: State<'_, Core>, id: String) -> CmdResult<Vec<LogLine>> {
    let lines = core.games.log_lines(&id);
    if !lines.is_empty() || core.games.is_running(&id) {
        return Ok(lines);
    }
    let path = session_log_path(&core, &id);
    blocking(move || Ok(read_session_log(&path))).await
}

/// Where the log of the instance's last session is kept: `<data>/logs/games/<instance id>.jsonl`.
fn session_log_path(core: &Core, id: &str) -> PathBuf {
    core.paths.logs().join("games").join(format!("{id}.jsonl"))
}

/// Saves the lines of a session that just ended, one JSON object per line, replacing the previous one.
fn save_session_log(path: &Path, lines: &[LogLine]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("jsonl.part");
    let mut out = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
    for line in lines {
        serde_json::to_writer(&mut out, line)?;
        out.write_all(b"\n")?;
    }
    out.into_inner().map_err(|e| e.into_error())?.sync_all()?;
    std::fs::rename(&tmp, path)
}

/// Reads a saved session log; a missing file or a damaged line gives what could be read.
fn read_session_log(path: &Path) -> Vec<LogLine> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

#[tauri::command]
pub fn kill_instance(core: State<'_, Core>, id: String) -> CmdResult<()> {
    core.games.kill(&id).msg()
}

pub(super) fn loader_spec(loader: &Loader) -> CmdResult<LoaderSpec> {
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
pub(super) async fn prepare(
    core: &Core,
    inst: &Instance,
    settings: &Settings,
    task: &str,
) -> CmdResult<(cubby_common::contracts::InstalledVersion, JavaInstall)> {
    let spec = loader_spec(&inst.loader)?;
    let sink: &dyn EventSink = core.sink.as_ref();
    core.sink.progress(task, "Checking the version", 0, 1);
    let version = cubby_meta::resolve(&core.http, &core.paths, &inst.minecraft, &spec)
        .await
        .msg()?;
    let game_dir = core.store.game_dir(inst);
    std::fs::create_dir_all(&game_dir).map_err(|e| e.to_string())?;
    // Java first: Forge and NeoForge run their installer steps with it.
    let java = java_for(core, inst, settings, &version, task).await?;
    let installed = cubby_meta::install_loader(
        &core.http,
        &core.paths,
        &version,
        &spec,
        Some(&java.executable),
        &game_dir,
        download_concurrency(core),
        sink,
        task,
    )
    .await
    .msg()?;
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

/// Installs what is missing, then starts the game, joining `server` or opening the world folder
/// `world` right away when given (Quick Play; worlds need Minecraft 1.20 or newer).
/// Progress goes out as events on task `launch:<id>`; the command returns once the game process
/// is running (or with the error).
#[tauri::command]
pub async fn launch_instance(
    app: AppHandle,
    core: State<'_, Core>,
    id: String,
    offline: bool,
    server: Option<String>,
    world: Option<String>,
) -> CmdResult<()> {
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

    let jvm_args = inst
        .java
        .jvm_args
        .clone()
        .unwrap_or_else(|| settings.java.jvm_args.clone());
    let window = inst
        .window
        .map(|w| (w.width, w.height))
        .unwrap_or((settings.game.width, settings.game.height));
    let opts = LaunchOptions {
        instance_id: inst.id.clone(),
        game_dir,
        java,
        min_memory_mb: inst
            .java
            .min_memory_mb
            .unwrap_or(settings.java.min_memory_mb),
        max_memory_mb: inst
            .java
            .max_memory_mb
            .unwrap_or(settings.java.max_memory_mb),
        extra_jvm_args: jvm_args.split_whitespace().map(str::to_owned).collect(),
        window: Some(window),
        fullscreen: settings.game.fullscreen,
        quick_play: quick_play(server, world, demo),
        demo,
    };
    let command = cubby_launch::build_command(&installed, &creds, &opts, &natives_dir).msg()?;

    let handle = core
        .games
        .launch(command, &inst.id, core.sink.clone() as Arc<dyn EventSink>)
        .await
        .msg()?;
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
        // Keep the log of this session, so it can still be read after Cubby restarts.
        let (path, lines) = (session_log_path(&core, &id), core.games.log_lines(&id));
        if !lines.is_empty()
            && let Err(e) = blocking(move || save_session_log(&path, &lines).map_err(|e| e.to_string())).await
        {
            log::warn!("could not save the log of the last session of {id}: {e}");
        }
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

/// Where the game goes right after it starts. The demo plays its own world and can't join servers.
pub(super) fn quick_play(
    server: Option<String>,
    world: Option<String>,
    demo: bool,
) -> Option<QuickPlay> {
    let clean = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if demo {
        return None;
    }
    clean(server)
        .map(QuickPlay::Multiplayer)
        .or_else(|| clean(world).map(QuickPlay::Singleplayer))
}

/// Identity for this launch, and whether it is the demo: without a signed-in account that owns the
/// game, Cubby only starts the Minecraft demo. Development builds play the full game offline
/// instead, so the launcher can be tested without signing in.
pub(super) async fn credentials(
    core: &Core,
    offline: bool,
) -> CmdResult<(LaunchCredentials, bool)> {
    let mut accounts = core.accounts.lock().await;
    let Some(account) = accounts.default_account().cloned() else {
        if cfg!(debug_assertions) {
            return Ok((dev_player(), false));
        }
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
    cubby_auth::refresh(&core.http, &client_id, &mut account)
        .await
        .msg()?;
    let creds = account.launch_credentials();
    accounts.add_or_update(account);
    accounts.save().msg()?;
    Ok((creds, false))
}

/// Offline player of development builds. `CUBBY_DEV_PLAYER` picks the in-game name (3 to 16
/// letters, digits or `_`), "Dev" otherwise. The profile id stays the same for a given name, so
/// worlds keep the player's inventory between launches.
pub(super) fn dev_player() -> LaunchCredentials {
    let valid = |n: &String| {
        (3..=16).contains(&n.len()) && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    let name = std::env::var("CUBBY_DEV_PLAYER")
        .ok()
        .filter(valid)
        .unwrap_or_else(|| "Dev".into());
    let id = uuid::Uuid::new_v3(
        &uuid::Uuid::NAMESPACE_OID,
        format!("OfflinePlayer:{name}").as_bytes(),
    );
    LaunchCredentials {
        username: name,
        uuid: id.simple().to_string(),
        access_token: "0".into(),
        user_type: "legacy".into(),
        xuid: String::new(),
    }
}

/// Java for this launch: the instance override, else an automatic pick (downloading Mojang's
/// runtime when needed), else the best installed one.
pub(super) async fn java_for(
    core: &Core,
    inst: &Instance,
    settings: &Settings,
    version: &cubby_common::version::VersionJson,
    task: &str,
) -> CmdResult<JavaInstall> {
    if let Some(exe) = &inst.java.executable {
        return cubby_java::probe(exe).await.msg();
    }
    if settings.java.auto {
        return cubby_java::find_or_download(
            &core.http,
            &core.paths,
            version,
            core.sink.as_ref(),
            task,
        )
        .await
        .msg();
    }
    let major = cubby_java::required_major(version);
    let installs = cubby_java::detect(&core.paths).await;
    cubby_java::pick(&installs, major).ok_or_else(|| format!("Java {major} is required but wasn't found. Turn on \"Choose Java automatically\" in Settings."))
}

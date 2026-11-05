//! What is inside an instance: mods, worlds, screenshots, packs, servers and world backups.

use super::*;

// Instance content

pub(super) fn game_dir(core: &Core, id: &str) -> CmdResult<PathBuf> {
    let inst = core.store.get(id).msg()?;
    Ok(core.store.game_dir(&inst))
}

#[tauri::command]
pub async fn list_mods(core: State<'_, Core>, id: String) -> CmdResult<Vec<ModFileDto>> {
    let dir = game_dir(&core, &id)?;
    blocking(move || {
        Ok(cubby_instance::list_mods(&dir)
            .into_iter()
            .map(Into::into)
            .collect())
    })
    .await
}

#[tauri::command]
pub fn set_mod_enabled(
    core: State<'_, Core>,
    id: String,
    file_name: String,
    enabled: bool,
) -> CmdResult<()> {
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
        Ok(cubby_instance::list_worlds(&dir)
            .into_iter()
            .map(|w| WorldDto::new(w, &saves))
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn list_screenshots(core: State<'_, Core>, id: String) -> CmdResult<Vec<ScreenshotDto>> {
    let dir = game_dir(&core, &id)?;
    blocking(move || {
        Ok(cubby_instance::list_screenshots(&dir)
            .into_iter()
            .map(Into::into)
            .collect())
    })
    .await
}

// Servers

#[tauri::command]
pub async fn ping_server(address: String) -> ServerStatusDto {
    cubby_launch::ping(&address, std::time::Duration::from_secs(5))
        .await
        .into()
}

#[tauri::command]
pub async fn list_servers(core: State<'_, Core>, id: String) -> CmdResult<Vec<ServerDto>> {
    let dir = game_dir(&core, &id)?;
    blocking(move || {
        Ok(cubby_instance::read_servers(&dir)
            .into_iter()
            .map(Into::into)
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn save_servers(
    core: State<'_, Core>,
    id: String,
    servers: Vec<ServerDto>,
) -> CmdResult<()> {
    let dir = game_dir(&core, &id)?;
    let entries: Vec<_> = servers.into_iter().map(Into::into).collect();
    blocking(move || cubby_instance::write_servers(&dir, &entries).msg()).await
}

// Packs and screenshots

pub(super) fn pack_kind(kind: &str) -> CmdResult<PackKind> {
    match kind {
        "resource" => Ok(PackKind::Resource),
        "shader" => Ok(PackKind::Shader),
        other => Err(format!("Unknown pack type: {other}.")),
    }
}

#[tauri::command]
pub async fn list_packs(
    core: State<'_, Core>,
    id: String,
    kind: String,
) -> CmdResult<Vec<cubby_instance::PackFile>> {
    let (dir, kind) = (game_dir(&core, &id)?, pack_kind(&kind)?);
    blocking(move || Ok(cubby_instance::list_packs(&dir, kind))).await
}

#[tauri::command]
pub fn set_resource_packs(
    core: State<'_, Core>,
    id: String,
    enabled: Vec<String>,
) -> CmdResult<()> {
    cubby_instance::set_resource_packs(&game_dir(&core, &id)?, &enabled).msg()
}

#[tauri::command]
pub fn set_shader_pack(
    core: State<'_, Core>,
    id: String,
    file_name: Option<String>,
) -> CmdResult<()> {
    cubby_instance::set_shader_pack(&game_dir(&core, &id)?, file_name.as_deref()).msg()
}

#[tauri::command]
pub fn delete_screenshot(core: State<'_, Core>, id: String, file_name: String) -> CmdResult<()> {
    cubby_instance::delete_screenshot(&game_dir(&core, &id)?, &file_name).msg()
}

// Worlds

/// Zips a world into `<instance>/backups/` (task `backup:<id>`); returns the backup when done.
#[tauri::command]
pub async fn backup_world(
    app: AppHandle,
    core: State<'_, Core>,
    instance_id: String,
    folder: String,
) -> CmdResult<WorldBackupDto> {
    core.store.get(&instance_id).msg()?;
    run_task(
        &app,
        format!("backup:{instance_id}"),
        move |app, task| async move {
            blocking(move || {
                let core = app.state::<Core>();
                let mut progress = |label: &'static str, done: u64, total: u64| {
                    core.sink.progress(&task, label, done, total)
                };
                Ok(core
                    .store
                    .backup_world(&instance_id, &folder, &mut progress)
                    .msg()?
                    .into())
            })
            .await
        },
    )
    .await
}

#[tauri::command]
pub async fn list_backups(
    core: State<'_, Core>,
    instance_id: String,
) -> CmdResult<Vec<WorldBackupDto>> {
    let store = core.store.clone();
    blocking(move || {
        Ok(store
            .list_backups(&instance_id)
            .msg()?
            .into_iter()
            .map(Into::into)
            .collect())
    })
    .await
}

/// Replaces the world with the backup, after backing up the current world (task `restore:<id>`).
#[tauri::command]
pub async fn restore_backup(
    app: AppHandle,
    core: State<'_, Core>,
    instance_id: String,
    file_name: String,
) -> CmdResult<()> {
    core.store.get(&instance_id).msg()?;
    ensure_stopped(&core, &instance_id)?;
    run_task(
        &app,
        format!("restore:{instance_id}"),
        move |app, task| async move {
            blocking(move || {
                let core = app.state::<Core>();
                let mut progress = |label: &'static str, done: u64, total: u64| {
                    core.sink.progress(&task, label, done, total)
                };
                core.store
                    .restore_backup(&instance_id, &file_name, &mut progress)
                    .msg()?;
                Ok(())
            })
            .await
        },
    )
    .await
}

/// Moves the world folder to the launcher trash.
#[tauri::command]
pub fn delete_world(core: State<'_, Core>, instance_id: String, folder: String) -> CmdResult<()> {
    ensure_stopped(&core, &instance_id)?;
    core.store.delete_world(&instance_id, &folder).msg()?;
    Ok(())
}

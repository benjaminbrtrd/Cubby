//! The datapacks of a world: listing, turning on and off, adding from files or from Modrinth.

use cubby_common::net::Download;
use cubby_instance::DatapackFile;
use tauri_plugin_dialog::DialogExt;

use super::content::game_dir;
use super::*;

#[tauri::command]
pub async fn list_datapacks(
    core: State<'_, Core>,
    id: String,
    world: String,
) -> CmdResult<Vec<DatapackFile>> {
    let dir = game_dir(&core, &id)?;
    blocking(move || cubby_instance::list_datapacks(&dir, &world).msg()).await
}

#[tauri::command]
pub fn set_datapack_enabled(
    core: State<'_, Core>,
    id: String,
    world: String,
    file_name: String,
    enabled: bool,
) -> CmdResult<()> {
    cubby_instance::set_datapack_enabled(&game_dir(&core, &id)?, &world, &file_name, enabled).msg()
}

#[tauri::command]
pub fn remove_datapack(
    core: State<'_, Core>,
    id: String,
    world: String,
    file_name: String,
) -> CmdResult<()> {
    cubby_instance::remove_datapack(&game_dir(&core, &id)?, &world, &file_name).msg()?;
    Ok(())
}

/// Asks for datapack .zip files and copies them into the world. Returns the names added (empty when cancelled).
#[tauri::command]
pub async fn add_datapack_files(
    app: AppHandle,
    core: State<'_, Core>,
    id: String,
    world: String,
) -> CmdResult<Vec<String>> {
    let dir = game_dir(&core, &id)?;
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Datapacks", &["zip"])
            .blocking_pick_files()
    })
    .await
    .map_err(|e| format!("The file dialog stopped unexpectedly ({e})."))?;
    let Some(files) = picked else {
        return Ok(Vec::new());
    };
    let paths = files
        .into_iter()
        .map(|f| f.into_path().map_err(|e| format!("Couldn't use the chosen file: {e}")))
        .collect::<CmdResult<Vec<_>>>()?;
    blocking(move || cubby_instance::import_datapacks(&dir, &world, &paths).msg()).await
}

/// Modrinth datapacks made for the instance's Minecraft version.
#[tauri::command]
pub async fn search_datapacks(
    core: State<'_, Core>,
    id: String,
    query: String,
    offset: u32,
) -> CmdResult<SearchResultsDto> {
    let inst = core.store.get(&id).msg()?;
    Ok(core
        .modrinth
        .search_datapacks(&query, &inst.minecraft, offset)
        .await
        .msg()?
        .into())
}

/// Downloads the newest version of a Modrinth datapack for the instance's Minecraft version into the world.
/// Returns the file name.
#[tauri::command]
pub async fn install_datapack(
    core: State<'_, Core>,
    id: String,
    world: String,
    project_id: String,
) -> CmdResult<String> {
    let inst = core.store.get(&id).msg()?;
    let game = core.store.game_dir(&inst);
    let file = core
        .modrinth
        .datapack_file(project_id.trim(), &inst.minecraft)
        .await
        .msg()?;
    let path = cubby_instance::datapack_path(&game, &world, &file.file_name).msg()?;
    let mut job = Download::new(file.url, &path).with_size(file.size);
    if file.sha1.len() == 40 {
        job = job.with_sha1(file.sha1);
    }
    cubby_common::net::ensure_file(&core.http, &job).await.msg()?;
    Ok(file.file_name)
}

// World map

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldMapDto {
    /// The PNG, as an asset URL (with a version, so a new drawing isn't hidden by the cache).
    pub url: String,
    pub min_x: i32,
    pub min_z: i32,
    pub size: u32,
    pub chunks: u32,
}

/// Draws the map around the world's spawn (task `map:<id>`), or returns the one drawn since the world last changed.
#[tauri::command]
pub async fn render_world_map(
    app: AppHandle,
    core: State<'_, Core>,
    id: String,
    world: String,
) -> CmdResult<WorldMapDto> {
    let inst = core.store.get(&id).msg()?;
    let game = core.store.game_dir(&inst);
    let cache = core.paths.instance(&inst.id).join("maps");
    run_task(&app, format!("map:{id}"), move |app, task| async move {
        let sink = app.state::<Core>().sink.clone();
        let map = blocking(move || {
            cubby_instance::render_map(&game, &world, &cache, |done, total| {
                sink.progress(&task, "Drawing the map", done, total)
            })
            .msg()
        })
        .await?;
        let stamp = std::fs::metadata(&map.path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Ok(WorldMapDto {
            url: format!("{}?v={stamp}", dto::asset_url(&map.path)),
            min_x: map.min_x,
            min_z: map.min_z,
            size: map.size,
            chunks: map.chunks,
        })
    })
    .await
}

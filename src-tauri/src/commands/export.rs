//! Exporting an instance as a modpack file: `.mrpack` (Modrinth) or `.zip` (CurseForge).

use cubby_common::export::ExportCandidate;
use cubby_modrinth::{ExportSpec, PackLoader as MrLoader};
use serde::{Deserialize, Serialize};
use tauri_plugin_dialog::DialogExt;

use super::*;

/// What can go into the pack besides mods, resource packs and shader packs.
#[tauri::command]
pub async fn export_candidates(core: State<'_, Core>, id: String) -> CmdResult<Vec<ExportCandidate>> {
    let dir = content::game_dir(&core, &id)?;
    blocking(move || Ok(cubby_common::export::export_candidates(&dir))).await
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    /// "mrpack" or "curseforge".
    pub format: String,
    pub name: String,
    pub version: String,
    /// Summary (Modrinth) or author (CurseForge).
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub include: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResultDto {
    pub path: String,
    /// Files listed for download from Modrinth or CurseForge.
    pub listed: usize,
    /// Files stored inside the pack.
    pub stored: usize,
    /// CurseForge without an API key: every mod is stored in the zip.
    pub without_key: bool,
}

fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { '-' } else { c })
        .collect();
    if cleaned.trim().is_empty() { "modpack".into() } else { cleaned }
}

/// Asks where to save, then writes the pack (task `export:<id>`). `None` when the dialog was cancelled.
#[tauri::command]
pub async fn export_instance(
    app: AppHandle,
    core: State<'_, Core>,
    id: String,
    request: ExportRequest,
) -> CmdResult<Option<ExportResultDto>> {
    let inst = core.store.get(&id).msg()?;
    let mrpack = match request.format.as_str() {
        "mrpack" => true,
        "curseforge" => false,
        other => return Err(format!("Unknown pack format: {other}.")),
    };
    let ext = if mrpack { "mrpack" } else { "zip" };
    let version = request.version.trim();
    let default_name = if version.is_empty() {
        format!("{}.{ext}", safe_file_name(&request.name))
    } else {
        format!("{} {}.{ext}", safe_file_name(&request.name), safe_file_name(version))
    };
    let dialog = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        dialog
            .dialog()
            .file()
            .set_file_name(default_name)
            .add_filter(if mrpack { "Modrinth modpack" } else { "CurseForge modpack" }, &[ext])
            .blocking_save_file()
    })
    .await
    .map_err(|e| format!("The file dialog stopped unexpectedly ({e})."))?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let mut dest = picked
        .into_path()
        .map_err(|e| format!("Couldn't use the chosen file: {e}"))?;
    if dest.extension().is_none_or(|e| !e.eq_ignore_ascii_case(ext)) {
        dest.set_extension(ext);
    }

    let game_dir = core.store.game_dir(&inst);
    let loader = dto::LoaderDto::from(&inst.loader);
    run_task(&app, format!("export:{id}"), move |app, task| async move {
        let core = app.state::<Core>();
        core.sink.progress(&task, "Writing the pack", 0, 1);
        let (listed, stored, without_key) = if mrpack {
            let kind = match loader.kind.as_str() {
                "fabric" => Some(MrLoader::Fabric),
                "quilt" => Some(MrLoader::Quilt),
                "forge" => Some(MrLoader::Forge),
                "neoforge" => Some(MrLoader::NeoForge),
                _ => None,
            };
            let spec = ExportSpec {
                name: request.name.clone(),
                version: request.version.clone(),
                summary: Some(request.note.clone()),
                minecraft: inst.minecraft.clone(),
                loader: kind.zip(loader.version.clone()),
                include: request.include.clone(),
            };
            let s = core.modrinth.export_mrpack(&game_dir, &dest, &spec).await.msg()?;
            (s.downloads, s.included, false)
        } else {
            let client = core.curseforge();
            let spec = cubby_curseforge::CfExportSpec {
                name: request.name.clone(),
                version: request.version.clone(),
                author: request.note.clone(),
                minecraft: inst.minecraft.clone(),
                loader_id: loader
                    .version
                    .clone()
                    .filter(|_| loader.kind != "vanilla")
                    .map(|v| format!("{}-{v}", loader.kind)),
                include: request.include.clone(),
            };
            let (listed, stored) = cubby_curseforge::export_curseforge(client.as_ref(), &game_dir, &dest, &spec)
                .await
                .msg()?;
            (listed, stored, client.is_none())
        };
        core.sink.progress(&task, "Writing the pack", 1, 1);
        Ok(Some(ExportResultDto {
            path: dest.to_string_lossy().into_owned(),
            listed,
            stored,
            without_key,
        }))
    })
    .await
}

/// Shows an exported file in Finder or Explorer.
#[tauri::command]
pub fn reveal_file(app: AppHandle, path: String) -> CmdResult<()> {
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| e.to_string())
}

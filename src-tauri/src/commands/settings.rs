//! Settings, Java and the data folders.

use super::*;

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

// Data folders

/// Total size of the files under `path` (0 when missing). Symlinks are not followed.
pub(super) fn dir_size(path: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(path) else {
        return 0;
    };
    rd.flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

pub(super) fn data_folder(core: &Core, key: &str) -> CmdResult<(&'static str, PathBuf)> {
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

pub(super) const DATA_FOLDERS: [&str; 7] = [
    "instances",
    "libraries",
    "assets",
    "java",
    "meta",
    "logs",
    "trash",
];

#[tauri::command]
pub async fn data_folders(app: AppHandle) -> CmdResult<Vec<DataFolderDto>> {
    blocking(move || {
        let core = app.state::<Core>();
        DATA_FOLDERS
            .iter()
            .map(|key| {
                let (label, path) = data_folder(&core, key)?;
                Ok(DataFolderDto {
                    key,
                    label,
                    size_bytes: dir_size(&path),
                    path: path.to_string_lossy().into_owned(),
                })
            })
            .collect()
    })
    .await
}

#[tauri::command]
pub fn open_data_folder(app: AppHandle, core: State<'_, Core>, key: String) -> CmdResult<()> {
    let (_, path) = data_folder(&core, &key)?;
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
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

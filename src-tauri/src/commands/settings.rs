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
    core.update_presence();
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

/// Deletes what is inside `dir` (the folder itself stays). A missing folder is fine.
fn empty_dir(dir: &Path) -> Result<(), String> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("Couldn't read {}: {e}", dir.display())),
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let result = match entry.file_type() {
            Ok(t) if t.is_dir() => std::fs::remove_dir_all(&path),
            _ => std::fs::remove_file(&path),
        };
        if let Err(e) = result
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(format!("Couldn't delete {}: {e}", path.display()));
        }
    }
    Ok(())
}

/// Empties the trash for good: what was deleted from the launcher (instances) and what instances moved aside
/// (mods and packs removed from their folders).
#[tauri::command]
pub async fn clear_trash(core: State<'_, Core>) -> CmdResult<()> {
    let launcher_trash = core.paths.root().join("trash");
    let instance_trash: Vec<PathBuf> = core
        .store
        .list()
        .unwrap_or_default()
        .iter()
        .map(|i| core.store.game_dir(i).join(".trash"))
        .collect();
    blocking(move || {
        for dir in std::iter::once(launcher_trash).chain(instance_trash) {
            empty_dir(&dir)?;
        }
        Ok(())
    })
    .await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emptying_a_folder_keeps_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("old-instance/saves")).unwrap();
        std::fs::write(dir.path().join("old-instance/saves/level.dat"), "x").unwrap();
        std::fs::write(dir.path().join("a.jar-20261107"), "x").unwrap();
        empty_dir(dir.path()).unwrap();
        assert!(dir.path().is_dir());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        // a folder that doesn't exist is fine
        empty_dir(&dir.path().join("missing")).unwrap();
    }
}

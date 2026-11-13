//! Instances and the game versions they can use.

use super::*;

// Instances

/// Marker written once the files of an instance's version are downloaded: holds the version key,
/// so changing the version or the loader makes the instance "not installed" again.
pub(super) const INSTALLED_MARKER: &str = ".cubby-installed";

pub(super) fn version_key(inst: &Instance) -> String {
    let loader = dto::LoaderDto::from(&inst.loader);
    format!(
        "{}|{}|{}",
        inst.minecraft,
        loader.kind,
        loader.version.unwrap_or_default()
    )
}

pub(crate) fn to_dto(core: &Core, inst: &Instance) -> InstanceDto {
    let dir = core.paths.instance(&inst.id);
    let installed = std::fs::read_to_string(dir.join(INSTALLED_MARKER))
        .is_ok_and(|k| k.trim() == version_key(inst));
    let blocked = cubby_packinstall::install::count_missing(&dir, &core.store.game_dir(inst));
    let mut dto = InstanceDto::new(inst, &dir, installed, blocked);
    dto.recommended_memory_mb = recommended_memory(core, inst);
    dto
}

/// The computer's memory in MB (0 when it can't be read).
pub(crate) fn total_memory_mb() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.total_memory() / (1024 * 1024)
}

/// What "Set memory automatically" gives this instance, from its mods and the computer's memory.
pub(crate) fn recommended_memory(core: &Core, inst: &Instance) -> u32 {
    let mods = cubby_instance::count_enabled_mods(&core.store.game_dir(inst));
    cubby_instance::recommended_memory_mb(core.total_memory_mb, mods)
}

/// Minimum and maximum heap for a launch: the instance's own values, else the automatic amount when the
/// global settings ask for it, else the global values.
pub(crate) fn memory_for(core: &Core, inst: &Instance, settings: &Settings) -> (u32, u32) {
    let max = inst.java.max_memory_mb.unwrap_or_else(|| {
        if settings.java.auto_memory {
            recommended_memory(core, inst)
        } else {
            settings.java.max_memory_mb
        }
    });
    let min = inst
        .java
        .min_memory_mb
        .unwrap_or(settings.java.min_memory_mb)
        .min(max);
    (min, max)
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
            group: spec
                .group
                .map(|g| g.trim().to_string())
                .filter(|g| !g.is_empty()),
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
    inst.group = instance
        .group
        .map(|g| g.trim().to_string())
        .filter(|g| !g.is_empty());
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
pub fn duplicate_instance(
    core: State<'_, Core>,
    id: String,
    name: String,
) -> CmdResult<InstanceDto> {
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
pub fn open_folder(
    app: AppHandle,
    core: State<'_, Core>,
    id: String,
    sub: Option<String>,
) -> CmdResult<()> {
    let inst = core.store.get(&id).msg()?;
    let mut dir = core.store.game_dir(&inst);
    if let Some(sub) = sub.filter(|s| !s.is_empty()) {
        let plain = Path::new(&sub)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
        if !plain {
            return Err(format!("\"{sub}\" isn't a valid folder."));
        }
        dir = dir.join(sub);
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

// Versions

#[tauri::command]
pub async fn list_versions(core: State<'_, Core>) -> CmdResult<Vec<GameVersionDto>> {
    let manifest = cubby_meta::fetch_manifest(&core.http, &core.paths)
        .await
        .msg()?;
    Ok(manifest
        .versions
        .into_iter()
        .map(|v| GameVersionDto {
            id: v.id,
            kind: v.kind,
            release_time: v.release_time,
        })
        .collect())
}

pub(super) fn loader_kind(kind: &str) -> CmdResult<LoaderKind> {
    match kind {
        "fabric" => Ok(LoaderKind::Fabric),
        "quilt" => Ok(LoaderKind::Quilt),
        "neoforge" => Ok(LoaderKind::NeoForge),
        "forge" => Ok(LoaderKind::Forge),
        other => Err(format!("Unknown loader: {other}.")),
    }
}

#[tauri::command]
pub async fn list_loader_versions(
    core: State<'_, Core>,
    kind: String,
    game: String,
) -> CmdResult<Vec<cubby_meta::LoaderVersion>> {
    cubby_meta::loader_versions(&core.http, loader_kind(&kind)?, &game)
        .await
        .msg()
}

/// Cubby's own icon, for a desktop shortcut: the app executable on Windows (it holds the icon), the bundle's
/// `.icns` on macOS, an installed PNG on Linux. `None` in development builds without a bundle.
fn app_icon() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    if cfg!(target_os = "windows") {
        return Some(exe);
    }
    if cfg!(target_os = "macos") {
        let resources = exe.parent()?.parent()?.join("Resources");
        return std::fs::read_dir(resources)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|e| e == "icns"));
    }
    ["/usr/share/icons/hicolor/128x128/apps/cubby.png", "/usr/share/pixmaps/cubby.png"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

/// Puts a shortcut on the Desktop that starts this instance. Returns its path.
#[tauri::command]
pub fn create_desktop_shortcut(
    app: AppHandle,
    core: State<'_, Core>,
    id: String,
) -> CmdResult<String> {
    let inst = core.store.get(&id).msg()?;
    let desktop = app.path().desktop_dir().map_err(|e| e.to_string())?;
    let path = cubby_instance::shortcut::create_shortcut(&desktop, &inst.id, &inst.name, app_icon().as_deref())
        .msg()?;
    Ok(path.to_string_lossy().into_owned())
}

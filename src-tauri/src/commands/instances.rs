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

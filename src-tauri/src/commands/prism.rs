//! Importing instances from Prism Launcher.

use super::*;

// Prism import

pub(super) fn prism_root(app: &AppHandle) -> CmdResult<PathBuf> {
    Ok(app
        .path()
        .data_dir()
        .map_err(|e| e.to_string())?
        .join("PrismLauncher"))
}

#[tauri::command]
pub async fn list_prism_instances(app: AppHandle) -> CmdResult<Vec<PrismInstanceDto>> {
    let root = prism_root(&app)?;
    blocking(move || {
        if !root.is_dir() {
            return Ok(Vec::new());
        }
        Ok(cubby_instance::list_prism_instances(&root)
            .msg()?
            .into_iter()
            .map(Into::into)
            .collect())
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
            let mut inst =
                cubby_instance::import_prism(&core.store, Path::new(&path), true).msg()?;
            inst.icon = inst
                .icon
                .take()
                .and_then(|key| prism_icon(&core, &inst, &root, &key));
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
pub(super) fn prism_icon(
    core: &Core,
    inst: &Instance,
    prism_root: &Path,
    key: &str,
) -> Option<String> {
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

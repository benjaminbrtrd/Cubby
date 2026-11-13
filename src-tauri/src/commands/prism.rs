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

// Other launchers

/// An instance of another launcher, for the import dialog.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignInstanceDto {
    /// "prism", "multiMc", "polyMc", "curseForge", "atLauncher", "gdLauncher", "modrinthApp", "official".
    pub launcher: cubby_instance::Launcher,
    pub launcher_name: &'static str,
    pub path: String,
    pub name: String,
    /// `None` when the launcher doesn't say: such an instance can't be imported.
    pub minecraft: Option<String>,
    pub loader: LoaderDto,
    pub group: Option<String>,
    pub game_dir: String,
}

impl From<cubby_instance::ForeignInstance> for ForeignInstanceDto {
    fn from(f: cubby_instance::ForeignInstance) -> Self {
        Self {
            launcher: f.launcher,
            launcher_name: f.launcher.title(),
            path: f.path,
            name: f.name,
            minecraft: f.minecraft,
            loader: (&f.loader).into(),
            group: f.group,
            game_dir: f.game_dir.to_string_lossy().into_owned(),
        }
    }
}

fn system_dirs(app: &AppHandle) -> CmdResult<cubby_instance::SystemDirs> {
    let path = app.path();
    Ok(cubby_instance::SystemDirs {
        data: path.data_dir().map_err(|e| e.to_string())?,
        home: path.home_dir().map_err(|e| e.to_string())?,
        documents: path.document_dir().ok(),
    })
}

/// Instances of every launcher Cubby knows, found in their usual folders.
#[tauri::command]
pub async fn list_launcher_instances(app: AppHandle) -> CmdResult<Vec<ForeignInstanceDto>> {
    let dirs = system_dirs(&app)?;
    blocking(move || {
        Ok(cubby_instance::list_all_foreign(&dirs)
            .into_iter()
            .map(Into::into)
            .collect())
    })
    .await
}

/// Imports instances of other launchers (by the `path` the listing gave), copying their game folders.
/// Their files are only read.
#[tauri::command]
pub async fn import_launcher_instances(
    app: AppHandle,
    paths: Vec<String>,
) -> CmdResult<Vec<InstanceDto>> {
    let dirs = system_dirs(&app)?;
    let app2 = app.clone();
    let imported = blocking(move || {
        let core = app2.state::<Core>();
        let found = cubby_instance::list_all_foreign(&dirs);
        let mut out = Vec::new();
        for path in paths {
            let f = found
                .iter()
                .find(|f| f.path == path)
                .ok_or_else(|| format!("{path} isn't there anymore."))?;
            let mut inst = cubby_instance::import_foreign(&core.store, f).msg()?;
            if matches!(
                f.launcher,
                cubby_instance::Launcher::Prism
                    | cubby_instance::Launcher::MultiMc
                    | cubby_instance::Launcher::PolyMc
            ) {
                // Their icons live in <launcher root>/icons, next to instances/.
                let root = Path::new(&f.path)
                    .parent()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
                    .unwrap_or_default();
                inst.icon = inst
                    .icon
                    .take()
                    .and_then(|key| prism_icon(&core, &inst, &root, &key));
                core.store.save(&inst).msg()?;
            }
            out.push(inst);
        }
        Ok(out)
    })
    .await?;
    let core = app.state::<Core>();
    Ok(imported.iter().map(|i| to_dto(&core, i)).collect())
}

//! What the New instance wizard adds to a new instance: mods, resource packs, shader packs and datapacks from
//! Modrinth or CurseForge, installed once the instance (and its modpack, if any) is ready.
//!
//! Datapacks belong to worlds, and a new instance has none. On a modded instance, a Modrinth datapack that also
//! ships as a mod for the instance's loader is installed as that mod, so it shapes every world from its creation.
//! The others wait in `<instance>/world-datapacks.json` and go into each new world when the game closes
//! ([`apply_world_datapacks`]); each world remembers what it got in `<world>/cubby-datapacks.json`.

use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use cubby_common::net::Download;
use cubby_instance::Instance;
use cubby_modrinth::{Folder, ProjectVersion};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::commands::{download_concurrency, run_task, target_of, CmdResult, OrMessage, PackSelection};
use crate::state::Core;

const PENDING_FILE: &str = "world-datapacks.json";
const WORLD_MARKER: &str = "cubby-datapacks.json";
/// How long to wait for the modpack install the extras follow.
const PACK_WAIT: Duration = Duration::from_secs(4 * 60 * 60);

/// One project picked in the wizard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraDto {
    /// "modrinth" or "curseforge".
    pub source: String,
    pub project_id: String,
    /// "mod", "resourcepack", "shader" or "datapack".
    pub kind: String,
    pub title: String,
}

/// What `install_extras` did.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtrasReportDto {
    /// Files downloaded, the required mods included.
    pub installed: usize,
    /// Projects with nothing for the instance's Minecraft version and loader.
    pub missing: Vec<String>,
    /// Datapacks waiting for the instance's new worlds.
    pub pending_datapacks: usize,
}

/// A datapack waiting for new worlds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingDatapack {
    pub source: String,
    pub project_id: String,
    pub title: String,
}

impl PendingDatapack {
    fn key(&self) -> String {
        format!("{}:{}", self.source, self.project_id)
    }
}

// Bookkeeping

pub(crate) fn load_pending(instance_dir: &Path) -> Vec<PendingDatapack> {
    std::fs::read(instance_dir.join(PENDING_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Adds datapacks to the pending list (each project once).
fn add_pending(instance_dir: &Path, add: &[PendingDatapack]) -> std::io::Result<()> {
    let mut list = load_pending(instance_dir);
    for p in add {
        if !list.iter().any(|x| x.key() == p.key()) {
            list.push(p.clone());
        }
    }
    std::fs::write(instance_dir.join(PENDING_FILE), serde_json::to_vec_pretty(&list).map_err(std::io::Error::other)?)
}

/// The pending datapacks a world hasn't got yet.
fn missing_in_world(world_dir: &Path, pending: &[PendingDatapack]) -> Vec<PendingDatapack> {
    let done = applied_keys(world_dir);
    pending.iter().filter(|p| !done.contains(&p.key())).cloned().collect()
}

fn applied_keys(world_dir: &Path) -> BTreeSet<String> {
    std::fs::read(world_dir.join(WORLD_MARKER)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn mark_applied(world_dir: &Path, keys: &[String]) -> std::io::Result<()> {
    let mut done = applied_keys(world_dir);
    done.extend(keys.iter().cloned());
    std::fs::write(world_dir.join(WORLD_MARKER), serde_json::to_vec_pretty(&done).map_err(std::io::Error::other)?)
}

// Installing

/// The newest release among the versions (else the newest version), newest first as given.
fn best_version(versions: &[ProjectVersion]) -> Option<&ProjectVersion> {
    versions.iter().find(|v| v.channel == "release").or_else(|| versions.first())
}

fn loader_key(inst: &Instance) -> String {
    crate::dto::LoaderDto::from(&inst.loader).kind
}

/// Waits while the instance's modpack installs; `Err` when the instance went away meanwhile (a failed install).
async fn wait_for_pack(core: &Core, id: &str) -> CmdResult<Instance> {
    let task = format!("modpack:{id}");
    let started = std::time::Instant::now();
    while core.tasks.lock().expect("tasks lock").contains(&task) {
        if started.elapsed() > PACK_WAIT {
            return Err("The modpack is still installing. Add the rest from the instance once it's done.".into());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    core.store.get(id).map_err(|_| "The instance is gone: its modpack couldn't be installed.".to_string())
}

/// Installs the projects picked in the wizard into a new instance (task `install:<id>`), after its modpack when
/// it has one (with no project, it only waits for the modpack). `turn_on` turns the new resource packs and shader
/// pack on. Returns when done.
#[tauri::command]
pub async fn install_extras(app: AppHandle, core: State<'_, Core>, instance_id: String, items: Vec<ExtraDto>, turn_on: bool) -> CmdResult<ExtrasReportDto> {
    // First the modpack, if one is installing: "Create and play" waits on this even with nothing to add.
    let inst = wait_for_pack(&core, &instance_id).await?;
    if items.is_empty() {
        return Ok(ExtrasReportDto::default());
    }
    crate::commands::ensure_stopped(&core, &inst.id)?;
    run_task(&app, format!("install:{}", inst.id), move |app, task| async move {
        let core = app.state::<Core>();
        install_items(&app, &core, &inst, &items, turn_on, &task).await
    })
    .await
}

async fn install_items(app: &AppHandle, core: &Core, inst: &Instance, items: &[ExtraDto], turn_on: bool, task: &str) -> CmdResult<ExtrasReportDto> {
    let game_dir = core.store.game_dir(inst);
    let loader = loader_key(inst);
    let modded = loader != "vanilla";
    let mut report = ExtrasReportDto::default();
    let mut version_ids: Vec<String> = Vec::new();
    let mut pending: Vec<PendingDatapack> = Vec::new();
    let mut cf_items: Vec<&ExtraDto> = Vec::new();
    core.sink.progress(task, "Finding versions", 0, items.len() as u64);

    for (i, item) in items.iter().enumerate() {
        core.sink.progress(task, "Finding versions", i as u64, items.len() as u64);
        let wait = || PendingDatapack { source: item.source.clone(), project_id: item.project_id.clone(), title: item.title.clone() };
        match (item.source.as_str(), item.kind.as_str()) {
            ("modrinth", "datapack") => {
                // A datapack that also ships as a mod for this loader shapes worlds from their creation.
                let as_mod = if modded {
                    core.modrinth
                        .versions(&item.project_id, Some(&inst.minecraft), Some(&loader))
                        .await
                        .ok()
                        .and_then(|vs| best_version(&vs.into_iter().filter(|v| v.loaders.iter().any(|l| l == &loader)).collect::<Vec<_>>()).map(|v| v.id.clone()))
                } else {
                    None
                };
                match as_mod {
                    Some(id) => version_ids.push(id),
                    None => pending.push(wait()),
                }
            }
            ("curseforge", "datapack") => pending.push(wait()),
            ("modrinth", kind) => {
                let mod_loader = (kind == "mod").then_some(loader.as_str());
                match core.modrinth.versions(&item.project_id, Some(&inst.minecraft), mod_loader).await {
                    Ok(vs) => match best_version(&vs) {
                        Some(v) => version_ids.push(v.id.clone()),
                        None => report.missing.push(item.title.clone()),
                    },
                    Err(e) => {
                        log::warn!("no version of {} for {}: {e}", item.title, inst.id);
                        report.missing.push(item.title.clone());
                    }
                }
            }
            ("curseforge", _) => cf_items.push(item),
            (other, _) => return Err(format!("Unknown source: {other}.")),
        }
    }

    let mut new_files: Vec<(Folder, String)> = Vec::new();
    if !version_ids.is_empty() {
        let plan = core.modrinth.plan_install_many(&version_ids, &target_of(inst), &game_dir).await.msg()?;
        let selection = PackSelection::capture(&game_dir);
        let applied = core.modrinth.install(&plan, &game_dir, download_concurrency(core), core.sink.as_ref(), task).await.msg()?;
        selection.follow(&game_dir, &applied.renamed);
        let files: Vec<(&str, &str)> = plan.items.iter().filter(|i| i.download).map(|i| (i.folder.as_str(), i.file_name.as_str())).collect();
        crate::origins::record(&core.paths.instance(&inst.id), &files, "modrinth");
        new_files.extend(plan.items.iter().filter(|i| i.download).map(|i| (i.folder, i.file_name.clone())));
        report.missing.extend(plan.missing.iter().filter_map(|m| m.title.clone()));
        applied.into_result().msg()?;
    }
    for item in cf_items {
        let (Ok(kind), Ok(project)) = (crate::cf_resources::resource_kind(&item.kind), crate::cf_resources::parse_project_id(&item.project_id)) else {
            report.missing.push(item.title.clone());
            continue;
        };
        match crate::cf_resources::install_planned(app, core, inst, project, None, kind, task).await {
            Ok((_, files)) => new_files.extend(files),
            Err(e) => {
                log::warn!("couldn't install {} from CurseForge: {e}", item.title);
                report.missing.push(item.title.clone());
            }
        }
    }
    report.installed = new_files.len();

    if turn_on {
        let packs: Vec<String> = new_files.iter().filter(|(f, _)| *f == Folder::ResourcePacks).map(|(_, n)| n.clone()).collect();
        if !packs.is_empty() {
            let mut on = packs;
            on.extend(cubby_instance::list_packs(&game_dir, cubby_instance::PackKind::Resource).into_iter().filter(|p| p.enabled).map(|p| p.file_name));
            on.dedup();
            if let Err(e) = cubby_instance::set_resource_packs(&game_dir, &on) {
                log::warn!("couldn't turn the new resource packs on: {e}");
            }
        }
        if let Some((_, shader)) = new_files.iter().find(|(f, _)| *f == Folder::ShaderPacks)
            && let Err(e) = cubby_instance::set_shader_pack(&game_dir, Some(shader))
        {
            log::warn!("couldn't turn the shader pack on: {e}");
        }
    }

    if !pending.is_empty() {
        add_pending(&core.paths.instance(&inst.id), &pending).map_err(|e| format!("Couldn't save the datapacks for new worlds: {e}"))?;
        report.pending_datapacks = load_pending(&core.paths.instance(&inst.id)).len();
    }
    core.sink.progress(task, "Done", 1, 1);
    Ok(report)
}

// Datapacks for new worlds

/// Puts the instance's pending datapacks into each world that hasn't got them. Failures are logged: a world gets
/// what downloaded, and the rest is tried again next time. Returns how many datapacks went into worlds.
pub(crate) async fn apply_world_datapacks(core: &Core, id: &str) -> usize {
    let instance_dir = core.paths.instance(id);
    let pending = load_pending(&instance_dir);
    if pending.is_empty() {
        return 0;
    }
    let Ok(inst) = core.store.get(id) else { return 0 };
    let game_dir = core.store.game_dir(&inst);
    let mut added = 0;
    for world in cubby_instance::list_worlds(&game_dir) {
        let world_dir = game_dir.join("saves").join(&world.folder_name);
        let todo = missing_in_world(&world_dir, &pending);
        let mut done = Vec::new();
        for p in todo {
            match download_into_world(core, &inst, &world.folder_name, &p).await {
                Ok(()) => {
                    added += 1;
                    done.push(p.key());
                }
                Err(e) => log::warn!("couldn't add the datapack {} to {}: {e}", p.title, world.folder_name),
            }
        }
        if !done.is_empty()
            && let Err(e) = mark_applied(&world_dir, &done)
        {
            log::warn!("couldn't remember the datapacks of {}: {e}", world.folder_name);
        }
    }
    added
}

async fn download_into_world(core: &Core, inst: &Instance, world: &str, p: &PendingDatapack) -> CmdResult<()> {
    match p.source.as_str() {
        "modrinth" => {
            let file = core.modrinth.datapack_file(&p.project_id, None, &inst.minecraft).await.msg()?;
            let path = cubby_instance::datapack_path(&core.store.game_dir(inst), world, &file.file_name).msg()?;
            let mut job = Download::new(file.url, &path).with_size(file.size);
            if file.sha1.len() == 40 {
                job = job.with_sha1(file.sha1);
            }
            cubby_common::net::ensure_file(&core.http, &job).await.msg()
        }
        "curseforge" => crate::cf_resources::download_datapack(core, inst, world, crate::cf_resources::parse_project_id(&p.project_id)?, None).await.map(|_| ()),
        other => Err(format!("Unknown source: {other}.")),
    }
}

/// Puts the instance's datapacks for new worlds into the worlds that haven't got them yet. Returns how many.
#[tauri::command]
pub async fn apply_world_datapacks_now(core: State<'_, Core>, instance_id: String) -> CmdResult<usize> {
    core.store.get(&instance_id).msg()?;
    Ok(apply_world_datapacks(&core, &instance_id).await)
}

/// The datapacks waiting for the instance's new worlds.
#[tauri::command]
pub fn world_datapacks(core: State<'_, Core>, instance_id: String) -> Vec<PendingDatapack> {
    load_pending(&core.paths.instance(&instance_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cubby-extras-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn dp(source: &str, id: &str) -> PendingDatapack {
        PendingDatapack { source: source.into(), project_id: id.into(), title: id.into() }
    }

    #[test]
    fn pending_list_keeps_each_project_once() {
        let dir = temp("pending");
        assert!(load_pending(&dir).is_empty());
        add_pending(&dir, &[dp("modrinth", "terralith"), dp("curseforge", "123")]).unwrap();
        add_pending(&dir, &[dp("modrinth", "terralith"), dp("modrinth", "incendium")]).unwrap();
        let keys: Vec<String> = load_pending(&dir).iter().map(PendingDatapack::key).collect();
        assert_eq!(keys, vec!["modrinth:terralith", "curseforge:123", "modrinth:incendium"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_world_gets_each_datapack_once() {
        let world = temp("world");
        let pending = vec![dp("modrinth", "terralith"), dp("curseforge", "123")];
        assert_eq!(missing_in_world(&world, &pending).len(), 2);
        mark_applied(&world, &["modrinth:terralith".to_string()]).unwrap();
        assert_eq!(missing_in_world(&world, &pending), vec![dp("curseforge", "123")]);
        mark_applied(&world, &["curseforge:123".to_string()]).unwrap();
        assert!(missing_in_world(&world, &pending).is_empty());
        let _ = std::fs::remove_dir_all(&world);
    }

    #[test]
    fn releases_come_first() {
        let v = |id: &str, channel: &str| ProjectVersion {
            id: id.into(),
            project_id: "p".into(),
            name: id.into(),
            version_number: id.into(),
            channel: channel.into(),
            game_versions: vec![],
            loaders: vec![],
            published: String::new(),
            downloads: 0,
            changelog: String::new(),
            files: vec![],
            dependencies: vec![],
        };
        assert_eq!(best_version(&[v("b2", "beta"), v("r1", "release")]).map(|x| x.id.as_str()), Some("r1"));
        assert_eq!(best_version(&[v("b2", "beta"), v("a1", "alpha")]).map(|x| x.id.as_str()), Some("b2"));
        assert!(best_version(&[]).is_none());
    }
}

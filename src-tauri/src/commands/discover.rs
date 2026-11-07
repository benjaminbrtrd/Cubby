//! Discover: Modrinth projects, installs and updates.

use super::*;

// Discover (Modrinth)

/// Game version and loader of an instance, as Modrinth sees them.
pub(super) fn target_of(inst: &Instance) -> Target {
    Target::new(
        inst.minecraft.clone(),
        Some(dto::LoaderDto::from(&inst.loader).kind.as_str()),
    )
}

/// Active resource packs and shader pack, carried over when an install or update renames their files.
pub(super) struct PackSelection {
    resource: Vec<String>,
    shader: Option<String>,
}

impl PackSelection {
    fn capture(game_dir: &Path) -> Self {
        Self {
            resource: cubby_instance::list_packs(game_dir, PackKind::Resource)
                .into_iter()
                .filter(|p| p.enabled)
                .map(|p| p.file_name)
                .collect(),
            shader: cubby_instance::list_packs(game_dir, PackKind::Shader)
                .into_iter()
                .find(|p| p.enabled)
                .map(|p| p.file_name),
        }
    }

    fn follow(&self, game_dir: &Path, renamed: &[Renamed]) {
        let new_name = |folder: Folder, old: &str| {
            renamed
                .iter()
                .find(|r| r.folder == folder && r.old == old)
                .map(|r| r.new.clone())
        };
        if self
            .resource
            .iter()
            .any(|n| new_name(Folder::ResourcePacks, n).is_some())
        {
            let list: Vec<String> = self
                .resource
                .iter()
                .map(|n| new_name(Folder::ResourcePacks, n).unwrap_or_else(|| n.clone()))
                .collect();
            if let Err(e) = cubby_instance::set_resource_packs(game_dir, &list) {
                log::warn!("could not update the active resource packs: {e}");
            }
        }
        if let Some(new) = self
            .shader
            .as_deref()
            .and_then(|s| new_name(Folder::ShaderPacks, s))
            && let Err(e) = cubby_instance::set_shader_pack(game_dir, Some(&new))
        {
            log::warn!("could not update the selected shader pack: {e}");
        }
        // Iris keeps each pack's settings next to it as `<pack file>.txt`: carry them over to the new file name.
        let dir = game_dir.join("shaderpacks");
        for r in renamed.iter().filter(|r| r.folder == Folder::ShaderPacks) {
            let (old, new) = (
                dir.join(format!("{}.txt", r.old)),
                dir.join(format!("{}.txt", r.new)),
            );
            if old.is_file()
                && !new.exists()
                && let Err(e) = std::fs::rename(&old, &new)
            {
                log::warn!("could not carry over the settings of {}: {e}", r.old);
            }
        }
    }
}

#[tauri::command]
pub async fn search_projects(
    core: State<'_, Core>,
    query: ProjectQueryDto,
) -> CmdResult<SearchResultsDto> {
    let query = SearchQuery::try_from(query)?;
    Ok(core.modrinth.search(&query).await.msg()?.into())
}

#[tauri::command]
pub async fn get_project(core: State<'_, Core>, id_or_slug: String) -> CmdResult<ProjectDto> {
    Ok(core.modrinth.project(id_or_slug.trim()).await.msg()?.into())
}

/// Newest first; `game_version` / `loader` (launcher key) `None` lists every version.
#[tauri::command]
pub async fn list_project_versions(
    core: State<'_, Core>,
    id_or_slug: String,
    game_version: Option<String>,
    loader: Option<String>,
) -> CmdResult<Vec<ProjectVersionDto>> {
    let versions = core
        .modrinth
        .versions(
            id_or_slug.trim(),
            game_version.as_deref(),
            loader.as_deref(),
        )
        .await
        .msg()?;
    Ok(versions.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn plan_install(
    core: State<'_, Core>,
    instance_id: String,
    version_id: String,
) -> CmdResult<InstallPlanDto> {
    let inst = core.store.get(&instance_id).msg()?;
    let plan = core
        .modrinth
        .plan_install(
            version_id.trim(),
            &target_of(&inst),
            &core.store.game_dir(&inst),
        )
        .await
        .msg()?;
    Ok(InstallPlanDto::new(&inst.id, plan))
}

/// Plans again, then downloads what is missing (task `install:<id>`); returns when done.
#[tauri::command]
pub async fn install_version(
    app: AppHandle,
    core: State<'_, Core>,
    instance_id: String,
    version_id: String,
) -> CmdResult<()> {
    let inst = core.store.get(&instance_id).msg()?;
    ensure_stopped(&core, &inst.id)?;
    run_task(
        &app,
        format!("install:{}", inst.id),
        move |app, task| async move {
            let core = app.state::<Core>();
            let game_dir = core.store.game_dir(&inst);
            core.sink.progress(&task, "Checking dependencies", 0, 1);
            let plan = core
                .modrinth
                .plan_install(version_id.trim(), &target_of(&inst), &game_dir)
                .await
                .msg()?;
            let selection = PackSelection::capture(&game_dir);
            let applied = core
                .modrinth
                .install(
                    &plan,
                    &game_dir,
                    download_concurrency(&core),
                    core.sink.as_ref(),
                    &task,
                )
                .await
                .msg()?;
            selection.follow(&game_dir, &applied.renamed);
            applied.into_result().msg().map(|_| ())
        },
    )
    .await
}

/// Names, versions and sizes for Modrinth version ids (shared pack codes list only ids). At most 400 ids.
#[tauri::command]
pub async fn describe_versions(
    core: State<'_, Core>,
    version_ids: Vec<String>,
) -> CmdResult<Vec<VersionBriefDto>> {
    if version_ids.len() > 400 {
        return Err("That pack lists too many files.".into());
    }
    let ids: Vec<String> = version_ids.iter().map(|v| v.trim().to_string()).collect();
    Ok(core
        .modrinth
        .describe_versions(&ids)
        .await
        .msg()?
        .into_iter()
        .map(Into::into)
        .collect())
}

/// Plans every version together, then downloads everything missing in one go (task `install:<id>`):
/// the download queue. A version that is already in the instance is skipped; returns when done.
#[tauri::command]
pub async fn install_versions(
    app: AppHandle,
    core: State<'_, Core>,
    instance_id: String,
    version_ids: Vec<String>,
) -> CmdResult<()> {
    let version_ids: Vec<String> = version_ids
        .into_iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();
    if version_ids.is_empty() {
        return Err("Nothing to install.".into());
    }
    let inst = core.store.get(&instance_id).msg()?;
    ensure_stopped(&core, &inst.id)?;
    run_task(
        &app,
        format!("install:{}", inst.id),
        move |app, task| async move {
            let core = app.state::<Core>();
            let game_dir = core.store.game_dir(&inst);
            core.sink.progress(&task, "Checking dependencies", 0, 1);
            let plan = core
                .modrinth
                .plan_install_many(&version_ids, &target_of(&inst), &game_dir)
                .await
                .msg()?;
            let selection = PackSelection::capture(&game_dir);
            let applied = core
                .modrinth
                .install(
                    &plan,
                    &game_dir,
                    download_concurrency(&core),
                    core.sink.as_ref(),
                    &task,
                )
                .await
                .msg()?;
            selection.follow(&game_dir, &applied.renamed);
            applied.into_result().msg().map(|_| ())
        },
    )
    .await
}

/// Copies an image (any https address, at most 8 MiB) into the instance folder as `icon.<ext>`; `None` for other URLs or on failure.
pub(crate) async fn download_icon(core: &Core, id: &str, url: &str) -> Option<String> {
    if !url.starts_with("https://") {
        log::warn!("icon {url} ignored: only https images are downloaded");
        return None;
    }
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let ext = path
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .filter(|e| ["png", "jpg", "jpeg", "webp", "gif"].contains(&e.as_str()))
        .unwrap_or_else(|| "png".into());
    let bytes = match core.http.get_bytes(url).await {
        Ok(bytes) => bytes,
        Err(e) => {
            log::warn!("could not download the modpack icon: {e}");
            return None;
        }
    };
    if bytes.len() > 8 * 1024 * 1024 {
        log::warn!("icon {url} ignored: it is larger than 8 MiB");
        return None;
    }
    let name = format!("icon.{ext}");
    match std::fs::write(core.paths.instance(id).join(&name), bytes) {
        Ok(()) => Some(name),
        Err(e) => {
            log::warn!("could not save the modpack icon: {e}");
            None
        }
    }
}

/// Downloads the modpack, creates its instance and returns it; the pack's files are installed in the
/// background (task `modpack:<id>`). `icon`: a block key, or a Modrinth image URL copied into the instance.
#[tauri::command]
pub async fn install_modpack(
    app: AppHandle,
    core: State<'_, Core>,
    version_id: String,
    name: String,
    group: Option<String>,
    icon: Option<String>,
) -> CmdResult<InstanceDto> {
    let pack = core
        .modrinth
        .fetch_modpack(version_id.trim(), &core.paths.meta().join("modrinth"))
        .await
        .msg()?;
    let loader = match &pack.loader {
        None => Loader::Vanilla,
        Some((PackLoader::Fabric, version)) => Loader::Fabric {
            version: version.clone(),
        },
        Some((PackLoader::Quilt, version)) => Loader::Quilt {
            version: version.clone(),
        },
        Some((PackLoader::Forge, version)) => Loader::Forge {
            version: version.clone(),
        },
        Some((PackLoader::NeoForge, version)) => Loader::NeoForge {
            version: version.clone(),
        },
    };
    let name = match name.trim() {
        "" => pack.title.clone(),
        name => name.to_string(),
    };
    let (icon_key, icon_url) = match icon.map(|i| i.trim().to_string()).filter(|i| !i.is_empty()) {
        Some(url) if url.contains("://") => (None, Some(url)),
        key => (key, None),
    };
    let mut inst = core
        .store
        .create(NewInstance {
            name,
            group: group
                .map(|g| g.trim().to_string())
                .filter(|g| !g.is_empty()),
            minecraft: pack.minecraft.clone(),
            loader,
            icon: icon_key,
        })
        .msg()?;
    inst.modpack = Some(ModpackInfo {
        provider: Some("modrinth".into()),
        project_id: pack.project_id.clone(),
        version_id: pack.version_id.clone(),
        title: pack.title.clone(),
        version_number: pack.version_number.clone(),
    });
    if let Some(url) = icon_url {
        inst.icon = download_icon(&core, &inst.id, &url).await;
    }
    if let Err(e) = core.store.save(&inst) {
        let _ = core.store.delete(&inst.id);
        return Err(dto::message(e));
    }
    let game_dir = core.store.game_dir(&inst);
    spawn_task(
        &app,
        format!("modpack:{}", inst.id),
        move |app, task| async move {
            let core = app.state::<Core>();
            let result = core
                .modrinth
                .install_modpack(
                    &pack,
                    &game_dir,
                    download_concurrency(&core),
                    core.sink.as_ref(),
                    &task,
                )
                .await
                .msg();
            if let Err(e) = std::fs::remove_file(&pack.archive) {
                log::warn!("could not remove {}: {e}", pack.archive.display());
            }
            result
        },
    )?;
    Ok(to_dto(&core, &inst))
}

/// Mods, resource packs and shader packs of the instance found on Modrinth.
#[tauri::command]
pub async fn content_sources(
    core: State<'_, Core>,
    instance_id: String,
) -> CmdResult<Vec<ContentSourceDto>> {
    let dir = game_dir(&core, &instance_id)?;
    Ok(core
        .modrinth
        .content_sources(&dir)
        .await
        .msg()?
        .into_iter()
        .map(Into::into)
        .collect())
}

#[tauri::command]
pub async fn check_updates(
    core: State<'_, Core>,
    instance_id: String,
) -> CmdResult<Vec<ContentUpdateDto>> {
    let inst = core.store.get(&instance_id).msg()?;
    let updates = core
        .modrinth
        .check_updates(&target_of(&inst), &core.store.game_dir(&inst))
        .await
        .msg()?;
    Ok(updates.into_iter().map(Into::into).collect())
}

/// Updates the given files (task `update:<id>`), after copying them to `<instance>/backups/content-<date>/`;
/// returns when done.
#[tauri::command]
pub async fn apply_updates(
    app: AppHandle,
    core: State<'_, Core>,
    instance_id: String,
    file_names: Vec<String>,
) -> CmdResult<()> {
    let inst = core.store.get(&instance_id).msg()?;
    ensure_stopped(&core, &inst.id)?;
    let backup_dir = core
        .store
        .backups_dir(&inst.id)
        .msg()?
        .join(format!("content-{}", cubby_instance::backup_stamp()));
    run_task(
        &app,
        format!("update:{}", inst.id),
        move |app, task| async move {
            let core = app.state::<Core>();
            let game_dir = core.store.game_dir(&inst);
            let selection = PackSelection::capture(&game_dir);
            let applied = core
                .modrinth
                .apply_updates(
                    &target_of(&inst),
                    &game_dir,
                    &backup_dir,
                    &file_names,
                    download_concurrency(&core),
                    core.sink.as_ref(),
                    &task,
                )
                .await
                .msg()?;
            selection.follow(&game_dir, &applied.renamed);
            applied.into_result().msg().map(|_| ())
        },
    )
    .await
}

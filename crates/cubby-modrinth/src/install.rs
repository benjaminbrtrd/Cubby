//! Running install plans, checking and applying updates, and installing modpacks.
//!
//! Downloads go through `cubby_common::net` (written to a `.part` file, SHA-1 checked, then renamed over
//! the target), so a content file is never left half-written.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use futures::StreamExt;
use cubby_common::events::{Event, EventSink};
use cubby_common::net::{ensure_file, Download, Http};
use cubby_common::{Error, Result};

use crate::api;
use crate::client::{check_id, Modrinth};
use crate::local::{find_file, LocalFile, DISABLED_SUFFIX};
use crate::model::*;
use crate::mrpack::{extract_overrides, MrpackIndex, PackDownload};
use crate::resolve::plain_file_name;

/// Shader "loaders" sent when checking shader packs for updates.
const SHADER_LOADERS: [&str; 4] = ["iris", "optifine", "canvas", "vanilla"];

fn emit(sink: &dyn EventSink, task: &str, label: &str, done: u64, total: u64) {
    sink.emit(Event::Progress { task: task.to_string(), label: label.to_string(), done, total });
}

/// One file to download from the first URL that works.
#[derive(Debug, Clone)]
struct Job {
    urls: Vec<String>,
    path: PathBuf,
    sha1: String,
    size: Option<u64>,
}

async fn download_any(http: &Http, job: &Job) -> Result<()> {
    let mut last = None;
    for url in &job.urls {
        let mut download = Download::new(url.clone(), job.path.clone()).with_sha1(job.sha1.clone());
        if let Some(size) = job.size {
            download = download.with_size(size);
        }
        match ensure_file(http, &download).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                tracing::warn!("download of {url} failed: {e}");
                last = Some(e);
            }
        }
    }
    Err(last.unwrap_or_else(|| Error::Other(format!("There's no download link for {}.", job.path.display()))))
}

/// Downloads every job, `concurrency` at a time; one result per job, in order.
async fn run_jobs(http: &Http, jobs: &[Job], concurrency: usize, mut on_progress: impl FnMut(u64, u64)) -> Vec<Result<()>> {
    let total = jobs.len() as u64;
    on_progress(0, total);
    let mut results: Vec<Option<Result<()>>> = (0..jobs.len()).map(|_| None).collect();
    // Owned jobs: a closure taking `&Job` makes the future not provably `Send` (rust-lang/rust#102211).
    let mut stream = futures::stream::iter(jobs.iter().cloned().enumerate().map(|(i, job): (usize, Job)| {
        let http = http.clone();
        async move { (i, download_any(&http, &job).await) }
    }))
    .buffer_unordered(concurrency.max(1));
    let mut done = 0;
    while let Some((i, result)) = stream.next().await {
        results[i] = Some(result);
        done += 1;
        on_progress(done, total);
    }
    results.into_iter().map(|r| r.unwrap_or_else(|| Err(Error::Cancelled))).collect()
}

/// Moves a replaced content file to `<game_dir>/.trash/<folder>/<name>-<timestamp>` (like removing a mod).
fn trash_file(game_dir: &Path, folder: Folder, file_name: &str) -> Result<()> {
    let src = game_dir.join(folder.as_str()).join(file_name);
    if !src.exists() {
        return Ok(());
    }
    let trash = game_dir.join(".trash").join(folder.as_str());
    std::fs::create_dir_all(&trash).map_err(|e| Error::io(&trash, e))?;
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let mut dest = trash.join(format!("{file_name}-{stamp}"));
    let mut n = 1;
    while dest.exists() {
        n += 1;
        dest = trash.join(format!("{file_name}-{stamp}-{n}"));
    }
    std::fs::rename(&src, &dest).map_err(|e| Error::io(&src, e))
}

fn remove_file(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::io(path, e)),
    }
}

/// Loaders sent to `/version_files/update` for the files of a folder.
fn update_loaders(folder: Folder, target: &Target) -> Vec<String> {
    match folder {
        Folder::Mods => target.mod_loaders(),
        Folder::ResourcePacks => vec!["minecraft".into()],
        Folder::ShaderPacks => SHADER_LOADERS.iter().map(|l| l.to_string()).collect(),
    }
}

/// A file with a newer version, and that version's primary file.
struct PendingUpdate {
    local: LocalFile,
    latest: api::Version,
    file: api::VersionFile,
}

impl Modrinth {
    /// Downloads the plan's items marked `download` into the game directory, then moves the files they
    /// replace to `<game_dir>/.trash/<folder>/`. Emits progress on `task` (the caller emits `TaskFinished`).
    /// `Err` means nothing was changed; download failures are reported in [`Applied::error`].
    pub async fn install(&self, plan: &InstallPlan, game_dir: &Path, concurrency: usize, sink: &dyn EventSink, task: &str) -> Result<Applied> {
        let items: Vec<&PlanItem> = plan.items.iter().filter(|i| i.download).collect();
        for item in &items {
            if !plain_file_name(&item.file_name) || item.url.is_empty() || item.sha1.is_empty() {
                return Err(Error::Other(format!("Can't download {}: the install plan is invalid. Try again.", item.file_name)));
            }
        }
        let jobs: Vec<Job> = items
            .iter()
            .map(|i| Job {
                urls: vec![i.url.clone()],
                path: game_dir.join(i.folder.as_str()).join(&i.file_name),
                sha1: i.sha1.clone(),
                size: Some(i.size),
            })
            .collect();
        let results = run_jobs(&self.http, &jobs, concurrency, |done, total| emit(sink, task, "Downloading content", done, total)).await;

        let mut renamed = Vec::new();
        let mut first_error = None;
        for (item, result) in items.iter().zip(results) {
            if let Err(e) = result {
                first_error.get_or_insert(e);
                continue;
            }
            let Some((old_folder, old_name)) = &item.replaces else { continue };
            if *old_folder == item.folder && *old_name == item.file_name {
                continue;
            }
            match trash_file(game_dir, *old_folder, old_name) {
                Ok(()) => renamed.push(Renamed { folder: *old_folder, old: old_name.clone(), new: item.file_name.clone() }),
                Err(e) => {
                    first_error.get_or_insert(e);
                }
            }
        }
        emit(sink, task, "Done", 1, 1);
        Ok(Applied { renamed, error: first_error })
    }

    /// Newest version of each file (by SHA-1) for the target, from `/version_files/update`, one request per folder.
    async fn latest_for(&self, files: &[&LocalFile], target: &Target) -> Result<HashMap<String, api::Version>> {
        let mut out = HashMap::new();
        for folder in Folder::ALL {
            let hashes: Vec<String> = files.iter().filter(|f| f.folder == folder).map(|f| f.sha1.clone()).collect();
            let loaders = update_loaders(folder, target);
            if hashes.is_empty() || loaders.is_empty() {
                continue;
            }
            out.extend(self.latest_versions(&hashes, &loaders, std::slice::from_ref(&target.game_version)).await?);
        }
        Ok(out)
    }

    fn pending_updates(files: Vec<LocalFile>, latest: &HashMap<String, api::Version>, current: Option<&HashMap<String, api::Version>>) -> Vec<PendingUpdate> {
        files
            .into_iter()
            .filter_map(|local| {
                let latest = latest.get(&local.sha1)?;
                if latest.has_sha1(&local.sha1) || current.and_then(|c| c.get(&local.sha1)).is_some_and(|c| c.id == latest.id) {
                    return None;
                }
                let file = latest.primary_file()?.clone();
                plain_file_name(&file.filename).then_some(())?;
                Some(PendingUpdate { local, latest: latest.clone(), file })
            })
            .collect()
    }

    /// Installed files with a newer version for the target, sorted by title.
    pub async fn check_updates(&self, target: &Target, game_dir: &Path) -> Result<Vec<ContentUpdate>> {
        let files = self.local_files(game_dir).await;
        let current = self.identify(&files).await?;
        let known: Vec<LocalFile> = files.into_iter().filter(|f| current.contains_key(&f.sha1)).collect();
        let latest = self.latest_for(&known.iter().collect::<Vec<_>>(), target).await?;
        let pending = Self::pending_updates(known, &latest, Some(&current));
        if pending.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<String> = pending.iter().map(|p| p.latest.project_id.clone()).collect();
        let projects: HashMap<String, api::Project> = self.projects_raw(&ids).await?.into_iter().map(|p| (p.id.clone(), p)).collect();
        let mut out: Vec<ContentUpdate> = pending
            .into_iter()
            .map(|p| {
                let current_version = current.get(&p.local.sha1).map(|v| v.version_number.clone()).unwrap_or_default();
                ContentUpdate {
                    title: projects
                        .get(&p.latest.project_id)
                        .map(|pr| pr.title.clone())
                        .filter(|t| !t.is_empty())
                        .unwrap_or_else(|| p.local.file_name.clone()),
                    file_name: p.local.file_name,
                    folder: p.local.folder,
                    project_id: p.latest.project_id,
                    current_version,
                    latest_version_id: p.latest.id,
                    latest_version: p.latest.version_number,
                    changelog: p.latest.changelog,
                }
            })
            .collect();
        out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()).then_with(|| a.file_name.cmp(&b.file_name)));
        Ok(out)
    }

    /// Replaces each named file (looked up in mods, then resourcepacks, then shaderpacks) by its newest
    /// version for the target. The old files are first copied to `backup_dir/<folder>/`; a disabled mod stays
    /// disabled. Files already up to date are skipped. `Err` means nothing was changed; download failures
    /// are reported in [`Applied::error`].
    #[allow(clippy::too_many_arguments)]
    pub async fn apply_updates(
        &self,
        target: &Target,
        game_dir: &Path,
        backup_dir: &Path,
        file_names: &[String],
        concurrency: usize,
        sink: &dyn EventSink,
        task: &str,
    ) -> Result<Applied> {
        emit(sink, task, "Checking for updates", 0, 1);
        let mut selected = Vec::new();
        for name in file_names {
            if !plain_file_name(name) {
                return Err(Error::Other(format!("{name:?} isn't a valid file name.")));
            }
            let file = find_file(game_dir, name).ok_or_else(|| Error::NotFound(format!("content file {name}")))?;
            if !selected.iter().any(|f: &LocalFile| f.path == file.path) {
                selected.push(file);
            }
        }
        let files = self.hash_files(selected).await;
        let latest = self.latest_for(&files.iter().collect::<Vec<_>>(), target).await?;
        let pending = Self::pending_updates(files, &latest, None);
        if pending.is_empty() {
            emit(sink, task, "Done", 1, 1);
            return Ok(Applied::default());
        }

        let total = pending.len() as u64;
        for (i, p) in pending.iter().enumerate() {
            emit(sink, task, "Backing up files", i as u64, total);
            let dir = backup_dir.join(p.local.folder.as_str());
            std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
            let dest = dir.join(&p.local.file_name);
            std::fs::copy(&p.local.path, &dest).map_err(|e| Error::io(&p.local.path, e))?;
        }
        emit(sink, task, "Backing up files", total, total);

        let mut jobs = Vec::with_capacity(pending.len());
        let mut new_names = Vec::with_capacity(pending.len());
        for p in &pending {
            let sha1 = p.file.sha1().filter(|h| h.len() == 40).ok_or_else(|| Error::Other(format!("Modrinth has no SHA-1 for {}, so it can't be verified.", p.file.filename)))?;
            let new_name = if p.local.enabled { p.file.filename.clone() } else { format!("{}{DISABLED_SUFFIX}", p.file.filename) };
            jobs.push(Job {
                urls: vec![p.file.url.clone()],
                path: game_dir.join(p.local.folder.as_str()).join(&new_name),
                sha1: sha1.to_lowercase(),
                size: Some(p.file.size),
            });
            new_names.push(new_name);
        }
        let results = run_jobs(&self.http, &jobs, concurrency, |done, total| emit(sink, task, "Downloading updates", done, total)).await;

        let mut renamed = Vec::new();
        let mut first_error = None;
        for ((p, new_name), result) in pending.iter().zip(new_names).zip(results) {
            if let Err(e) = result {
                first_error.get_or_insert(e);
                continue;
            }
            if p.local.file_name == new_name {
                continue;
            }
            // The old file is in the backup folder.
            match remove_file(&p.local.path) {
                Ok(()) => renamed.push(Renamed { folder: p.local.folder, old: p.local.file_name.clone(), new: new_name }),
                Err(e) => {
                    first_error.get_or_insert(e);
                }
            }
        }
        emit(sink, task, "Done", 1, 1);
        Ok(Applied { renamed, error: first_error })
    }

    /// Downloads the primary `.mrpack` of a modpack version into `cache_dir` (SHA-1 checked) and reads its
    /// index. Fails before anything is created when the pack is unusable (no Minecraft version, unsafe paths...).
    pub async fn fetch_modpack(&self, version_id: &str, cache_dir: &Path) -> Result<Modpack> {
        check_id(version_id)?;
        let version = self.version_raw(version_id).await?;
        let project = self.project_raw(&version.project_id).await?;
        if ProjectKind::from_api(&project.project_type) != ProjectKind::Modpack {
            return Err(Error::Other(format!("{} isn't a modpack.", project.title)));
        }
        let file = version
            .files
            .iter()
            .filter(|f| f.filename.to_lowercase().ends_with(".mrpack"))
            .min_by_key(|f| !f.primary)
            .ok_or_else(|| Error::Other(format!("{} {} has no .mrpack file.", project.title, version.version_number)))?;
        let sha1 = file.sha1().ok_or_else(|| Error::Other(format!("Modrinth has no SHA-1 for {}, so it can't be verified.", file.filename)))?;
        let archive = cache_dir.join(format!("{version_id}.mrpack"));
        ensure_file(&self.http, &Download::new(file.url.clone(), archive.clone()).with_sha1(sha1.to_lowercase())).await?;

        let path = archive.clone();
        let index = tokio::task::spawn_blocking(move || MrpackIndex::read(&path)).await.map_err(|e| Error::Other(e.to_string()))??;
        let minecraft = index.minecraft().ok_or_else(|| Error::Other("The modpack doesn't say which Minecraft version it needs.".into()))?.to_string();
        index.client_downloads()?;
        Ok(Modpack {
            project_id: project.id.clone(),
            version_id: version.id.clone(),
            title: project.title.clone(),
            version_number: version.version_number.clone(),
            icon_url: project.icon_url.clone().filter(|u| !u.is_empty()),
            loader: index.loader(),
            minecraft,
            index,
            archive,
        })
    }

    /// Downloads the pack's client files into `game_dir` (SHA-1 checked), then extracts `overrides/` and
    /// `client-overrides/`. Emits progress on `task`; the caller emits `TaskFinished`.
    pub async fn install_modpack(&self, pack: &Modpack, game_dir: &Path, concurrency: usize, sink: &dyn EventSink, task: &str) -> Result<()> {
        let jobs: Vec<Job> = pack
            .index
            .client_downloads()?
            .into_iter()
            .map(|PackDownload { path, urls, sha1, size }| Job { urls, path: game_dir.join(path), sha1, size })
            .collect();
        let results = run_jobs(&self.http, &jobs, concurrency, |done, total| emit(sink, task, "Downloading modpack files", done, total)).await;
        if let Some(e) = results.into_iter().find_map(|r| r.err()) {
            return Err(e);
        }

        let (archive, dir, sink_task) = (pack.archive.clone(), game_dir.to_path_buf(), task.to_string());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(u64, u64)>();
        let extraction = tokio::task::spawn_blocking(move || {
            extract_overrides(&archive, &dir, |done, total| {
                let _ = tx.send((done, total));
            })
        });
        while let Some((done, total)) = rx.recv().await {
            emit(sink, &sink_task, "Copying modpack files", done, total);
        }
        extraction.await.map_err(|e| Error::Other(e.to_string()))??;
        emit(sink, task, "Done", 1, 1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(folder: Folder, name: &str, sha1: &str, enabled: bool) -> LocalFile {
        LocalFile { folder, file_name: name.into(), path: PathBuf::from(name), enabled, size: 1, sha1: sha1.into() }
    }

    fn version(id: &str, file: &str, sha1: &str) -> api::Version {
        api::Version {
            id: id.into(),
            project_id: format!("p-{id}"),
            version_number: id.into(),
            files: vec![api::VersionFile {
                url: format!("https://cdn/{file}"),
                filename: file.into(),
                primary: true,
                size: 3,
                hashes: HashMap::from([("sha1".to_string(), sha1.to_string())]),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn pending_updates_skip_current_versions() {
        let files = vec![
            local(Folder::Mods, "a-1.jar", "h1", true),
            local(Folder::Mods, "b.jar", "h2", true),
            local(Folder::ShaderPacks, "c.zip", "h3", true),
            local(Folder::Mods, "d.jar", "h4", true),
        ];
        let latest = HashMap::from([
            ("h1".to_string(), version("a2", "a-2.jar", "n1")),
            ("h2".to_string(), version("b1", "b.jar", "h2")),
            ("h3".to_string(), version("c2", "../c.zip", "n3")),
        ]);
        let pending = Modrinth::pending_updates(files.clone(), &latest, None);
        assert_eq!(pending.iter().map(|p| p.local.file_name.as_str()).collect::<Vec<_>>(), ["a-1.jar"]);
        // Same version id as the current one: no update, even with different hashes.
        let current = HashMap::from([("h1".to_string(), version("a2", "a-2.jar", "other"))]);
        assert!(Modrinth::pending_updates(files, &latest, Some(&current)).is_empty());
    }

    #[test]
    fn update_loaders_per_folder() {
        let target = Target::new("1.21.1", Some("quilt"));
        assert_eq!(update_loaders(Folder::Mods, &target), ["quilt", "fabric"]);
        assert_eq!(update_loaders(Folder::ResourcePacks, &target), ["minecraft"]);
        assert!(update_loaders(Folder::ShaderPacks, &target).contains(&"iris".to_string()));
        assert!(update_loaders(Folder::Mods, &Target::new("1.21.1", None)).is_empty());
    }

    /// `POST /version_files` and `/version_files/update` against the real API, without downloading.
    #[tokio::test]
    #[ignore = "needs network"]
    async fn live_lookup_by_hash() {
        let m = Modrinth::new(Http::new().unwrap());
        let target = Target::new("1.21.1", Some("fabric"));
        let mut versions = m.project_versions_raw("lithium", &target.mod_loaders(), std::slice::from_ref(&target.game_version)).await.unwrap();
        versions.sort_by_key(|v| v.published_millis());
        let (oldest, newest) = (versions.first().unwrap(), versions.last().unwrap());
        let sha1 = oldest.primary_file().unwrap().sha1().unwrap().to_lowercase();
        let current = m.version_files(std::slice::from_ref(&sha1)).await.unwrap();
        assert_eq!(current.get(&sha1).map(|v| v.id.as_str()), Some(oldest.id.as_str()));
        let local = local(Folder::Mods, "lithium.jar", &sha1, true);
        let latest = m.latest_for(&[&local], &target).await.unwrap();
        assert_eq!(latest.get(&sha1).map(|v| v.id.as_str()), Some(newest.id.as_str()));
    }

    #[tokio::test]
    async fn modpack_without_downloads_extracts_overrides() {
        use std::io::Write;
        use std::sync::Mutex;
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/modrinth/testdata").join(format!("pack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let archive = dir.join("pack.mrpack");
        let index = r#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"P","files":[],"dependencies":{"minecraft":"1.21.1"}}"#;
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        for (name, body) in [("modrinth.index.json", index), ("overrides/config/a.txt", "a"), ("client-overrides/config/a.txt", "client")] {
            zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();

        let index = MrpackIndex::read(&archive).unwrap();
        let pack = Modpack {
            project_id: "p".into(),
            version_id: "v".into(),
            title: "P".into(),
            version_number: "1".into(),
            icon_url: None,
            minecraft: index.minecraft().unwrap().into(),
            loader: index.loader(),
            index,
            archive,
        };
        let events = Mutex::new(Vec::new());
        let sink = |e: Event| events.lock().unwrap().push(e);
        let game = dir.join(".minecraft");
        Modrinth::new(Http::new().unwrap()).install_modpack(&pack, &game, 4, &sink, "modpack:p").await.unwrap();
        assert_eq!(std::fs::read_to_string(game.join("config/a.txt")).unwrap(), "client");
        let labels: Vec<String> = events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                Event::Progress { task, label, done, total } if task == "modpack:p" => Some(format!("{label} {done}/{total}")),
                _ => None,
            })
            .collect();
        assert_eq!(labels.first().map(String::as_str), Some("Downloading modpack files 0/0"));
        assert!(labels.contains(&"Copying modpack files 2/2".to_string()), "{labels:?}");
        assert_eq!(labels.last().map(String::as_str), Some("Done 1/1"));
    }

    #[test]
    fn trash_keeps_replaced_files() {
        let game = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/modrinth/testdata").join(format!("trash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&game);
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::write(game.join("mods/old.jar"), "x").unwrap();
        trash_file(&game, Folder::Mods, "old.jar").unwrap();
        assert!(!game.join("mods/old.jar").exists());
        let trashed: Vec<_> = std::fs::read_dir(game.join(".trash/mods")).unwrap().flatten().collect();
        assert_eq!(trashed.len(), 1);
        trash_file(&game, Folder::Mods, "missing.jar").unwrap();
    }
}

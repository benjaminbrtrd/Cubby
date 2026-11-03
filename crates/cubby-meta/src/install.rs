//! Resolving a game version (with an optional loader) and installing its files.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use cubby_common::contracts::{InstalledVersion, NativeJar};
use cubby_common::events::{Event, EventSink};
use cubby_common::net::{download_all, ensure_file, Download, Http};
use cubby_common::paths::Paths;
use cubby_common::version::{maven_path, Artifact, AssetIndex, Library, RuleEnv, VersionJson, ASSETS_BASE_URL, LIBRARIES_BASE_URL};
use cubby_common::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::forge::Forge;
use crate::loaders::{loader_profile, LoaderKind};
use crate::mojang::fetch_version;
use crate::profile::merge;

/// Which loader to put on top of a vanilla version.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "version", rename_all = "lowercase")]
pub enum LoaderSpec {
    /// No loader.
    Vanilla,
    /// Fabric loader with this version.
    Fabric(String),
    /// Quilt loader with this version.
    Quilt(String),
    /// Forge with this version, without the Minecraft prefix (`47.2.0`, as listed by `loader_versions`).
    Forge(String),
    /// NeoForge with this version (`21.1.172`; for 1.20.1 the legacy `47.1.106`).
    NeoForge(String),
}

impl LoaderSpec {
    /// The installer-based loader kind and version, if this is Forge or NeoForge.
    fn installer_loader(&self) -> Option<(LoaderKind, &str)> {
        match self {
            LoaderSpec::Forge(v) => Some((LoaderKind::Forge, v)),
            LoaderSpec::NeoForge(v) => Some((LoaderKind::NeoForge, v)),
            _ => None,
        }
    }
}

/// Fetches the vanilla JSON of `game_version` and merges the loader profile on top when there is one.
///
/// For Forge and NeoForge this downloads the loader's installer jar into the meta cache (once) to read its profile.
pub async fn resolve(http: &Http, paths: &Paths, game_version: &str, loader: &LoaderSpec) -> Result<VersionJson> {
    let vanilla = fetch_version(http, paths, game_version).await?;
    let (kind, loader_version) = match loader {
        LoaderSpec::Vanilla => return Ok(vanilla),
        LoaderSpec::Fabric(v) => (LoaderKind::Fabric, v),
        LoaderSpec::Quilt(v) => (LoaderKind::Quilt, v),
        LoaderSpec::Forge(v) => (LoaderKind::Forge, v),
        LoaderSpec::NeoForge(v) => (LoaderKind::NeoForge, v),
    };
    let profile = loader_profile(http, paths, kind, game_version, loader_version).await?;
    Ok(merge(vanilla, profile))
}

/// Libraries of a version for one environment.
#[derive(Debug, Clone, Default)]
pub struct LibraryPlan {
    /// Files to download (classpath libraries and legacy natives jars).
    pub downloads: Vec<Download>,
    /// Classpath jars, in order.
    pub classpath: Vec<PathBuf>,
    /// Legacy natives jars.
    pub natives: Vec<NativeJar>,
}

fn artifact_download(artifact: &Artifact, path: String, root: &Path) -> Download {
    let mut job = Download::new(&artifact.url, root.join(&path));
    job.sha1 = artifact.sha1.clone();
    job.size = artifact.size;
    job
}

/// Decides which library files a version needs on `env`, and where they go under `libraries_dir`.
///
/// Libraries that do not apply (rules) are skipped. A library with a `natives` map is a legacy natives
/// library: only the classifier jar of the environment is used, and it stays off the classpath.
pub fn plan_libraries(version: &VersionJson, libraries_dir: &Path, env: &RuleEnv) -> LibraryPlan {
    let mut plan = LibraryPlan::default();
    for lib in version.libraries.iter().filter(|l| l.applies(env)) {
        if lib.natives.as_ref().is_some_and(|n| !n.is_empty()) {
            if let Some((job, exclude)) = native_job(lib, libraries_dir, env) {
                if !plan.natives.iter().any(|n| n.path == job.path) {
                    plan.natives.push(NativeJar { path: job.path.clone(), exclude });
                    plan.downloads.push(job);
                }
            }
            continue;
        }
        if let Some(job) = library_job(lib, libraries_dir) {
            if !plan.classpath.contains(&job.path) {
                plan.classpath.push(job.path.clone());
                plan.downloads.push(job);
            }
        }
    }
    plan
}

pub(crate) fn library_job(lib: &Library, root: &Path) -> Option<Download> {
    if let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
        let path = artifact.path.clone().or_else(|| lib.maven_path())?;
        if artifact.url.is_empty() {
            // Some libraries ship an artifact entry without URL: fall back to the Maven layout.
            let mut job = Download::new(format!("{LIBRARIES_BASE_URL}/{path}"), root.join(&path));
            job.sha1 = artifact.sha1.clone();
            return Some(job);
        }
        return Some(artifact_download(artifact, path, root));
    }
    let path = lib.maven_path()?;
    let base = lib.url.as_deref().filter(|u| !u.is_empty()).unwrap_or(LIBRARIES_BASE_URL);
    let mut job = Download::new(format!("{}/{path}", base.trim_end_matches('/')), root.join(&path));
    job.sha1 = lib.sha1.clone();
    job.size = lib.size;
    Some(job)
}

fn native_job(lib: &Library, root: &Path, env: &RuleEnv) -> Option<(Download, Vec<String>)> {
    let classifier = lib.natives_classifier(env)?;
    let artifact = lib.downloads.as_ref()?.classifiers.as_ref()?.get(&classifier)?;
    let path = artifact.path.clone().or_else(|| maven_path(&lib.name, Some(&classifier)))?;
    let exclude = lib.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default();
    Some((artifact_download(artifact, path, root), exclude))
}

fn emit(sink: &dyn EventSink, task: &str, label: &str, done: u64, total: u64) {
    sink.emit(Event::Progress { task: task.to_string(), label: label.to_string(), done, total });
}

/// Downloads everything a resolved version needs and returns what the launcher needs to start it.
///
/// Emits `Event::Progress` for `task` only; the caller emits `TaskFinished`. `game_dir` is where
/// `resources/` goes for the oldest asset indexes.
/// `concurrency`: how many files download at the same time (Settings › Downloads).
///
/// Does not handle Forge or NeoForge (it needs their installer and a Java): use [`install_loader`] for those.
pub async fn install(http: &Http, paths: &Paths, version: &VersionJson, game_dir: &Path, concurrency: usize, sink: &dyn EventSink, task: &str) -> Result<InstalledVersion> {
    install_loader(http, paths, version, &LoaderSpec::Vanilla, None, game_dir, concurrency, sink, task).await
}

/// Like [`install`], for a version resolved with `loader`.
///
/// Forge and NeoForge also get their installer's own libraries and its client-side processors run
/// (binary patching of the client...), which needs `java`, a path to a Java the installer supports (the one the
/// game will use is right). Without it Forge versions that need processors fail with a Java error; legacy Forge
/// (1.12.2 and older) and the other loaders don't use it. Once done, installing again is a quick check.
#[allow(clippy::too_many_arguments)]
pub async fn install_loader(
    http: &Http,
    paths: &Paths,
    version: &VersionJson,
    loader: &LoaderSpec,
    java: Option<&Path>,
    game_dir: &Path,
    concurrency: usize,
    sink: &dyn EventSink,
    task: &str,
) -> Result<InstalledVersion> {
    emit(sink, task, "Checking the version", 0, 1);
    let mc = version.inherits_from.clone().unwrap_or_else(|| version.id.clone());
    let env = RuleEnv::current();

    // Client jar.
    let client = version
        .downloads
        .as_ref()
        .and_then(|d| d.client.as_ref())
        .ok_or_else(|| Error::NotFound(format!("the client download of {mc}")))?;
    let client_jar = paths.versions().join(&mc).join(format!("{mc}.jar"));
    let mut job = Download::new(&client.url, &client_jar);
    job.sha1 = client.sha1.clone();
    job.size = client.size;
    emit(sink, task, "Downloading Minecraft", 0, 1);
    ensure_file(http, &job).await?;
    emit(sink, task, "Downloading Minecraft", 1, 1);

    // Forge / NeoForge: the installer brings some libraries and the data its processors need.
    let forge = match loader.installer_loader() {
        Some((kind, loader_version)) => {
            emit(sink, task, "Preparing the loader", 0, 1);
            let forge = Forge::open(http, paths, kind, &mc, loader_version).await?;
            forge.extract(paths).await?;
            Some(forge)
        }
        None => None,
    };

    // Libraries.
    let plan = plan_libraries(version, &paths.libraries(), &env);
    let mut downloads = plan.downloads.clone();
    let mut produced = HashSet::new();
    if let Some(forge) = &forge {
        downloads.extend(forge.profile_downloads(paths));
        // Libraries without a URL come out of the installer or of its processors, not from the network.
        produced = forge.local_only(version, paths);
        downloads.retain(|d| !produced.contains(&d.path) || d.path.exists());
    }
    download_all(http, downloads, concurrency, |done, total| emit(sink, task, "Downloading libraries", done, total)).await?;
    if let Some(forge) = &forge {
        let label = if forge.kind == LoaderKind::Forge { "Installing Forge" } else { "Installing NeoForge" };
        forge.run_processors(paths, &client_jar, java, |done, total| emit(sink, task, label, done, total)).await?;
        if let Some(missing) = produced.iter().find(|p| !p.exists()) {
            return Err(Error::NotFound(format!("{}, which the loader installer should have created", missing.display())));
        }
    }

    // Asset index.
    let index_ref = version.asset_index.as_ref().ok_or_else(|| Error::NotFound(format!("the asset index of {}", version.id)))?;
    let index_path = paths.assets().join("indexes").join(format!("{}.json", index_ref.id));
    let mut job = Download::new(&index_ref.url, &index_path).with_sha1(&index_ref.sha1);
    job.size = Some(index_ref.size);
    emit(sink, task, "Downloading asset index", 0, 1);
    ensure_file(http, &job).await?;
    let bytes = tokio::fs::read(&index_path).await.map_err(|e| Error::io(&index_path, e))?;
    let index: AssetIndex = serde_json::from_slice(&bytes).map_err(|e| Error::json(format!("asset index {}", index_ref.id), e))?;

    // Asset objects.
    let objects_dir = paths.assets().join("objects");
    let jobs: Vec<Download> = index
        .objects
        .values()
        .map(|o| {
            let rel = o.relative_path();
            let mut job = Download::new(format!("{ASSETS_BASE_URL}/{rel}"), objects_dir.join(&rel)).with_sha1(&o.hash);
            job.size = Some(o.size);
            job
        })
        .collect();
    download_all(http, jobs, concurrency, |done, total| emit(sink, task, "Downloading assets", done, total)).await?;

    // Legacy layouts.
    let mut game_assets_dir = paths.assets();
    let copy_target = if index.is_virtual {
        Some(paths.assets().join("virtual").join(&index_ref.id))
    } else if index.map_to_resources {
        Some(game_dir.join("resources"))
    } else {
        None
    };
    if let Some(target) = copy_target {
        emit(sink, task, "Copying legacy assets", 0, 1);
        copy_assets(&index, &objects_dir, &target).await?;
        game_assets_dir = target;
    }

    // Log4j configuration.
    let mut logging_argument = None;
    if let Some(config) = version.logging.as_ref().and_then(|l| l.client.as_ref()) {
        let path = paths.assets().join("log_configs").join(&config.file.id);
        let mut job = Download::new(&config.file.url, &path).with_sha1(&config.file.sha1);
        job.size = Some(config.file.size);
        emit(sink, task, "Downloading log settings", 0, 1);
        ensure_file(http, &job).await?;
        logging_argument = Some(config.argument.replace("${path}", &path.to_string_lossy()));
    }

    emit(sink, task, "Done", 1, 1);
    // Forge and NeoForge's `-DignoreList` names `${version_name}.jar`: the jar on the classpath must carry the
    // merged version's id, like the official launcher does.
    let client_jar = match &forge {
        Some(_) if version.id != mc => link_client_jar(paths, &version.id, &client_jar).await?,
        _ => client_jar,
    };

    Ok(InstalledVersion {
        version: version.clone(),
        client_jar,
        libraries: plan.classpath,
        natives: plan.natives,
        assets_root: paths.assets(),
        assets_index_id: index_ref.id.clone(),
        game_assets_dir,
        logging_argument,
    })
}

/// Makes `versions/<id>/<id>.jar` a copy (hard link when possible) of the vanilla client jar and returns its path.
async fn link_client_jar(paths: &Paths, id: &str, vanilla: &Path) -> Result<PathBuf> {
    let dest = paths.versions().join(id).join(format!("{id}.jar"));
    let (source, target) = (vanilla.to_path_buf(), dest.clone());
    tokio::task::spawn_blocking(move || {
        let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).ok();
        if size(&target).is_some() && size(&target) == size(&source) {
            return Ok(());
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        let _ = std::fs::remove_file(&target);
        if std::fs::hard_link(&source, &target).is_err() {
            std::fs::copy(&source, &target).map_err(|e| Error::io(&target, e))?;
        }
        Ok(())
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))??;
    Ok(dest)
}

/// Copies every object of the index to `<target>/<asset name>` (skipping files already up to date).
async fn copy_assets(index: &AssetIndex, objects_dir: &Path, target: &Path) -> Result<()> {
    let entries: Vec<(String, u64, PathBuf)> = index.objects.iter().map(|(name, o)| (name.clone(), o.size, objects_dir.join(o.relative_path()))).collect();
    let target = target.to_path_buf();
    tokio::task::spawn_blocking(move || {
        for (name, size, source) in entries {
            let dest = target.join(&name);
            if std::fs::metadata(&dest).is_ok_and(|m| m.len() == size) {
                continue;
            }
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
            }
            std::fs::copy(&source, &dest).map_err(|e| Error::io(&dest, e))?;
        }
        Ok(())
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_spec_serde() {
        let json = serde_json::to_string(&LoaderSpec::Fabric("0.20.1".into())).unwrap();
        assert_eq!(json, r#"{"kind":"fabric","version":"0.20.1"}"#);
        assert_eq!(serde_json::from_str::<LoaderSpec>(&json).unwrap(), LoaderSpec::Fabric("0.20.1".into()));
        let v = serde_json::to_string(&LoaderSpec::Vanilla).unwrap();
        assert_eq!(v, r#"{"kind":"vanilla"}"#);
        assert_eq!(serde_json::from_str::<LoaderSpec>(&v).unwrap(), LoaderSpec::Vanilla);
        for (spec, json) in [(LoaderSpec::Forge("47.2.0".into()), r#"{"kind":"forge","version":"47.2.0"}"#), (LoaderSpec::NeoForge("21.1.172".into()), r#"{"kind":"neoforge","version":"21.1.172"}"#)] {
            assert_eq!(serde_json::to_string(&spec).unwrap(), json);
            assert_eq!(serde_json::from_str::<LoaderSpec>(json).unwrap(), spec);
        }
    }
}

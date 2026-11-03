//! Installing a [`PackPlan`] into a game directory, whatever provider made it, and the files the player has
//! to download by hand. No Tauri in here: the commands (`packs.rs`) call it, and the tests run it as is.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use futures::StreamExt;
use serde::{Deserialize, Serialize};
use cubby_common::events::{Event, EventSink};
use cubby_common::net::{ensure_file, Download, Http};
use cubby_common::packs::{safe_relative_path, BlockedFile, PackFile, PackLoaderKind, PackOverrides, PackPlan};
use cubby_common::{Error, Result};
use cubby_modrinth::{MrpackIndex, PackLoader};

/// Name of the file, in the instance folder (next to `instance.toml`), that lists the files still to be
/// downloaded by hand.
pub const BLOCKED_FILE: &str = "blocked-files.json";

fn progress(sink: &dyn EventSink, task: &str, label: &str, done: u64, total: u64) {
    sink.emit(Event::Progress { task: task.to_string(), label: label.to_string(), done, total });
}

// Installing a plan

/// What [`install_pack_plan`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed {
    /// Files downloaded (or already there).
    pub files: usize,
    /// Files copied out of the overrides archives.
    pub overrides: u64,
    /// Files of the plan the player has to download by hand, whether they are in place or not.
    pub blocked: Vec<BlockedEntry>,
}

/// Downloads the `files` of the plan (`concurrency` at a time, SHA-1 checked when the plan has one, each
/// file's other URLs tried when the first fails), then extracts the `overrides` in order, so later ones
/// win. Progress goes out on `task` like the Modrinth modpack install ("Downloading modpack files",
/// "Copying modpack files"); the caller emits the final `TaskFinished`.
///
/// `remove_archives`: delete the overrides archives afterwards, on success and on failure. Only for plans
/// that own them (a provider's cached downloads); never for a file the player picked.
pub async fn install_pack_plan(http: &Http, plan: &PackPlan, game_dir: &Path, concurrency: usize, remove_archives: bool, sink: &dyn EventSink, task: &str) -> Result<Installed> {
    let result = install_inner(http, plan, game_dir, concurrency, sink, task).await;
    if remove_archives {
        remove_plan_archives(plan);
    }
    result
}

async fn install_inner(http: &Http, plan: &PackPlan, game_dir: &Path, concurrency: usize, sink: &dyn EventSink, task: &str) -> Result<Installed> {
    fs::create_dir_all(game_dir).map_err(|e| Error::io(game_dir, e))?;

    // Files, deduplicated by path (the first one wins, like `download_all`).
    let mut seen = HashSet::new();
    let mut files: Vec<(PathBuf, PackFile)> = Vec::new();
    for file in &plan.files {
        let rel = safe_relative_path(&file.path.to_string_lossy()).ok_or_else(|| Error::Other(format!("The modpack contains an unsafe path: {:?}.", file.path)))?;
        if seen.insert(rel.clone()) {
            files.push((game_dir.join(rel), file.clone()));
        }
    }
    let total = files.len() as u64;
    progress(sink, task, "Downloading modpack files", 0, total);
    let mut done = 0u64;
    let mut first_error = None;
    let mut results = futures::stream::iter(files.into_iter().map(|(dest, file)| {
        let http = http.clone();
        async move { download_file(&http, &file, dest).await }
    }))
    .buffer_unordered(concurrency.max(1));
    while let Some(result) = results.next().await {
        if let Err(error) = result {
            tracing_warn(&error);
            first_error.get_or_insert(error);
        }
        done += 1;
        progress(sink, task, "Downloading modpack files", done, total);
    }
    drop(results);
    if let Some(error) = first_error {
        return Err(error);
    }

    // Overrides, in order.
    let mut written = 0;
    for overrides in &plan.overrides {
        let (archive, prefix, dir) = (overrides.archive.clone(), overrides.prefix.clone(), game_dir.to_path_buf());
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(u64, u64)>();
        let extraction = tokio::task::spawn_blocking(move || {
            extract_overrides(&archive, &prefix, &dir, |done, total| {
                let _ = tx.send((done, total));
            })
        });
        while let Some((done, total)) = rx.recv().await {
            progress(sink, task, "Copying modpack files", done, total);
        }
        written += extraction.await.map_err(|e| Error::Other(e.to_string()))??;
    }

    let blocked = plan.blocked.iter().filter_map(BlockedEntry::from_plan).collect();
    progress(sink, task, "Done", 1, 1);
    Ok(Installed { files: seen.len(), overrides: written, blocked })
}

fn tracing_warn(error: &Error) {
    tracing::warn!("modpack download failed: {error}");
}

/// Tries each URL of the file in turn.
async fn download_file(http: &Http, file: &PackFile, dest: PathBuf) -> Result<()> {
    let mut last = None;
    for url in &file.urls {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            last = Some(Error::Other(format!("The modpack points to {url}, which isn't a web address.")));
            continue;
        }
        let mut job = Download::new(url.clone(), dest.clone());
        if let Some(sha1) = &file.sha1 {
            job = job.with_sha1(sha1.to_lowercase());
        }
        if let Some(size) = file.size {
            job = job.with_size(size);
        }
        match ensure_file(http, &job).await {
            Ok(()) => return Ok(()),
            Err(error) => last = Some(error),
        }
    }
    Err(last.unwrap_or_else(|| Error::Other(format!("The modpack has no download for {}.", file.path.display()))))
}

/// Deletes the archives the plan owns (and their folder when that leaves it empty). Errors are logged only.
pub fn remove_plan_archives(plan: &PackPlan) {
    let archives: HashSet<&PathBuf> = plan.overrides.iter().map(|o| &o.archive).collect();
    for archive in archives {
        if let Err(e) = fs::remove_file(archive) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!("could not remove {}: {e}", archive.display());
            }
        }
        if let Some(parent) = archive.parent() {
            let _ = fs::remove_dir(parent); // only succeeds when empty
        }
    }
}

/// Copies the entries of the zip under `prefix` ("overrides/"; empty for the whole archive) into `game_dir`,
/// replacing existing files. Every entry is checked with [`safe_relative_path`] before anything is
/// written: one path that is absolute, goes up (`..`) or names a drive rejects the whole archive.
/// `on_progress(done, total)` counts files. Returns the number of files written.
pub fn extract_overrides(archive: &Path, prefix: &str, game_dir: &Path, mut on_progress: impl FnMut(u64, u64)) -> Result<u64> {
    let file = fs::File::open(archive).map_err(|e| Error::io(archive, e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| Error::Other(format!("{} isn't a valid zip file ({e}).", archive.display())))?;
    let mut prefix = prefix.replace('\\', "/");
    while let Some(rest) = prefix.strip_prefix("./") {
        prefix = rest.to_string();
    }
    if !prefix.is_empty() && !prefix.ends_with('/') {
        prefix.push('/');
    }

    let mut entries: Vec<(usize, PathBuf, bool)> = Vec::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| Error::Other(format!("Couldn't read {} ({e}).", archive.display())))?;
        let name = entry.name().replace('\\', "/");
        let Some(rel) = name.strip_prefix(prefix.as_str()) else { continue };
        if rel.trim_matches('/').is_empty() {
            continue;
        }
        let path = safe_relative_path(rel).ok_or_else(|| Error::Other(format!("The modpack contains an unsafe path: {:?}.", entry.name())))?;
        entries.push((i, path, entry.is_dir()));
    }

    let total = entries.iter().filter(|e| !e.2).count() as u64;
    let mut done = 0;
    on_progress(done, total);
    for (i, rel, is_dir) in entries {
        let dest = game_dir.join(&rel);
        if is_dir {
            fs::create_dir_all(&dest).map_err(|e| Error::io(&dest, e))?;
            continue;
        }
        let parent = dest.parent().unwrap_or(game_dir);
        fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        let mut tmp_name = dest.file_name().unwrap_or_default().to_os_string();
        tmp_name.push(".cubby-tmp");
        let tmp = parent.join(tmp_name);
        let mut entry = zip.by_index(i).map_err(|e| Error::Other(format!("Couldn't read {} ({e}).", archive.display())))?;
        let copied = (|| -> std::io::Result<()> {
            let mut out = fs::File::create(&tmp)?;
            std::io::copy(&mut entry, &mut out)?;
            out.flush()
        })();
        if let Err(e) = copied {
            let _ = fs::remove_file(&tmp);
            return Err(Error::io(&dest, e));
        }
        if let Err(e) = fs::rename(&tmp, &dest) {
            let _ = fs::remove_file(&tmp);
            return Err(Error::io(&dest, e));
        }
        done += 1;
        on_progress(done, total);
    }
    Ok(done)
}

// Local files

/// The two kinds of modpack file the player can import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackFormat {
    /// `.mrpack`: `modrinth.index.json`.
    Mrpack,
    /// CurseForge `.zip`: `manifest.json`.
    Curseforge,
}

/// Which format a zip is, from the files at its root (not from its extension: Modrinth packs get renamed).
pub fn detect_format(archive: &Path) -> Result<PackFormat> {
    let meta = fs::metadata(archive).map_err(|e| Error::io(archive, e))?;
    if !meta.is_file() {
        return Err(Error::Other(format!("{} isn't a file.", archive.display())));
    }
    let file = fs::File::open(archive).map_err(|e| Error::io(archive, e))?;
    let zip = zip::ZipArchive::new(file).map_err(|_| Error::Other("This file isn't a modpack: it isn't a valid zip archive.".into()))?;
    if zip.file_names().any(|n| n == cubby_modrinth::mrpack::INDEX_FILE) {
        Ok(PackFormat::Mrpack)
    } else if zip.file_names().any(|n| n.eq_ignore_ascii_case("manifest.json")) {
        Ok(PackFormat::Curseforge)
    } else {
        Err(Error::Other("This file isn't a Modrinth (.mrpack) or CurseForge modpack: it has neither modrinth.index.json nor manifest.json.".into()))
    }
}

/// Maps the Modrinth loader of a pack to the shared one.
pub fn loader_kind(loader: PackLoader) -> PackLoaderKind {
    match loader {
        PackLoader::Fabric => PackLoaderKind::Fabric,
        PackLoader::Quilt => PackLoaderKind::Quilt,
        PackLoader::Forge => PackLoaderKind::Forge,
        PackLoader::NeoForge => PackLoaderKind::NeoForge,
    }
}

/// Plans an `.mrpack` the player has: the plan's overrides point to `archive` itself, so it must not be
/// deleted after the install.
pub fn plan_from_mrpack(archive: &Path) -> Result<PackPlan> {
    let index = MrpackIndex::read(archive)?;
    let minecraft = index.minecraft().ok_or_else(|| Error::Other("The modpack doesn't say which Minecraft version it needs.".into()))?.to_string();
    let files = index
        .client_downloads()?
        .into_iter()
        .map(|d| PackFile { path: d.path, urls: d.urls, sha1: Some(d.sha1), size: d.size })
        .collect();
    let title = if index.name.trim().is_empty() { "Modrinth modpack".to_string() } else { index.name.trim().to_string() };
    let overrides = cubby_modrinth::mrpack::OVERRIDE_FOLDERS
        .iter()
        .map(|prefix| PackOverrides { archive: archive.to_path_buf(), prefix: (*prefix).to_string() })
        .collect();
    Ok(PackPlan {
        provider: "modrinth".into(),
        pack_id: String::new(),
        version_id: index.version_id.clone(),
        title,
        version_name: index.version_id.clone(),
        minecraft,
        loader: index.loader().map(|(kind, version)| (loader_kind(kind), version)),
        icon_url: None,
        files,
        overrides,
        blocked: Vec::new(),
        recommended_memory_mib: None,
    })
}

// Blocked files

/// A file the provider doesn't let launchers download, as saved in [`BLOCKED_FILE`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedEntry {
    pub file_name: String,
    pub page_url: String,
    /// Relative to the game directory, `/`-separated.
    pub path: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

impl BlockedEntry {
    fn from_plan(file: &BlockedFile) -> Option<Self> {
        let path = safe_relative_path(&file.path.to_string_lossy())?;
        Some(Self {
            file_name: file.file_name.clone(),
            page_url: file.page_url.clone(),
            path: path.to_string_lossy().replace('\\', "/"),
            sha1: file.sha1.clone(),
            size: file.size,
        })
    }

    /// Where the file goes, or `None` when the saved path isn't safe.
    fn target(&self, game_dir: &Path) -> Option<PathBuf> {
        safe_relative_path(&self.path).map(|rel| game_dir.join(rel))
    }

    pub fn found(&self, game_dir: &Path) -> bool {
        self.target(game_dir).is_some_and(|p| p.is_file())
    }
}

/// Writes the list next to `instance.toml`; an empty list removes the file.
pub fn save_blocked(instance_dir: &Path, entries: &[BlockedEntry]) -> std::io::Result<()> {
    let file = instance_dir.join(BLOCKED_FILE);
    if entries.is_empty() {
        return match fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let json = serde_json::to_vec_pretty(entries).map_err(std::io::Error::other)?;
    let tmp = instance_dir.join(format!("{BLOCKED_FILE}.tmp"));
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &file)
}

/// The saved list; missing or unreadable means empty. Entries with an unsafe path are dropped.
pub fn load_blocked(instance_dir: &Path) -> Vec<BlockedEntry> {
    let Ok(bytes) = fs::read(instance_dir.join(BLOCKED_FILE)) else { return Vec::new() };
    let entries: Vec<BlockedEntry> = serde_json::from_slice(&bytes).unwrap_or_default();
    entries.into_iter().filter(|e| safe_relative_path(&e.path).is_some()).collect()
}

/// How many saved files are still missing from the game directory.
pub fn count_missing(instance_dir: &Path, game_dir: &Path) -> u32 {
    load_blocked(instance_dir).iter().filter(|e| !e.found(game_dir)).count() as u32
}

/// Looks in `folder` (the player's Downloads) for each missing file and copies the matches into place.
/// A file matches by name (case-insensitive, also "name (1).ext" as browsers number duplicates); when the
/// plan knows a SHA-1 the content must match too, and a file the browser renamed to something else is
/// still found by SHA-1 among the files of the same size. Copies, never moves. Returns how many were copied.
pub fn find_in_folder(folder: &Path, game_dir: &Path, entries: &[BlockedEntry]) -> usize {
    let Ok(read) = fs::read_dir(folder) else { return 0 };
    let candidates: Vec<(String, PathBuf, u64)> = read
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            meta.is_file().then(|| (e.file_name().to_string_lossy().to_lowercase(), e.path(), meta.len()))
        })
        .collect();
    let mut copied = 0;
    for entry in entries {
        let Some(target) = entry.target(game_dir) else { continue };
        if target.is_file() {
            continue;
        }
        let wanted = entry.file_name.to_lowercase();
        let mut named: Vec<&(String, PathBuf, u64)> = candidates.iter().filter(|(name, ..)| name_matches(name, &wanted)).collect();
        named.sort_by_key(|(name, ..)| *name != wanted); // exact name first
        let mut found = named.into_iter().find(|(_, path, _)| content_ok(path, entry.sha1.as_deref()));
        if found.is_none() {
            if let (Some(sha1), Some(size)) = (entry.sha1.as_deref(), entry.size) {
                found = candidates.iter().find(|(_, path, len)| *len == size && content_ok(path, Some(sha1)));
            }
        }
        if let Some((_, source, _)) = found {
            match copy_into_place(source, &target) {
                Ok(()) => copied += 1,
                Err(e) => tracing::warn!("could not copy {} to {}: {e}", source.display(), target.display()),
            }
        }
    }
    copied
}

/// `name` is `wanted` or `wanted` with a " (n)" before its extension.
fn name_matches(name: &str, wanted: &str) -> bool {
    if name == wanted {
        return true;
    }
    let (stem, ext) = match wanted.rsplit_once('.') {
        Some((stem, ext)) => (stem, format!(".{ext}")),
        None => (wanted, String::new()),
    };
    let Some(middle) = name.strip_prefix(stem).and_then(|rest| rest.strip_suffix(ext.as_str())) else { return false };
    middle.strip_prefix(" (").and_then(|m| m.strip_suffix(')')).is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn content_ok(path: &Path, sha1: Option<&str>) -> bool {
    match sha1 {
        None => true,
        Some(expected) => cubby_common::net::sha1_file_blocking(path).ok().is_some_and(|actual| actual.eq_ignore_ascii_case(expected)),
    }
}

fn copy_into_place(source: &Path, target: &Path) -> std::io::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp_name = target.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".cubby-tmp");
    let tmp = target.with_file_name(tmp_name);
    let result = fs::copy(source, &tmp).and_then(|_| fs::rename(&tmp, target));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubby_common::events::NoopSink;

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            if name.ends_with('/') {
                zip.add_directory(*name, options).unwrap();
            } else {
                zip.start_file(*name, options).unwrap();
                zip.write_all(data).unwrap();
            }
        }
        zip.finish().unwrap();
    }

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn extracts_under_prefix_only() {
        let dir = tmp();
        let zip = dir.path().join("p.zip");
        make_zip(&zip, &[("manifest.json", b"{}"), ("overrides/", b""), ("overrides/config/a.toml", b"a"), ("overrides/options.txt", b"o"), ("other/x.txt", b"x")]);
        let game = dir.path().join("game");
        let mut last = (0, 0);
        let n = extract_overrides(&zip, "overrides/", &game, |d, t| last = (d, t)).unwrap();
        assert_eq!(n, 2);
        assert_eq!(last, (2, 2));
        assert_eq!(fs::read(game.join("config/a.toml")).unwrap(), b"a");
        assert_eq!(fs::read(game.join("options.txt")).unwrap(), b"o");
        assert!(!game.join("manifest.json").exists() && !game.join("other").exists() && !game.join("x.txt").exists());
    }

    #[test]
    fn prefix_without_trailing_slash_and_empty_prefix() {
        let dir = tmp();
        let zip = dir.path().join("p.zip");
        make_zip(&zip, &[("overrides/a.txt", b"1"), ("mods/b.jar", b"2")]);
        let game = dir.path().join("game");
        assert_eq!(extract_overrides(&zip, "overrides", &game, |_, _| {}).unwrap(), 1);
        assert!(game.join("a.txt").is_file());
        // The whole archive.
        let all = dir.path().join("all");
        assert_eq!(extract_overrides(&zip, "", &all, |_, _| {}).unwrap(), 2);
        assert!(all.join("overrides/a.txt").is_file() && all.join("mods/b.jar").is_file());
    }

    #[test]
    fn later_overrides_replace_earlier_files() {
        let dir = tmp();
        let (one, two) = (dir.path().join("1.zip"), dir.path().join("2.zip"));
        make_zip(&one, &[("overrides/a.txt", b"first")]);
        make_zip(&two, &[("overrides/a.txt", b"second")]);
        let game = dir.path().join("game");
        extract_overrides(&one, "overrides/", &game, |_, _| {}).unwrap();
        extract_overrides(&two, "overrides/", &game, |_, _| {}).unwrap();
        assert_eq!(fs::read(game.join("a.txt")).unwrap(), b"second");
        assert!(!game.join("a.txt.cubby-tmp").exists());
    }

    #[test]
    fn zip_slip_is_rejected_before_anything_is_written() {
        let dir = tmp();
        for bad in ["overrides/../../evil.txt", "overrides/..\\evil.txt", "overrides//etc/passwd", "overrides/C:/evil.txt"] {
            let zip = dir.path().join("evil.zip");
            // `good.txt` comes first: it must not be written either.
            make_zip(&zip, &[("overrides/good.txt", b"g"), (bad, b"x")]);
            let game = dir.path().join("game");
            let error = extract_overrides(&zip, "overrides/", &game, |_, _| {}).unwrap_err().to_string();
            assert!(error.contains("unsafe path"), "{bad}: {error}");
            assert!(!game.join("good.txt").exists(), "{bad}: wrote before checking");
            assert!(!dir.path().join("evil.txt").exists());
        }
        // An unsafe path outside the prefix is not ours to judge.
        let zip = dir.path().join("fine.zip");
        make_zip(&zip, &[("overrides/ok.txt", b"1"), ("../outside.txt", b"x")]);
        assert_eq!(extract_overrides(&zip, "overrides/", &dir.path().join("g2"), |_, _| {}).unwrap(), 1);
    }

    #[test]
    fn not_a_zip_is_a_clear_error() {
        let dir = tmp();
        let file = dir.path().join("x.zip");
        fs::write(&file, b"nope").unwrap();
        assert!(extract_overrides(&file, "", dir.path(), |_, _| {}).unwrap_err().to_string().contains("isn't a valid zip"));
        assert!(detect_format(&file).unwrap_err().to_string().contains("isn't a valid zip"));
    }

    #[test]
    fn detects_formats_by_content() {
        let dir = tmp();
        let (m, c, n) = (dir.path().join("a.zip"), dir.path().join("b.mrpack"), dir.path().join("c.zip"));
        make_zip(&m, &[("modrinth.index.json", b"{}")]);
        make_zip(&c, &[("manifest.json", b"{}")]);
        make_zip(&n, &[("readme.txt", b"hi")]);
        assert_eq!(detect_format(&m).unwrap(), PackFormat::Mrpack);
        assert_eq!(detect_format(&c).unwrap(), PackFormat::Curseforge);
        assert!(detect_format(&n).unwrap_err().to_string().contains("neither"));
    }

    #[test]
    fn mrpack_plan_uses_the_players_file_for_overrides() {
        let dir = tmp();
        let zip = dir.path().join("p.mrpack");
        let index = br#"{"formatVersion":1,"game":"minecraft","versionId":"1.2.3","name":"My Pack","files":[
            {"path":"mods/a.jar","hashes":{"sha1":"0123456789abcdef0123456789abcdef01234567"},"downloads":["https://cdn.example/a.jar"],"fileSize":3},
            {"path":"mods/server.jar","hashes":{"sha1":"0123456789abcdef0123456789abcdef01234567"},"env":{"client":"unsupported"},"downloads":["https://cdn.example/s.jar"]}],
            "dependencies":{"minecraft":"1.20.1","fabric-loader":"0.15.0"}}"#;
        make_zip(&zip, &[("modrinth.index.json", index), ("overrides/options.txt", b"o")]);
        let plan = plan_from_mrpack(&zip).unwrap();
        assert_eq!((plan.title.as_str(), plan.minecraft.as_str()), ("My Pack", "1.20.1"));
        assert_eq!(plan.loader, Some((PackLoaderKind::Fabric, "0.15.0".to_string())));
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].path, PathBuf::from("mods").join("a.jar"));
        assert!(plan.overrides.iter().all(|o| o.archive == zip));
        assert_eq!(plan.overrides.iter().map(|o| o.prefix.as_str()).collect::<Vec<_>>(), ["overrides/", "client-overrides/"]);
    }

    fn plan_with(files: Vec<PackFile>, overrides: Vec<PackOverrides>) -> PackPlan {
        PackPlan {
            provider: "test".into(),
            pack_id: "1".into(),
            version_id: "1".into(),
            title: "T".into(),
            version_name: "1".into(),
            minecraft: "1.20.1".into(),
            loader: None,
            icon_url: None,
            files,
            overrides,
            blocked: vec![BlockedFile { file_name: "x.jar".into(), page_url: "https://example/x".into(), path: PathBuf::from("mods").join("x.jar"), sha1: None, size: None }],
            recommended_memory_mib: None,
        }
    }

    #[tokio::test]
    async fn install_plan_without_downloads_extracts_in_order_and_keeps_players_archive() {
        let dir = tmp();
        let (one, two) = (dir.path().join("cache-one.zip"), dir.path().join("mine.zip"));
        make_zip(&one, &[("base/a.txt", b"first"), ("base/b.txt", b"b")]);
        make_zip(&two, &[("overrides/a.txt", b"second")]);
        let plan = plan_with(
            vec![],
            vec![PackOverrides { archive: one.clone(), prefix: "base/".into() }, PackOverrides { archive: two.clone(), prefix: "overrides/".into() }],
        );
        let game = dir.path().join("game");
        let http = Http::new().unwrap();
        let out = install_pack_plan(&http, &plan, &game, 4, false, &NoopSink, "t").await.unwrap();
        assert_eq!((out.files, out.overrides), (0, 3));
        assert_eq!(fs::read(game.join("a.txt")).unwrap(), b"second");
        assert_eq!(out.blocked.len(), 1);
        assert_eq!(out.blocked[0].path, "mods/x.jar");
        assert!(one.exists() && two.exists(), "the archives belong to the caller when remove_archives is false");

        // With remove_archives the archives go, on success...
        install_pack_plan(&http, &plan, &game, 4, true, &NoopSink, "t").await.unwrap();
        assert!(!one.exists() && !two.exists());
    }

    #[tokio::test]
    async fn archives_are_removed_even_when_the_install_fails() {
        let dir = tmp();
        let bad = dir.path().join("bad.zip");
        make_zip(&bad, &[("overrides/../x", b"1")]);
        let plan = plan_with(vec![], vec![PackOverrides { archive: bad.clone(), prefix: "overrides/".into() }]);
        let http = Http::new().unwrap();
        assert!(install_pack_plan(&http, &plan, &dir.path().join("g"), 4, true, &NoopSink, "t").await.is_err());
        assert!(!bad.exists());
    }

    #[tokio::test]
    async fn unsafe_file_paths_fail_before_any_download() {
        let dir = tmp();
        let plan = plan_with(vec![PackFile { path: PathBuf::from("..").join("evil.jar"), urls: vec!["https://127.0.0.1:1/x".into()], sha1: None, size: None }], vec![]);
        let http = Http::new().unwrap();
        let error = install_pack_plan(&http, &plan, &dir.path().join("g"), 4, false, &NoopSink, "t").await.unwrap_err().to_string();
        assert!(error.contains("unsafe path"), "{error}");
    }

    #[test]
    fn blocked_list_round_trip_and_count() {
        let dir = tmp();
        let (inst, game) = (dir.path().to_path_buf(), dir.path().join("minecraft"));
        fs::create_dir_all(&game).unwrap();
        let entries = vec![
            BlockedEntry { file_name: "a.jar".into(), page_url: "https://x/a".into(), path: "mods/a.jar".into(), sha1: None, size: None },
            BlockedEntry { file_name: "b.jar".into(), page_url: "https://x/b".into(), path: "mods/b.jar".into(), sha1: None, size: None },
            BlockedEntry { file_name: "evil".into(), page_url: String::new(), path: "../evil".into(), sha1: None, size: None },
        ];
        save_blocked(&inst, &entries).unwrap();
        assert_eq!(load_blocked(&inst).len(), 2, "the unsafe path is dropped");
        assert_eq!(count_missing(&inst, &game), 2);
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::write(game.join("mods/a.jar"), b"a").unwrap();
        assert_eq!(count_missing(&inst, &game), 1);
        save_blocked(&inst, &[]).unwrap();
        assert!(!inst.join(BLOCKED_FILE).exists());
        assert_eq!(count_missing(&inst, &game), 0);
    }

    #[test]
    fn finds_blocked_files_in_downloads_by_name_and_hash() {
        use sha1::{Digest, Sha1};
        let dir = tmp();
        let (downloads, game) = (dir.path().join("Downloads"), dir.path().join("minecraft"));
        fs::create_dir_all(&downloads).unwrap();
        let sha = |data: &[u8]| hex::encode(Sha1::digest(data));
        fs::write(downloads.join("Exact.jar"), b"exact").unwrap();
        fs::write(downloads.join("twice (1).jar"), b"twice").unwrap();
        fs::write(downloads.join("wrongcontent.jar"), b"not what the pack wants").unwrap();
        fs::write(downloads.join("renamed-by-browser.bin"), b"renamed").unwrap();
        fs::write(downloads.join("unrelated.jar"), b"zzz").unwrap();
        let entry = |name: &str, data: Option<&[u8]>| BlockedEntry {
            file_name: name.into(),
            page_url: "https://x".into(),
            path: format!("mods/{name}"),
            sha1: data.map(sha),
            size: data.map(|d| d.len() as u64),
        };
        let entries = vec![
            entry("exact.jar", None),                       // case-insensitive name, no hash known
            entry("twice.jar", Some(b"twice")),             // " (1)" duplicate, hash verified
            entry("wrongcontent.jar", Some(b"expected")),   // name matches but content doesn't
            entry("renamed.jar", Some(b"renamed")),         // found by size + hash
            entry("missing.jar", None),
        ];
        assert_eq!(find_in_folder(&downloads, &game, &entries), 3);
        assert_eq!(fs::read(game.join("mods/exact.jar")).unwrap(), b"exact");
        assert_eq!(fs::read(game.join("mods/twice.jar")).unwrap(), b"twice");
        assert_eq!(fs::read(game.join("mods/renamed.jar")).unwrap(), b"renamed");
        assert!(!game.join("mods/wrongcontent.jar").exists() && !game.join("mods/missing.jar").exists());
        // Copied, not moved.
        assert!(downloads.join("Exact.jar").exists());
        // Idempotent: found files are left alone.
        assert_eq!(find_in_folder(&downloads, &game, &entries), 0);
        assert_eq!(find_in_folder(&dir.path().join("no such folder"), &game, &entries), 0);
    }

    #[test]
    fn duplicate_name_pattern() {
        assert!(name_matches("a (2).jar", "a.jar"));
        assert!(name_matches("a.jar", "a.jar"));
        assert!(!name_matches("a (x).jar", "a.jar"));
        assert!(!name_matches("a () .jar", "a.jar"));
        assert!(!name_matches("ab.jar", "a.jar"));
        assert!(name_matches("noext (1)", "noext"));
    }
}

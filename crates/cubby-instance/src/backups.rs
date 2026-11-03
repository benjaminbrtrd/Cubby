//! World backups and world deletion.
//!
//! Backups are `<instance>/backups/<world folder>-<YYYYMMDD-HHMMSS>.zip` (local time), with every entry
//! under `<world folder>/` like the game's own "Make Backup". Restoring also accepts flat archives.

use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone, Timelike, Utc};
use serde::Serialize;
use cubby_common::{Error, Result};
use zip::write::SimpleFileOptions;

use crate::store::InstanceStore;
use crate::util::{check_plain_name, move_to_trash, mtime};

const STAMP_FORMAT: &str = "%Y%m%d-%H%M%S";
/// Length of `YYYYMMDD-HHMMSS`.
const STAMP_LEN: usize = 15;
/// Never archived: the game keeps it locked while the world is open, and it is recreated on load.
const SKIPPED_FILES: [&str; 1] = ["session.lock"];

/// A world backup archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldBackup {
    /// File name in `<instance>/backups/`.
    pub file_name: String,
    /// World folder (in `saves/`) the backup was made from.
    pub world: String,
    pub created: DateTime<Utc>,
    pub size: u64,
}

/// The local-time stamp used in backup names, `YYYYMMDD-HHMMSS`.
pub fn backup_stamp() -> String {
    Local::now().format(STAMP_FORMAT).to_string()
}

/// Progress callback: `(label, done, total)`.
pub type Progress<'a> = &'a mut dyn FnMut(&'static str, u64, u64);

impl InstanceStore {
    /// Zips `saves/<folder>` into `<instance>/backups/<folder>-<stamp>.zip`.
    pub fn backup_world(&self, id: &str, folder: &str, on_progress: Progress<'_>) -> Result<WorldBackup> {
        let inst = self.get(id)?;
        backup_world(&self.game_dir(&inst).join("saves"), &self.backups_dir(id)?, folder, on_progress)
    }

    /// World backups of an instance, newest first.
    pub fn list_backups(&self, id: &str) -> Result<Vec<WorldBackup>> {
        self.get(id)?;
        Ok(list_backups(&self.backups_dir(id)?))
    }

    /// Replaces the world folder with the content of a backup. The current folder, if any, is backed up
    /// first; that safety backup is returned.
    pub fn restore_backup(&self, id: &str, file_name: &str, on_progress: Progress<'_>) -> Result<Option<WorldBackup>> {
        let inst = self.get(id)?;
        restore_backup(&self.game_dir(&inst).join("saves"), &self.backups_dir(id)?, file_name, on_progress)
    }

    /// Moves `saves/<folder>` to the launcher trash (`<root>/trash/<id>-world-<folder>-<stamp>`).
    pub fn delete_world(&self, id: &str, folder: &str) -> Result<PathBuf> {
        let inst = self.get(id)?;
        check_plain_name(folder)?;
        let src = self.game_dir(&inst).join("saves").join(folder);
        if !src.is_dir() {
            return Err(Error::NotFound(format!("world {folder}")));
        }
        move_to_trash(&src, &self.trash_dir(), &format!("{id}-world-{folder}"))
    }
}

fn zip_error(path: &Path, e: zip::result::ZipError) -> Error {
    match e {
        zip::result::ZipError::Io(source) => Error::io(path, source),
        other => Error::Other(format!("{} isn't a valid zip file ({other}).", path.display())),
    }
}

/// Splits `<world>-<stamp>[-<n>].zip` into the world folder and the creation time.
fn parse_backup_name(file_name: &str) -> Option<(String, DateTime<Utc>)> {
    let stem = file_name.strip_suffix(".zip").or_else(|| file_name.strip_suffix(".ZIP"))?;
    let parse = |s: &str| -> Option<(String, DateTime<Utc>)> {
        let split = s.len().checked_sub(STAMP_LEN)?;
        if !s.is_char_boundary(split) {
            return None;
        }
        let (world, stamp) = s.split_at(split);
        let world = world.strip_suffix('-').filter(|w| !w.is_empty())?;
        let naive = NaiveDateTime::parse_from_str(stamp, STAMP_FORMAT).ok()?;
        let created = Local.from_local_datetime(&naive).earliest()?.with_timezone(&Utc);
        Some((world.to_string(), created))
    };
    parse(stem).or_else(|| {
        let (rest, n) = stem.rsplit_once('-')?;
        n.parse::<u32>().ok()?;
        parse(rest)
    })
}

fn backup_info(path: &Path) -> Option<WorldBackup> {
    let meta = fs::metadata(path).ok().filter(|m| m.is_file())?;
    let file_name = path.file_name()?.to_string_lossy().into_owned();
    if !file_name.to_lowercase().ends_with(".zip") {
        return None;
    }
    let (world, created) = match parse_backup_name(&file_name) {
        Some(parsed) => parsed,
        None => (file_name[..file_name.len() - 4].to_string(), mtime(path).unwrap_or_else(Utc::now)),
    };
    Some(WorldBackup { file_name, world, created, size: meta.len() })
}

pub(crate) fn list_backups(backups: &Path) -> Vec<WorldBackup> {
    let Ok(rd) = fs::read_dir(backups) else { return Vec::new() };
    let mut out: Vec<WorldBackup> = rd.flatten().filter_map(|e| backup_info(&e.path())).collect();
    out.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.file_name.cmp(&a.file_name)));
    out
}

/// Every entry under `root` as (`/`-separated relative path, absolute path, is_dir), sorted, symlinks skipped.
fn walk(root: &Path) -> Result<Vec<(String, PathBuf, bool)>> {
    let mut out = Vec::new();
    let mut stack = vec![(String::new(), root.to_path_buf())];
    while let Some((prefix, dir)) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|e| Error::io(&dir, e))? {
            let entry = entry.map_err(|e| Error::io(&dir, e))?;
            let ty = entry.file_type().map_err(|e| Error::io(entry.path(), e))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
            if ty.is_dir() {
                out.push((rel.clone(), entry.path(), true));
                stack.push((rel, entry.path()));
            } else if ty.is_file() && !(prefix.is_empty() && SKIPPED_FILES.contains(&name.as_str())) {
                out.push((rel, entry.path(), false));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Modification time of a file in the zip format (local time, 2-second precision, 1980..=2107).
fn zip_time(meta: &fs::Metadata) -> zip::DateTime {
    let Ok(modified) = meta.modified() else { return zip::DateTime::default() };
    let t: DateTime<Local> = modified.into();
    let (Ok(year), Ok(month), Ok(day), Ok(hour), Ok(minute), Ok(second)) = (
        u16::try_from(t.year()),
        u8::try_from(t.month()),
        u8::try_from(t.day()),
        u8::try_from(t.hour()),
        u8::try_from(t.minute()),
        u8::try_from(t.second().min(58)),
    ) else {
        return zip::DateTime::default();
    };
    zip::DateTime::from_date_and_time(year, month, day, hour, minute, second).unwrap_or_default()
}

fn write_world_zip(world: &Path, folder: &str, dest: &Path, on_progress: Progress<'_>) -> Result<()> {
    const LABEL: &str = "Backing up world";
    let entries = walk(world)?;
    let total = entries.iter().filter(|e| !e.2).count() as u64;
    on_progress(LABEL, 0, total);
    let file = fs::File::create(dest).map_err(|e| Error::io(dest, e))?;
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let dir_options = SimpleFileOptions::default();
    zip.add_directory(format!("{folder}/"), dir_options).map_err(|e| zip_error(dest, e))?;
    let mut done = 0;
    for (rel, path, is_dir) in entries {
        let name = format!("{folder}/{rel}");
        if is_dir {
            zip.add_directory(format!("{name}/"), dir_options).map_err(|e| zip_error(dest, e))?;
            continue;
        }
        let meta = fs::metadata(&path).map_err(|e| Error::io(&path, e))?;
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(meta.len() >= u64::from(u32::MAX))
            .last_modified_time(zip_time(&meta));
        zip.start_file(name, options).map_err(|e| zip_error(dest, e))?;
        let mut src = fs::File::open(&path).map_err(|e| Error::io(&path, e))?;
        std::io::copy(&mut src, &mut zip).map_err(|e| Error::io(&path, e))?;
        done += 1;
        on_progress(LABEL, done, total);
    }
    let mut writer = zip.finish().map_err(|e| zip_error(dest, e))?;
    writer.flush().map_err(|e| Error::io(dest, e))?;
    let file = writer.into_inner().map_err(|e| Error::io(dest, e.into_error()))?;
    file.sync_all().map_err(|e| Error::io(dest, e))
}

pub(crate) fn backup_world(saves: &Path, backups: &Path, folder: &str, on_progress: Progress<'_>) -> Result<WorldBackup> {
    check_plain_name(folder)?;
    let world = saves.join(folder);
    if !world.is_dir() {
        return Err(Error::NotFound(format!("world {folder}")));
    }
    fs::create_dir_all(backups).map_err(|e| Error::io(backups, e))?;
    let stamp = backup_stamp();
    let mut file_name = format!("{folder}-{stamp}.zip");
    let mut n = 1;
    while backups.join(&file_name).exists() {
        n += 1;
        file_name = format!("{folder}-{stamp}-{n}.zip");
    }
    let dest = backups.join(&file_name);
    let tmp = backups.join(format!("{file_name}.tmp"));
    if let Err(e) = write_world_zip(&world, folder, &tmp, on_progress) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fs::rename(&tmp, &dest).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        Error::io(&dest, e)
    })?;
    backup_info(&dest).ok_or_else(|| Error::NotFound(format!("backup {file_name}")))
}

/// A relative path made only of normal components, or `None`.
fn plain_relative(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

fn extract_world(archive_path: &Path, file_name: &str, into: &Path, on_progress: Progress<'_>) -> Result<String> {
    const LABEL: &str = "Restoring world";
    let file = fs::File::open(archive_path).map_err(|e| Error::io(archive_path, e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| zip_error(archive_path, e))?;

    let mut entries = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| zip_error(archive_path, e))?;
        let path = entry
            .enclosed_name()
            .and_then(|p| plain_relative(&p))
            .ok_or_else(|| Error::Other(format!("{file_name} contains an unsafe path ({}), so it can't be restored.", entry.name())))?;
        entries.push((i, path, entry.is_dir()));
    }
    // Strip a single top-level folder (the game's backup layout) unless files sit at the root.
    let top = entries.first().and_then(|(_, p, _)| p.components().next()).map(|c| c.as_os_str().to_owned());
    let root_file = entries.iter().any(|(_, p, is_dir)| !is_dir && p.components().count() == 1);
    let strip = top.filter(|t| !root_file && entries.iter().all(|(_, p, _)| p.components().next().map(|c| c.as_os_str()) == Some(t.as_os_str())));
    let world = parse_backup_name(file_name)
        .map(|(w, _)| w)
        .or_else(|| strip.as_ref().map(|t| t.to_string_lossy().into_owned()))
        .unwrap_or_else(|| file_name[..file_name.len() - 4].to_string());
    check_plain_name(&world)?;

    let total = entries.iter().filter(|e| !e.2).count() as u64;
    if total == 0 {
        return Err(Error::Other(format!("{file_name} is empty.")));
    }
    on_progress(LABEL, 0, total);
    fs::create_dir_all(into).map_err(|e| Error::io(into, e))?;
    let mut done = 0;
    for (i, path, is_dir) in entries {
        let rel = match &strip {
            Some(top) => path.strip_prefix(top).map(Path::to_path_buf).unwrap_or_default(),
            None => path,
        };
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = into.join(&rel);
        if is_dir {
            fs::create_dir_all(&dest).map_err(|e| Error::io(&dest, e))?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        let mut entry = zip.by_index(i).map_err(|e| zip_error(archive_path, e))?;
        let mut out = fs::File::create(&dest).map_err(|e| Error::io(&dest, e))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| Error::io(&dest, e))?;
        done += 1;
        on_progress(LABEL, done, total);
    }
    Ok(world)
}

pub(crate) fn restore_backup(saves: &Path, backups: &Path, file_name: &str, on_progress: Progress<'_>) -> Result<Option<WorldBackup>> {
    check_plain_name(file_name)?;
    if !file_name.to_lowercase().ends_with(".zip") {
        return Err(Error::Other(format!("{file_name} isn't a world backup.")));
    }
    let archive = backups.join(file_name);
    if !archive.is_file() {
        return Err(Error::NotFound(format!("backup {file_name}")));
    }
    // Extract next to the backups (same volume as saves/), so the final swap is two renames. The world
    // folder comes from the archive name, else from its content.
    let staging = backups.join(format!(".restoring-{file_name}"));
    let _ = fs::remove_dir_all(&staging);
    let world = match extract_world(&archive, file_name, &staging, on_progress) {
        Ok(world) => world,
        Err(e) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(e);
        }
    };

    let target = saves.join(&world);
    let safety = if target.exists() {
        match backup_world(saves, backups, &world, on_progress) {
            Ok(b) => Some(b),
            Err(e) => {
                let _ = fs::remove_dir_all(&staging);
                return Err(e);
            }
        }
    } else {
        None
    };

    fs::create_dir_all(saves).map_err(|e| Error::io(saves, e))?;
    let replaced = backups.join(format!(".replaced-{world}"));
    let _ = fs::remove_dir_all(&replaced);
    if target.exists() {
        if let Err(e) = fs::rename(&target, &replaced) {
            let _ = fs::remove_dir_all(&staging);
            return Err(Error::io(&target, e));
        }
    }
    if let Err(e) = fs::rename(&staging, &target) {
        if replaced.exists() {
            let _ = fs::rename(&replaced, &target);
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(Error::io(&target, e));
    }
    // The previous folder is in the safety backup.
    if replaced.exists() {
        if let Err(e) = fs::remove_dir_all(&replaced) {
            tracing::warn!("could not remove {}: {e}", replaced.display());
        }
    }
    Ok(safety)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Loader, NewInstance};
    use crate::testutil::{temp_dir, write_zip};
    use cubby_common::paths::Paths;

    fn setup(name: &str) -> (InstanceStore, String, PathBuf) {
        let store = InstanceStore::new(Paths::new(temp_dir(name)));
        let inst = store
            .create(NewInstance { name: "W".into(), group: None, minecraft: "1.21.1".into(), loader: Loader::Vanilla, icon: None })
            .unwrap();
        let world = store.game_dir(&inst).join("saves").join("New World");
        fs::create_dir_all(world.join("region")).unwrap();
        fs::create_dir_all(world.join("empty")).unwrap();
        fs::write(world.join("level.dat"), "level-v1").unwrap();
        fs::write(world.join("region/r.0.0.mca"), vec![7u8; 10_000]).unwrap();
        fs::write(world.join("session.lock"), "lock").unwrap();
        (store, inst.id, world)
    }

    fn noop() -> impl FnMut(&'static str, u64, u64) {
        |_, _, _| {}
    }

    #[test]
    fn backup_names() {
        let (world, created) = parse_backup_name("New World-20261102-153012.zip").unwrap();
        assert_eq!(world, "New World");
        assert_eq!(created.with_timezone(&Local).format(STAMP_FORMAT).to_string(), "20261102-153012");
        assert_eq!(parse_backup_name("a-b-20261102-153012-3.zip").unwrap().0, "a-b");
        assert_eq!(parse_backup_name("Été-20261102-153012.zip").unwrap().0, "Été");
        assert!(parse_backup_name("-20261102-153012.zip").is_none());
        assert!(parse_backup_name("world.zip").is_none());
        assert!(parse_backup_name("é.zip").is_none());
        assert!(parse_backup_name("World-20261102-153012.tar").is_none());
    }

    #[test]
    fn backup_list_restore() {
        let (store, id, world) = setup("backup-roundtrip");
        let mut steps = Vec::new();
        let b1 = store
            .backup_world(&id, "New World", &mut |label: &'static str, done: u64, total: u64| steps.push((label, done, total)))
            .unwrap();
        assert!(b1.file_name.starts_with("New World-") && b1.file_name.ends_with(".zip"));
        assert_eq!(b1.world, "New World");
        assert!(b1.size > 0);
        assert_eq!(steps.first(), Some(&("Backing up world", 0, 2)));
        assert_eq!(steps.last(), Some(&("Backing up world", 2, 2)));

        // Same second: a distinct name.
        let b2 = store.backup_world(&id, "New World", &mut noop()).unwrap();
        assert_ne!(b1.file_name, b2.file_name);
        let list = store.list_backups(&id).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|b| b.world == "New World"));

        // Archive layout: everything under the world folder, no session.lock.
        let backups = store.backups_dir(&id).unwrap();
        let mut zip = zip::ZipArchive::new(fs::File::open(backups.join(&b1.file_name)).unwrap()).unwrap();
        let names: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().to_string()).collect();
        assert!(names.contains(&"New World/level.dat".to_string()), "{names:?}");
        assert!(names.contains(&"New World/region/r.0.0.mca".to_string()));
        assert!(names.contains(&"New World/empty/".to_string()));
        assert!(!names.iter().any(|n| n.ends_with("session.lock")));

        fs::write(world.join("level.dat"), "level-v2").unwrap();
        fs::write(world.join("new.txt"), "x").unwrap();
        let safety = store.restore_backup(&id, &b1.file_name, &mut noop()).unwrap().expect("safety backup");
        assert_eq!(fs::read_to_string(world.join("level.dat")).unwrap(), "level-v1");
        assert!(!world.join("new.txt").exists());
        assert_eq!(fs::read(world.join("region/r.0.0.mca")).unwrap().len(), 10_000);
        assert!(world.join("empty").is_dir());
        assert_eq!(store.list_backups(&id).unwrap().len(), 3);
        // The safety backup holds the state before the restore.
        store.restore_backup(&id, &safety.file_name, &mut noop()).unwrap();
        assert_eq!(fs::read_to_string(world.join("level.dat")).unwrap(), "level-v2");
        assert!(world.join("new.txt").exists());
        // No staging folders left behind.
        let leftovers: Vec<_> = fs::read_dir(&backups).unwrap().flatten().filter(|e| e.path().is_dir()).collect();
        assert!(leftovers.is_empty());

        assert!(store.backup_world(&id, "../x", &mut noop()).is_err());
        assert!(matches!(store.backup_world(&id, "Nope", &mut noop()), Err(Error::NotFound(_))));
        assert!(store.restore_backup(&id, "../x.zip", &mut noop()).is_err());
        assert!(matches!(store.restore_backup(&id, "Nope-20260101-000000.zip", &mut noop()), Err(Error::NotFound(_))));
    }

    #[test]
    fn restore_deleted_world_and_flat_archive() {
        let (store, id, world) = setup("backup-flat");
        let backups = store.backups_dir(&id).unwrap();
        fs::create_dir_all(&backups).unwrap();
        write_zip(&backups.join("Flat-20260101-120000.zip"), &[("level.dat", "flat"), ("region/r.0.0.mca", "r")]);
        let saves = world.parent().unwrap().to_path_buf();
        assert!(store.restore_backup(&id, "Flat-20260101-120000.zip", &mut noop()).unwrap().is_none());
        assert_eq!(fs::read_to_string(saves.join("Flat/level.dat")).unwrap(), "flat");
        assert!(saves.join("Flat/region/r.0.0.mca").is_file());

        write_zip(&backups.join("evil-20260101-120000.zip"), &[("../escape.txt", "x")]);
        assert!(store.restore_backup(&id, "evil-20260101-120000.zip", &mut noop()).is_err());
        assert!(!saves.join("escape.txt").exists() && !saves.parent().unwrap().join("escape.txt").exists());
        assert!(!saves.join("evil").exists());
    }

    #[test]
    fn delete_world_goes_to_trash() {
        let (store, id, world) = setup("world-delete");
        let dest = store.delete_world(&id, "New World").unwrap();
        assert!(!world.exists());
        assert!(dest.starts_with(store.trash_dir()));
        assert!(dest.join("level.dat").is_file());
        assert!(matches!(store.delete_world(&id, "New World"), Err(Error::NotFound(_))));
        assert!(store.delete_world(&id, "..").is_err());
    }
}

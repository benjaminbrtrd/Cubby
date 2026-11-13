//! Undoing a content update.
//!
//! Before an update replaces files, the old ones are copied to `<instance>/backups/content-<stamp>/<folder>/`.
//! Once it is done, [`record_update`] writes `cubby-update.json` there: which old file became which new one.
//! [`InstanceStore::undo_update`] puts the old files back, moves the new ones to `<game dir>/.trash/<folder>/`
//! and renames the backup `content-<stamp>-undone`, so it isn't offered again.

use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use cubby_common::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::store::InstanceStore;
use crate::util::{atomic_write, check_plain_name, move_to_trash};

const MANIFEST: &str = "cubby-update.json";
const PREFIX: &str = "content-";
const UNDONE: &str = "-undone";
const FOLDERS: [&str; 3] = ["mods", "resourcepacks", "shaderpacks"];

/// One file an update replaced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedFile {
    /// "mods", "resourcepacks" or "shaderpacks".
    pub folder: String,
    /// The file before the update (kept in the backup).
    pub old: String,
    /// The file the update put in its place.
    pub new: String,
}

/// An update that can be undone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBackup {
    /// Folder name in `<instance>/backups/`.
    pub name: String,
    pub created: DateTime<Utc>,
    pub files: Vec<UpdatedFile>,
}

/// Writes the manifest of an update whose old files were copied to `backup_dir`. `renamed` lists the files
/// replaced by a file with another name as `(folder, old, new)`; the others kept their name. Does nothing when
/// the update backed up nothing.
pub fn record_update(backup_dir: &Path, renamed: &[(String, String, String)]) -> Result<()> {
    let mut files = Vec::new();
    for folder in FOLDERS {
        let Ok(rd) = fs::read_dir(backup_dir.join(folder)) else { continue };
        let mut names: Vec<String> = rd.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        for old in names {
            let new = renamed.iter().find(|(f, o, _)| f == folder && *o == old).map(|(_, _, n)| n.clone()).unwrap_or_else(|| old.clone());
            files.push(UpdatedFile { folder: folder.to_string(), old, new });
        }
    }
    if files.is_empty() {
        return Ok(());
    }
    let manifest = UpdateBackup { name: dir_name(backup_dir), created: Utc::now(), files };
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| Error::Other(e.to_string()))?;
    atomic_write(&backup_dir.join(MANIFEST), text.as_bytes())
}

fn dir_name(dir: &Path) -> String {
    dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn read_manifest(dir: &Path) -> Option<UpdateBackup> {
    let text = fs::read_to_string(dir.join(MANIFEST)).ok()?;
    let mut backup: UpdateBackup = serde_json::from_str(&text).ok()?;
    backup.name = dir_name(dir);
    Some(backup)
}

/// Updates of a backups folder that can be undone, newest first.
pub fn list_update_backups(backups: &Path) -> Vec<UpdateBackup> {
    let Ok(rd) = fs::read_dir(backups) else { return Vec::new() };
    let mut out: Vec<UpdateBackup> = rd
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with(PREFIX) && !name.ends_with(UNDONE) && e.path().is_dir()
        })
        .filter_map(|e| read_manifest(&e.path()))
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.name.cmp(&a.name)));
    out
}

/// Puts back the files an update replaced. Returns how many files came back.
pub fn undo_update(game_dir: &Path, backups: &Path, name: &str) -> Result<usize> {
    check_plain_name(name)?;
    if !name.starts_with(PREFIX) || name.ends_with(UNDONE) {
        return Err(Error::NotFound(format!("update backup {name}")));
    }
    let dir = backups.join(name);
    let backup = read_manifest(&dir).ok_or_else(|| Error::NotFound(format!("update backup {name}")))?;
    // Check everything first, so a missing backup file doesn't leave the instance half undone.
    for f in &backup.files {
        check_plain_name(&f.old)?;
        check_plain_name(&f.new)?;
        if !FOLDERS.contains(&f.folder.as_str()) {
            return Err(Error::Other(format!("{} isn't a content folder.", f.folder)));
        }
        if !dir.join(&f.folder).join(&f.old).is_file() {
            return Err(Error::Other(format!("The backup of {} is missing, so this update can't be undone.", f.old)));
        }
    }
    for f in &backup.files {
        let folder = game_dir.join(&f.folder);
        fs::create_dir_all(&folder).map_err(|e| Error::io(&folder, e))?;
        // The new file may have been turned off or on since: look for both names.
        let new_names = [f.new.clone(), toggled(&f.new)];
        for new in &new_names {
            let path = folder.join(new);
            if path.is_file() && *new != f.old {
                move_to_trash(&path, &game_dir.join(".trash").join(&f.folder), new)?;
            }
        }
        let src = dir.join(&f.folder).join(&f.old);
        fs::copy(&src, folder.join(&f.old)).map_err(|e| Error::io(&src, e))?;
    }
    let done = backups.join(format!("{name}{UNDONE}"));
    fs::rename(&dir, &done).map_err(|e| Error::io(&dir, e))?;
    Ok(backup.files.len())
}

/// `a.jar` ↔ `a.jar.disabled`.
fn toggled(name: &str) -> String {
    match name.strip_suffix(".disabled") {
        Some(base) => base.to_string(),
        None => format!("{name}.disabled"),
    }
}

impl InstanceStore {
    /// Updates of an instance that can be undone, newest first.
    pub fn list_update_backups(&self, id: &str) -> Result<Vec<UpdateBackup>> {
        self.get(id)?;
        Ok(list_update_backups(&self.backups_dir(id)?))
    }

    /// Puts back the files replaced by the update backed up in `name`. Returns how many came back.
    pub fn undo_update(&self, id: &str, name: &str) -> Result<usize> {
        let inst = self.get(id)?;
        undo_update(&self.game_dir(&inst), &self.backups_dir(id)?, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    #[test]
    fn records_lists_and_undoes() {
        let root = temp_dir("update-undo");
        let game = root.join(".minecraft");
        let backups = root.join("backups");
        let backup = backups.join("content-20261108-101500");
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::create_dir_all(backup.join("mods")).unwrap();
        fs::create_dir_all(backup.join("resourcepacks")).unwrap();
        // sodium was renamed by the update, the pack kept its name, lithium was turned off after.
        fs::write(backup.join("mods/sodium-0.5.jar"), "old sodium").unwrap();
        fs::write(backup.join("mods/lithium-1.jar"), "old lithium").unwrap();
        fs::write(backup.join("resourcepacks/Faithful.zip"), "old pack").unwrap();
        fs::write(game.join("mods/sodium-0.6.jar"), "new sodium").unwrap();
        fs::write(game.join("mods/lithium-2.jar.disabled"), "new lithium").unwrap();
        fs::create_dir_all(game.join("resourcepacks")).unwrap();
        fs::write(game.join("resourcepacks/Faithful.zip"), "new pack").unwrap();

        let renamed = vec![
            ("mods".to_string(), "sodium-0.5.jar".to_string(), "sodium-0.6.jar".to_string()),
            ("mods".to_string(), "lithium-1.jar".to_string(), "lithium-2.jar".to_string()),
        ];
        record_update(&backup, &renamed).unwrap();
        record_update(&backups.join("content-empty"), &[]).unwrap();
        let list = list_update_backups(&backups);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "content-20261108-101500");
        assert_eq!(list[0].files.len(), 3);

        assert_eq!(undo_update(&game, &backups, "content-20261108-101500").unwrap(), 3);
        assert_eq!(fs::read_to_string(game.join("mods/sodium-0.5.jar")).unwrap(), "old sodium");
        assert_eq!(fs::read_to_string(game.join("mods/lithium-1.jar")).unwrap(), "old lithium");
        assert_eq!(fs::read_to_string(game.join("resourcepacks/Faithful.zip")).unwrap(), "old pack");
        assert!(!game.join("mods/sodium-0.6.jar").exists());
        assert!(!game.join("mods/lithium-2.jar.disabled").exists());
        assert!(game.join(".trash/mods").read_dir().unwrap().count() >= 2);
        assert!(list_update_backups(&backups).is_empty());
        assert!(undo_update(&game, &backups, "content-20261108-101500").is_err());
        assert!(undo_update(&game, &backups, "../x").is_err());
    }

    #[test]
    fn refuses_when_a_backup_file_is_missing() {
        let root = temp_dir("update-undo-missing");
        let backups = root.join("backups");
        let backup = backups.join("content-1");
        fs::create_dir_all(backup.join("mods")).unwrap();
        fs::write(backup.join("mods/a.jar"), "a").unwrap();
        record_update(&backup, &[]).unwrap();
        fs::remove_file(backup.join("mods/a.jar")).unwrap();
        assert!(undo_update(&root.join(".minecraft"), &backups, "content-1").is_err());
    }
}

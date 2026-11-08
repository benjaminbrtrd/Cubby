//! The datapacks of a world: `saves/<world>/datapacks`.
//!
//! The game loads every `.zip` and every folder with a `pack.mcmeta` it finds there. A datapack turned off in
//! Cubby moves to `saves/<world>/datapacks.disabled`, which the game never reads, and comes back when turned on.
//! Removed ones go to `<game dir>/.trash/datapacks/`.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use cubby_common::{Error, Result};
use serde::Serialize;

use crate::util::{check_plain_name, copy_dir_all, move_to_trash, tree_size};

const ON: &str = "datapacks";
const OFF: &str = "datapacks.disabled";
const MAX_MCMETA_BYTES: u64 = 256 * 1024;

/// A datapack of a world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatapackFile {
    /// File or folder name on disk.
    pub file_name: String,
    /// The file name without `.zip`.
    pub name: String,
    pub is_dir: bool,
    pub enabled: bool,
    pub size: u64,
    pub modified: Option<DateTime<Utc>>,
    /// `pack.description` from `pack.mcmeta`, as plain text.
    pub description: Option<String>,
}

fn world_dir(game_dir: &Path, world: &str) -> Result<PathBuf> {
    check_plain_name(world)?;
    let dir = game_dir.join("saves").join(world);
    if !dir.is_dir() {
        return Err(Error::NotFound(format!("world {world}")));
    }
    Ok(dir)
}

fn is_pack(path: &Path, file_name: &str) -> bool {
    !file_name.starts_with('.') && (file_name.to_lowercase().ends_with(".zip") && path.is_file() || path.join("pack.mcmeta").is_file())
}

/// The text of a chat component: a string, `{"text": ..., "extra": [...]}` or a list of them.
fn component_text(value: &serde_json::Value, out: &mut String) {
    match value {
        serde_json::Value::String(s) => out.push_str(s),
        serde_json::Value::Array(items) => items.iter().for_each(|v| component_text(v, out)),
        serde_json::Value::Object(map) => {
            if let Some(text) = map.get("text").or_else(|| map.get("translate")) {
                component_text(text, out);
            }
            if let Some(extra) = map.get("extra") {
                component_text(extra, out);
            }
        }
        serde_json::Value::Number(n) => out.push_str(&n.to_string()),
        _ => {}
    }
}

/// Removes `§x` formatting codes.
fn strip_codes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

fn read_mcmeta(path: &Path, is_dir: bool) -> Option<String> {
    if is_dir {
        let file = path.join("pack.mcmeta");
        if fs::metadata(&file).ok()?.len() > MAX_MCMETA_BYTES {
            return None;
        }
        return fs::read_to_string(file).ok();
    }
    let mut zip = zip::ZipArchive::new(fs::File::open(path).ok()?).ok()?;
    let entry = zip.by_name("pack.mcmeta").ok()?;
    if entry.size() > MAX_MCMETA_BYTES {
        return None;
    }
    let mut text = String::new();
    entry.take(MAX_MCMETA_BYTES).read_to_string(&mut text).ok()?;
    Some(text)
}

fn description(path: &Path, is_dir: bool) -> Option<String> {
    let text = read_mcmeta(path, is_dir)?;
    let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let mut out = String::new();
    component_text(json.get("pack")?.get("description")?, &mut out);
    let out = strip_codes(&out).trim().to_string();
    (!out.is_empty()).then_some(out)
}

fn list_folder(dir: &Path, enabled: bool, out: &mut Vec<DatapackFile>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if !is_pack(&path, &file_name) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let is_dir = meta.is_dir();
        let name = if is_dir { file_name.clone() } else { file_name[..file_name.len() - 4].to_string() };
        out.push(DatapackFile {
            description: description(&path, is_dir),
            size: if is_dir { tree_size(&path) } else { meta.len() },
            modified: meta.modified().ok().map(DateTime::<Utc>::from),
            file_name,
            name,
            is_dir,
            enabled,
        });
    }
}

/// The datapacks of a world, turned on and off, sorted by name.
pub fn list_datapacks(game_dir: &Path, world: &str) -> Result<Vec<DatapackFile>> {
    let dir = world_dir(game_dir, world)?;
    let mut out = Vec::new();
    list_folder(&dir.join(ON), true, &mut out);
    list_folder(&dir.join(OFF), false, &mut out);
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| b.enabled.cmp(&a.enabled)));
    Ok(out)
}

/// Turns a datapack on (moves it into `datapacks/`) or off (into `datapacks.disabled/`).
pub fn set_datapack_enabled(game_dir: &Path, world: &str, file_name: &str, enabled: bool) -> Result<()> {
    check_plain_name(file_name)?;
    let dir = world_dir(game_dir, world)?;
    let (from, to) = if enabled { (OFF, ON) } else { (ON, OFF) };
    let src = dir.join(from).join(file_name);
    if !src.exists() {
        // Already where it should be: nothing to do.
        return if dir.join(to).join(file_name).exists() { Ok(()) } else { Err(Error::NotFound(format!("datapack {file_name}"))) };
    }
    let dest_dir = dir.join(to);
    fs::create_dir_all(&dest_dir).map_err(|e| Error::io(&dest_dir, e))?;
    let dest = dest_dir.join(file_name);
    if dest.exists() {
        return Err(Error::Other(format!("{file_name} is in this world twice. Remove one of them.")));
    }
    fs::rename(&src, &dest).map_err(|e| Error::io(src, e))
}

/// Moves a datapack to `<game dir>/.trash/datapacks/`. Returns its new location.
pub fn remove_datapack(game_dir: &Path, world: &str, file_name: &str) -> Result<PathBuf> {
    check_plain_name(file_name)?;
    let dir = world_dir(game_dir, world)?;
    let src = [ON, OFF].iter().map(|f| dir.join(f).join(file_name)).find(|p| p.exists()).ok_or_else(|| Error::NotFound(format!("datapack {file_name}")))?;
    move_to_trash(&src, &game_dir.join(".trash").join("datapacks"), &format!("{world}-{file_name}"))
}

/// Where a datapack named `file_name` would be downloaded or copied in a world, turned on.
pub fn datapack_path(game_dir: &Path, world: &str, file_name: &str) -> Result<PathBuf> {
    check_plain_name(file_name)?;
    Ok(world_dir(game_dir, world)?.join(ON).join(file_name))
}

/// Copies `.zip` files and datapack folders into a world, turned on. A datapack of the same name is replaced.
/// Returns the names added; anything that isn't a datapack is an error, and nothing after it is copied.
pub fn import_datapacks(game_dir: &Path, world: &str, sources: &[PathBuf]) -> Result<Vec<String>> {
    let dir = world_dir(game_dir, world)?.join(ON);
    fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    let mut added = Vec::new();
    for src in sources {
        let file_name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !is_pack(src, &file_name) {
            return Err(Error::Other(format!("{file_name} isn't a datapack: pick a .zip file or a folder with a pack.mcmeta.")));
        }
        let dest = dir.join(&file_name);
        let off = dir.with_file_name(OFF).join(&file_name);
        for old in [&dest, &off] {
            if old.is_dir() {
                fs::remove_dir_all(old).map_err(|e| Error::io(old, e))?;
            } else if old.exists() {
                fs::remove_file(old).map_err(|e| Error::io(old, e))?;
            }
        }
        if src.is_dir() {
            copy_dir_all(src, &dest)?;
        } else {
            fs::copy(src, &dest).map_err(|e| Error::io(src, e))?;
        }
        added.push(file_name);
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{temp_dir, write_zip};

    fn world(name: &str) -> PathBuf {
        let game = temp_dir(name);
        fs::create_dir_all(game.join("saves/World/datapacks")).unwrap();
        game
    }

    #[test]
    fn lists_toggles_and_removes() {
        let game = world("datapacks-list");
        let packs = game.join("saves/World/datapacks");
        write_zip(&packs.join("Terralith.zip"), &[("pack.mcmeta", r#"{"pack":{"pack_format":48,"description":[{"text":"§aNew "},{"text":"biomes"}]}}"#)]);
        fs::create_dir_all(packs.join("my pack")).unwrap();
        fs::write(packs.join("my pack/pack.mcmeta"), r#"{"pack":{"description":"Mine"}}"#).unwrap();
        fs::create_dir_all(packs.join("not a pack")).unwrap();
        fs::write(packs.join("notes.txt"), "x").unwrap();

        let list = list_datapacks(&game, "World").unwrap();
        let names: Vec<_> = list.iter().map(|p| (p.name.as_str(), p.enabled, p.description.as_deref())).collect();
        assert_eq!(names, [("my pack", true, Some("Mine")), ("Terralith", true, Some("New biomes"))]);

        set_datapack_enabled(&game, "World", "Terralith.zip", false).unwrap();
        assert!(game.join("saves/World/datapacks.disabled/Terralith.zip").is_file());
        assert!(!list_datapacks(&game, "World").unwrap().iter().find(|p| p.name == "Terralith").unwrap().enabled);
        set_datapack_enabled(&game, "World", "Terralith.zip", false).unwrap();
        set_datapack_enabled(&game, "World", "Terralith.zip", true).unwrap();
        assert!(packs.join("Terralith.zip").is_file());

        let trashed = remove_datapack(&game, "World", "my pack").unwrap();
        assert!(trashed.join("pack.mcmeta").is_file());
        assert_eq!(list_datapacks(&game, "World").unwrap().len(), 1);
        assert!(list_datapacks(&game, "../x").is_err());
        assert!(list_datapacks(&game, "Nope").is_err());
    }

    #[test]
    fn imports_zips_and_folders() {
        let game = world("datapacks-import");
        let src = temp_dir("datapacks-import-src");
        write_zip(&src.join("a.zip"), &[("pack.mcmeta", "{}")]);
        fs::create_dir_all(src.join("b")).unwrap();
        fs::write(src.join("b/pack.mcmeta"), "{}").unwrap();
        fs::write(src.join("c.txt"), "x").unwrap();
        let added = import_datapacks(&game, "World", &[src.join("a.zip"), src.join("b")]).unwrap();
        assert_eq!(added, ["a.zip", "b"]);
        assert!(game.join("saves/World/datapacks/b/pack.mcmeta").is_file());
        assert!(import_datapacks(&game, "World", &[src.join("c.txt")]).is_err());
        // Importing again replaces, also a copy that was turned off.
        set_datapack_enabled(&game, "World", "a.zip", false).unwrap();
        import_datapacks(&game, "World", &[src.join("a.zip")]).unwrap();
        assert_eq!(list_datapacks(&game, "World").unwrap().iter().filter(|p| p.enabled).count(), 2);
        assert!(!game.join("saves/World/datapacks.disabled/a.zip").exists());
    }
}

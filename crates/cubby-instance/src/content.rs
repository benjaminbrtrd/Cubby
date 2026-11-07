//! Content listings for the instance tabs: mods, resource/shader packs, worlds and screenshots.
//!
//! Everything here is synchronous, cheap and tolerant: unreadable or junk files are skipped (or reported
//! without metadata) rather than failing the whole listing.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use cubby_common::{Error, Result};
use serde::Serialize;

use crate::nbt;
use crate::util::{atomic_write, check_plain_name, move_to_trash, mtime, tree_size};

const DISABLED_SUFFIX: &str = ".disabled";
/// Metadata files larger than this inside a jar are ignored.
const MAX_META_BYTES: u64 = 2 * 1024 * 1024;

// mods

/// Which loader a mod was built for, guessed from its metadata file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LoaderKind {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

/// Metadata read from inside a mod jar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModMeta {
    pub id: String,
    /// Display name (falls back to the id).
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub loader_hint: LoaderKind,
}

/// A file in `mods/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    /// Actual file name on disk, including a trailing `.disabled` when disabled.
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    pub modified: Option<DateTime<Utc>>,
    pub meta: Option<ModMeta>,
}

fn is_mod_name(lower: &str) -> bool {
    let base = lower.strip_suffix(DISABLED_SUFFIX).unwrap_or(lower);
    base.ends_with(".jar") || base.ends_with(".zip") || base.ends_with(".litemod")
}

/// Lists `<game_dir>/mods` (jar/zip/litemod files, optionally suffixed `.disabled`), sorted by file name.
pub fn list_mods(game_dir: &Path) -> Vec<ModFile> {
    let Ok(rd) = fs::read_dir(game_dir.join("mods")) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if !is_mod_name(&file_name.to_lowercase()) {
            continue;
        }
        out.push(ModFile {
            enabled: !file_name.to_lowercase().ends_with(DISABLED_SUFFIX),
            size: meta.len(),
            modified: meta.modified().ok().map(DateTime::<Utc>::from),
            meta: read_mod_meta(&entry.path()),
            file_name,
        });
    }
    out.sort_by_key(|m| m.file_name.to_lowercase());
    out
}

/// Enables or disables a mod by renaming it with or without the `.disabled` suffix.
/// `file_name` may be given in either state. Returns the resulting file name.
pub fn set_mod_enabled(game_dir: &Path, file_name: &str, enabled: bool) -> Result<String> {
    check_plain_name(file_name)?;
    let dir = game_dir.join("mods");
    let src = dir.join(file_name);
    if !src.is_file() {
        return Err(Error::NotFound(format!("mod file {file_name}")));
    }
    let is_disabled = file_name.to_lowercase().ends_with(DISABLED_SUFFIX);
    if is_disabled == !enabled {
        return Ok(file_name.to_string());
    }
    let new_name = if enabled {
        file_name[..file_name.len() - DISABLED_SUFFIX.len()].to_string()
    } else {
        format!("{file_name}{DISABLED_SUFFIX}")
    };
    let dest = dir.join(&new_name);
    if dest.exists() {
        return Err(Error::Other(format!("A file named {new_name} already exists in the mods folder.")));
    }
    fs::rename(&src, &dest).map_err(|e| Error::io(src, e))?;
    Ok(new_name)
}

/// Moves a mod to `<game_dir>/.trash/mods/` (never a hard delete). Returns the new location.
pub fn remove_mod(game_dir: &Path, file_name: &str) -> Result<PathBuf> {
    check_plain_name(file_name)?;
    let src = game_dir.join("mods").join(file_name);
    if !src.is_file() {
        return Err(Error::NotFound(format!("mod file {file_name}")));
    }
    move_to_trash(&src, &game_dir.join(".trash").join("mods"), file_name)
}

/// Moves a resource pack or shader pack (zip or folder) to `<game_dir>/.trash/<folder>/`, never a hard delete.
/// A pack the game has selected is taken out of its options first, so the game doesn't look for a file that is gone.
/// Returns the new location.
pub fn remove_pack(game_dir: &Path, kind: PackKind, file_name: &str) -> Result<PathBuf> {
    check_plain_name(file_name)?;
    let folder = match kind {
        PackKind::Resource => "resourcepacks",
        PackKind::Shader => "shaderpacks",
    };
    let src = game_dir.join(folder).join(file_name);
    if !src.exists() {
        return Err(Error::NotFound(format!("pack {file_name}")));
    }
    match kind {
        PackKind::Shader => {
            if selected_shader_pack(game_dir).as_deref() == Some(file_name) {
                set_shader_pack(game_dir, None)?;
            }
        }
        PackKind::Resource => {
            let enabled = enabled_resource_packs(game_dir);
            if enabled.iter().any(|n| n == file_name) {
                let rest: Vec<String> = enabled.into_iter().filter(|n| n != file_name).collect();
                set_resource_packs(game_dir, &rest)?;
            }
        }
    }
    move_to_trash(&src, &game_dir.join(".trash").join(folder), file_name)
}

fn read_entry(zip: &mut zip::ZipArchive<fs::File>, name: &str) -> Option<String> {
    let entry = zip.by_name(name).ok()?;
    if entry.size() > MAX_META_BYTES {
        return None;
    }
    let mut buf = Vec::new();
    entry.take(MAX_META_BYTES).read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    Some(text.strip_prefix('\u{feff}').map(str::to_owned).unwrap_or(text))
}

fn read_mod_meta(path: &Path) -> Option<ModMeta> {
    let mut zip = zip::ZipArchive::new(fs::File::open(path).ok()?).ok()?;
    if let Some(text) = read_entry(&mut zip, "fabric.mod.json") {
        if let Some(m) = parse_fabric(&text) {
            return Some(m);
        }
    }
    if let Some(text) = read_entry(&mut zip, "quilt.mod.json") {
        if let Some(m) = parse_quilt(&text) {
            return Some(m);
        }
    }
    for (file, kind) in [("META-INF/neoforge.mods.toml", LoaderKind::NeoForge), ("META-INF/mods.toml", LoaderKind::Forge)] {
        if let Some(text) = read_entry(&mut zip, file) {
            let manifest = read_entry(&mut zip, "META-INF/MANIFEST.MF");
            if let Some(m) = parse_forge_toml(&text, kind, manifest.as_deref()) {
                return Some(m);
            }
        }
    }
    None
}

fn json_str(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|s| s.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

fn parse_fabric(text: &str) -> Option<ModMeta> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let id = json_str(&v, "id")?;
    let authors = v
        .get("authors")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| match x {
                    serde_json::Value::String(s) => Some(s.clone()),
                    o => json_str(o, "name"),
                })
                .collect()
        })
        .unwrap_or_default();
    Some(ModMeta {
        name: json_str(&v, "name").unwrap_or_else(|| id.clone()),
        version: json_str(&v, "version"),
        description: json_str(&v, "description"),
        authors,
        id,
        loader_hint: LoaderKind::Fabric,
    })
}

fn parse_quilt(text: &str) -> Option<ModMeta> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let loader = v.get("quilt_loader")?;
    let id = json_str(loader, "id")?;
    let meta = loader.get("metadata");
    let authors = meta
        .and_then(|m| m.get("contributors"))
        .and_then(|c| c.as_object())
        .map(|c| c.keys().cloned().collect())
        .unwrap_or_default();
    Some(ModMeta {
        name: meta.and_then(|m| json_str(m, "name")).unwrap_or_else(|| id.clone()),
        version: json_str(loader, "version"),
        description: meta.and_then(|m| json_str(m, "description")),
        authors,
        id,
        loader_hint: LoaderKind::Quilt,
    })
}

fn toml_str(v: &toml::Value, key: &str) -> Option<String> {
    v.get(key).and_then(|s| s.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

fn manifest_value(manifest: &str, key: &str) -> Option<String> {
    manifest.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim().eq_ignore_ascii_case(key)).then(|| v.trim().to_string()).filter(|v| !v.is_empty())
    })
}

fn parse_forge_toml(text: &str, kind: LoaderKind, manifest: Option<&str>) -> Option<ModMeta> {
    let root: toml::Value = toml::from_str(text).ok()?;
    let entry = root.get("mods")?.as_array()?.first()?;
    let id = toml_str(entry, "modId")?;
    let version = toml_str(entry, "version").and_then(|v| {
        if v.contains("${") {
            manifest.and_then(|m| manifest_value(m, "Implementation-Version"))
        } else {
            Some(v)
        }
    });
    let authors_raw = toml_str(entry, "authors").or_else(|| toml_str(&root, "authors"));
    let authors = authors_raw
        .map(|a| a.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    Some(ModMeta {
        name: toml_str(entry, "displayName").unwrap_or_else(|| id.clone()),
        description: toml_str(entry, "description"),
        version,
        authors,
        id,
        loader_hint: kind,
    })
}

// packs

/// Kind of pack folder to list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PackKind {
    /// `resourcepacks/`
    Resource,
    /// `shaderpacks/`
    Shader,
}

/// A resource pack or shader pack (zip or folder).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    /// File or folder name on disk (this is what the game stores in its options).
    pub file_name: String,
    /// Display name: the file name without a `.zip` extension.
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<DateTime<Utc>>,
    /// Resource packs: listed in `options.txt`. Shader packs: selected in `optionsshaders.txt`.
    pub enabled: bool,
}

fn enabled_resource_list(game_dir: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(game_dir.join("options.txt")) else { return Vec::new() };
    let Some(line) = text.lines().find_map(|l| l.strip_prefix("resourcePacks:")) else { return Vec::new() };
    serde_json::from_str(line.trim()).unwrap_or_default()
}

/// Enabled resource packs (file names), highest priority first. The last `options.txt` entry wins in game.
fn enabled_resource_packs(game_dir: &Path) -> Vec<String> {
    enabled_resource_list(game_dir)
        .into_iter()
        .rev()
        .map(|p| p.strip_prefix("file/").map(str::to_owned).unwrap_or(p))
        .collect()
}

/// Returns `text` with the line whose key (before `sep`) is `key` replaced by `key<sep>value`, or the line
/// appended when absent. Every other line, and the replaced line's terminator, are kept byte for byte.
fn set_line(text: &str, key: &str, sep: char, value: &str) -> String {
    let new_line = format!("{key}{sep}{value}");
    let mut out = String::with_capacity(text.len() + new_line.len() + 2);
    let mut found = false;
    for raw in text.split_inclusive('\n') {
        let body = raw.trim_end_matches(['\n', '\r']);
        let is_key = !found && body.split_once(sep).is_some_and(|(k, _)| k.trim() == key);
        if is_key {
            found = true;
            out.push_str(&new_line);
            out.push_str(&raw[body.len()..]);
        } else {
            out.push_str(raw);
        }
    }
    if !found {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&new_line);
        out.push('\n');
    }
    out
}

/// Reads a value from `key=value` / `key:value` text (first match, trimmed).
fn get_line<'a>(text: &'a str, key: &str, sep: char) -> Option<&'a str> {
    text.lines().find_map(|l| {
        let (k, v) = l.split_once(sep)?;
        (k.trim() == key).then(|| v.trim())
    })
}

/// Sets the resource packs enabled in `options.txt`, highest priority first (`enabled_top_first[0]` wins).
///
/// Non-file entries already in the list (`vanilla`, `fabric`, ...) are kept at the front; every other line of
/// `options.txt` is left untouched. The file is created if missing.
pub fn set_resource_packs(game_dir: &Path, enabled_top_first: &[String]) -> Result<()> {
    for name in enabled_top_first {
        check_plain_name(name)?;
    }
    let path = game_dir.join("options.txt");
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(Error::io(&path, e)),
    };
    let mut list: Vec<String> = enabled_resource_list(game_dir).into_iter().filter(|p| !p.starts_with("file/")).collect();
    if list.is_empty() && !text.lines().any(|l| l.starts_with("resourcePacks:")) {
        list.push("vanilla".into());
    }
    list.extend(enabled_top_first.iter().rev().map(|n| format!("file/{n}")));
    let json = serde_json::to_string(&list).map_err(|e| Error::Other(e.to_string()))?;
    atomic_write(&path, set_line(&text, "resourcePacks", ':', &json).as_bytes())
}

/// Selects a shader pack (`None` disables shaders) in Iris's `config/iris.properties` and, when present,
/// OptiFine's `optionsshaders.txt`.
pub fn set_shader_pack(game_dir: &Path, file_name: Option<&str>) -> Result<()> {
    if let Some(name) = file_name {
        check_plain_name(name)?;
    }
    let iris = game_dir.join("config").join("iris.properties");
    let text = match fs::read_to_string(&iris) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(Error::io(&iris, e)),
    };
    let text = match file_name {
        Some(name) => set_line(&set_line(&text, "shaderPack", '=', name), "enableShaders", '=', "true"),
        None => set_line(&text, "enableShaders", '=', "false"),
    };
    atomic_write(&iris, text.as_bytes())?;

    let optifine = game_dir.join("optionsshaders.txt");
    if let Ok(text) = fs::read_to_string(&optifine) {
        atomic_write(&optifine, set_line(&text, "shaderPack", '=', file_name.unwrap_or("OFF")).as_bytes())?;
    }
    Ok(())
}

fn selected_shader_pack(game_dir: &Path) -> Option<String> {
    if let Ok(text) = fs::read_to_string(game_dir.join("config").join("iris.properties")) {
        let enabled = get_line(&text, "enableShaders", '=') != Some("false");
        if let Some(pack) = get_line(&text, "shaderPack", '=').filter(|s| enabled && !s.is_empty()) {
            return Some(pack.to_string());
        }
    }
    let text = fs::read_to_string(game_dir.join("optionsshaders.txt")).ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("shaderPack="))
        .map(|s| s.trim().trim_end_matches("(internal)").to_string())
        .filter(|s| !s.is_empty())
}

/// Lists `resourcepacks/` or `shaderpacks/` (zip files and folders). Shaders are sorted by name; resource packs
/// list the enabled ones first (highest priority first), then the disabled ones by name.
pub fn list_packs(game_dir: &Path, kind: PackKind) -> Vec<PackFile> {
    let (folder, enabled_order, selected) = match kind {
        PackKind::Resource => ("resourcepacks", enabled_resource_packs(game_dir), None),
        PackKind::Shader => ("shaderpacks", Vec::new(), selected_shader_pack(game_dir)),
    };
    let Ok(rd) = fs::read_dir(game_dir.join(folder)) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = meta.is_dir();
        if file_name.starts_with('.') || !(is_dir || file_name.to_lowercase().ends_with(".zip")) {
            continue;
        }
        let name = if is_dir { file_name.clone() } else { file_name[..file_name.len() - 4].to_string() };
        let enabled = match kind {
            PackKind::Resource => enabled_order.contains(&file_name),
            PackKind::Shader => selected.as_deref() == Some(file_name.as_str()),
        };
        out.push(PackFile {
            size: if is_dir { tree_size(&entry.path()) } else { meta.len() },
            modified: meta.modified().ok().map(DateTime::<Utc>::from),
            file_name,
            name,
            is_dir,
            enabled,
        });
    }
    out.sort_by_key(|p| (!p.enabled, enabled_order.iter().position(|e| *e == p.file_name), p.file_name.to_lowercase()));
    out
}

// worlds

/// A world in `saves/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldInfo {
    pub folder_name: String,
    /// `LevelName` from `level.dat`, or the folder name.
    pub display_name: String,
    /// Last time the world was played (`LastPlayed` in `level.dat`, else the folder's modification time).
    pub last_played: Option<DateTime<Utc>>,
    pub size: u64,
    pub has_icon: bool,
}

/// Lists `saves/*/` (folders containing a `level.dat` or an `icon.png`), most recently played first.
pub fn list_worlds(game_dir: &Path) -> Vec<WorldInfo> {
    let Ok(rd) = fs::read_dir(game_dir.join("saves")) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let level_dat = path.join("level.dat");
        let has_icon = path.join("icon.png").is_file();
        if !level_dat.is_file() && !has_icon {
            continue;
        }
        let folder_name = entry.file_name().to_string_lossy().into_owned();
        let data = nbt::read_file(&level_dat).and_then(|root| root.get("Data").cloned());
        let display_name = data
            .as_ref()
            .and_then(|d| d.get("LevelName"))
            .and_then(|n| n.as_str())
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(String::from)
            .unwrap_or_else(|| folder_name.clone());
        let last_played = data
            .as_ref()
            .and_then(|d| d.get("LastPlayed"))
            .and_then(|t| t.as_i64())
            .filter(|ms| *ms > 0)
            .and_then(DateTime::<Utc>::from_timestamp_millis)
            .or_else(|| mtime(&level_dat))
            .or_else(|| mtime(&path));
        out.push(WorldInfo { size: tree_size(&path), folder_name, display_name, last_played, has_icon });
    }
    out.sort_by(|a, b| b.last_played.cmp(&a.last_played).then_with(|| a.folder_name.cmp(&b.folder_name)));
    out
}

// screenshots

/// A screenshot in `screenshots/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotFile {
    pub file_name: String,
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<DateTime<Utc>>,
}

/// Lists the `.png` files of `screenshots/`, newest first.
pub fn list_screenshots(game_dir: &Path) -> Vec<ScreenshotFile> {
    let Ok(rd) = fs::read_dir(game_dir.join("screenshots")) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if !meta.is_file() || !file_name.to_lowercase().ends_with(".png") {
            continue;
        }
        out.push(ScreenshotFile {
            path: entry.path(),
            size: meta.len(),
            modified: meta.modified().ok().map(DateTime::<Utc>::from),
            file_name,
        });
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| b.file_name.cmp(&a.file_name)));
    out
}

/// Deletes `<game_dir>/screenshots/<file_name>`.
pub fn delete_screenshot(game_dir: &Path, file_name: &str) -> Result<()> {
    check_plain_name(file_name)?;
    let path = game_dir.join("screenshots").join(file_name);
    if !path.is_file() {
        return Err(Error::NotFound(format!("screenshot {file_name}")));
    }
    fs::remove_file(&path).map_err(|e| Error::io(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nbt::tests::level_dat_bytes;
    use crate::testutil::{temp_dir, write_zip};
    use std::io::Write;

    fn mods_dir(name: &str) -> (PathBuf, PathBuf) {
        let game = temp_dir(name);
        let mods = game.join("mods");
        fs::create_dir_all(&mods).unwrap();
        (game, mods)
    }

    #[test]
    fn mod_metadata_all_loaders() {
        let (game, mods) = mods_dir("mods-meta");
        write_zip(
            &mods.join("fab.jar"),
            &[(
                "fabric.mod.json",
                r#"{"schemaVersion":1,"id":"sodium","name":"Sodium","version":"0.5.3","description":"Fast","authors":["jellysquid3",{"name":"IMS"}]}"#,
            )],
        );
        write_zip(
            &mods.join("quilt.jar"),
            &[(
                "quilt.mod.json",
                r#"{"quilt_loader":{"id":"qsl","version":"1.2","metadata":{"name":"QSL","description":"lib","contributors":{"Quilt":"Owner"}}}}"#,
            )],
        );
        write_zip(
            &mods.join("forge.jar"),
            &[
                (
                    "META-INF/mods.toml",
                    "authors=\"Alice, Bob\"\n[[mods]]\nmodId=\"jei\"\nversion=\"${file.jarVersion}\"\ndisplayName=\"JEI\"\ndescription='''Items'''\n",
                ),
                ("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\nImplementation-Version: 15.2.0\n"),
            ],
        );
        write_zip(
            &mods.join("neo.jar.disabled"),
            &[("META-INF/neoforge.mods.toml", "[[mods]]\nmodId=\"neo\"\nversion=\"2.0\"\n")],
        );
        write_zip(&mods.join("plain.jar"), &[("hello.txt", "hi")]);
        fs::write(mods.join("corrupt.jar"), "not a zip").unwrap();
        fs::write(mods.join("notes.txt"), "ignored").unwrap();
        fs::create_dir(mods.join("subdir")).unwrap();

        let list = list_mods(&game);
        let names: Vec<_> = list.iter().map(|m| m.file_name.as_str()).collect();
        assert_eq!(names, ["corrupt.jar", "fab.jar", "forge.jar", "neo.jar.disabled", "plain.jar", "quilt.jar"]);
        let by = |n: &str| list.iter().find(|m| m.file_name == n).unwrap();

        let fab = by("fab.jar").meta.clone().unwrap();
        assert_eq!((fab.id.as_str(), fab.name.as_str(), fab.version.as_deref()), ("sodium", "Sodium", Some("0.5.3")));
        assert_eq!(fab.authors, ["jellysquid3", "IMS"]);
        assert_eq!(fab.loader_hint, LoaderKind::Fabric);

        let q = by("quilt.jar").meta.clone().unwrap();
        assert_eq!((q.id.as_str(), q.loader_hint, q.authors.clone()), ("qsl", LoaderKind::Quilt, vec!["Quilt".to_string()]));

        let f = by("forge.jar").meta.clone().unwrap();
        assert_eq!(f.version.as_deref(), Some("15.2.0"));
        assert_eq!(f.authors, ["Alice", "Bob"]);
        assert_eq!(f.loader_hint, LoaderKind::Forge);
        assert_eq!(f.description.as_deref(), Some("Items"));

        let n = by("neo.jar.disabled");
        assert!(!n.enabled);
        assert_eq!(n.meta.as_ref().unwrap().loader_hint, LoaderKind::NeoForge);
        assert_eq!(n.meta.as_ref().unwrap().name, "neo");

        assert!(by("plain.jar").meta.is_none());
        assert!(by("corrupt.jar").meta.is_none());
        assert!(by("plain.jar").enabled);
        assert!(by("plain.jar").size > 0 && by("plain.jar").modified.is_some());
    }

    #[test]
    fn disabled_toggle_and_remove() {
        let (game, mods) = mods_dir("mods-toggle");
        fs::write(mods.join("a.jar"), "x").unwrap();
        assert_eq!(set_mod_enabled(&game, "a.jar", false).unwrap(), "a.jar.disabled");
        assert!(mods.join("a.jar.disabled").exists() && !mods.join("a.jar").exists());
        assert_eq!(set_mod_enabled(&game, "a.jar.disabled", false).unwrap(), "a.jar.disabled");
        assert_eq!(set_mod_enabled(&game, "a.jar.disabled", true).unwrap(), "a.jar");
        assert!(mods.join("a.jar").exists());
        assert!(set_mod_enabled(&game, "missing.jar", true).is_err());
        assert!(set_mod_enabled(&game, "../a.jar", true).is_err());

        let moved = remove_mod(&game, "a.jar").unwrap();
        assert!(moved.exists() && !mods.join("a.jar").exists());
        assert!(list_mods(&game).is_empty());
        assert!(remove_mod(&game, "a.jar").is_err());
    }

    #[test]
    fn packs() {
        let game = temp_dir("packs");
        let rp = game.join("resourcepacks");
        fs::create_dir_all(rp.join("Folder Pack")).unwrap();
        fs::write(rp.join("Folder Pack/pack.mcmeta"), "{}").unwrap();
        fs::write(rp.join("Faithful.zip"), "zip").unwrap();
        fs::write(rp.join("other.txt"), "no").unwrap();
        fs::write(game.join("options.txt"), "lang:fr_fr\nresourcePacks:[\"vanilla\",\"file/Faithful.zip\"]\n").unwrap();
        let sp = game.join("shaderpacks");
        fs::create_dir_all(&sp).unwrap();
        fs::write(sp.join("BSL.zip"), "zip").unwrap();
        fs::write(sp.join("Comp.zip"), "zip").unwrap();
        fs::write(game.join("optionsshaders.txt"), "shaderPack=BSL.zip\n").unwrap();

        let r = list_packs(&game, PackKind::Resource);
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].file_name.as_str(), r[0].name.as_str(), r[0].enabled), ("Faithful.zip", "Faithful", true));
        assert_eq!((r[1].name.as_str(), r[1].is_dir, r[1].enabled, r[1].size), ("Folder Pack", true, false, 2));
        let s = list_packs(&game, PackKind::Shader);
        assert_eq!(s.iter().map(|p| (p.name.as_str(), p.enabled)).collect::<Vec<_>>(), [("BSL", true), ("Comp", false)]);
        assert!(list_packs(&temp_dir("packs-empty"), PackKind::Resource).is_empty());
    }

    #[test]
    fn removing_packs_deselects_them() {
        let game = temp_dir("remove-packs");
        let sp = game.join("shaderpacks");
        fs::create_dir_all(&sp).unwrap();
        fs::write(sp.join("BSL.zip"), "zip").unwrap();
        fs::write(sp.join("Comp.zip"), "zip").unwrap();
        fs::write(game.join("optionsshaders.txt"), "shaderPack=BSL.zip\n").unwrap();
        let rp = game.join("resourcepacks");
        fs::create_dir_all(rp.join("Folder Pack")).unwrap();
        fs::write(rp.join("Faithful.zip"), "zip").unwrap();
        fs::write(game.join("options.txt"), "lang:fr_fr\nresourcePacks:[\"vanilla\",\"file/Faithful.zip\"]\n").unwrap();

        // the selected shader pack goes to the trash and is deselected
        let moved = remove_pack(&game, PackKind::Shader, "BSL.zip").unwrap();
        assert!(moved.exists() && !sp.join("BSL.zip").exists());
        assert!(list_packs(&game, PackKind::Shader).iter().all(|p| !p.enabled));
        assert!(sp.join("Comp.zip").exists());
        // an active resource pack leaves the list; a folder pack moves whole
        remove_pack(&game, PackKind::Resource, "Faithful.zip").unwrap();
        // built-in entries such as "vanilla" stay
        assert!(!enabled_resource_packs(&game).iter().any(|n| n == "Faithful.zip"));
        assert!(remove_pack(&game, PackKind::Resource, "Folder Pack").unwrap().is_dir());
        // nothing else: missing files and names with a path are refused
        assert!(remove_pack(&game, PackKind::Shader, "BSL.zip").is_err());
        assert!(remove_pack(&game, PackKind::Shader, "../options.txt").is_err());
    }

    #[test]
    fn worlds() {
        let game = temp_dir("worlds");
        let saves = game.join("saves");
        let w1 = saves.join("New World");
        fs::create_dir_all(&w1).unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&level_dat_bytes("Ma Belle Partie", 1_700_000_000_000)).unwrap();
        fs::write(w1.join("level.dat"), gz.finish().unwrap()).unwrap();
        fs::write(w1.join("icon.png"), "png").unwrap();
        let w2 = saves.join("Broken");
        fs::create_dir_all(&w2).unwrap();
        fs::write(w2.join("level.dat"), "garbage").unwrap();
        fs::create_dir_all(saves.join("not-a-world")).unwrap();
        fs::write(saves.join("file.txt"), "x").unwrap();

        let list = list_worlds(&game);
        assert_eq!(list.len(), 2);
        let w = list.iter().find(|w| w.folder_name == "New World").unwrap();
        assert_eq!(w.display_name, "Ma Belle Partie");
        assert!(w.has_icon && w.size > 0);
        assert_eq!(w.last_played.unwrap().timestamp_millis(), 1_700_000_000_000);
        let b = list.iter().find(|w| w.folder_name == "Broken").unwrap();
        assert_eq!(b.display_name, "Broken");
        assert!(!b.has_icon);
        assert!(b.last_played.is_some());
    }

    #[test]
    fn screenshots_newest_first() {
        let game = temp_dir("shots");
        let dir = game.join("screenshots");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.png"), "1").unwrap();
        fs::write(dir.join("b.txt"), "1").unwrap();
        let f = fs::File::create(dir.join("b.png")).unwrap();
        f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(60)).unwrap();
        let list = list_screenshots(&game);
        assert_eq!(list.iter().map(|s| s.file_name.as_str()).collect::<Vec<_>>(), ["b.png", "a.png"]);
        assert!(list_screenshots(&temp_dir("shots-none")).is_empty());
    }

    #[test]
    fn resource_pack_order_and_rewrite() {
        let game = temp_dir("rp-order");
        let rp = game.join("resourcepacks");
        for n in ["A.zip", "B.zip", "C.zip", "D.zip"] {
            fs::create_dir_all(&rp).unwrap();
            fs::write(rp.join(n), "z").unwrap();
        }
        let opts = "lang:fr_fr\r\nresourcePacks:[\"vanilla\",\"fabric\",\"file/C.zip\",\"file/A.zip\"]\r\nfov:0.5\r\n";
        fs::write(game.join("options.txt"), opts).unwrap();
        let names = |g: &Path| list_packs(g, PackKind::Resource).iter().map(|p| (p.file_name.clone(), p.enabled)).collect::<Vec<_>>();
        // Last entry (A) has the highest priority.
        assert_eq!(
            names(&game),
            [("A.zip".into(), true), ("C.zip".into(), true), ("B.zip".into(), false), ("D.zip".into(), false)]
        );
        set_resource_packs(&game, &["D.zip".into(), "B.zip".into()]).unwrap();
        let text = fs::read_to_string(game.join("options.txt")).unwrap();
        assert_eq!(
            text,
            "lang:fr_fr\r\nresourcePacks:[\"vanilla\",\"fabric\",\"file/B.zip\",\"file/D.zip\"]\r\nfov:0.5\r\n"
        );
        assert_eq!(names(&game)[0], ("D.zip".into(), true));
        assert_eq!(names(&game)[1], ("B.zip".into(), true));
        assert!(set_resource_packs(&game, &["../x.zip".into()]).is_err());
        assert!(set_resource_packs(&game, &["a/b.zip".into()]).is_err());

        let fresh = temp_dir("rp-fresh");
        set_resource_packs(&fresh, &["A.zip".into()]).unwrap();
        assert_eq!(fs::read_to_string(fresh.join("options.txt")).unwrap(), "resourcePacks:[\"vanilla\",\"file/A.zip\"]\n");
        let appended = temp_dir("rp-append");
        fs::write(appended.join("options.txt"), "lang:en_us").unwrap();
        set_resource_packs(&appended, &[]).unwrap();
        assert_eq!(fs::read_to_string(appended.join("options.txt")).unwrap(), "lang:en_us\nresourcePacks:[\"vanilla\"]\n");
    }

    #[test]
    fn shader_selection() {
        let game = temp_dir("shader-sel");
        let sp = game.join("shaderpacks");
        fs::create_dir_all(&sp).unwrap();
        fs::write(sp.join("BSL.zip"), "z").unwrap();
        fs::write(sp.join("Comp.zip"), "z").unwrap();
        let sel = |g: &Path| list_packs(g, PackKind::Shader).iter().filter(|p| p.enabled).map(|p| p.file_name.clone()).collect::<Vec<_>>();
        assert!(sel(&game).is_empty());

        fs::create_dir_all(game.join("config")).unwrap();
        fs::write(game.join("config/iris.properties"), "#c\ncolumns=2\n").unwrap();
        fs::write(game.join("optionsshaders.txt"), "shaderPack=OFF\nother=1\n").unwrap();
        set_shader_pack(&game, Some("BSL.zip")).unwrap();
        let iris = fs::read_to_string(game.join("config/iris.properties")).unwrap();
        assert_eq!(iris, "#c\ncolumns=2\nshaderPack=BSL.zip\nenableShaders=true\n");
        assert_eq!(fs::read_to_string(game.join("optionsshaders.txt")).unwrap(), "shaderPack=BSL.zip\nother=1\n");
        assert_eq!(sel(&game), ["BSL.zip"]);

        set_shader_pack(&game, Some("Comp.zip")).unwrap();
        assert_eq!(sel(&game), ["Comp.zip"]);
        set_shader_pack(&game, None).unwrap();
        let iris = fs::read_to_string(game.join("config/iris.properties")).unwrap();
        assert!(iris.contains("shaderPack=Comp.zip") && iris.contains("enableShaders=false"), "{iris}");
        assert_eq!(fs::read_to_string(game.join("optionsshaders.txt")).unwrap(), "shaderPack=OFF\nother=1\n");
        assert!(sel(&game).is_empty());
        assert!(set_shader_pack(&game, Some("../x")).is_err());
    }

    #[test]
    fn delete_screenshot_checks() {
        let game = temp_dir("shot-del");
        let dir = game.join("screenshots");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.png"), "1").unwrap();
        fs::write(game.join("keep.png"), "1").unwrap();
        assert!(delete_screenshot(&game, "../keep.png").is_err());
        assert!(game.join("keep.png").exists());
        delete_screenshot(&game, "a.png").unwrap();
        assert!(!dir.join("a.png").exists());
        assert!(matches!(delete_screenshot(&game, "a.png"), Err(Error::NotFound(_))));
    }
}

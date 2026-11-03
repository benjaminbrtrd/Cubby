//! Import of Prism Launcher instances (`instance.cfg`, `mmc-pack.json`, `instgroups.json`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use cubby_common::{Error, Result};
use serde::Serialize;

use crate::model::{Instance, JavaOverrides, Loader, WindowSize};
use crate::store::InstanceStore;
use crate::util::copy_dir_all;

/// Summary of a Prism instance found on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrismInstanceInfo {
    /// Folder name inside Prism's `instances` directory.
    pub folder_name: String,
    pub path: PathBuf,
    pub name: String,
    pub minecraft: Option<String>,
    pub loader: Loader,
    pub group: Option<String>,
    pub last_played: Option<DateTime<Utc>>,
    pub total_played_secs: u64,
}

/// Everything we read from one Prism instance folder.
struct PrismData {
    name: String,
    icon: Option<String>,
    minecraft: Option<String>,
    loader: Loader,
    group: Option<String>,
    last_played: Option<DateTime<Utc>>,
    total_played_secs: u64,
    java: JavaOverrides,
    window: Option<WindowSize>,
}

/// Parses the `[General]` section of a Prism `instance.cfg` (a Qt INI file) into a key/value map.
fn parse_ini(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut in_general = true;
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(section) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_general = section.eq_ignore_ascii_case("General");
            continue;
        }
        if !in_general {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim();
            let v = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(v);
            out.insert(k.trim().to_string(), v.to_string());
        }
    }
    out
}

fn flag(cfg: &HashMap<String, String>, key: &str) -> Option<bool> {
    cfg.get(key).map(|v| v.eq_ignore_ascii_case("true"))
}

/// A value is used unless its `Override...` flag is explicitly false.
fn overridden<'a>(cfg: &'a HashMap<String, String>, flag_key: &str, value_key: &str) -> Option<&'a str> {
    if flag(cfg, flag_key) == Some(false) {
        return None;
    }
    cfg.get(value_key).map(String::as_str).map(str::trim).filter(|v| !v.is_empty())
}

fn read_components(dir: &Path) -> (Option<String>, Loader) {
    let Ok(text) = fs::read_to_string(dir.join("mmc-pack.json")) else { return (None, Loader::Vanilla) };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return (None, Loader::Vanilla) };
    let mut minecraft = None;
    let mut loader = Loader::Vanilla;
    for c in v.get("components").and_then(|c| c.as_array()).into_iter().flatten() {
        let uid = c.get("uid").and_then(|u| u.as_str()).unwrap_or_default();
        let version = c.get("version").and_then(|u| u.as_str()).unwrap_or_default().to_string();
        match uid {
            "net.minecraft" => minecraft = Some(version),
            "net.fabricmc.fabric-loader" => loader = Loader::Fabric { version },
            "org.quiltmc.quilt-loader" => loader = Loader::Quilt { version },
            "net.neoforged" => loader = Loader::NeoForge { version },
            "net.minecraftforge" => loader = Loader::Forge { version },
            _ => {}
        }
    }
    (minecraft, loader)
}

fn read_group(dir: &Path) -> Option<String> {
    let folder = dir.file_name()?.to_string_lossy().into_owned();
    let text = fs::read_to_string(dir.parent()?.join("instgroups.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("groups")?.as_object()?.iter().find_map(|(group, info)| {
        let listed = info.get("instances")?.as_array()?.iter().any(|i| i.as_str() == Some(folder.as_str()));
        (listed && !group.is_empty()).then(|| group.clone())
    })
}

fn read_prism(dir: &Path) -> Result<PrismData> {
    let cfg_path = dir.join("instance.cfg");
    let text = match fs::read_to_string(&cfg_path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::NotFound(format!("instance.cfg in {}", dir.display())))
        }
        Err(e) => return Err(Error::io(cfg_path, e)),
    };
    let cfg = parse_ini(&text);
    let (minecraft, loader) = read_components(dir);
    let folder = dir.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let name = cfg.get("name").map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or(folder);
    let num = |key: &str| cfg.get(key).and_then(|v| v.trim().parse::<i64>().ok());
    let mem = |key: &str| overridden(&cfg, "OverrideMemory", key).and_then(|v| v.parse::<u32>().ok());
    let window = if flag(&cfg, "OverrideWindow") == Some(true) {
        match (num("MinecraftWinWidth"), num("MinecraftWinHeight")) {
            (Some(w), Some(h)) if w > 0 && h > 0 => Some(WindowSize { width: w as u32, height: h as u32 }),
            _ => None,
        }
    } else {
        None
    };
    Ok(PrismData {
        name,
        icon: cfg.get("iconKey").map(|k| k.trim().to_string()).filter(|k| !k.is_empty() && k != "default"),
        minecraft,
        loader,
        group: read_group(dir),
        last_played: num("lastLaunchTime").filter(|ms| *ms > 0).and_then(DateTime::<Utc>::from_timestamp_millis),
        total_played_secs: num("totalTimePlayed").map(|s| s.max(0) as u64).unwrap_or(0),
        java: JavaOverrides {
            executable: overridden(&cfg, "OverrideJavaLocation", "JavaPath").map(PathBuf::from),
            min_memory_mb: mem("MinMemAlloc"),
            max_memory_mb: mem("MaxMemAlloc"),
            jvm_args: overridden(&cfg, "OverrideJavaArgs", "JvmArgs").map(String::from),
        },
        window,
    })
}

/// Prism keeps game files in `minecraft/` (current) or `.minecraft/` (older versions).
fn prism_game_dir(dir: &Path) -> Option<PathBuf> {
    ["minecraft", ".minecraft"].iter().map(|n| dir.join(n)).find(|p| p.is_dir())
}

/// Lists the instances of a Prism data directory. `prism_root` is either Prism's data folder
/// (containing `instances/`) or the `instances` folder itself. Folders without `instance.cfg` are skipped.
pub fn list_prism_instances(prism_root: &Path) -> Result<Vec<PrismInstanceInfo>> {
    let instances = if prism_root.join("instances").is_dir() { prism_root.join("instances") } else { prism_root.to_path_buf() };
    let rd = fs::read_dir(&instances).map_err(|e| Error::io(&instances, e))?;
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if !path.join("instance.cfg").is_file() {
            continue;
        }
        match read_prism(&path) {
            Ok(d) => out.push(PrismInstanceInfo {
                folder_name: entry.file_name().to_string_lossy().into_owned(),
                path,
                name: d.name,
                minecraft: d.minecraft,
                loader: d.loader,
                group: d.group,
                last_played: d.last_played,
                total_played_secs: d.total_played_secs,
            }),
            Err(e) => tracing::warn!("skipping Prism instance {}: {e}", path.display()),
        }
    }
    out.sort_by_key(|i| i.name.to_lowercase());
    Ok(out)
}

/// Imports a Prism instance folder into `store`. The Prism folder is only read.
///
/// With `copy_files` the game folder (`minecraft/` or `.minecraft/`) is copied into the new instance;
/// without it only the settings and metadata are imported and the game folder starts empty.
pub fn import_prism(store: &InstanceStore, prism_instance_dir: &Path, copy_files: bool) -> Result<Instance> {
    let data = read_prism(prism_instance_dir)?;
    let minecraft = data
        .minecraft
        .clone()
        .ok_or_else(|| Error::Other(format!("Can't import {}: its mmc-pack.json doesn't name a Minecraft version.", prism_instance_dir.display())))?;
    let id = store.reserve_id(&data.name)?;
    let inst = Instance {
        id,
        name: data.name,
        group: data.group,
        icon: data.icon,
        minecraft,
        loader: data.loader,
        created: Utc::now(),
        last_played: data.last_played,
        total_played_secs: data.total_played_secs,
        java: data.java,
        window: data.window,
        notes: None,
        modpack: None,
    };
    if copy_files {
        if let Some(src) = prism_game_dir(prism_instance_dir) {
            let dest = store.game_dir(&inst);
            if let Err(e) = copy_dir_all(&src, &dest) {
                let _ = fs::remove_dir_all(store.paths().instance(&inst.id));
                return Err(e);
            }
        }
    }
    store.insert(inst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;
    use cubby_common::paths::Paths;

    fn fixture() -> (PathBuf, PathBuf) {
        let root = temp_dir("prism-fixture");
        let inst = root.join("instances/My Pack");
        fs::create_dir_all(inst.join(".minecraft/mods")).unwrap();
        fs::write(inst.join(".minecraft/mods/a.jar"), "x").unwrap();
        fs::write(
            inst.join("instance.cfg"),
            "[General]\nConfigVersion=1.3\nname=Mon Pack\niconKey=flame\nJavaPath=C:/java/bin/javaw.exe\nOverrideJavaLocation=true\n\
             OverrideMemory=true\nMinMemAlloc=512\nMaxMemAlloc=6144\nOverrideJavaArgs=true\nJvmArgs=-XX:+UseZGC\n\
             OverrideWindow=true\nMinecraftWinWidth=1000\nMinecraftWinHeight=700\n\
             lastLaunchTime=1700000000000\ntotalTimePlayed=3600\n\n[UI]\nmods_Page\\Columns=\"AAA=\"\n",
        )
        .unwrap();
        fs::write(
            inst.join("mmc-pack.json"),
            r#"{"components":[{"uid":"org.lwjgl3","version":"3.3.1"},{"uid":"net.minecraft","version":"1.20.1"},
            {"uid":"net.neoforged","version":"47.1.0"}],"formatVersion":1}"#,
        )
        .unwrap();
        fs::write(
            root.join("instances/instgroups.json"),
            r#"{"formatVersion":"1","groups":{"Modpacks":{"hidden":false,"instances":["My Pack"]}}}"#,
        )
        .unwrap();
        // A junk folder and a second instance with a vanilla layout.
        fs::create_dir_all(root.join("instances/_LAUNCHER_TEMP")).unwrap();
        let v = root.join("instances/Vanilla");
        fs::create_dir_all(v.join("minecraft")).unwrap();
        fs::write(v.join("instance.cfg"), "[General]\nname=Vanilla\nOverrideJavaLocation=false\nJavaPath=C:/x/javaw.exe\n").unwrap();
        fs::write(v.join("mmc-pack.json"), r#"{"components":[{"uid":"net.minecraft","version":"1.21"}]}"#).unwrap();
        (root, inst)
    }

    #[test]
    fn lists_and_imports_fixture() {
        let (root, inst) = fixture();
        let list = list_prism_instances(&root).unwrap();
        assert_eq!(list.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), ["Mon Pack", "Vanilla"]);
        assert_eq!(list[0].group.as_deref(), Some("Modpacks"));
        assert_eq!(list[0].loader, Loader::NeoForge { version: "47.1.0".into() });
        assert_eq!(list[1].loader, Loader::Vanilla);
        assert_eq!(list_prism_instances(&root.join("instances")).unwrap().len(), 2);

        let store = InstanceStore::new(Paths::new(temp_dir("prism-import")));
        let imported = import_prism(&store, &inst, true).unwrap();
        assert_eq!(imported.id, "mon-pack");
        assert_eq!(imported.name, "Mon Pack");
        assert_eq!(imported.minecraft, "1.20.1");
        assert_eq!(imported.group.as_deref(), Some("Modpacks"));
        assert_eq!(imported.icon.as_deref(), Some("flame"));
        assert_eq!(imported.total_played_secs, 3600);
        assert_eq!(imported.last_played.unwrap().timestamp_millis(), 1_700_000_000_000);
        assert_eq!(imported.java.executable, Some(PathBuf::from("C:/java/bin/javaw.exe")));
        assert_eq!((imported.java.min_memory_mb, imported.java.max_memory_mb), (Some(512), Some(6144)));
        assert_eq!(imported.java.jvm_args.as_deref(), Some("-XX:+UseZGC"));
        assert_eq!(imported.window, Some(WindowSize { width: 1000, height: 700 }));
        assert!(store.game_dir(&imported).join("mods/a.jar").exists());
        assert_eq!(store.get("mon-pack").unwrap(), imported);
        // The source is untouched.
        assert!(inst.join(".minecraft/mods/a.jar").exists());

        // Without copying files, and with a disabled java override.
        let v = import_prism(&store, &root.join("instances/Vanilla"), false).unwrap();
        assert_eq!(v.java.executable, None);
        assert_eq!(v.loader, Loader::Vanilla);
        assert!(store.game_dir(&v).is_dir());
        assert!(!store.game_dir(&v).join("mods").exists());
        // Importing twice yields a unique id.
        assert_eq!(import_prism(&store, &inst, false).unwrap().id, "mon-pack-2");
    }

    #[test]
    fn import_errors() {
        let store = InstanceStore::new(Paths::new(temp_dir("prism-err")));
        let empty = temp_dir("prism-empty");
        assert!(matches!(import_prism(&store, &empty, true), Err(Error::NotFound(_))));
        fs::write(empty.join("instance.cfg"), "[General]\nname=x\n").unwrap();
        assert!(import_prism(&store, &empty, true).is_err());
        assert!(store.list().unwrap().is_empty());
        assert!(!store.paths().instance("x").exists());
    }
}

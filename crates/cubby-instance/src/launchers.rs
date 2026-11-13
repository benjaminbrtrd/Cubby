//! Importing instances from other launchers: MultiMC and PolyMC (Prism's format), the CurseForge app,
//! ATLauncher, GDLauncher, the Modrinth App and the official launcher's installations.
//!
//! Their folders are only read. Each instance's game folder is copied into a new Cubby instance.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use cubby_common::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{Instance, JavaOverrides, Loader};
use crate::prism;
use crate::store::InstanceStore;
use crate::util::copy_dir_all;

/// A launcher Cubby can import from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Launcher {
    Prism,
    MultiMc,
    PolyMc,
    CurseForge,
    AtLauncher,
    GdLauncher,
    ModrinthApp,
    Official,
}

impl Launcher {
    pub fn title(self) -> &'static str {
        match self {
            Launcher::Prism => "Prism Launcher",
            Launcher::MultiMc => "MultiMC",
            Launcher::PolyMc => "PolyMC",
            Launcher::CurseForge => "CurseForge",
            Launcher::AtLauncher => "ATLauncher",
            Launcher::GdLauncher => "GDLauncher",
            Launcher::ModrinthApp => "Modrinth App",
            Launcher::Official => "Minecraft Launcher",
        }
    }
}

/// The usual folders, from the system's data folder (`%APPDATA%`, `~/Library/Application Support`,
/// `~/.local/share`), the home folder and the documents folder.
#[derive(Debug, Clone)]
pub struct SystemDirs {
    pub data: PathBuf,
    pub home: PathBuf,
    pub documents: Option<PathBuf>,
}

/// Where each launcher keeps its data, most likely first. Folders that don't exist are left in: callers skip them.
pub fn known_locations(dirs: &SystemDirs) -> Vec<(Launcher, PathBuf)> {
    let d = &dirs.data;
    let mut out = vec![
        (Launcher::Prism, d.join("PrismLauncher")),
        (Launcher::MultiMc, d.join("multimc")),
        (Launcher::MultiMc, d.join("MultiMC")),
        (Launcher::MultiMc, dirs.home.join("MultiMC")),
        (Launcher::PolyMc, d.join("PolyMC")),
        (Launcher::CurseForge, dirs.home.join("curseforge").join("minecraft").join("Instances")),
        (Launcher::AtLauncher, d.join("ATLauncher")),
        (Launcher::AtLauncher, d.join("atlauncher")),
        (Launcher::GdLauncher, d.join("gdlauncher_next")),
        (Launcher::GdLauncher, d.join("gdlauncher_carbon").join("data")),
        (Launcher::ModrinthApp, d.join("ModrinthApp")),
        (Launcher::ModrinthApp, d.join("com.modrinth.theseus")),
        (Launcher::Official, d.join("minecraft")),
        (Launcher::Official, d.join(".minecraft")),
        (Launcher::Official, dirs.home.join(".minecraft")),
    ];
    if let Some(docs) = &dirs.documents {
        out.insert(6, (Launcher::CurseForge, docs.join("curseforge").join("minecraft").join("Instances")));
    }
    out
}

/// An instance of another launcher.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignInstance {
    pub launcher: Launcher,
    /// What identifies it for [`import_foreign`]: the instance folder, or for the official launcher
    /// `launcher_profiles.json#<profile id>`.
    pub path: String,
    pub name: String,
    /// `None` when the launcher doesn't say (newer Modrinth App versions keep it in a database).
    pub minecraft: Option<String>,
    pub loader: Loader,
    pub group: Option<String>,
    pub last_played: Option<DateTime<Utc>>,
    pub total_played_secs: u64,
    /// The folder copied into the new instance.
    pub game_dir: PathBuf,
}

fn read_json(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

fn str_at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut cur = v;
    for key in path {
        cur = cur.get(key)?;
    }
    cur.as_str().map(str::trim).filter(|s| !s.is_empty())
}

fn folder_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// A loader from a kind word ("fabric", "Forge", "neoforge"...) and a version.
fn loader_of(kind: &str, version: &str) -> Loader {
    let version = version.trim().to_string();
    if version.is_empty() {
        return Loader::Vanilla;
    }
    match kind.trim().to_lowercase().replace(['-', '_'], "").as_str() {
        "fabric" | "fabricloader" => Loader::Fabric { version },
        "quilt" | "quiltloader" => Loader::Quilt { version },
        "forge" | "minecraftforge" => Loader::Forge { version },
        "neoforge" | "neoforged" => Loader::NeoForge { version },
        _ => Loader::Vanilla,
    }
}

fn iso_time(s: Option<&str>) -> Option<DateTime<Utc>> {
    s.and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Utc)).filter(|d| d.timestamp() > 0)
}

fn sub_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    out.sort();
    out
}

// CurseForge app: <Instances>/<name>/minecraftinstance.json, the game folder is the instance folder.

/// "forge-47.2.0", "fabric-0.15.11-1.20.1", "neoforge-21.1.5" → a loader.
fn curseforge_loader(id: &str, minecraft: &str) -> Loader {
    let Some((kind, rest)) = id.split_once('-') else { return Loader::Vanilla };
    let version = rest.strip_suffix(&format!("-{minecraft}")).unwrap_or(rest);
    loader_of(kind, version)
}

fn read_curseforge(dir: &Path) -> Option<ForeignInstance> {
    let v = read_json(&dir.join("minecraftinstance.json"))?;
    let minecraft = str_at(&v, &["gameVersion"]).map(String::from);
    let loader = str_at(&v, &["baseModLoader", "name"]).map(|id| curseforge_loader(id, minecraft.as_deref().unwrap_or(""))).unwrap_or(Loader::Vanilla);
    Some(ForeignInstance {
        launcher: Launcher::CurseForge,
        path: dir.to_string_lossy().into_owned(),
        name: str_at(&v, &["name"]).map(String::from).unwrap_or_else(|| folder_name(dir)),
        minecraft,
        loader,
        group: None,
        last_played: iso_time(str_at(&v, &["lastPlayed"])),
        total_played_secs: 0,
        game_dir: dir.to_path_buf(),
    })
}

// ATLauncher: <root>/instances/<name>/instance.json.

fn read_atlauncher(dir: &Path) -> Option<ForeignInstance> {
    let v = read_json(&dir.join("instance.json"))?;
    let launcher = v.get("launcher").unwrap_or(&Value::Null);
    let loader = match (str_at(launcher, &["loaderVersion", "type"]), str_at(launcher, &["loaderVersion", "version"])) {
        (Some(kind), Some(version)) => loader_of(kind, version),
        _ => Loader::Vanilla,
    };
    Some(ForeignInstance {
        launcher: Launcher::AtLauncher,
        path: dir.to_string_lossy().into_owned(),
        name: str_at(launcher, &["name"]).map(String::from).unwrap_or_else(|| folder_name(dir)),
        minecraft: str_at(&v, &["id"]).map(String::from),
        loader,
        group: None,
        last_played: iso_time(str_at(launcher, &["lastPlayed"])),
        total_played_secs: launcher.get("totalPlayTime").and_then(Value::as_u64).map(|ms| ms / 1000).unwrap_or(0),
        game_dir: dir.to_path_buf(),
    })
}

// GDLauncher: the old one keeps <root>/instances/<name>/config.json, GDLauncher Carbon (2.0)
// <root>/instances/<name>/instance.json with the game in instance/.

fn read_gdlauncher(dir: &Path) -> Option<ForeignInstance> {
    if let Some(v) = read_json(&dir.join("config.json")).filter(|v| v.get("loader").is_some()) {
        let l = &v["loader"];
        return Some(ForeignInstance {
            launcher: Launcher::GdLauncher,
            path: dir.to_string_lossy().into_owned(),
            name: folder_name(dir),
            minecraft: str_at(l, &["mcVersion"]).map(String::from),
            loader: loader_of(str_at(l, &["loaderType"]).unwrap_or(""), str_at(l, &["loaderVersion"]).unwrap_or("")),
            group: None,
            last_played: v.get("lastPlayed").and_then(Value::as_i64).filter(|ms| *ms > 0).and_then(DateTime::<Utc>::from_timestamp_millis),
            total_played_secs: v.get("timePlayed").and_then(Value::as_u64).unwrap_or(0),
            game_dir: dir.to_path_buf(),
        });
    }
    let v = read_json(&dir.join("instance.json"))?;
    let version = v.get("game_configuration").and_then(|g| g.get("version")).or_else(|| v.get("version")).unwrap_or(&Value::Null);
    let minecraft = str_at(version, &["release"]).or_else(|| str_at(version, &["Standard", "release"])).map(String::from);
    let modloaders = version.get("modloaders").or_else(|| version.get("Standard").and_then(|s| s.get("modloaders")));
    let loader = modloaders
        .and_then(Value::as_array)
        .and_then(|l| l.first())
        .map(|m| loader_of(str_at(m, &["type_"]).or_else(|| str_at(m, &["type"])).unwrap_or(""), str_at(m, &["version"]).unwrap_or("")))
        .unwrap_or(Loader::Vanilla);
    let game = dir.join("instance");
    Some(ForeignInstance {
        launcher: Launcher::GdLauncher,
        path: dir.to_string_lossy().into_owned(),
        name: str_at(&v, &["name"]).map(String::from).unwrap_or_else(|| folder_name(dir)),
        minecraft,
        loader,
        group: None,
        last_played: iso_time(str_at(&v, &["last_played"])),
        total_played_secs: v.get("seconds_played").and_then(Value::as_u64).unwrap_or(0),
        game_dir: if game.is_dir() { game } else { dir.to_path_buf() },
    })
}

// Modrinth App: <root>/profiles/<name>/, with profile.json in versions before 0.8. Newer versions keep the
// version in a database, so those profiles come without a Minecraft version.

fn read_modrinth_app(dir: &Path) -> Option<ForeignInstance> {
    let v = read_json(&dir.join("profile.json"));
    let meta = v.as_ref().and_then(|v| v.get("metadata"));
    let (minecraft, loader, name) = match meta {
        Some(m) => {
            let version = str_at(m, &["loader_version", "id"]).unwrap_or("");
            let mc = str_at(m, &["game_version"]).map(String::from);
            let version = version.strip_prefix(&format!("{}-", mc.as_deref().unwrap_or("-"))).unwrap_or(version);
            (mc, loader_of(str_at(m, &["loader"]).unwrap_or(""), version), str_at(m, &["name"]).map(String::from))
        }
        None => (None, Loader::Vanilla, None),
    };
    // A profile is a folder with game content in it.
    if meta.is_none() && !["mods", "saves", "options.txt"].iter().any(|n| dir.join(n).exists()) {
        return None;
    }
    Some(ForeignInstance {
        launcher: Launcher::ModrinthApp,
        path: dir.to_string_lossy().into_owned(),
        name: name.unwrap_or_else(|| folder_name(dir)),
        minecraft,
        loader,
        group: None,
        last_played: None,
        total_played_secs: 0,
        game_dir: dir.to_path_buf(),
    })
}

// The official launcher: installations in launcher_profiles.json.

/// "fabric-loader-0.15.11-1.20.1", "quilt-loader-0.26.0-1.20.1", "1.20.1-forge-47.2.0", "neoforge-21.1.5",
/// "1.21.1" → (Minecraft version, loader). `None` for "latest-release" and the like.
pub(crate) fn official_version(id: &str) -> Option<(String, Loader)> {
    let id = id.trim();
    for (prefix, kind) in [("fabric-loader-", "fabric"), ("quilt-loader-", "quilt")] {
        if let Some(rest) = id.strip_prefix(prefix) {
            let (version, mc) = rest.split_once('-')?;
            return Some((mc.to_string(), loader_of(kind, version)));
        }
    }
    if let Some((mc, version)) = id.split_once("-forge-") {
        return Some((mc.to_string(), loader_of("forge", version)));
    }
    if let Some(version) = id.strip_prefix("neoforge-") {
        // NeoForge 21.1.x is for 1.21.1, 20.4.x for 1.20.4; a .0 minor is the plain release.
        let mut parts = version.split('.');
        let (major, minor) = (parts.next()?, parts.next()?);
        let mc = if minor == "0" { format!("1.{major}") } else { format!("1.{major}.{minor}") };
        return Some((mc, loader_of("neoforge", version)));
    }
    if id.is_empty() || id.starts_with("latest-") || id.contains("optifine") || id.contains("OptiFine") {
        return None;
    }
    Some((id.to_string(), Loader::Vanilla))
}

/// Game content worth copying from a shared `.minecraft` (not the launcher's versions, libraries and assets).
const OFFICIAL_CONTENT: [&str; 9] = ["mods", "saves", "resourcepacks", "shaderpacks", "config", "screenshots", "options.txt", "servers.dat", "optionsshaders.txt"];

fn read_official(root: &Path) -> Vec<ForeignInstance> {
    let file = root.join("launcher_profiles.json");
    let Some(v) = read_json(&file) else { return Vec::new() };
    let Some(profiles) = v.get("profiles").and_then(Value::as_object) else { return Vec::new() };
    let mut out = Vec::new();
    for (key, p) in profiles {
        let Some((minecraft, loader)) = str_at(p, &["lastVersionId"]).and_then(official_version) else { continue };
        let game_dir = str_at(p, &["gameDir"]).map(PathBuf::from).unwrap_or_else(|| root.to_path_buf());
        let name = str_at(p, &["name"]).map(String::from).unwrap_or_else(|| format!("Minecraft {minecraft}"));
        out.push(ForeignInstance {
            launcher: Launcher::Official,
            path: format!("{}#{key}", file.to_string_lossy()),
            name,
            minecraft: Some(minecraft),
            loader,
            group: None,
            last_played: iso_time(str_at(p, &["lastUsed"])),
            total_played_secs: 0,
            game_dir,
        });
    }
    out
}

/// The instances of one launcher folder (see [`known_locations`]).
pub fn list_foreign(launcher: Launcher, root: &Path) -> Vec<ForeignInstance> {
    let mut out: Vec<ForeignInstance> = match launcher {
        Launcher::Prism | Launcher::MultiMc | Launcher::PolyMc => prism::list_prism_instances(root)
            .unwrap_or_default()
            .into_iter()
            .map(|p| ForeignInstance {
                launcher,
                game_dir: ["minecraft", ".minecraft"].iter().map(|n| p.path.join(n)).find(|d| d.is_dir()).unwrap_or_else(|| p.path.clone()),
                path: p.path.to_string_lossy().into_owned(),
                name: p.name,
                minecraft: p.minecraft,
                loader: p.loader,
                group: p.group,
                last_played: p.last_played,
                total_played_secs: p.total_played_secs,
            })
            .collect(),
        Launcher::CurseForge => sub_dirs(root).iter().filter_map(|d| read_curseforge(d)).collect(),
        Launcher::AtLauncher => sub_dirs(&root.join("instances")).iter().filter_map(|d| read_atlauncher(d)).collect(),
        Launcher::GdLauncher => sub_dirs(&root.join("instances")).iter().filter_map(|d| read_gdlauncher(d)).collect(),
        Launcher::ModrinthApp => sub_dirs(&root.join("profiles")).iter().filter_map(|d| read_modrinth_app(d)).collect(),
        Launcher::Official => read_official(root),
    };
    out.sort_by_key(|i| i.name.to_lowercase());
    out
}

/// Every instance of every launcher found in the usual folders. Two locations of the same folder are listed once.
pub fn list_all_foreign(dirs: &SystemDirs) -> Vec<ForeignInstance> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (launcher, root) in known_locations(dirs) {
        let Ok(real) = fs::canonicalize(&root) else { continue };
        if !seen.insert(real) {
            continue;
        }
        out.extend(list_foreign(launcher, &root));
    }
    out
}

fn copy_selected(src: &Path, dest: &Path, names: &[&str]) -> Result<()> {
    fs::create_dir_all(dest).map_err(|e| Error::io(dest, e))?;
    for name in names {
        let from = src.join(name);
        if from.is_dir() {
            copy_dir_all(&from, &dest.join(name))?;
        } else if from.is_file() {
            fs::copy(&from, dest.join(name)).map_err(|e| Error::io(&from, e))?;
        }
    }
    Ok(())
}

/// Things other launchers keep in the instance folder that aren't game content.
const SKIPPED: [&str; 9] = ["minecraftinstance.json", "instance.json", "config.json", "profile.json", "natives", "logs", "crash-reports", ".fabric", "instance.cfg"];

/// Copies a game folder, leaving out the other launcher's own files.
fn copy_game(src: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).map_err(|e| Error::io(dest, e))?;
    for entry in fs::read_dir(src).map_err(|e| Error::io(src, e))?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if SKIPPED.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        let Ok(ty) = entry.file_type() else { continue };
        if ty.is_dir() {
            copy_dir_all(&path, &dest.join(&name))?;
        } else if ty.is_file() {
            fs::copy(&path, dest.join(&name)).map_err(|e| Error::io(&path, e))?;
        }
    }
    Ok(())
}

/// Imports an instance listed by [`list_foreign`] into `store`, copying its game folder.
pub fn import_foreign(store: &InstanceStore, found: &ForeignInstance) -> Result<Instance> {
    if matches!(found.launcher, Launcher::Prism | Launcher::MultiMc | Launcher::PolyMc) {
        return prism::import_prism(store, Path::new(&found.path), true);
    }
    let minecraft = found.minecraft.clone().ok_or_else(|| {
        Error::Other(format!("{} doesn't say which Minecraft version {} uses. Export it as a .mrpack from {0}, then import that file.", found.launcher.title(), found.name))
    })?;
    if !found.game_dir.is_dir() {
        return Err(Error::NotFound(format!("the game folder of {}", found.name)));
    }
    let id = store.reserve_id(&found.name)?;
    let inst = Instance {
        id,
        name: found.name.clone(),
        group: found.group.clone(),
        icon: None,
        minecraft,
        loader: found.loader.clone(),
        created: Utc::now(),
        last_played: found.last_played,
        total_played_secs: found.total_played_secs,
        java: JavaOverrides::default(),
        window: None,
        notes: Some(format!("Imported from {}.", found.launcher.title())),
        modpack: None,
    };
    let dest = store.game_dir(&inst);
    // The official launcher's default installation is the shared .minecraft: only its game content is copied.
    let shared = found.launcher == Launcher::Official && found.game_dir.join("versions").is_dir();
    let copied = if shared { copy_selected(&found.game_dir, &dest, &OFFICIAL_CONTENT) } else { copy_game(&found.game_dir, &dest) };
    if let Err(e) = copied {
        let _ = fs::remove_dir_all(store.paths().instance(&inst.id));
        return Err(e);
    }
    store.insert(inst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;
    use cubby_common::paths::Paths;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn official_version_ids() {
        assert_eq!(official_version("fabric-loader-0.15.11-1.20.1"), Some(("1.20.1".into(), Loader::Fabric { version: "0.15.11".into() })));
        assert_eq!(official_version("1.20.1-forge-47.2.0"), Some(("1.20.1".into(), Loader::Forge { version: "47.2.0".into() })));
        assert_eq!(official_version("neoforge-21.1.5"), Some(("1.21.1".into(), Loader::NeoForge { version: "21.1.5".into() })));
        assert_eq!(official_version("neoforge-21.0.10"), Some(("1.21".into(), Loader::NeoForge { version: "21.0.10".into() })));
        assert_eq!(official_version("1.21.1"), Some(("1.21.1".into(), Loader::Vanilla)));
        assert_eq!(official_version("latest-release"), None);
    }

    #[test]
    fn curseforge_loader_ids() {
        assert_eq!(curseforge_loader("forge-47.2.0", "1.20.1"), Loader::Forge { version: "47.2.0".into() });
        assert_eq!(curseforge_loader("fabric-0.15.11-1.20.1", "1.20.1"), Loader::Fabric { version: "0.15.11".into() });
        assert_eq!(curseforge_loader("neoforge-21.1.5", "1.21.1"), Loader::NeoForge { version: "21.1.5".into() });
    }

    #[test]
    fn lists_each_launcher() {
        let root = temp_dir("launchers");
        write(
            &root.join("cf/All the Mods/minecraftinstance.json"),
            r#"{"name":"All the Mods","gameVersion":"1.20.1","baseModLoader":{"name":"forge-47.2.0"},"lastPlayed":"2026-10-01T10:00:00Z"}"#,
        );
        write(&root.join("cf/All the Mods/mods/a.jar"), "x");
        write(
            &root.join("atl/instances/Vanilla Plus/instance.json"),
            r#"{"id":"1.21.1","launcher":{"name":"Vanilla Plus","loaderVersion":{"type":"Fabric","version":"0.16.5"},"totalPlayTime":7200000}}"#,
        );
        write(&root.join("gd/instances/Old/config.json"), r#"{"loader":{"loaderType":"forge","loaderVersion":"47.2.0","mcVersion":"1.20.1"},"timePlayed":60}"#);
        write(
            &root.join("gd/instances/New/instance.json"),
            r#"{"name":"New One","game_configuration":{"version":{"Standard":{"release":"1.21.1","modloaders":[{"type_":"Neoforge","version":"21.1.5"}]}}}}"#,
        );
        write(&root.join("gd/instances/New/instance/mods/b.jar"), "x");
        write(&root.join("mr/profiles/Fabulous/profile.json"), r#"{"metadata":{"name":"Fabulous","game_version":"1.21.1","loader":"fabric","loader_version":{"id":"0.16.5"}}}"#);
        write(&root.join("mr/profiles/Newer/mods/c.jar"), "x");
        write(&root.join("mr/profiles/caches/x"), "x");
        write(
            &root.join("official/launcher_profiles.json"),
            r#"{"profiles":{"a":{"name":"Fabric","lastVersionId":"fabric-loader-0.16.5-1.21.1","gameDir":"/x"},"b":{"type":"latest-release","lastVersionId":"latest-release"}}}"#,
        );

        let cf = list_foreign(Launcher::CurseForge, &root.join("cf"));
        assert_eq!((cf[0].name.as_str(), cf[0].minecraft.as_deref(), &cf[0].loader), ("All the Mods", Some("1.20.1"), &Loader::Forge { version: "47.2.0".into() }));
        assert!(cf[0].last_played.is_some());

        let atl = list_foreign(Launcher::AtLauncher, &root.join("atl"));
        assert_eq!((atl[0].loader.clone(), atl[0].total_played_secs), (Loader::Fabric { version: "0.16.5".into() }, 7200));

        let gd = list_foreign(Launcher::GdLauncher, &root.join("gd"));
        let names: Vec<_> = gd.iter().map(|i| (i.name.as_str(), i.minecraft.as_deref())).collect();
        assert_eq!(names, [("New One", Some("1.21.1")), ("Old", Some("1.20.1"))]);
        assert_eq!(gd[0].loader, Loader::NeoForge { version: "21.1.5".into() });
        assert!(gd[0].game_dir.ends_with("instance"));

        let mr = list_foreign(Launcher::ModrinthApp, &root.join("mr"));
        let names: Vec<_> = mr.iter().map(|i| (i.name.as_str(), i.minecraft.as_deref())).collect();
        assert_eq!(names, [("Fabulous", Some("1.21.1")), ("Newer", None)]);

        let official = list_foreign(Launcher::Official, &root.join("official"));
        assert_eq!(official.len(), 1);
        assert_eq!(official[0].game_dir, PathBuf::from("/x"));
    }

    #[test]
    fn imports_and_skips_launcher_files() {
        let root = temp_dir("launchers-import");
        write(&root.join("cf/Pack/minecraftinstance.json"), r#"{"name":"Pack","gameVersion":"1.20.1","baseModLoader":{"name":"forge-47.2.0"}}"#);
        write(&root.join("cf/Pack/mods/a.jar"), "x");
        write(&root.join("cf/Pack/saves/W/level.dat"), "x");
        let store = InstanceStore::new(Paths::new(root.join("cubby")));
        let found = list_foreign(Launcher::CurseForge, &root.join("cf")).remove(0);
        let inst = import_foreign(&store, &found).unwrap();
        let game = store.game_dir(&inst);
        assert!(game.join("mods/a.jar").is_file());
        assert!(game.join("saves/W/level.dat").is_file());
        assert!(!game.join("minecraftinstance.json").exists());
        assert_eq!(inst.loader, Loader::Forge { version: "47.2.0".into() });

        // The shared .minecraft of the official launcher: only game content comes over.
        let mc = root.join("official");
        write(&mc.join("launcher_profiles.json"), r#"{"profiles":{"a":{"name":"Release","lastVersionId":"1.21.1"}}}"#);
        write(&mc.join("versions/1.21.1/1.21.1.json"), "{}");
        write(&mc.join("saves/S/level.dat"), "x");
        let found = list_foreign(Launcher::Official, &mc).remove(0);
        let inst = import_foreign(&store, &found).unwrap();
        assert!(store.game_dir(&inst).join("saves/S/level.dat").is_file());
        assert!(!store.game_dir(&inst).join("versions").exists());

        let unknown = ForeignInstance { minecraft: None, ..found };
        assert!(import_foreign(&store, &unknown).is_err());
    }
}

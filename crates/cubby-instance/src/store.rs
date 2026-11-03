//! The instance store: CRUD on `instances/<id>/instance.toml`.

use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use cubby_common::paths::Paths;
use cubby_common::{Error, Result};
use serde::Serialize;

use crate::model::{Instance, JavaOverrides, NewInstance};
use crate::util::{atomic_write, check_plain_name, copy_dir_all, move_to_trash, slugify};

const INSTANCE_FILE: &str = "instance.toml";
const GAME_DIR: &str = ".minecraft";

/// A group name with the number of instances in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfo {
    pub name: String,
    pub count: usize,
}

/// Manages the instances of one launcher data directory.
#[derive(Debug, Clone)]
pub struct InstanceStore {
    paths: Paths,
}

impl InstanceStore {
    /// Creates a store over the given data directory (nothing is touched on disk yet).
    pub fn new(paths: Paths) -> Self {
        Self { paths }
    }

    /// The data directory layout this store works on.
    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    /// All valid instances sorted by name. Broken folders are skipped and logged.
    pub fn list(&self) -> Result<Vec<Instance>> {
        let dir = self.paths.instances();
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Error::io(dir, e)),
        };
        let mut out = Vec::new();
        for entry in rd.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            if !entry.path().join(INSTANCE_FILE).exists() {
                tracing::warn!("skipping {}: no {INSTANCE_FILE}", entry.path().display());
                continue;
            }
            match self.get(&id) {
                Ok(i) => out.push(i),
                Err(e) => tracing::warn!("skipping broken instance folder {id}: {e}"),
            }
        }
        out.sort_by_key(|i| i.name.to_lowercase());
        Ok(out)
    }

    /// Loads one instance by id (its folder name).
    pub fn get(&self, id: &str) -> Result<Instance> {
        check_plain_name(id)?;
        let file = self.paths.instance(id).join(INSTANCE_FILE);
        let text = match std::fs::read_to_string(&file) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Error::NotFound(format!("instance {id}"))),
            Err(e) => return Err(Error::io(file, e)),
        };
        let mut inst: Instance =
            toml::from_str(&text).map_err(|e| Error::Toml { path: file, message: e.to_string() })?;
        inst.id = id.to_string();
        Ok(inst)
    }

    /// Creates an instance (folder, `instance.toml` and an empty `.minecraft`).
    pub fn create(&self, new: NewInstance) -> Result<Instance> {
        let name = new.name.trim().to_string();
        if name.is_empty() {
            return Err(Error::Other("Give the instance a name.".into()));
        }
        let id = self.reserve_id(&name)?;
        let inst = Instance {
            id,
            name,
            group: new.group.filter(|g| !g.trim().is_empty()),
            icon: new.icon,
            minecraft: new.minecraft,
            loader: new.loader,
            created: Utc::now(),
            last_played: None,
            total_played_secs: 0,
            java: JavaOverrides::default(),
            window: None,
            notes: None,
            modpack: None,
        };
        self.insert(inst)
    }

    /// Saves an already reserved instance and creates its game folder; removes the folder on failure.
    pub(crate) fn insert(&self, inst: Instance) -> Result<Instance> {
        let game = self.game_dir(&inst);
        let result = std::fs::create_dir_all(&game).map_err(|e| Error::io(&game, e)).and_then(|_| self.save(&inst));
        if let Err(e) = result {
            let _ = std::fs::remove_dir_all(self.paths.instance(&inst.id));
            return Err(e);
        }
        Ok(inst)
    }

    /// Atomically writes `instance.toml`.
    pub fn save(&self, inst: &Instance) -> Result<()> {
        check_plain_name(&inst.id)?;
        let file = self.paths.instance(&inst.id).join(INSTANCE_FILE);
        let text = toml::to_string_pretty(inst).map_err(|e| Error::Toml { path: file.clone(), message: e.to_string() })?;
        atomic_write(&file, text.as_bytes())
    }

    /// Changes the display name (the id and folder stay the same).
    pub fn rename(&self, id: &str, new_name: &str) -> Result<Instance> {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return Err(Error::Other("Give the instance a name.".into()));
        }
        let mut inst = self.get(id)?;
        inst.name = new_name.to_string();
        self.save(&inst)?;
        Ok(inst)
    }

    /// Moves the instance to a group (`None` or an empty name removes it from any group).
    pub fn set_group(&self, id: &str, group: Option<String>) -> Result<Instance> {
        let mut inst = self.get(id)?;
        inst.group = group.map(|g| g.trim().to_string()).filter(|g| !g.is_empty());
        self.save(&inst)?;
        Ok(inst)
    }

    /// Copies the whole instance folder under a new name; play statistics are reset.
    pub fn duplicate(&self, id: &str, new_name: &str) -> Result<Instance> {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return Err(Error::Other("Give the instance a name.".into()));
        }
        let mut inst = self.get(id)?;
        let new_id = self.reserve_id(new_name)?;
        let dest = self.paths.instance(&new_id);
        if let Err(e) = copy_dir_all(&self.paths.instance(id), &dest) {
            let _ = std::fs::remove_dir_all(&dest);
            return Err(e);
        }
        inst.id = new_id;
        inst.name = new_name.to_string();
        inst.created = Utc::now();
        inst.last_played = None;
        inst.total_played_secs = 0;
        if let Err(e) = self.save(&inst) {
            let _ = std::fs::remove_dir_all(&dest);
            return Err(e);
        }
        Ok(inst)
    }

    /// Moves the instance folder to `<root>/trash/<id>-<timestamp>` (never a hard delete). Returns the new path.
    pub fn delete(&self, id: &str) -> Result<PathBuf> {
        check_plain_name(id)?;
        let dir = self.paths.instance(id);
        if !dir.is_dir() {
            return Err(Error::NotFound(format!("instance {id}")));
        }
        move_to_trash(&dir, &self.trash_dir(), id)
    }

    /// `<root>/trash`: where deleted instances and worlds go.
    pub fn trash_dir(&self) -> PathBuf {
        self.paths.root().join("trash")
    }

    /// `<instance>/backups`: world backups (`<world>-<date>.zip`) and content backups (`content-<date>/`).
    pub fn backups_dir(&self, id: &str) -> Result<PathBuf> {
        check_plain_name(id)?;
        Ok(self.paths.instance(id).join("backups"))
    }

    /// Records a finished play session: sets `last_played` to now and adds `played` to the total.
    pub fn record_session(&self, id: &str, played: Duration) -> Result<Instance> {
        let mut inst = self.get(id)?;
        inst.last_played = Some(Utc::now());
        inst.total_played_secs = inst.total_played_secs.saturating_add(played.as_secs());
        self.save(&inst)?;
        Ok(inst)
    }

    /// The game directory (`.minecraft`) of an instance.
    pub fn game_dir(&self, inst: &Instance) -> PathBuf {
        self.paths.instance(&inst.id).join(GAME_DIR)
    }

    /// Distinct group names with their instance counts, sorted by name.
    pub fn groups(&self) -> Result<Vec<GroupInfo>> {
        let mut map = std::collections::BTreeMap::<String, usize>::new();
        for inst in self.list()? {
            if let Some(g) = inst.group {
                *map.entry(g).or_default() += 1;
            }
        }
        Ok(map.into_iter().map(|(name, count)| GroupInfo { name, count }).collect())
    }

    /// Atomically claims a unique folder name derived from `name` by creating the folder.
    pub(crate) fn reserve_id(&self, name: &str) -> Result<String> {
        let base = slugify(name);
        let root = self.paths.instances();
        std::fs::create_dir_all(&root).map_err(|e| Error::io(&root, e))?;
        for n in 1..10_000u32 {
            let id = if n == 1 { base.clone() } else { format!("{base}-{n}") };
            let dir = root.join(&id);
            match std::fs::create_dir(&dir) {
                Ok(()) => return Ok(id),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(Error::io(dir, e)),
            }
        }
        Err(Error::Other(format!("Couldn't find a free folder name for {name:?}. Try another name.")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Loader;
    use crate::testutil::temp_dir;

    fn store(name: &str) -> InstanceStore {
        InstanceStore::new(Paths::new(temp_dir(name)))
    }

    fn new(name: &str, group: Option<&str>) -> NewInstance {
        NewInstance {
            name: name.into(),
            group: group.map(String::from),
            minecraft: "1.20.1".into(),
            loader: Loader::Fabric { version: "0.15.0".into() },
            icon: Some("grass".into()),
        }
    }

    #[test]
    fn unique_slugs() {
        let s = store("store-slug");
        let a = s.create(new("My Pack", None)).unwrap();
        let b = s.create(new("my pack", None)).unwrap();
        let c = s.create(new("My Pack!", None)).unwrap();
        assert_eq!((a.id.as_str(), b.id.as_str(), c.id.as_str()), ("my-pack", "my-pack-2", "my-pack-3"));
        assert!(s.create(new("  ", None)).is_err());
    }

    #[test]
    fn crud_round_trip() {
        let s = store("store-crud");
        let i = s.create(new("Alpha", Some("Survie"))).unwrap();
        assert!(s.game_dir(&i).is_dir());
        let got = s.get(&i.id).unwrap();
        assert_eq!(got, i);
        assert_eq!(got.loader, Loader::Fabric { version: "0.15.0".into() });

        let mut edited = got.clone();
        edited.java.max_memory_mb = Some(8192);
        edited.java.executable = Some(PathBuf::from("C:/java/bin/javaw.exe"));
        edited.window = Some(crate::WindowSize { width: 800, height: 600 });
        edited.notes = Some("Seed 1234\n\"village\" near spawn".into());
        edited.modpack = Some(crate::ModpackInfo {
            provider: None,
            project_id: "1KVo5zza".into(),
            version_id: "abcd1234".into(),
            title: "Fabulously Optimized".into(),
            version_number: "5.4.1".into(),
        });
        s.save(&edited).unwrap();
        assert_eq!(s.get(&i.id).unwrap(), edited);

        let r = s.rename(&i.id, "Beta").unwrap();
        assert_eq!((r.name.as_str(), r.id.as_str()), ("Beta", "alpha"));
        let g = s.set_group(&i.id, None).unwrap();
        assert_eq!(g.group, None);
        assert!(matches!(s.get("nope"), Err(Error::NotFound(_))));
        assert!(s.get("../x").is_err());
    }

    #[test]
    fn old_instance_files_load_without_new_fields() {
        let s = store("store-old");
        let dir = s.paths().instance("old");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(INSTANCE_FILE),
            "name = \"Old\"\nminecraft = \"1.20.1\"\ncreated = \"2024-01-01T00:00:00Z\"\n[loader]\ntype = \"vanilla\"\n",
        )
        .unwrap();
        let i = s.get("old").unwrap();
        assert_eq!((i.notes.as_deref(), i.modpack.as_ref()), (None, None));
    }

    #[test]
    fn list_skips_broken_and_groups() {
        let s = store("store-list");
        s.create(new("B", Some("G1"))).unwrap();
        s.create(new("a", Some("G1"))).unwrap();
        s.create(new("c", Some("G2"))).unwrap();
        s.create(new("d", None)).unwrap();
        let broken = s.paths().instance("broken");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join("instance.toml"), "not toml [").unwrap();
        std::fs::create_dir_all(s.paths().instance("empty")).unwrap();
        let names: Vec<_> = s.list().unwrap().into_iter().map(|i| i.name).collect();
        assert_eq!(names, ["a", "B", "c", "d"]);
        let groups = s.groups().unwrap();
        assert_eq!(groups, [GroupInfo { name: "G1".into(), count: 2 }, GroupInfo { name: "G2".into(), count: 1 }]);
    }

    #[test]
    fn delete_moves_to_trash() {
        let s = store("store-trash");
        let i = s.create(new("Gone", None)).unwrap();
        std::fs::write(s.game_dir(&i).join("keep.txt"), "x").unwrap();
        let dest = s.delete(&i.id).unwrap();
        assert!(!s.paths().instance(&i.id).exists());
        assert!(dest.starts_with(s.paths().root().join("trash")));
        assert!(dest.join(".minecraft/keep.txt").exists());
        assert!(s.list().unwrap().is_empty());
        assert!(matches!(s.delete(&i.id), Err(Error::NotFound(_))));
    }

    #[test]
    fn play_time() {
        let s = store("store-play");
        let i = s.create(new("P", None)).unwrap();
        assert!(i.last_played.is_none());
        s.record_session(&i.id, Duration::from_secs(90)).unwrap();
        let r = s.record_session(&i.id, Duration::from_secs(30)).unwrap();
        assert_eq!(r.total_played_secs, 120);
        assert!(s.get(&i.id).unwrap().last_played.is_some());
    }

    #[test]
    fn duplicate_copies_folder() {
        let s = store("store-dup");
        let i = s.create(new("Orig", Some("G"))).unwrap();
        std::fs::create_dir_all(s.game_dir(&i).join("mods")).unwrap();
        std::fs::write(s.game_dir(&i).join("mods/a.jar"), "x").unwrap();
        s.record_session(&i.id, Duration::from_secs(10)).unwrap();
        let d = s.duplicate(&i.id, "Copy").unwrap();
        assert_eq!(d.id, "copy");
        assert_eq!(d.total_played_secs, 0);
        assert_eq!(d.group.as_deref(), Some("G"));
        assert!(s.game_dir(&d).join("mods/a.jar").exists());
        assert_eq!(s.get("copy").unwrap().name, "Copy");
        assert_eq!(s.get(&i.id).unwrap().total_played_secs, 10);
    }
}

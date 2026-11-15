//! Where Cubby downloaded each content file of an instance from, in `<instance>/content-origins.json`
//! (`{"mods/x.jar": "curseforge"}`). The same file is often on Modrinth and CurseForge: identifying it by hash
//! finds both, and this says which one the player picked.

use std::collections::BTreeMap;
use std::path::Path;

const FILE: &str = "content-origins.json";

/// `folder/file name` → "modrinth" or "curseforge".
pub type Origins = BTreeMap<String, String>;

pub fn load(instance_dir: &Path) -> Origins {
    std::fs::read(instance_dir.join(FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Records that these files (`(folder, file name)`) came from `source`. A file name with ".disabled" counts
/// as the file itself. Failures are only logged: the origin is a hint.
pub fn record(instance_dir: &Path, files: &[(&str, &str)], source: &str) {
    if files.is_empty() {
        return;
    }
    let mut origins = load(instance_dir);
    for (folder, name) in files {
        origins.insert(key(folder, name), source.to_string());
    }
    let written = serde_json::to_vec_pretty(&origins).map_err(std::io::Error::other).and_then(|b| std::fs::write(instance_dir.join(FILE), b));
    if let Err(e) = written {
        log::warn!("could not record where content came from: {e}");
    }
}

pub fn key(folder: &str, name: &str) -> String {
    format!("{folder}/{}", name.strip_suffix(".disabled").unwrap_or(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_reads_back() {
        let dir = std::env::temp_dir().join(format!("cubby-origins-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(load(&dir).is_empty());
        record(&dir, &[("mods", "a.jar"), ("mods", "b.jar.disabled")], "curseforge");
        record(&dir, &[("mods", "a.jar")], "modrinth");
        let o = load(&dir);
        assert_eq!(o.get("mods/a.jar").map(String::as_str), Some("modrinth"));
        assert_eq!(o.get(&key("mods", "b.jar")).map(String::as_str), Some("curseforge"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

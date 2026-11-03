//! The Modrinth modpack format (`.mrpack`): a zip with `modrinth.index.json` (files to download,
//! game and loader versions) and `overrides/` / `client-overrides/` folders copied into the game directory.
//! See <https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack>.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use cubby_common::{Error, Result};

pub const INDEX_FILE: &str = "modrinth.index.json";
/// Applied in this order, the second one winning.
pub const OVERRIDE_FOLDERS: [&str; 2] = ["overrides/", "client-overrides/"];
const MAX_INDEX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MrpackIndex {
    pub format_version: u32,
    pub game: String,
    /// The pack's own version number.
    pub version_id: String,
    pub name: String,
    pub summary: Option<String>,
    pub files: Vec<MrpackFile>,
    /// "minecraft", "fabric-loader", "quilt-loader", "forge", "neoforge" -> version.
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MrpackFile {
    /// Relative to the game directory, `/`-separated.
    pub path: String,
    /// "sha1", "sha512" -> hex.
    pub hashes: HashMap<String, String>,
    pub env: Option<MrpackEnv>,
    pub downloads: Vec<String>,
    pub file_size: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MrpackEnv {
    /// "required", "optional" or "unsupported".
    pub client: Option<String>,
    pub server: Option<String>,
}

/// Mod loader of a pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackLoader {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl PackLoader {
    /// The launcher key: "fabric", "quilt", "forge", "neoforge".
    pub fn as_str(self) -> &'static str {
        match self {
            PackLoader::Fabric => "fabric",
            PackLoader::Quilt => "quilt",
            PackLoader::Forge => "forge",
            PackLoader::NeoForge => "neoforge",
        }
    }
}

/// A file to download for the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackDownload {
    pub path: PathBuf,
    pub urls: Vec<String>,
    pub sha1: String,
    pub size: Option<u64>,
}

impl MrpackIndex {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
        let index: Self = serde_json::from_slice(bytes).map_err(|e| Error::json(INDEX_FILE, e))?;
        if !index.game.is_empty() && index.game != "minecraft" {
            return Err(Error::Other(format!("This modpack is for \"{}\", not Minecraft.", index.game)));
        }
        Ok(index)
    }

    /// Reads `modrinth.index.json` from a `.mrpack` file.
    pub fn read(archive: &Path) -> Result<Self> {
        let mut zip = open_zip(archive)?;
        let entry = zip
            .by_name(INDEX_FILE)
            .map_err(|_| Error::Other(format!("{} isn't a Modrinth modpack (no {INDEX_FILE}).", archive.display())))?;
        let mut bytes = Vec::new();
        entry.take(MAX_INDEX_BYTES).read_to_end(&mut bytes).map_err(|e| Error::io(archive, e))?;
        Self::parse(&bytes)
    }

    pub fn minecraft(&self) -> Option<&str> {
        self.dependencies.get("minecraft").map(String::as_str).filter(|v| !v.is_empty())
    }

    /// The loader and its version; `None` for a vanilla pack.
    pub fn loader(&self) -> Option<(PackLoader, String)> {
        [
            ("neoforge", PackLoader::NeoForge),
            ("forge", PackLoader::Forge),
            ("quilt-loader", PackLoader::Quilt),
            ("fabric-loader", PackLoader::Fabric),
        ]
        .into_iter()
        .find_map(|(key, loader)| self.dependencies.get(key).filter(|v| !v.is_empty()).map(|v| (loader, v.clone())))
    }

    /// Files the client needs (`env.client` is not "unsupported"), with checked paths. A path that is
    /// absolute or goes up (`..`) fails the whole pack, as does a file without SHA-1 or HTTPS download.
    pub fn client_downloads(&self) -> Result<Vec<PackDownload>> {
        let mut out = Vec::new();
        for file in &self.files {
            if file.env.as_ref().and_then(|e| e.client.as_deref()) == Some("unsupported") {
                continue;
            }
            let path = sanitize_path(&file.path).ok_or_else(|| Error::Other(format!("The modpack contains an unsafe path: {:?}.", file.path)))?;
            let sha1 = file
                .hashes
                .get("sha1")
                .filter(|h| h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit()))
                .ok_or_else(|| Error::Other(format!("The modpack has no SHA-1 for {}.", file.path)))?;
            let urls: Vec<String> = file.downloads.iter().filter(|u| u.starts_with("https://")).cloned().collect();
            if urls.is_empty() {
                return Err(Error::Other(format!("The modpack has no HTTPS download for {}.", file.path)));
            }
            out.push(PackDownload { path, urls, sha1: sha1.to_lowercase(), size: file.file_size });
        }
        Ok(out)
    }
}

/// A relative path made of plain names, or `None` when it is empty, absolute (`/x`, `\x`, `C:...`), goes up
/// (`..`) or contains a NUL. Both `/` and `\` separate components; empty and `.` components are dropped.
pub fn sanitize_path(path: &str) -> Option<PathBuf> {
    if path.is_empty() || path.contains(['\0', ':']) || path.starts_with(['/', '\\']) {
        return None;
    }
    let mut out = PathBuf::new();
    for part in path.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return None,
            name => out.push(name),
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

fn open_zip(archive: &Path) -> Result<zip::ZipArchive<fs::File>> {
    let file = fs::File::open(archive).map_err(|e| Error::io(archive, e))?;
    zip::ZipArchive::new(file).map_err(|e| Error::Other(format!("{} isn't a valid zip file ({e}).", archive.display())))
}

/// Copies `overrides/` then `client-overrides/` into `game_dir`. Every entry is checked before anything is
/// written; one unsafe path rejects the whole archive. Files are written through a temporary file and
/// replace existing ones. `on_progress(done, total)` counts files. Returns the number of files written.
pub fn extract_overrides(archive: &Path, game_dir: &Path, mut on_progress: impl FnMut(u64, u64)) -> Result<u64> {
    let mut zip = open_zip(archive)?;
    let mut plan: Vec<(usize, PathBuf, bool)> = Vec::new();
    for prefix in OVERRIDE_FOLDERS {
        for i in 0..zip.len() {
            let entry = zip.by_index(i).map_err(|e| Error::Other(format!("Couldn't read {} ({e}).", archive.display())))?;
            let name = entry.name().replace('\\', "/");
            let Some(rel) = name.strip_prefix(prefix) else { continue };
            if rel.trim_matches('/').is_empty() {
                continue;
            }
            let path = sanitize_path(rel).ok_or_else(|| Error::Other(format!("The modpack contains an unsafe path: {:?}.", entry.name())))?;
            plan.push((i, path, entry.is_dir()));
        }
    }
    let total = plan.iter().filter(|p| !p.2).count() as u64;
    let mut done = 0;
    on_progress(done, total);
    for (i, rel, is_dir) in plan {
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
        let written = (|| -> std::io::Result<()> {
            let mut out = fs::File::create(&tmp)?;
            std::io::copy(&mut entry, &mut out)?;
            out.flush()
        })();
        if let Err(e) = written.and_then(|_| fs::rename(&tmp, &dest)) {
            let _ = fs::remove_file(&tmp);
            return Err(Error::io(&dest, e));
        }
        done += 1;
        on_progress(done, total);
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &str = r#"{
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "5.4.1",
        "name": "Fabulously Optimized",
        "summary": "Improve your graphics and performance",
        "files": [
            {
                "path": "mods/sodium-fabric-0.5.11.jar",
                "hashes": { "sha1": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "sha512": "00" },
                "env": { "client": "required", "server": "unsupported" },
                "downloads": ["https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar"],
                "fileSize": 1234
            },
            {
                "path": "mods/server-only.jar",
                "hashes": { "sha1": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" },
                "env": { "client": "unsupported", "server": "required" },
                "downloads": ["https://cdn.modrinth.com/data/x/server.jar"],
                "fileSize": 1
            },
            {
                "path": "config\\no-env.json",
                "hashes": { "sha1": "cccccccccccccccccccccccccccccccccccccccc" },
                "downloads": ["http://insecure.example/a", "https://github.com/a/b/releases/download/1/no-env.json"]
            }
        ],
        "dependencies": { "minecraft": "1.21.1", "fabric-loader": "0.16.5" }
    }"#;

    #[test]
    fn parses_index() {
        let index = MrpackIndex::parse(INDEX.as_bytes()).unwrap();
        assert_eq!((index.name.as_str(), index.version_id.as_str()), ("Fabulously Optimized", "5.4.1"));
        assert_eq!(index.minecraft(), Some("1.21.1"));
        assert_eq!(index.loader(), Some((PackLoader::Fabric, "0.16.5".to_string())));
        let files = index.client_downloads().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, PathBuf::from("mods").join("sodium-fabric-0.5.11.jar"));
        assert_eq!(files[0].sha1, "a".repeat(40));
        assert_eq!(files[0].size, Some(1234));
        assert_eq!(files[1].path, PathBuf::from("config").join("no-env.json"));
        assert_eq!(files[1].urls, ["https://github.com/a/b/releases/download/1/no-env.json"]);

        let with_bom = [b"\xEF\xBB\xBF".as_slice(), INDEX.as_bytes()].concat();
        assert!(MrpackIndex::parse(&with_bom).is_ok());
    }

    #[test]
    fn loaders_and_bad_packs() {
        let parse = |deps: &str| MrpackIndex::parse(format!(r#"{{"game":"minecraft","dependencies":{deps}}}"#).as_bytes()).unwrap();
        assert_eq!(parse(r#"{"minecraft":"1.20.1","quilt-loader":"0.26.0"}"#).loader(), Some((PackLoader::Quilt, "0.26.0".into())));
        assert_eq!(parse(r#"{"minecraft":"1.20.1","forge":"47.2.0"}"#).loader(), Some((PackLoader::Forge, "47.2.0".into())));
        assert_eq!(parse(r#"{"minecraft":"1.21.1","neoforge":"21.1.66"}"#).loader(), Some((PackLoader::NeoForge, "21.1.66".into())));
        let vanilla = parse(r#"{"minecraft":"1.21.1"}"#);
        assert_eq!((vanilla.loader(), vanilla.minecraft()), (None, Some("1.21.1")));
        assert_eq!(parse("{}").minecraft(), None);

        assert!(MrpackIndex::parse(br#"{"game":"terraria"}"#).is_err());
        assert!(MrpackIndex::parse(b"not json").is_err());
        let evil = r#"{"files":[{"path":"../../evil.jar","hashes":{"sha1":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"downloads":["https://x/y"]}]}"#;
        assert!(MrpackIndex::parse(evil.as_bytes()).unwrap().client_downloads().is_err());
        let no_hash = r#"{"files":[{"path":"mods/a.jar","hashes":{},"downloads":["https://x/y"]}]}"#;
        assert!(MrpackIndex::parse(no_hash.as_bytes()).unwrap().client_downloads().is_err());
        let no_https = r#"{"files":[{"path":"mods/a.jar","hashes":{"sha1":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"downloads":["http://x/y"]}]}"#;
        assert!(MrpackIndex::parse(no_https.as_bytes()).unwrap().client_downloads().is_err());
    }

    #[test]
    fn path_sanitization() {
        let ok = |p: &str, parts: &[&str]| assert_eq!(sanitize_path(p), Some(parts.iter().collect::<PathBuf>()), "{p}");
        ok("mods/a.jar", &["mods", "a.jar"]);
        ok("config\\sub/b.json", &["config", "sub", "b.json"]);
        ok("./mods//a.jar", &["mods", "a.jar"]);
        ok("options.txt", &["options.txt"]);
        ok("a..b/c", &["a..b", "c"]);
        for bad in ["", "/etc/passwd", "\\\\server\\share", "C:\\Windows\\x", "C:x", "../a", "mods/../../a", "a/..", "a\\..\\..\\b", ".", "./", "a\0b"] {
            assert_eq!(sanitize_path(bad), None, "{bad:?}");
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/modrinth/testdata").join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_zip(path: &Path, entries: &[(&str, &str)]) {
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, body) in entries {
            if name.ends_with('/') {
                zip.add_directory(*name, zip::write::SimpleFileOptions::default()).unwrap();
            } else {
                zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                zip.write_all(body.as_bytes()).unwrap();
            }
        }
        zip.finish().unwrap();
    }

    #[test]
    fn overrides_then_client_overrides() {
        let dir = temp_dir("overrides");
        let archive = dir.join("pack.mrpack");
        write_zip(
            &archive,
            &[
                (INDEX_FILE, INDEX),
                ("overrides/", ""),
                ("overrides/config/a.txt", "base"),
                ("overrides/options.txt", "base"),
                ("overrides/empty/", ""),
                ("client-overrides/options.txt", "client"),
                ("server-overrides/server.properties", "no"),
                ("other.txt", "no"),
            ],
        );
        assert_eq!(MrpackIndex::read(&archive).unwrap().name, "Fabulously Optimized");
        let game = dir.join(".minecraft");
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("options.txt"), "old").unwrap();
        let mut last = (0, 0);
        assert_eq!(extract_overrides(&archive, &game, |d, t| last = (d, t)).unwrap(), 3);
        assert_eq!(last, (3, 3));
        assert_eq!(fs::read_to_string(game.join("config/a.txt")).unwrap(), "base");
        assert_eq!(fs::read_to_string(game.join("options.txt")).unwrap(), "client");
        assert!(game.join("empty").is_dir());
        assert!(!game.join("server.properties").exists() && !game.join("other.txt").exists());

        let evil = dir.join("evil.mrpack");
        write_zip(&evil, &[(INDEX_FILE, INDEX), ("overrides/ok.txt", "x"), ("overrides/../../escape.txt", "x")]);
        let game2 = dir.join("game2");
        assert!(extract_overrides(&evil, &game2, |_, _| {}).is_err());
        assert!(!game2.join("ok.txt").exists(), "nothing is written when one path is unsafe");
        assert!(!dir.join("escape.txt").exists());

        let not_pack = dir.join("not.mrpack");
        write_zip(&not_pack, &[("hello.txt", "x")]);
        assert!(MrpackIndex::read(&not_pack).is_err());
    }
}

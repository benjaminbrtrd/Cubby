//! Forge and NeoForge: version lists from Maven and installing from the official installer jar.
//!
//! Modern installers (Forge 1.13+, all NeoForge) run client-side processors with a Java to patch jars;
//! legacy ones (Forge 1.5.2 to 1.12.2) only carry a universal jar. Everything is cached under `meta/<kind>/`.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;
use cubby_common::net::{ensure_file, sha1_file, Download, Http};
use cubby_common::paths::Paths;
use cubby_common::version::{maven_path, Library, RuleEnv, VersionJson};
use cubby_common::{Error, Result};

use crate::loaders::{LoaderKind, LoaderVersion};

const FORGE_MAVEN: &str = "https://maven.minecraftforge.net";
const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases";
const FORGE_METADATA_URL: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";
const FORGE_PROMOTIONS_URL: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEOFORGE_VERSIONS_URL: &str = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
const NEOFORGE_LEGACY_VERSIONS_URL: &str = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/forge";
/// NeoForge started as a fork of Forge: for this Minecraft version its builds live in the `forge` artifact.
const NEOFORGE_LEGACY_GAME: &str = "1.20.1";

/// Version lists younger than this are served from memory.
const LIST_TTL: Duration = Duration::from_secs(10 * 60);

// Version lists

type ListCache = Mutex<HashMap<String, (Instant, Vec<u8>)>>;

fn list_cache() -> &'static ListCache {
    static CACHE: std::sync::OnceLock<ListCache> = std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// GETs `url`, reusing the answer for ten minutes and falling back to a stale one when the network fails.
async fn cached_get(http: &Http, url: &str) -> Result<Vec<u8>> {
    let lookup = |fresh_only: bool| {
        let guard = list_cache().lock().ok()?;
        let (at, bytes) = guard.get(url)?;
        (!fresh_only || at.elapsed() < LIST_TTL).then(|| bytes.clone())
    };
    if let Some(bytes) = lookup(true) {
        return Ok(bytes);
    }
    match http.get_bytes(url).await {
        Ok(bytes) => {
            if let Ok(mut guard) = list_cache().lock() {
                guard.insert(url.to_string(), (Instant::now(), bytes.clone()));
            }
            Ok(bytes)
        }
        Err(error) => lookup(false).ok_or(error),
    }
}

/// Every `<version>` of a `maven-metadata.xml`, in document order.
pub(crate) fn parse_maven_versions(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<version>") {
        let after = &rest[start + "<version>".len()..];
        let Some(end) = after.find("</version>") else { break };
        out.push(after[..end].trim().to_string());
        rest = &after[end + "</version>".len()..];
    }
    out
}

/// Splits a Forge Maven version into the Minecraft version and the rest: `1.20.1-47.2.0` -> `("1.20.1", "47.2.0")`.
pub(crate) fn split_forge_version(full: &str) -> Option<(&str, &str)> {
    full.split_once('-').filter(|(mc, rest)| !mc.is_empty() && !rest.is_empty())
}

/// Numeric sort key: `"10.13.4.1614-1.7.10"` -> `[10, 13, 4, 1614, 1, 7, 10]`.
fn version_key(version: &str) -> Vec<u64> {
    version.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap_or(0)).collect()
}

fn sort_newest_first(versions: &mut [LoaderVersion]) {
    versions.sort_by_key(|v| std::cmp::Reverse(version_key(&v.version)));
}

/// Forge versions for one game version from the Maven metadata and the promotions (`stable` = the
/// recommended build and older ones; without a recommended build only the latest is stable). Newest first.
pub(crate) fn forge_versions_from(metadata: &str, promotions: Option<&str>, game_version: &str) -> Vec<LoaderVersion> {
    let promos: HashMap<String, String> = promotions
        .and_then(|json| {
            #[derive(Deserialize)]
            struct Promos {
                promos: HashMap<String, String>,
            }
            serde_json::from_str::<Promos>(json).ok()
        })
        .map(|p| p.promos)
        .unwrap_or_default();
    let mut versions: Vec<LoaderVersion> = parse_maven_versions(metadata)
        .into_iter()
        .filter_map(|full| match split_forge_version(&full) {
            Some((mc, rest)) if mc == game_version => Some(LoaderVersion { version: rest.to_string(), stable: false }),
            _ => None,
        })
        .collect();
    sort_newest_first(&mut versions);
    match promos.get(&format!("{game_version}-recommended")) {
        Some(recommended) => {
            let limit = version_key(recommended);
            for v in &mut versions {
                let key = version_key(&v.version);
                v.stable = key[..key.len().min(limit.len())] <= limit[..];
            }
        }
        None => {
            if let Some(first) = versions.first_mut() {
                first.stable = true;
            }
        }
    }
    versions
}

/// Prefix of the NeoForge versions of a game version: `1.21.1` -> `21.1.`, `1.21` -> `21.0.`; game versions
/// that are not `1.x` (year-based names) use their own name.
pub(crate) fn neoforge_prefix(game_version: &str) -> String {
    match game_version.strip_prefix("1.") {
        Some(rest) => {
            let mut parts = rest.split('.');
            let minor = parts.next().unwrap_or("0");
            let patch = parts.next().unwrap_or("0");
            format!("{minor}.{patch}.")
        }
        None => format!("{game_version}."),
    }
}

fn parse_neoforge_list(json: &[u8], what: &str) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct List {
        versions: Vec<String>,
    }
    let list: List = serde_json::from_slice(json).map_err(|e| Error::json(what, e))?;
    Ok(list.versions)
}

/// NeoForge versions of a game version: stable unless the version carries a suffix (`-beta`, `+snapshot`). Newest first.
pub(crate) fn neoforge_versions_from(all: &[String], game_version: &str) -> Vec<LoaderVersion> {
    let legacy_prefix = format!("{game_version}-");
    let prefix = neoforge_prefix(game_version);
    let mut versions: Vec<LoaderVersion> = all
        .iter()
        .filter_map(|v| {
            if game_version == NEOFORGE_LEGACY_GAME {
                v.strip_prefix(&legacy_prefix).map(|rest| LoaderVersion { version: rest.to_string(), stable: !rest.contains('-') })
            } else if v.starts_with(&prefix) {
                Some(LoaderVersion { version: v.clone(), stable: !v.contains('-') && !v.contains('+') })
            } else {
                None
            }
        })
        .collect();
    sort_newest_first(&mut versions);
    versions
}

/// Loader versions of `kind` (Forge or NeoForge) for `game_version`, newest first.
pub async fn versions(http: &Http, kind: LoaderKind, game_version: &str) -> Result<Vec<LoaderVersion>> {
    match kind {
        LoaderKind::Forge => {
            let metadata = cached_get(http, FORGE_METADATA_URL).await?;
            // The promotions are only needed for the stable flag: do without them when they are down.
            let promotions = cached_get(http, FORGE_PROMOTIONS_URL).await.ok();
            Ok(forge_versions_from(&String::from_utf8_lossy(&metadata), promotions.as_deref().map(String::from_utf8_lossy).as_deref(), game_version))
        }
        LoaderKind::NeoForge => {
            let url = if game_version == NEOFORGE_LEGACY_GAME { NEOFORGE_LEGACY_VERSIONS_URL } else { NEOFORGE_VERSIONS_URL };
            let all = parse_neoforge_list(&cached_get(http, url).await?, url)?;
            Ok(neoforge_versions_from(&all, game_version))
        }
        _ => Err(Error::Other(format!("{} doesn't use the Forge installer.", kind.as_str()))),
    }
}

/// Game versions that have at least one build of `kind` (Forge or NeoForge), newest first.
pub async fn game_versions(http: &Http, kind: LoaderKind) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |v: String| {
        if !out.contains(&v) {
            out.push(v);
        }
    };
    match kind {
        LoaderKind::Forge => {
            let metadata = cached_get(http, FORGE_METADATA_URL).await?;
            for full in parse_maven_versions(&String::from_utf8_lossy(&metadata)).iter().rev() {
                if let Some((mc, _)) = split_forge_version(full) {
                    push(mc.to_string());
                }
            }
        }
        LoaderKind::NeoForge => {
            let all = parse_neoforge_list(&cached_get(http, NEOFORGE_VERSIONS_URL).await?, NEOFORGE_VERSIONS_URL)?;
            for v in all.iter().rev() {
                // `21.1.172` -> `1.21.1`, `21.0.5` -> `1.21`, `26.1.0.3` -> `26.1`.
                let mut parts = v.split(|c: char| !c.is_ascii_digit());
                let (Some(a), Some(b)) = (parts.next(), parts.next()) else { continue };
                let Ok(major) = a.parse::<u32>() else { continue };
                push(match (major < 26, b) {
                    (true, "0") => format!("1.{a}"),
                    (true, _) => format!("1.{a}.{b}"),
                    (false, _) => format!("{a}.{b}"),
                });
            }
            push(NEOFORGE_LEGACY_GAME.to_string());
        }
        _ => return Err(Error::Other(format!("{} doesn't use the Forge installer.", kind.as_str()))),
    }
    Ok(out)
}

// Locating the installer

/// Where the installer of a loader version is published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallerRef {
    /// Maven version, e.g. `1.20.1-47.2.0` (Forge) or `21.1.172` (NeoForge).
    pub full: String,
    /// Maven artifact id: `forge` or `neoforge`.
    pub artifact: &'static str,
    pub url: String,
}

/// Maps `(game version, loader version)` to the installer: Forge versions are `<mc>-<version>[-<branch>]` on
/// Maven, NeoForge ones are bare except the legacy 1.20.1 `forge` artifact.
///
/// `known` is the Maven version list of Forge, used to find builds with a branch suffix
/// (`1.7.10-10.13.4.1614-1.7.10`) from the bare number.
pub(crate) fn installer_ref(kind: LoaderKind, game_version: &str, loader_version: &str, known: &[String]) -> Result<InstallerRef> {
    match kind {
        LoaderKind::Forge => {
            let exact = format!("{game_version}-{loader_version}");
            let with_suffix = format!("{exact}-");
            let full = if known.contains(&exact) {
                exact
            } else {
                known.iter().find(|v| v.starts_with(&with_suffix)).cloned().unwrap_or(exact)
            };
            let url = format!("{FORGE_MAVEN}/net/minecraftforge/forge/{full}/forge-{full}-installer.jar");
            Ok(InstallerRef { full, artifact: "forge", url })
        }
        LoaderKind::NeoForge => {
            let (artifact, full) = if game_version == NEOFORGE_LEGACY_GAME {
                let full = if loader_version.starts_with(&format!("{game_version}-")) { loader_version.to_string() } else { format!("{game_version}-{loader_version}") };
                ("forge", full)
            } else {
                ("neoforge", loader_version.to_string())
            };
            let url = format!("{NEOFORGE_MAVEN}/net/neoforged/{artifact}/{full}/{artifact}-{full}-installer.jar");
            Ok(InstallerRef { full, artifact, url })
        }
        _ => Err(Error::Other(format!("{} doesn't use the Forge installer.", kind.as_str()))),
    }
}

// Installer contents

/// `install_profile.json` (both formats; the fields of the other format stay empty).
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct InstallProfile {
    /// Where the launcher profile is in the installer (modern), e.g. `/version.json`.
    #[serde(default)]
    pub json: Option<String>,
    #[serde(default)]
    pub data: HashMap<String, DataEntry>,
    #[serde(default)]
    pub processors: Vec<Processor>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    /// Legacy: the launcher profile itself.
    #[serde(default, rename = "versionInfo")]
    pub version_info: Option<serde_json::Value>,
    /// Legacy: where the universal jar is.
    #[serde(default)]
    pub install: Option<LegacyInstall>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct DataEntry {
    #[serde(default)]
    pub client: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct Processor {
    /// Absent: runs on both sides.
    #[serde(default)]
    pub sides: Option<Vec<String>>,
    /// Maven coordinates of the jar to run.
    pub jar: String,
    #[serde(default)]
    pub classpath: Vec<String>,
    #[serde(default)]
    pub args: Vec<String>,
    /// File (path or `[coordinates]`) -> expected SHA-1 (quoted literal or `{TOKEN}`).
    #[serde(default)]
    pub outputs: HashMap<String, String>,
}

impl Processor {
    fn runs_on_client(&self) -> bool {
        self.sides.as_ref().is_none_or(|s| s.iter().any(|x| x == "client"))
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct LegacyInstall {
    /// Maven coordinates of the Forge jar, e.g. `net.minecraftforge:forge:1.12.2-14.23.5.2860`.
    #[serde(default)]
    pub path: String,
    /// Name of the universal jar inside the installer.
    #[serde(default, rename = "filePath")]
    pub file_path: String,
}

impl InstallProfile {
    /// Whether this is the old format (a `versionInfo` and no processors).
    pub fn is_legacy(&self) -> bool {
        self.version_info.is_some() && self.processors.is_empty()
    }
}

fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    let file = std::fs::File::open(path).map_err(|e| Error::io(path, e))?;
    zip::ZipArchive::new(file).map_err(|e| Error::Other(format!("{} isn't a valid jar: {e}", path.display())))
}

/// Reads one entry of a zip, `None` when it doesn't exist.
fn read_entry(jar: &Path, name: &str) -> Result<Option<Vec<u8>>> {
    let mut zip = open_zip(jar)?;
    let mut entry = match zip.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(Error::Other(format!("Couldn't read {name} in {}: {e}", jar.display()))),
    };
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes).map_err(|e| Error::io(jar, e))?;
    Ok(Some(bytes))
}

fn write_file(dest: &Path, reader: &mut impl Read) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    let part = dest.with_extension("part");
    let mut file = std::fs::File::create(&part).map_err(|e| Error::io(&part, e))?;
    std::io::copy(reader, &mut file).map_err(|e| Error::io(&part, e))?;
    drop(file);
    std::fs::rename(&part, dest).map_err(|e| Error::io(dest, e))
}

/// Extracts one entry to `dest`; false when the entry doesn't exist.
fn extract_entry(jar: &Path, name: &str, dest: &Path) -> Result<bool> {
    let mut zip = open_zip(jar)?;
    let mut entry = match zip.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(false),
        Err(e) => return Err(Error::Other(format!("Couldn't read {name} in {}: {e}", jar.display()))),
    };
    write_file(dest, &mut entry)?;
    Ok(true)
}

/// Extracts every file under `prefix/` of the zip into `dest_root`, skipping files that already have the same size.
fn extract_tree(jar: &Path, prefix: &str, dest_root: &Path) -> Result<usize> {
    let mut zip = open_zip(jar)?;
    let mut count = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| Error::Other(format!("Couldn't read {}: {e}", jar.display())))?;
        if entry.is_dir() {
            continue;
        }
        let Some(relative) = entry.enclosed_name().and_then(|p| p.strip_prefix(prefix).ok().map(Path::to_path_buf)) else { continue };
        let dest = dest_root.join(relative);
        if std::fs::metadata(&dest).is_ok_and(|m| m.len() == entry.size()) {
            continue;
        }
        write_file(&dest, &mut entry)?;
        count += 1;
    }
    Ok(count)
}

/// `Main-Class` of a jar's manifest.
fn main_class(jar: &Path) -> Result<String> {
    let manifest = read_entry(jar, "META-INF/MANIFEST.MF")?.ok_or_else(|| Error::Other(format!("{} has no manifest, so it can't be run.", jar.display())))?;
    parse_main_class(&String::from_utf8_lossy(&manifest)).ok_or_else(|| Error::Other(format!("{} has no Main-Class, so it can't be run.", jar.display())))
}

/// `Main-Class` of manifest text (continuation lines start with a space).
pub(crate) fn parse_main_class(manifest: &str) -> Option<String> {
    let mut joined = String::new();
    let mut lines: Vec<String> = Vec::new();
    for line in manifest.lines() {
        match line.strip_prefix(' ') {
            Some(cont) => joined.push_str(cont),
            None => {
                if !joined.is_empty() {
                    lines.push(std::mem::take(&mut joined));
                }
                joined = line.to_string();
            }
        }
    }
    lines.push(joined);
    lines.iter().find_map(|l| l.strip_prefix("Main-Class:").map(|v| v.trim().to_string())).filter(|v| !v.is_empty())
}

// Tokens and processor arguments

/// Replaces `{NAME}` with the value of `NAME`. Unknown names are an error (a processor would get garbage).
pub(crate) fn replace_tokens(input: &str, tokens: &HashMap<String, String>) -> Result<String> {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return Ok(out);
        };
        let name = &after[..end];
        let value = tokens.get(name).ok_or_else(|| Error::Other(format!("The Forge installer uses an unknown value {{{name}}}.")))?;
        out.push_str(value);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Absolute path of `[group:artifact:version[:classifier][@ext]]` under `libraries_dir`.
pub(crate) fn library_file(libraries_dir: &Path, coordinates: &str) -> Result<PathBuf> {
    let relative = maven_path(coordinates, None).ok_or_else(|| Error::Other(format!("\"{coordinates}\" isn't valid Maven coordinates.")))?;
    Ok(libraries_dir.join(relative))
}

/// Substitutes the tokens of an argument, then turns a whole-argument `[coordinates]` into a library path.
pub(crate) fn resolve_arg(arg: &str, tokens: &HashMap<String, String>, libraries_dir: &Path) -> Result<String> {
    let text = replace_tokens(arg, tokens)?;
    match text.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
        Some(coordinates) => Ok(library_file(libraries_dir, coordinates)?.to_string_lossy().into_owned()),
        None => Ok(text),
    }
}

fn unquote(value: &str) -> &str {
    value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')).unwrap_or(value)
}

// The installer on disk

/// A downloaded installer with its parsed profile.
pub(crate) struct Forge {
    pub kind: LoaderKind,
    pub full: String,
    pub game_version: String,
    /// `meta/<kind>/<full>/`
    dir: PathBuf,
    installer: PathBuf,
    profile: InstallProfile,
}

/// Marker written once every processor ran, inside the cache folder.
const DONE_MARKER: &str = "processed";

impl Forge {
    /// Downloads (once) and parses the installer of `loader_version` for `game_version`.
    pub async fn open(http: &Http, paths: &Paths, kind: LoaderKind, game_version: &str, loader_version: &str) -> Result<Forge> {
        // The Maven list is only needed for the odd Forge builds with a branch suffix, and may be unavailable offline.
        let known = if kind == LoaderKind::Forge {
            match cached_get(http, FORGE_METADATA_URL).await {
                Ok(bytes) => parse_maven_versions(&String::from_utf8_lossy(&bytes)),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let reference = installer_ref(kind, game_version, loader_version, &known)?;
        let dir = paths.meta().join(kind.as_str()).join(&reference.full);
        let installer = dir.join("installer.jar");
        ensure_file(http, &Download::new(&reference.url, &installer)).await.map_err(|e| match e {
            Error::HttpStatus { status: 404, .. } => Error::Other(format!(
                "{} {loader_version} has no installer for Minecraft {game_version}, so it can't be installed. Pick another version.",
                if kind == LoaderKind::Forge { "Forge" } else { "NeoForge" }
            )),
            other => other,
        })?;
        let profile_bytes = {
            let installer = installer.clone();
            blocking(move || read_entry(&installer, "install_profile.json")).await?
        }
        .ok_or_else(|| Error::Other(format!("{} has no install_profile.json.", installer.display())))?;
        let profile: InstallProfile = serde_json::from_slice(&profile_bytes).map_err(|e| Error::json(format!("{} install profile", reference.full), e))?;
        Ok(Forge { kind, full: reference.full, game_version: game_version.to_string(), dir, installer, profile })
    }

    /// The launcher profile (a partial version JSON with `inheritsFrom`), from the installer.
    pub async fn version_profile(&self) -> Result<VersionJson> {
        let mut version = if self.profile.is_legacy() {
            legacy_version(self.profile.version_info.clone().unwrap_or_default())?
        } else {
            let name = self.profile.json.clone().unwrap_or_else(|| "/version.json".into());
            let name = name.trim_start_matches('/').to_string();
            let installer = self.installer.clone();
            let bytes = blocking(move || read_entry(&installer, &name)).await?.ok_or_else(|| Error::NotFound(format!("the launcher profile in the installer of {}", self.full)))?;
            serde_json::from_slice(&bytes).map_err(|e| Error::json(format!("{} version.json", self.full), e))?
        };
        if version.inherits_from.is_none() {
            version.inherits_from = Some(self.game_version.clone());
        }
        Ok(version)
    }

    fn libraries_dir(paths: &Paths) -> PathBuf {
        paths.libraries()
    }

    /// Absolute paths of libraries that no URL provides: extracted from the installer or produced by processors.
    pub fn local_only(&self, version: &VersionJson, paths: &Paths) -> HashSet<PathBuf> {
        let root = Self::libraries_dir(paths);
        let mut set = HashSet::new();
        for lib in version.libraries.iter().chain(self.profile.libraries.iter()) {
            let Some(artifact) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) else { continue };
            if artifact.url.is_empty() {
                if let Some(path) = artifact.path.clone().or_else(|| lib.maven_path()) {
                    set.insert(root.join(path));
                }
            }
        }
        set
    }

    /// Downloads for the libraries only the install profile lists (what the processors run on).
    pub fn profile_downloads(&self, paths: &Paths) -> Vec<Download> {
        let env = RuleEnv::current();
        let root = Self::libraries_dir(paths);
        self.profile.libraries.iter().filter(|l| l.applies(&env)).filter_map(|l| crate::install::library_job(l, &root)).collect()
    }

    /// Puts the files the installer carries in place: its `maven/` folder and (legacy) the universal jar
    /// in `libraries/`, and the `data/` files processors read in the cache folder.
    pub async fn extract(&self, paths: &Paths) -> Result<()> {
        let libraries = Self::libraries_dir(paths);
        let installer = self.installer.clone();
        let data_dir = self.dir.clone();
        let data_files: Vec<String> = self.profile.data.values().map(|d| d.client.as_str()).filter(|v| v.starts_with('/')).map(String::from).collect();
        let legacy = self.profile.install.clone().filter(|_| self.profile.is_legacy());
        blocking(move || {
            extract_tree(&installer, "maven", &libraries)?;
            for name in data_files {
                let entry = name.trim_start_matches('/');
                if !extract_entry(&installer, entry, &data_dir.join(entry))? {
                    return Err(Error::NotFound(format!("{entry} in the Forge installer")));
                }
            }
            if let Some(install) = legacy {
                let dest = library_file(&libraries, &install.path)?;
                if !dest.exists() && !extract_entry(&installer, &install.file_path, &dest)? {
                    return Err(Error::NotFound(format!("{} in the Forge installer", install.file_path)));
                }
            }
            Ok(())
        })
        .await
    }

    /// Token values of the installer: the built-ins and every `data` entry of the client side.
    fn tokens(&self, paths: &Paths, client_jar: &Path) -> Result<HashMap<String, String>> {
        let libraries = Self::libraries_dir(paths);
        let s = |p: &Path| p.to_string_lossy().into_owned();
        let mut tokens = HashMap::new();
        for (key, entry) in &self.profile.data {
            let value = entry.client.as_str();
            let resolved = if let Some(coordinates) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                s(&library_file(&libraries, coordinates)?)
            } else if value.starts_with('/') {
                s(&self.dir.join(value.trim_start_matches('/')))
            } else {
                unquote(value).to_string()
            };
            tokens.insert(key.clone(), resolved);
        }
        tokens.insert("SIDE".into(), "client".into());
        tokens.insert("MINECRAFT_JAR".into(), s(client_jar));
        tokens.insert("MINECRAFT_VERSION".into(), self.game_version.clone());
        tokens.insert("ROOT".into(), s(paths.root()));
        tokens.insert("INSTALLER".into(), s(&self.installer));
        tokens.insert("LIBRARY_DIR".into(), s(&libraries));
        Ok(tokens)
    }

    /// Resolved outputs of a processor: file -> expected SHA-1 (empty when none is given).
    fn outputs(&self, processor: &Processor, tokens: &HashMap<String, String>, libraries: &Path) -> Result<Vec<(PathBuf, String)>> {
        processor
            .outputs
            .iter()
            .map(|(file, sha1)| {
                let file = resolve_arg(file, tokens, libraries)?;
                let sha1 = unquote(&replace_tokens(sha1, tokens)?).to_string();
                Ok((PathBuf::from(file), sha1))
            })
            .collect()
    }

    /// Runs the client-side processors with `java`, skipping those whose outputs are already up to date.
    ///
    /// Idempotent: once every processor ran, a marker in the cache folder makes the next call only check that
    /// the outputs still exist. `on_step(done, total)` is called around each processor.
    pub async fn run_processors(&self, paths: &Paths, client_jar: &Path, java: Option<&Path>, mut on_step: impl FnMut(u64, u64)) -> Result<()> {
        let processors: Vec<&Processor> = self.profile.processors.iter().filter(|p| p.runs_on_client()).collect();
        if processors.is_empty() {
            return Ok(());
        }
        let libraries = Self::libraries_dir(paths);
        let tokens = self.tokens(paths, client_jar)?;
        let marker = self.dir.join(DONE_MARKER);
        if marker.exists() {
            let mut all_there = true;
            for p in &processors {
                all_there &= self.outputs(p, &tokens, &libraries)?.iter().all(|(file, _)| file.exists());
            }
            if all_there {
                return Ok(());
            }
            let _ = std::fs::remove_file(&marker);
        }
        let java = java.ok_or_else(|| Error::Java("Installing Forge needs Java, but none was given.".into()))?;
        let total = processors.len() as u64;
        on_step(0, total);
        for (index, processor) in processors.iter().enumerate() {
            let outputs = self.outputs(processor, &tokens, &libraries)?;
            if !outputs.is_empty() && outputs_valid(&outputs).await {
                on_step(index as u64 + 1, total);
                continue;
            }
            self.run_one(processor, &tokens, &libraries, java).await?;
            for (file, sha1) in &outputs {
                if !file.exists() {
                    return Err(Error::Other(format!("A Forge installer step didn't create {}.", file.display())));
                }
                if !sha1.is_empty() {
                    let actual = sha1_file(file).await?;
                    if !actual.eq_ignore_ascii_case(sha1) {
                        return Err(Error::HashMismatch { path: file.clone(), expected: sha1.clone(), actual });
                    }
                }
            }
            on_step(index as u64 + 1, total);
        }
        if let Err(e) = tokio::fs::write(&marker, b"ok").await {
            tracing::warn!("could not write {}: {e}", marker.display());
        }
        Ok(())
    }

    async fn run_one(&self, processor: &Processor, tokens: &HashMap<String, String>, libraries: &Path, java: &Path) -> Result<()> {
        let jar = library_file(libraries, &processor.jar)?;
        let mut classpath = vec![jar.clone()];
        for coordinates in &processor.classpath {
            classpath.push(library_file(libraries, coordinates)?);
        }
        if let Some(missing) = classpath.iter().find(|p| !p.exists()) {
            return Err(Error::NotFound(format!("{}, needed to install Forge", missing.display())));
        }
        let class = {
            let jar = jar.clone();
            blocking(move || main_class(&jar)).await?
        };
        let args: Vec<String> = processor.args.iter().map(|a| resolve_arg(a, tokens, libraries)).collect::<Result<_>>()?;
        let separator = if cfg!(windows) { ";" } else { ":" };
        let classpath = classpath.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join(separator);
        tracing::debug!("running {} ({class})", processor.jar);

        let mut command = tokio::process::Command::new(console_java(java));
        command.arg("-cp").arg(&classpath).arg(&class).args(&args).current_dir(tokens.get("ROOT").map(String::as_str).unwrap_or(".")).stdin(std::process::Stdio::null()).kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let output = tokio::time::timeout(Duration::from_secs(15 * 60), command.output())
            .await
            .map_err(|_| Error::Other(format!("The Forge installer step {} took more than 15 minutes.", processor.jar)))?
            .map_err(|e| Error::io(java, e))?;
        if !output.status.success() {
            let text = format!("{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
            let tail: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            let tail = tail[tail.len().saturating_sub(8)..].join("\n");
            return Err(Error::Other(format!(
                "A Forge installer step failed (exit code {}). Check that the Java used is recent enough.\n{tail}",
                output.status.code().map_or("none".to_string(), |c| c.to_string())
            )));
        }
        Ok(())
    }
}

/// `javaw.exe` -> `java.exe` when it exists, so the processors' output isn't lost.
fn console_java(java: &Path) -> PathBuf {
    if java.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case("javaw.exe")) {
        let console = java.with_file_name("java.exe");
        if console.is_file() {
            return console;
        }
    }
    java.to_path_buf()
}

/// Every output exists and matches its SHA-1 (when one is given).
async fn outputs_valid(outputs: &[(PathBuf, String)]) -> bool {
    for (file, sha1) in outputs {
        if !file.is_file() {
            return false;
        }
        if !sha1.is_empty() && !sha1_file(file).await.is_ok_and(|actual| actual.eq_ignore_ascii_case(sha1)) {
            return false;
        }
    }
    true
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| Error::Other(e.to_string()))?
}

/// Converts the `versionInfo` of a legacy profile, replacing the dead download hosts of the era.
///
/// `clientreq` / `serverreq` are deliberately ignored: launchers don't filter on them, and some libraries flagged
/// `clientreq: false` (jline for launchwrapper's logging) are needed anyway.
pub(crate) fn legacy_version(info: serde_json::Value) -> Result<VersionJson> {
    let mut version: VersionJson = serde_json::from_value(info).map_err(|e| Error::json("the legacy Forge profile", e))?;
    for lib in &mut version.libraries {
        if let Some(url) = &lib.url {
            lib.url = Some(modernize_url(url));
        }
    }
    Ok(version)
}

fn modernize_url(url: &str) -> String {
    let url = url.replace("files.minecraftforge.net/maven", "maven.minecraftforge.net");
    match url.strip_prefix("http://") {
        Some(rest) => format!("https://{rest}"),
        None => url,
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    const METADATA: &str = "<metadata><groupId>net.minecraftforge</groupId><artifactId>forge</artifactId><versioning><latest>1.21.1-52.0.1</latest><release>1.21.1-52.0.1</release><versions>
        <version>1.7.10-10.13.4.1614-1.7.10</version><version>1.12.2-14.23.5.2859</version><version>1.12.2-14.23.5.2860</version>
        <version>1.20.1-47.1.0</version><version>1.20.1-47.2.0</version><version>1.20.1-47.4.0</version><version>1.20.1-47.4.1</version></versions></versioning></metadata>";
    const PROMOS: &str = r#"{"homepage":"x","promos":{"1.20.1-latest":"47.4.1","1.20.1-recommended":"47.4.0","1.12.2-latest":"14.23.5.2860"}}"#;

    #[test]
    fn maven_metadata_versions() {
        let v = parse_maven_versions(METADATA);
        assert_eq!(v.len(), 7);
        assert_eq!(v[0], "1.7.10-10.13.4.1614-1.7.10");
        assert_eq!(split_forge_version("1.20.1-47.2.0"), Some(("1.20.1", "47.2.0")));
        assert_eq!(split_forge_version("1.7.10-10.13.4.1614-1.7.10"), Some(("1.7.10", "10.13.4.1614-1.7.10")));
        assert_eq!(split_forge_version("nonsense"), None);
    }

    #[test]
    fn forge_list_with_promotions() {
        let v = forge_versions_from(METADATA, Some(PROMOS), "1.20.1");
        let names: Vec<_> = v.iter().map(|x| (x.version.as_str(), x.stable)).collect();
        assert_eq!(names, [("47.4.1", false), ("47.4.0", true), ("47.2.0", true), ("47.1.0", true)]);
        // No recommended build: only the latest counts as stable.
        let v = forge_versions_from(METADATA, Some(PROMOS), "1.12.2");
        assert_eq!((v[0].version.as_str(), v[0].stable, v[1].stable), ("14.23.5.2860", true, false));
        // No promotions at all.
        assert!(forge_versions_from(METADATA, None, "1.20.1")[0].stable);
        assert!(forge_versions_from(METADATA, None, "1.99").is_empty());
        let old = forge_versions_from(METADATA, None, "1.7.10");
        assert_eq!(old[0].version, "10.13.4.1614-1.7.10");
    }

    #[test]
    fn neoforge_lists() {
        let all: Vec<String> = ["20.4.237", "21.0.167", "21.1.9", "21.1.172", "21.1.200-beta", "21.4.0-beta", "1.20.1-47.1.106", "1.20.1-47.1.99"].iter().map(|s| s.to_string()).collect();
        let v = neoforge_versions_from(&all, "1.21.1");
        assert_eq!(v.iter().map(|x| (x.version.as_str(), x.stable)).collect::<Vec<_>>(), [("21.1.200-beta", false), ("21.1.172", true), ("21.1.9", true)]);
        assert_eq!(neoforge_versions_from(&all, "1.21")[0].version, "21.0.167");
        let legacy = neoforge_versions_from(&all, "1.20.1");
        assert_eq!(legacy.iter().map(|x| x.version.as_str()).collect::<Vec<_>>(), ["47.1.106", "47.1.99"]);
        assert_eq!(neoforge_prefix("1.21.1"), "21.1.");
        assert_eq!(neoforge_prefix("1.21"), "21.0.");
        assert_eq!(neoforge_prefix("26.1"), "26.1.");
        let json = br#"{"isSnapshot":false,"versions":["20.2.3-beta","20.2.4"]}"#;
        assert_eq!(parse_neoforge_list(json, "t").unwrap(), ["20.2.3-beta", "20.2.4"]);
    }

    #[test]
    fn installer_urls() {
        let known = vec!["1.7.10-10.13.4.1614-1.7.10".to_string(), "1.20.1-47.2.0".to_string()];
        let f = installer_ref(LoaderKind::Forge, "1.20.1", "47.2.0", &known).unwrap();
        assert_eq!(f.full, "1.20.1-47.2.0");
        assert_eq!(f.url, "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-installer.jar");
        // Bare number of a build with a branch suffix.
        assert_eq!(installer_ref(LoaderKind::Forge, "1.7.10", "10.13.4.1614", &known).unwrap().full, "1.7.10-10.13.4.1614-1.7.10");
        // Unknown list (offline): the plain name.
        assert_eq!(installer_ref(LoaderKind::Forge, "1.12.2", "14.23.5.2860", &[]).unwrap().full, "1.12.2-14.23.5.2860");
        let n = installer_ref(LoaderKind::NeoForge, "1.21.1", "21.1.172", &[]).unwrap();
        assert_eq!(n.url, "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.172/neoforge-21.1.172-installer.jar");
        let l = installer_ref(LoaderKind::NeoForge, "1.20.1", "47.1.106", &[]).unwrap();
        assert_eq!(l.url, "https://maven.neoforged.net/releases/net/neoforged/forge/1.20.1-47.1.106/forge-1.20.1-47.1.106-installer.jar");
        assert!(installer_ref(LoaderKind::Fabric, "1.21", "0.1", &[]).is_err());
    }

    const MODERN_PROFILE: &str = r#"{"spec":1,"profile":"forge","version":"1.20.1-forge-47.2.0","json":"/version.json","minecraft":"1.20.1",
        "data":{"MAPPINGS":{"client":"[de.oceanlabs.mcp:mcp_config:1.20.1-20230612.114412:mappings@txt]","server":"x"},
                "BINPATCH":{"client":"/data/client.lzma","server":"/data/server.lzma"},
                "PATCHED":{"client":"[net.minecraftforge:forge:1.20.1-47.2.0:client]","server":"y"},
                "MC_SLIM_SHA":{"client":"'abc123'","server":"'zzz'"}},
        "processors":[
          {"sides":["server"],"jar":"a:server-only:1","classpath":[],"args":[]},
          {"jar":"net.minecraftforge:binarypatcher:1.1.1:fatjar","classpath":["commons-io:commons-io:2.11.0"],
           "args":["--clean","{MINECRAFT_JAR}","--output","{PATCHED}","--apply","{BINPATCH}","--side","{SIDE}"],
           "outputs":{"{PATCHED}":"{MC_SLIM_SHA}"}}],
        "libraries":[{"name":"commons-io:commons-io:2.11.0","downloads":{"artifact":{"path":"commons-io/commons-io/2.11.0/commons-io-2.11.0.jar","url":"https://x/c.jar","sha1":"aa","size":1}}},
                     {"name":"net.minecraftforge:forge:1.20.1-47.2.0:shim","downloads":{"artifact":{"path":"net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-shim.jar","url":"","sha1":"bb","size":1}}}]}"#;

    #[test]
    fn modern_profile_and_substitution() {
        let profile: InstallProfile = serde_json::from_str(MODERN_PROFILE).unwrap();
        assert!(!profile.is_legacy());
        assert_eq!(profile.processors.len(), 2);
        assert!(!profile.processors[0].runs_on_client());
        assert!(profile.processors[1].runs_on_client());
        let paths = Paths::new(std::env::temp_dir().join("cubby-forge-test"));
        let forge = Forge { kind: LoaderKind::Forge, full: "1.20.1-47.2.0".into(), game_version: "1.20.1".into(), dir: paths.meta().join("forge/1.20.1-47.2.0"), installer: PathBuf::from("i.jar"), profile };
        let client = paths.versions().join("1.20.1/1.20.1.jar");
        let tokens = forge.tokens(&paths, &client).unwrap();
        let libs = paths.libraries();
        let args: Vec<String> = forge.profile.processors[1].args.iter().map(|a| resolve_arg(a, &tokens, &libs).unwrap()).collect();
        assert_eq!(args[1], client.to_string_lossy());
        assert_eq!(args[3], libs.join("net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-client.jar").to_string_lossy());
        assert_eq!(args[5], paths.meta().join("forge/1.20.1-47.2.0").join("data/client.lzma").to_string_lossy());
        assert_eq!(args[7], "client");
        // Outputs: `{PATCHED}` -> path, `{MC_SLIM_SHA}` -> the unquoted literal.
        let outputs = forge.outputs(&forge.profile.processors[1], &tokens, &libs).unwrap();
        assert_eq!(outputs, vec![(libs.join("net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-client.jar"), "abc123".to_string())]);
        // A bracketed argument is a library path; an unknown token is an error.
        assert_eq!(resolve_arg("[a.b:c:1:x@txt]", &tokens, &libs).unwrap(), libs.join("a/b/c/1/c-1-x.txt").to_string_lossy());
        assert!(replace_tokens("{NOPE}", &tokens).is_err());
        assert_eq!(replace_tokens("pre-{SIDE}-post", &tokens).unwrap(), "pre-client-post");
        // The libraries without URL are the ones provided by the installer or the processors.
        let version = VersionJson::default();
        let local = forge.local_only(&version, &paths);
        assert_eq!(local.len(), 1);
        assert!(local.contains(&libs.join("net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-shim.jar")));
        assert_eq!(forge.profile_downloads(&paths).len(), 2);
    }

    #[test]
    fn legacy_profile() {
        let json = r#"{"install":{"profileName":"Forge","target":"Forge1.12.2-14.23.5.2860","path":"net.minecraftforge:forge:1.12.2-14.23.5.2860","version":"Forge 1.12.2-14.23.5.2860","filePath":"forge-1.12.2-14.23.5.2860-universal.jar","minecraft":"1.12.2"},
          "versionInfo":{"id":"1.12.2-forge1.12.2-14.23.5.2860","type":"release","mainClass":"net.minecraft.launchwrapper.Launch","inheritsFrom":"1.12.2",
            "minecraftArguments":"--username ${auth_player_name} --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker",
            "libraries":[{"name":"net.minecraftforge:forge:1.12.2-14.23.5.2860"},
                         {"name":"net.minecraft:launchwrapper:1.12"},
                         {"name":"org.ow2.asm:asm-all:5.2","url":"http://files.minecraftforge.net/maven/"},
                         {"name":"lzma:lzma:0.0.1","serverreq":true,"clientreq":false},
                         {"name":"java3d:vecmath:1.5.2","clientreq":true}]}}"#;
        let profile: InstallProfile = serde_json::from_str(json).unwrap();
        assert!(profile.is_legacy());
        assert_eq!(profile.install.as_ref().unwrap().file_path, "forge-1.12.2-14.23.5.2860-universal.jar");
        let version = legacy_version(profile.version_info.unwrap()).unwrap();
        assert_eq!(version.main_class.as_deref(), Some("net.minecraft.launchwrapper.Launch"));
        let names: Vec<_> = version.libraries.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["net.minecraftforge:forge:1.12.2-14.23.5.2860", "net.minecraft:launchwrapper:1.12", "org.ow2.asm:asm-all:5.2", "lzma:lzma:0.0.1", "java3d:vecmath:1.5.2"]);
        assert_eq!(version.libraries[2].url.as_deref(), Some("https://maven.minecraftforge.net/"));
    }

    #[test]
    fn manifest_main_class() {
        let manifest = "Manifest-Version: 1.0\r\nMain-Class: net.minecraftforge.binarypatcher.Con\r\n sole\r\nCreated-By: x\r\n";
        assert_eq!(parse_main_class(manifest).as_deref(), Some("net.minecraftforge.binarypatcher.Console"));
        assert_eq!(parse_main_class("Manifest-Version: 1.0\n"), None);
    }

    #[test]
    fn zip_helpers() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("cubby-forge-zip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let jar = dir.join("t.jar");
        {
            let mut z = zip::ZipWriter::new(std::fs::File::create(&jar).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            z.start_file("install_profile.json", opts).unwrap();
            z.write_all(b"{}").unwrap();
            z.start_file("maven/a/b/1/b-1.jar", opts).unwrap();
            z.write_all(b"jar").unwrap();
            z.start_file("META-INF/MANIFEST.MF", opts).unwrap();
            z.write_all(b"Main-Class: x.Y\n").unwrap();
            z.finish().unwrap();
        }
        assert_eq!(read_entry(&jar, "install_profile.json").unwrap().unwrap(), b"{}");
        assert!(read_entry(&jar, "nope").unwrap().is_none());
        assert_eq!(main_class(&jar).unwrap(), "x.Y");
        assert_eq!(extract_tree(&jar, "maven", &dir.join("libs")).unwrap(), 1);
        assert_eq!(extract_tree(&jar, "maven", &dir.join("libs")).unwrap(), 0);
        assert_eq!(std::fs::read(dir.join("libs/a/b/1/b-1.jar")).unwrap(), b"jar");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

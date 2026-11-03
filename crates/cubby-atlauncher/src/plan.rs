//! Turns a manifest and the player's optional-mod choices into what the app installs. The first half is
//! pure (no network, no disk) so it is tested on fixtures; the client then downloads the archives it asks for.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cubby_common::packs::{safe_relative_path, BlockedFile, PackFile, PackLoaderKind};
use cubby_common::{Error, Result};

use crate::model::{AtlManifest, AtlMod};
use crate::modtypes::ModKind;
use crate::optional::is_installed;

/// A zip a mod ships in, to unpack into the game directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArchiveJob {
    /// Mod name, for messages.
    pub name: String,
    pub url: String,
    pub md5: Option<String>,
    /// Folder of the game directory the content goes to (`/`-separated, empty for the game directory).
    pub dest: String,
    /// Folder of the zip whose content is taken, without a trailing `/`; empty for the whole zip.
    pub prefix: String,
    /// `decomp`: the single file of the zip to take (it lands at `dest/file`).
    pub file: Option<String>,
}

/// Everything decided without the network.
#[derive(Debug, Default)]
pub(crate) struct Built {
    pub loader: Option<(PackLoaderKind, String)>,
    pub files: Vec<PackFile>,
    pub blocked: Vec<BlockedFile>,
    pub archives: Vec<ArchiveJob>,
    pub md5: HashMap<PathBuf, String>,
    /// Things the plan can't do and that make the pack unusable.
    pub fatal: Vec<String>,
    /// Things left out that the player may want to know about.
    pub warnings: Vec<String>,
    pub memory_mib: Option<u32>,
}

/// Memory the manifest asks for, in MiB. A few old packs write gigabytes (3, 4).
pub(crate) fn memory_mib(raw: Option<u32>) -> Option<u32> {
    raw.filter(|&m| m > 0).map(|m| if m <= 32 { m * 1024 } else { m })
}

/// Joins a folder and a file name into a checked relative path.
fn target_path(folder: &str, file: &str) -> Option<PathBuf> {
    let file = safe_relative_path(file)?;
    if folder.is_empty() {
        Some(file)
    } else {
        safe_relative_path(&format!("{folder}/{}", slash(&file)))
    }
}

/// Download URL of a mod; `None` for "browser" (and unknown) downloads.
fn mod_url(m: &AtlMod, server: &str) -> std::result::Result<Option<String>, String> {
    match m.download.as_str() {
        "server" => {
            let rel = m.url.trim_start_matches('/');
            if rel.is_empty() || rel.split(['/', '\\']).any(|p| p == "..") {
                return Err(format!("{}: invalid server path \"{}\"", m.name, m.url));
            }
            Ok(Some(format!("{}/{}", server.trim_end_matches('/'), rel.replace('#', "%23").replace('?', "%3F"))))
        }
        "direct" => {
            if m.url.starts_with("https://") || m.url.starts_with("http://") {
                Ok(Some(m.url.clone()))
            } else {
                Err(format!("{}: \"{}\" isn't a web address", m.name, m.url))
            }
        }
        "browser" => Ok(None),
        other => Err(format!("{}: unknown download type \"{other}\"", m.name)),
    }
}

/// `extractFolder` as a checked folder of the zip: "%s%" stands for "/", leading and trailing "/" go.
fn extract_prefix(raw: &str) -> Option<String> {
    let cleaned = raw.replace("%s%", "/");
    let cleaned = cleaned.trim_matches('/');
    if cleaned.is_empty() {
        return Some(String::new());
    }
    safe_relative_path(cleaned).map(|p| slash(&p))
}

/// `/`-separated form of a relative path.
fn slash(path: &Path) -> String {
    path.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

/// The Forge version of a `forge` mod, without the Minecraft version some packs prepend ("1.9.4-12.17.0.1968").
fn forge_version(raw: &str, minecraft: &str) -> String {
    let raw = raw.trim();
    raw.strip_prefix(&format!("{minecraft}-")).unwrap_or(raw).to_string()
}

fn loader_from_block(manifest: &AtlManifest) -> Option<std::result::Result<(PackLoaderKind, String), String>> {
    let block = manifest.loader.as_ref().filter(|l| !l.kind.trim().is_empty())?;
    let meta = &block.metadata;
    let kind = block.kind.trim().to_ascii_lowercase();
    let (loader, version) = match kind.as_str() {
        "forge" => (PackLoaderKind::Forge, forge_version(&meta.version, &manifest.minecraft)),
        "neoforge" => (PackLoaderKind::NeoForge, meta.version.trim().to_string()),
        "fabric" => (PackLoaderKind::Fabric, meta.loader.trim().to_string()),
        "quilt" => (PackLoaderKind::Quilt, meta.loader.trim().to_string()),
        other => return Some(Err(format!("the loader \"{other}\" isn't supported"))),
    };
    if version.is_empty() {
        let why = if block.choose {
            "the player picks its version"
        } else if meta.latest || meta.recommended {
            "the launcher has to look up its latest version"
        } else {
            "the manifest has no version for it"
        };
        return Some(Err(format!("the {kind} version can't be known: {why}")));
    }
    Some(Ok((loader, version)))
}

/// Decides everything about a plan that needs neither network nor disk.
pub(crate) fn build(manifest: &AtlManifest, selected_optional: &[String], server_base: &str) -> Built {
    let mut out = Built { memory_mib: memory_mib(manifest.memory), ..Built::default() };
    let minecraft = manifest.minecraft.as_str();
    let selected: HashSet<&str> = selected_optional.iter().map(String::as_str).collect();
    let known: HashSet<&str> = manifest.mods.iter().filter(|m| m.optional).map(|m| m.name.as_str()).collect();
    for name in selected_optional.iter().filter(|n| !known.contains(n.as_str())) {
        out.warnings.push(format!("\"{name}\" isn't an optional mod of this version, ignored."));
    }

    let mut forge_mod: Option<&AtlMod> = None;
    let mut file_index: HashMap<PathBuf, usize> = HashMap::new();
    for m in manifest.mods.iter().filter(|m| is_installed(m, &selected)) {
        let mut kind = ModKind::parse(&m.kind);
        // Some packs ship Forge as a "jar" mod; Prism fixes the type up by name.
        if kind == ModKind::Jar && m.name == "Minecraft Forge" {
            kind = ModKind::Forge;
        }
        match kind {
            ModKind::Forge => {
                forge_mod.get_or_insert(m);
                continue;
            }
            ModKind::Jar => {
                out.fatal.push(format!("\"{}\" is a jar mod (it patches minecraft.jar), which isn't supported", m.name));
                continue;
            }
            ModKind::Mcpc => {
                out.warnings.push(format!("\"{}\" is an MCPC server jar, left out.", m.name));
                continue;
            }
            ModKind::Millenaire => {
                out.warnings.push(format!("\"{}\" is a Millenaire mod, which Prism doesn't support either; left out.", m.name));
                continue;
            }
            ModKind::Unknown => {
                out.fatal.push(format!("\"{}\" has the unknown type \"{}\"", m.name, m.kind));
                continue;
            }
            _ => {}
        }
        let url = match mod_url(m, server_base) {
            Ok(url) => url,
            Err(message) => {
                out.fatal.push(message);
                continue;
            }
        };

        if kind.is_archive() {
            let Some(url) = url else {
                out.warnings.push(format!("\"{}\" is an archive that has to be downloaded in a browser, left out.", m.name));
                continue;
            };
            match archive_job(m, kind, minecraft, url) {
                Ok(job) => out.archives.push(job),
                Err(message) => out.warnings.push(message),
            }
            continue;
        }

        let folder = kind.folder(minecraft).unwrap_or_default();
        let Some(path) = target_path(&folder, &m.file) else {
            out.fatal.push(format!("\"{}\": the file name \"{}\" isn't a safe path", m.name, m.file));
            continue;
        };
        if let Some(md5) = m.md5.as_deref().filter(|h| !h.is_empty()) {
            out.md5.insert(path.clone(), md5.to_ascii_lowercase());
        }
        match url {
            Some(url) => {
                let file = PackFile { path: path.clone(), urls: vec![url], sha1: None, size: m.filesize };
                // The same file twice: the later one wins, as when Prism copies them in order.
                match file_index.get(&path) {
                    Some(&i) => out.files[i] = file,
                    None => {
                        file_index.insert(path, out.files.len());
                        out.files.push(file);
                    }
                }
            }
            None => out.blocked.push(BlockedFile { file_name: m.file.clone(), page_url: m.url.clone(), path, sha1: None, size: m.filesize }),
        }
    }

    // Loader: the block when there is one, else the Forge mod of old packs.
    match loader_from_block(manifest) {
        Some(Ok(loader)) => {
            if let Some(m) = forge_mod {
                out.warnings.push(format!("\"{}\" is a Forge mod but the pack has a loader block; the loader block is used.", m.name));
            }
            out.loader = Some(loader);
        }
        Some(Err(message)) => out.fatal.push(message),
        None => match forge_mod {
            Some(m) if !m.version.trim().is_empty() => {
                out.loader = Some((PackLoaderKind::Forge, forge_version(&m.version, minecraft)));
                out.warnings.push(format!(
                    "Forge {} comes from the pack's Forge mod; Prism falls back to a jar mod when the launcher doesn't know that build.",
                    forge_version(&m.version, minecraft)
                ));
            }
            Some(m) => out.fatal.push(format!("\"{}\" is Forge as a jar mod without a version", m.name)),
            None => {}
        },
    }

    if !manifest.libraries.is_empty() {
        out.warnings.push(format!("{} extra librar{} of the pack can't be installed.", manifest.libraries.len(), if manifest.libraries.len() == 1 { "y" } else { "ies" }));
    }
    out
}

fn archive_job(m: &AtlMod, kind: ModKind, minecraft: &str, url: String) -> std::result::Result<ArchiveJob, String> {
    let md5 = m.md5.clone().filter(|h| !h.is_empty()).map(|h| h.to_ascii_lowercase());
    let dest_of = |raw: Option<&str>| -> std::result::Result<String, String> {
        let dest_kind = ModKind::parse(raw.unwrap_or(""));
        let dest = dest_kind.folder(minecraft).ok_or_else(|| format!("\"{}\" extracts to \"{}\", which isn't supported; left out.", m.name, raw.unwrap_or("")))?;
        if dest.is_empty() {
            return Ok(dest);
        }
        safe_relative_path(&dest).map(|p| slash(&p)).ok_or_else(|| format!("\"{}\" extracts to an unsafe folder; left out.", m.name))
    };
    let (dest, prefix, file) = match kind {
        ModKind::Extract => {
            let prefix = extract_prefix(m.extract_folder.as_deref().unwrap_or("")).ok_or_else(|| format!("\"{}\" extracts an unsafe folder; left out.", m.name))?;
            (dest_of(m.extract_to.as_deref())?, prefix, None)
        }
        ModKind::TexturePackExtract => ("texturepacks/extracted".to_string(), String::new(), None),
        ModKind::ResourcePackExtract => ("resourcepacks/extracted".to_string(), String::new(), None),
        _ => {
            let file = m.decomp_file.clone().filter(|f| !f.trim().is_empty()).ok_or_else(|| format!("\"{}\" has no file to extract; left out.", m.name))?;
            if safe_relative_path(&file).is_none() {
                return Err(format!("\"{}\" extracts an unsafe file; left out.", m.name));
            }
            (dest_of(m.decomp_type.as_deref())?, String::new(), Some(file))
        }
    };
    Ok(ArchiveJob { name: m.name.clone(), url, md5, dest, prefix, file })
}

/// Rewrites `src` into `dst` so that the part of it the job wants sits under the job's folder: the plan's
/// overrides can only copy a folder of an archive to the game directory, not to a subfolder of it.
/// Returns how many files were written.
pub(crate) fn repack(src: &Path, dst: &Path, job: &ArchiveJob) -> Result<usize> {
    let file = fs::File::open(src).map_err(|e| Error::io(src, e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| Error::Other(format!("{} isn't a valid zip file ({e}).", src.display())))?;
    let out = fs::File::create(dst).map_err(|e| Error::io(dst, e))?;
    let mut writer = zip::ZipWriter::new(out);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let prefix = if job.prefix.is_empty() { String::new() } else { format!("{}/", job.prefix) };
    let wanted_file = job.file.as_deref().and_then(safe_relative_path).map(|p| slash(&p));
    let mut written = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| Error::Other(format!("{} is damaged ({e}).", src.display())))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let name = name.trim_start_matches('/');
        let rest = match &wanted_file {
            Some(wanted) => {
                if safe_relative_path(name).map(|p| slash(&p)).as_deref() != Some(wanted.as_str()) {
                    continue;
                }
                wanted.clone()
            }
            None => match name.strip_prefix(prefix.as_str()) {
                Some(rest) => rest.to_string(),
                None => continue,
            },
        };
        let Some(target) = target_path(&job.dest, &rest) else {
            tracing::warn!("skipping unsafe entry {name:?} of {}", job.name);
            continue;
        };
        writer.start_file(slash(&target), options).map_err(|e| Error::Other(format!("Couldn't write {} ({e}).", dst.display())))?;
        io::copy(&mut entry, &mut writer).map_err(|e| Error::io(dst, e))?;
        written += 1;
    }
    writer.finish().map_err(|e| Error::Other(format!("Couldn't write {} ({e}).", dst.display())))?;
    Ok(written)
}

/// A name that is safe as part of a file name.
pub(crate) fn file_part(raw: &str) -> String {
    raw.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect()
}

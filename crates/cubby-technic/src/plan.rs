//! Planning a Technic install as a [`PackPlan`].
//!
//! Packs ship their loader inside `bin/`, which must not land in the game folder. `PackOverrides` cannot
//! exclude a folder, so each zip in the plan is a copy of the download without `bin/` (or unsafe paths).

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use cubby_common::net::{download_all, ensure_file, Download};
use cubby_common::packs::{safe_relative_path, PackOverrides, PackPlan};
use cubby_common::{Error, Result};

use crate::client::TechnicClient;
use crate::detect::{detect, Detected};
use crate::model::TechnicPack;

/// Simultaneous Solder mod downloads.
const SOLDER_CONCURRENCY: usize = 8;
/// `bin/modpack.jar` is a Forge jar, a few MiB; refuse anything absurd.
const MAX_BIN_FILE: u64 = 512 * 1024 * 1024;

/// A [`PackPlan`] with what it can't carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechnicPlan {
    pub plan: PackPlan,
    /// Legacy jar-mod packs (Forge for Minecraft 1.5.2 and older): the `bin/modpack.jar` to merge into
    /// `minecraft.jar`, extracted next to the archives and owned by the plan like them. `None` for packs
    /// whose loader is installed the regular way.
    pub jar_mod: Option<PathBuf>,
    /// The pack is a legacy jar-mod pack: `plan.loader` is only a hint (Forge, with the version of the
    /// jar's `forgeversion.properties` if it has one) and cannot be installed as a normal loader.
    pub legacy_jar_mod: bool,
    /// Things worth telling the player (dropped files, unknown loader).
    pub notes: Vec<String>,
}

impl TechnicClient {
    /// Plans the install of one version of a pack (the build of a Solder pack; any id for a zip pack,
    /// which only has its current version). The pack zips are downloaded into a folder of `cache_dir`;
    /// the plan owns them.
    pub async fn plan(&self, slug: &str, version_id: &str, cache_dir: &Path) -> Result<PackPlan> {
        Ok(self.plan_detailed(slug, version_id, cache_dir).await?.plan)
    }

    /// [`TechnicClient::plan`] with the extra information of a [`TechnicPlan`].
    pub async fn plan_detailed(&self, slug: &str, version_id: &str, cache_dir: &Path) -> Result<TechnicPlan> {
        let pack = self.pack(slug).await?;
        let version_id = version_id.trim();
        let (version_name, build_minecraft, memory, sources) = if pack.is_solder() {
            if version_id.is_empty() {
                return Err(Error::Other(format!("Choose a version of {}.", pack.display_name)));
            }
            let build = self.solder_build(&pack, version_id).await?;
            if build.mods.is_empty() {
                return Err(Error::Other(format!("Build {version_id} of {} has no mods.", pack.display_name)));
            }
            let sources: Vec<(String, String)> = build.mods.iter().map(|m| (m.url.clone(), m.name.clone())).collect();
            (version_id.to_string(), build.minecraft.filter(|m| !m.trim().is_empty()), build.memory, sources)
        } else {
            let url = pack.pack_url.clone().ok_or_else(|| Error::Other(format!("{} has nothing to download.", pack.display_name)))?;
            let version = pack.version.clone().unwrap_or_else(|| "current".to_string());
            (version, None, 0, vec![(url, "pack".to_string())])
        };
        let version_id = if version_id.is_empty() { version_name.clone() } else { version_id.to_string() };

        let dir = cache_dir.join(format!("technic-{}-{}", file_part(&pack.name), file_part(&version_id)));
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error::io(&dir, e)),
        }
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;

        let mut jobs = Vec::new();
        for (i, (url, _)) in sources.iter().enumerate() {
            check_url(url)?;
            jobs.push(Download::new(url.clone(), dir.join(format!("{i:03}.src.zip"))));
        }
        if jobs.len() == 1 {
            ensure_file(&self.http, &jobs[0]).await?;
        } else {
            download_all(&self.http, jobs, SOLDER_CONCURRENCY, |_, _| {}).await?;
        }

        let mut archives = Vec::new();
        let mut modpack_jar = None;
        let mut version_json = None;
        let mut notes = Vec::new();
        for (i, (_, name)) in sources.iter().enumerate() {
            let src = dir.join(format!("{i:03}.src.zip"));
            let dest = dir.join(format!("{i:03}-{}.zip", file_part(name)));
            let scan = {
                let (src, dest) = (src.clone(), dest.clone());
                tokio::task::spawn_blocking(move || strip_and_scan(&src, &dest)).await.map_err(|e| Error::Other(e.to_string()))??
            };
            if scan.dropped_unsafe > 0 {
                notes.push(format!("{} file(s) with an unsafe path were left out of \"{name}\".", scan.dropped_unsafe));
            }
            // Zips are extracted in order into one folder: a later bin/ file replaces an earlier one.
            modpack_jar = scan.modpack_jar.or(modpack_jar);
            version_json = scan.version_json.or(version_json);
            archives.push(dest);
        }

        let hint = build_minecraft.or_else(|| pack.minecraft.clone());
        let Detected { minecraft, loader, legacy_jar_mod } = detect(modpack_jar.as_deref(), version_json.as_deref(), hint.as_deref())?;

        let mut jar_mod = None;
        if legacy_jar_mod {
            if let Some(jar) = &modpack_jar {
                let path = dir.join("modpack.jar");
                std::fs::write(&path, jar).map_err(|e| Error::io(&path, e))?;
                jar_mod = Some(path);
            }
            notes.push("This pack patches Minecraft with a jar mod (Minecraft 1.5.2 or older): its Forge can't be installed as a regular loader.".to_string());
        } else if loader.is_none() && (modpack_jar.is_some() || version_json.is_some()) {
            notes.push("The pack's bin/ folder doesn't name a known mod loader.".to_string());
        }

        let plan = PackPlan {
            provider: "technic".to_string(),
            pack_id: pack.name.clone(),
            version_id,
            title: pack.display_name.clone(),
            version_name,
            minecraft,
            loader,
            icon_url: pack.icon_url.clone(),
            files: Vec::new(),
            overrides: archives.into_iter().map(|archive| PackOverrides { archive, prefix: String::new() }).collect(),
            blocked: Vec::new(),
            recommended_memory_mib: (memory > 0).then(|| u32::try_from(memory).unwrap_or(u32::MAX)),
        };
        log_plan(&pack, &plan);
        Ok(TechnicPlan { plan, jar_mod, legacy_jar_mod, notes })
    }
}

fn log_plan(pack: &TechnicPack, plan: &PackPlan) {
    tracing::debug!("Technic plan for {}: Minecraft {}, loader {:?}, {} archive(s)", pack.name, plan.minecraft, plan.loader, plan.overrides.len());
}

/// Solder serves mods from anywhere: only http(s) is downloaded.
fn check_url(url: &str) -> Result<()> {
    match reqwest::Url::parse(url) {
        Ok(u) if matches!(u.scheme(), "http" | "https") => Ok(()),
        _ => Err(Error::Other(format!("\"{url}\" isn't a downloadable address."))),
    }
}

/// A name that is safe as a file name part.
fn file_part(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect();
    let cleaned = cleaned.trim_matches('.').to_string();
    if cleaned.is_empty() {
        "_".to_string()
    } else {
        cleaned.chars().take(80).collect()
    }
}

/// What a pack zip held in `bin/`.
#[derive(Debug, Default)]
pub(crate) struct Scan {
    pub modpack_jar: Option<Vec<u8>>,
    pub version_json: Option<Vec<u8>>,
    /// Entries dropped for a path outside the game directory.
    pub dropped_unsafe: usize,
}

/// Reads `bin/modpack.jar` and `bin/version.json` of `src` and writes `dest`, a copy of `src` without the
/// `bin/` entries nor the entries with an unsafe path (`src` is renamed when nothing has to go; it is
/// deleted otherwise).
pub(crate) fn strip_and_scan(src: &Path, dest: &Path) -> Result<Scan> {
    let zip_error = |e: zip::result::ZipError| Error::Other(format!("{} isn't a valid archive ({e}).", src.display()));
    let file = File::open(src).map_err(|e| Error::io(src, e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error)?;

    let mut scan = Scan::default();
    let mut keep = Vec::new();
    let mut skipped = 0usize;
    for i in 0..archive.len() {
        let (name, is_dir) = {
            let entry = archive.by_index_raw(i).map_err(zip_error)?;
            (entry.name().replace('\\', "/"), entry.is_dir())
        };
        let lower = name.trim_start_matches("./").to_ascii_lowercase();
        if lower == "bin" || lower.starts_with("bin/") {
            skipped += 1;
            let wanted = match lower.as_str() {
                "bin/modpack.jar" => Some(&mut scan.modpack_jar),
                "bin/version.json" => Some(&mut scan.version_json),
                _ => None,
            };
            if let Some(slot) = wanted {
                let entry = archive.by_index(i).map_err(zip_error)?;
                let mut data = Vec::new();
                entry.take(MAX_BIN_FILE).read_to_end(&mut data).map_err(|e| Error::io(src, e))?;
                *slot = Some(data);
            }
            continue;
        }
        if safe_relative_path(&name).is_none() {
            // "./" and "/" alone are harmless directory entries.
            skipped += 1;
            if !is_dir {
                scan.dropped_unsafe += 1;
            }
            continue;
        }
        keep.push(i);
    }

    if skipped == 0 {
        std::fs::rename(src, dest).map_err(|e| Error::io(dest, e))?;
        return Ok(scan);
    }
    let out = File::create(dest).map_err(|e| Error::io(dest, e))?;
    let mut writer = zip::ZipWriter::new(out);
    for i in keep {
        let entry = archive.by_index_raw(i).map_err(zip_error)?;
        writer.raw_copy_file(entry).map_err(|e| Error::Other(format!("Couldn't write {} ({e}).", dest.display())))?;
    }
    let mut out = writer.finish().map_err(|e| Error::Other(format!("Couldn't write {} ({e}).", dest.display())))?;
    out.flush().map_err(|e| Error::io(dest, e))?;
    drop(out);
    drop(archive);
    std::fs::remove_file(src).map_err(|e| Error::io(src, e))?;
    Ok(scan)
}

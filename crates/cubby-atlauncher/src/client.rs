//! Read-only access to the ATLauncher CDN: the pack list, per-version manifests and the archives a plan needs.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cubby_common::net::{download_all, Download, Http};
use cubby_common::packs::{PackOverrides, PackPage, PackPlan, PackSort, PackSummary, PackVersion};
use cubby_common::{Error, Result};

use crate::index;
use crate::model::{AtlManifest, IndexPack};
use crate::plan::{self, ArchiveJob};

/// The ATLauncher download server (`BuildConfig.ATL_DOWNLOAD_SERVER_URL` in Prism, without the trailing `/`).
pub const ATL_BASE: &str = "https://download.nodecdn.net/containers/atl";

/// The list is about 500 KB; it is kept this long between requests.
const INDEX_TTL: Duration = Duration::from_secs(10 * 60);
/// Archives downloaded at once while planning.
const ARCHIVE_CONCURRENCY: usize = 4;

type IndexCache = Arc<Mutex<Option<(Instant, Arc<Vec<IndexPack>>)>>>;

/// A plan with what the contract has no room for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtlPlan {
    pub plan: PackPlan,
    /// MD5 of the files of `plan.files` and `plan.blocked`, by path, lowercase hex. ATLauncher only
    /// publishes MD5, so `PackFile::sha1` stays `None`; an app that wants to verify downloads can check these.
    pub md5: std::collections::HashMap<PathBuf, String>,
    /// What was left out or approximated, ready to show to the player.
    pub warnings: Vec<String>,
}

/// ATLauncher CDN client. Cheap to clone; clones share the cached pack list.
#[derive(Clone)]
pub struct AtLauncherClient {
    http: Http,
    base: String,
    index: IndexCache,
}

impl AtLauncherClient {
    /// Client for the real CDN over the shared HTTP client (and its proxy settings).
    pub fn new(http: Http) -> Self {
        Self::with_base_url(http, ATL_BASE)
    }

    /// Client for another server root (tests): it must serve `launcher/json/packsnew.json` and `packs/...`.
    pub fn with_base_url(http: Http, base: impl Into<String>) -> Self {
        Self { http, base: base.into().trim_end_matches('/').to_string(), index: Arc::new(Mutex::new(None)) }
    }

    pub fn http(&self) -> &Http {
        &self.http
    }

    async fn index(&self) -> Result<Arc<Vec<IndexPack>>> {
        if let Some((at, packs)) = self.index.lock().unwrap().as_ref() {
            if at.elapsed() < INDEX_TTL {
                return Ok(packs.clone());
            }
        }
        let url = format!("{}/launcher/json/packsnew.json", self.base);
        let bytes = self.http.get_bytes(&url).await?;
        let packs = Arc::new(index::parse_index(&bytes, &url)?);
        *self.index.lock().unwrap() = Some((Instant::now(), packs.clone()));
        Ok(packs)
    }

    /// Every public pack that has a release, in the CDN's order. Ids are the pack's folder name on the CDN.
    pub async fn packs(&self) -> Result<Vec<PackSummary>> {
        Ok(self.index().await?.iter().map(|p| index::summary(p, &self.base)).collect())
    }

    /// Packs whose name or description contains every word of `query`. The list is small and has no
    /// popularity data: `Downloads`, `Updated` and `Newest` keep the CDN's own order.
    pub async fn search(&self, query: &str, sort: PackSort) -> Result<PackPage> {
        Ok(index::search(&self.index().await?, query, sort, &self.base))
    }

    /// Released versions of a pack, newest first. Versions the pack marks as not recommended are betas.
    pub async fn versions(&self, pack_id: &str) -> Result<Vec<PackVersion>> {
        Ok(self.find(pack_id).await?.versions.iter().map(index::version).collect())
    }

    async fn find(&self, pack_id: &str) -> Result<IndexPack> {
        self.index()
            .await?
            .iter()
            .find(|p| index::safe_name(&p.name) == pack_id)
            .cloned()
            .ok_or_else(|| Error::NotFound(format!("the ATLauncher pack {pack_id}")))
    }

    fn version_url(&self, pack_id: &str, version: &str, file: &str) -> Result<String> {
        check_pack_id(pack_id)?;
        if version.trim().is_empty() || version == "." || version == ".." {
            return Err(Error::Other(format!("{version:?} isn't a valid ATLauncher pack version.")));
        }
        Ok(format!("{}/packs/{pack_id}/versions/{}/{file}", self.base, encode_segment(version)))
    }

    /// `Configs.json` of one version.
    pub async fn manifest(&self, pack_id: &str, version: &str) -> Result<AtlManifest> {
        let url = self.version_url(pack_id, version, "Configs.json")?;
        match self.http.get_json(&url).await {
            Err(Error::HttpStatus { status: 404, .. }) => Err(Error::NotFound(format!("version {version} of the ATLauncher pack {pack_id}"))),
            other => other,
        }
    }

    /// Plans an install. `selected_optional` holds the names of the optional mods to install (see
    /// [`crate::OptionalSelection`] for the picker rules). The configs archive and the archives of
    /// `extract` and `decomp` mods are downloaded into `cache_dir` and handed over as overrides: the app
    /// removes them after installing. Fails, listing why, for packs that can't be installed.
    pub async fn plan(&self, pack_id: &str, version: &str, selected_optional: &[String], cache_dir: &Path) -> Result<PackPlan> {
        Ok(self.plan_detailed(pack_id, version, selected_optional, cache_dir).await?.plan)
    }

    /// Like [`Self::plan`], with the MD5 hashes and warnings.
    pub async fn plan_detailed(&self, pack_id: &str, version: &str, selected_optional: &[String], cache_dir: &Path) -> Result<AtlPlan> {
        let manifest = self.manifest(pack_id, version).await?;
        let listed = self.find(pack_id).await.ok();
        let title = listed.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| pack_id.to_string());
        let icon_url = listed.as_ref().and_then(|p| index::summary(p, &self.base).icon_url);

        let mut built = plan::build(&manifest, selected_optional, &self.base);
        if !built.fatal.is_empty() {
            return Err(Error::Other(format!("{title} {version} can't be installed: {}.", built.fatal.join("; "))));
        }
        let minecraft = if manifest.minecraft.is_empty() {
            return Err(Error::Other(format!("{title} {version} doesn't say which Minecraft version it runs.")));
        } else {
            manifest.minecraft.clone()
        };

        tokio::fs::create_dir_all(cache_dir).await.map_err(|e| Error::io(cache_dir, e))?;
        let stem = format!("atl-{}-{}", plan::file_part(pack_id), plan::file_part(version));
        let mut created: Vec<PathBuf> = Vec::new();
        let result = self.fetch_overrides(pack_id, version, &manifest, &built.archives, cache_dir, &stem, &mut created).await;
        let (overrides, mut warnings) = match result {
            Ok(done) => done,
            Err(error) => {
                for path in &created {
                    let _ = tokio::fs::remove_file(path).await;
                }
                return Err(error);
            }
        };
        built.warnings.append(&mut warnings);

        let plan = PackPlan {
            provider: "atlauncher".into(),
            pack_id: pack_id.to_string(),
            version_id: version.to_string(),
            title,
            version_name: if manifest.version.is_empty() { version.to_string() } else { manifest.version.clone() },
            minecraft,
            loader: built.loader,
            icon_url,
            files: built.files,
            overrides,
            blocked: built.blocked,
            recommended_memory_mib: built.memory_mib,
        };
        Ok(AtlPlan { plan, md5: built.md5, warnings: built.warnings })
    }

    /// Downloads the configs and the archives of the extract mods; returns the overrides in the order
    /// Prism applies them (configs first, mods over them) and the warnings met. Every file it writes is
    /// pushed to `created`.
    #[allow(clippy::too_many_arguments)]
    async fn fetch_overrides(
        &self,
        pack_id: &str,
        version: &str,
        manifest: &AtlManifest,
        archives: &[ArchiveJob],
        cache_dir: &Path,
        stem: &str,
        created: &mut Vec<PathBuf>,
    ) -> Result<(Vec<PackOverrides>, Vec<String>)> {
        let mut warnings = Vec::new();
        let mut jobs = Vec::new();

        let configs = match manifest.configs.as_ref().filter(|_| !manifest.no_configs) {
            Some(configs) => {
                let path = cache_dir.join(format!("{stem}-configs.zip"));
                let mut job = Download::new(self.version_url(pack_id, version, "Configs.zip")?, &path);
                if let Some(sha1) = configs.sha1.as_deref().filter(|s| !s.is_empty()) {
                    job = job.with_sha1(sha1);
                }
                if let Some(size) = configs.filesize {
                    job = job.with_size(size);
                }
                jobs.push(job);
                created.push(path.clone());
                Some(path)
            }
            None => None,
        };

        let mut archive_paths = Vec::new();
        for (i, archive) in archives.iter().enumerate() {
            let path = cache_dir.join(format!("{stem}-extract-{i}-{}.zip", archive.md5.as_deref().map(plan::file_part).unwrap_or_else(|| "nohash".into())));
            if archive.md5.is_none() {
                // Nothing to tell a stale file from a good one.
                let _ = tokio::fs::remove_file(&path).await;
            }
            jobs.push(Download::new(archive.url.clone(), &path));
            created.push(path.clone());
            archive_paths.push(path);
        }
        download_all(&self.http, jobs, ARCHIVE_CONCURRENCY, |_, _| {}).await?;

        let mut overrides = Vec::new();
        if let Some(path) = configs {
            overrides.push(PackOverrides { archive: path, prefix: String::new() });
        }
        for (archive, path) in archives.iter().zip(archive_paths) {
            if archive.dest.is_empty() && archive.file.is_none() {
                // Already where the contract wants it: a folder of the zip into the game directory.
                let prefix = if archive.prefix.is_empty() { String::new() } else { format!("{}/", archive.prefix) };
                overrides.push(PackOverrides { archive: path, prefix });
                continue;
            }
            let repacked = path.with_extension("repack.zip");
            created.push(repacked.clone());
            let (src, dst, job) = (path.clone(), repacked.clone(), archive.clone());
            let written = tokio::task::spawn_blocking(move || plan::repack(&src, &dst, &job)).await.map_err(|e| Error::Other(e.to_string()))??;
            let _ = tokio::fs::remove_file(&path).await;
            if written == 0 {
                let _ = tokio::fs::remove_file(&repacked).await;
                warnings.push(format!("\"{}\" has nothing to extract in its archive, left out.", archive.name));
                continue;
            }
            overrides.push(PackOverrides { archive: repacked, prefix: String::new() });
        }
        Ok((overrides, warnings))
    }
}

impl AtlManifest {
    /// Why this version can't be installed with its default optional mods; empty when it can.
    /// Lets a browser mark packs that would fail (jar mods, a loader version the launcher has to pick).
    pub fn unsupported_reasons(&self) -> Vec<String> {
        let defaults: Vec<String> = crate::OptionalSelection::new(self).names();
        plan::build(self, &defaults, ATL_BASE).fatal
    }
}

/// Pack folders are letters and digits only (see [`index::safe_name`]); anything else could escape the URL path.
fn check_pack_id(id: &str) -> Result<()> {
    if !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        Ok(())
    } else {
        Err(Error::Other(format!("{id:?} isn't a valid ATLauncher pack ID.")))
    }
}

/// Percent-encodes a URL path segment (everything but letters, digits and `-._~`).
fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_segments() {
        assert!(check_pack_id("DigitalReality").is_ok());
        for bad in ["", "..", "a/b", "a b", "a.b", "é"] {
            assert!(check_pack_id(bad).is_err(), "{bad}");
        }
        assert_eq!(encode_segment("1.0.0-beta_2"), "1.0.0-beta_2");
        assert_eq!(encode_segment("v1 +x/../y"), "v1%20%2Bx%2F..%2Fy");
    }

    #[test]
    fn version_urls() {
        let client = AtLauncherClient::with_base_url(Http::new().unwrap(), "https://cdn.test/atl/");
        assert_eq!(client.version_url("Pack", "1.0 beta", "Configs.json").unwrap(), "https://cdn.test/atl/packs/Pack/versions/1.0%20beta/Configs.json");
        assert!(client.version_url("../x", "1", "Configs.json").is_err());
        assert!(client.version_url("Pack", "..", "Configs.json").is_err());
        assert!(client.version_url("Pack", "", "Configs.json").is_err());
    }
}

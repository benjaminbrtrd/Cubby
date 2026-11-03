//! HTTP access to the Technic Platform API and to Solder servers.
//!
//! Every platform request needs `?build=`; a missing or unknown build answers 401. Prism sends `multimc`,
//! which never goes stale. `/search` returns at most 15 packs, without paging, and rejects an empty query.
//! Solder servers list builds in any order, so the order is normalised with `latest`.

use reqwest::Url;
use serde::de::DeserializeOwned;
use cubby_common::net::Http;
use cubby_common::packs::{PackChannel, PackPage, PackSummary, PackVersion};
use cubby_common::{Error, Result};

use crate::model::*;

pub const API_BASE: &str = "https://api.technicpack.net";
/// The `build` Prism Launcher sends.
pub const API_BUILD: &str = "multimc";

/// Technic Platform client. Cheap to clone.
#[derive(Clone)]
pub struct TechnicClient {
    pub(crate) http: Http,
    base: String,
    build: String,
}

impl TechnicClient {
    /// Client for `https://api.technicpack.net` over the shared HTTP client (and its proxy settings).
    pub fn new(http: Http) -> Self {
        Self::with_base_url(http, API_BASE)
    }

    /// Client for another API root (tests).
    pub fn with_base_url(http: Http, base: impl Into<String>) -> Self {
        Self { http, base: base.into().trim_end_matches('/').to_string(), build: API_BUILD.to_string() }
    }

    /// Same client sending another launcher `build` number.
    pub fn with_build(mut self, build: impl Into<String>) -> Self {
        self.build = build.into();
        self
    }

    pub fn http(&self) -> &Http {
        &self.http
    }

    // browsing

    /// The trending packs (`/trending`).
    pub async fn featured(&self) -> Result<PackPage> {
        let url = make_url(&self.base, &["trending"], &[("build", &self.build)])?;
        let raw: RawSearch = self.get(&url, "Technic trending packs").await?;
        Ok(page(raw))
    }

    /// Packs matching `query` (at most 15; the API doesn't page). An empty query gives the trending packs,
    /// since the API refuses it.
    pub async fn search(&self, query: &str) -> Result<PackPage> {
        let query = query.trim();
        if query.is_empty() {
            return self.featured().await;
        }
        let url = make_url(&self.base, &["search"], &[("build", &self.build), ("q", query)])?;
        let raw: RawSearch = self.get(&url, "Technic search").await?;
        Ok(page(raw))
    }

    /// The page of one pack. `slug` is the id of a [`PackSummary`].
    pub async fn pack(&self, slug: &str) -> Result<TechnicPack> {
        let url = make_url(&self.base, &["modpack", check_segment(slug)?], &[("build", &self.build)])?;
        let raw: RawPack = self.get(&url, &format!("Technic pack {slug}")).await?;
        Ok(raw.into_pack())
    }

    /// The versions of a pack: the builds of a Solder pack (the recommended one first, marked in its name;
    /// newest first), or the single current version of a zip pack.
    pub async fn versions(&self, slug: &str) -> Result<Vec<PackVersion>> {
        let pack = self.pack(slug).await?;
        if !pack.is_solder() {
            let version = pack.version.clone().unwrap_or_else(|| "current".to_string());
            return Ok(vec![PackVersion {
                id: version.clone(),
                name: version,
                channel: PackChannel::Release,
                minecraft: pack.minecraft.clone(),
                loader: None,
                released: None,
            }]);
        }
        let listing = self.solder_pack(&pack).await?;
        Ok(solder_versions(listing))
    }

    /// The Minecraft version the pack installs: the recommended (else latest) Solder build's, which is
    /// right more often than the pack page's (Tekkit Legends' page says 1.4.7, its builds are 1.7.10).
    pub async fn minecraft_of(&self, pack: &TechnicPack) -> Result<Option<String>> {
        if !pack.is_solder() {
            return Ok(pack.minecraft.clone());
        }
        let listing = self.solder_pack(pack).await?;
        let Some(build) = listing.recommended.or(listing.latest).or_else(|| listing.builds.last().cloned()) else {
            return Ok(pack.minecraft.clone());
        };
        Ok(self.solder_build(pack, &build).await?.minecraft.or_else(|| pack.minecraft.clone()))
    }

    // Solder

    pub(crate) async fn solder_pack(&self, pack: &TechnicPack) -> Result<RawSolderPack> {
        let root = pack.solder_url.as_deref().ok_or_else(|| Error::Other(format!("{} isn't a Solder pack.", pack.display_name)))?;
        let url = make_url(root, &["modpack", check_segment(&pack.name)?], &[])?;
        self.get(&url, &format!("Solder pack {}", pack.name)).await
    }

    pub(crate) async fn solder_build(&self, pack: &TechnicPack, build: &str) -> Result<SolderBuild> {
        let root = pack.solder_url.as_deref().ok_or_else(|| Error::Other(format!("{} isn't a Solder pack.", pack.display_name)))?;
        let url = make_url(root, &["modpack", check_segment(&pack.name)?, check_segment(build)?], &[])?;
        self.get(&url, &format!("build {build} of {}", pack.display_name)).await
    }

    // plumbing

    /// GET and decode; 404 becomes `Error::NotFound`.
    async fn get<T: DeserializeOwned>(&self, url: &Url, what: &str) -> Result<T> {
        let response = self.http.client().get(url.clone()).send().await.map_err(|e| Error::http(url.as_str(), e))?;
        let status = response.status().as_u16();
        if status == 404 {
            return Err(Error::NotFound(what.to_string()));
        }
        if !response.status().is_success() {
            return Err(Error::HttpStatus { url: url.to_string(), status });
        }
        let bytes = response.bytes().await.map_err(|e| Error::http(url.as_str(), e))?;
        serde_json::from_slice(&bytes).map_err(|e| Error::json(url.as_str(), e))
    }
}

/// Slugs and builds go into a URL path: nothing that could change the path is accepted.
fn check_segment(s: &str) -> Result<&str> {
    if s.is_empty() || s == "." || s == ".." || s.len() > 200 || s.chars().any(|c| c.is_control() || c == '/' || c == '\\') {
        return Err(Error::Other(format!("\"{s}\" isn't a valid pack or build name.")));
    }
    Ok(s)
}

/// `base` + path segments (percent-encoded) + query.
pub(crate) fn make_url(base: &str, segments: &[&str], query: &[(&str, &str)]) -> Result<Url> {
    let mut url = Url::parse(base).map_err(|e| Error::Other(format!("\"{base}\" isn't a valid URL ({e}).")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(Error::Other(format!("\"{base}\" isn't an http(s) URL.")));
    }
    url.path_segments_mut().map_err(|_| Error::Other(format!("\"{base}\" can't be used as an API root.")))?.pop_if_empty().extend(segments);
    if !query.is_empty() {
        url.query_pairs_mut().extend_pairs(query);
    }
    Ok(url)
}

fn page(raw: RawSearch) -> PackPage {
    let hits: Vec<PackSummary> = raw
        .modpacks
        .into_iter()
        // Prism hides the placeholder "vanilla" pack.
        .filter(|h| h.slug != "vanilla")
        .map(|h| PackSummary {
            id: h.slug,
            name: h.name,
            summary: String::new(),
            authors: Vec::new(),
            icon_url: h.icon_url.filter(|u| !u.is_empty()),
            website_url: h.url.filter(|u| !u.is_empty()),
            downloads: None,
            updated: None,
            minecraft: Vec::new(),
            loaders: Vec::new(),
        })
        .collect();
    PackPage { hits, total: None }
}

/// Builds newest first, the recommended one at the front, channels from `recommended` and `latest`: the
/// recommended build is a release, the builds newer than it are betas, the older ones releases.
fn solder_versions(listing: RawSolderPack) -> Vec<PackVersion> {
    let mut builds = listing.builds;
    let recommended = listing.recommended.filter(|r| !r.is_empty());
    let latest = listing.latest.filter(|l| !l.is_empty());
    // Servers list oldest first or newest first; `latest` is at the end of the list that tells which.
    if let (Some(latest), Some(first), Some(last)) = (latest.as_deref(), builds.first(), builds.last()) {
        if first != latest && last == latest {
            builds.reverse();
        }
    }
    let newer_than_recommended: Vec<String> = match recommended.as_deref().and_then(|r| builds.iter().position(|b| b == r)) {
        Some(i) => builds[..i].to_vec(),
        None => Vec::new(),
    };
    if let Some(i) = recommended.as_deref().and_then(|r| builds.iter().position(|b| b == r)) {
        let build = builds.remove(i);
        builds.insert(0, build);
    }
    builds
        .into_iter()
        .map(|build| {
            let is_recommended = Some(&build) == recommended.as_ref();
            let is_latest = Some(&build) == latest.as_ref();
            let channel = if newer_than_recommended.contains(&build) || (is_latest && !is_recommended && recommended.is_some()) {
                PackChannel::Beta
            } else {
                PackChannel::Release
            };
            let name = match (is_recommended, is_latest) {
                (true, _) => format!("{build} (recommended)"),
                (false, true) => format!("{build} (latest)"),
                _ => build.clone(),
            };
            PackVersion { id: build, name, channel, minecraft: None, loader: None, released: None }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_encoded() {
        let url = make_url("https://api.technicpack.net/", &["search"], &[("build", "multimc"), ("q", "hexxit ii&x=1")]).unwrap();
        assert_eq!(url.as_str(), "https://api.technicpack.net/search?build=multimc&q=hexxit+ii%26x%3D1");
        let url = make_url("https://solder.example/api/", &["modpack", "my pack", "1.0/../x"], &[]).unwrap();
        assert_eq!(url.as_str(), "https://solder.example/api/modpack/my%20pack/1.0%2F..%2Fx");
        assert!(make_url("file:///etc", &["x"], &[]).is_err());
        assert!(check_segment("..").is_err() && check_segment("a/b").is_err() && check_segment("").is_err());
        assert!(check_segment("2.0.1b").is_ok());
    }
}

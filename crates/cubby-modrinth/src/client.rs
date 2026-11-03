//! HTTP access to the Modrinth API: every request carries the Cubby User-Agent (Modrinth requires a
//! unique one), 429 answers are retried after the rate-limit reset, 404 becomes `Error::NotFound`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::de::DeserializeOwned;
use serde::Serialize;
use cubby_common::net::Http;
use cubby_common::{Error, Result};

use crate::api;

pub const API_BASE: &str = "https://api.modrinth.com/v2";
pub const USER_AGENT: &str = concat!("benjaminbrtrd/Cubby/", env!("CARGO_PKG_VERSION"), " (+https://github.com/benjaminbrtrd/Cubby)");

/// Ids per batched lookup (`/projects?ids=`, `/versions?ids=`), to keep URLs short.
const IDS_PER_REQUEST: usize = 100;
/// Hashes per `POST /version_files` request.
const HASHES_PER_REQUEST: usize = 500;
const RATE_LIMIT_RETRIES: usize = 2;

/// SHA-1 of a file, valid while its size and modification time are unchanged.
#[derive(Debug, Clone)]
pub(crate) struct CachedHash {
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub sha1: String,
}

/// Modrinth API client. Cheap to clone; clones share the file hash cache.
#[derive(Clone)]
pub struct Modrinth {
    pub(crate) http: Http,
    base: String,
    pub(crate) hashes: Arc<Mutex<HashMap<PathBuf, CachedHash>>>,
}

impl Modrinth {
    /// Client for `https://api.modrinth.com/v2` over the shared HTTP client (and its proxy settings).
    pub fn new(http: Http) -> Self {
        Self::with_base_url(http, API_BASE)
    }

    /// Client for another API root (staging, tests).
    pub fn with_base_url(http: Http, base: impl Into<String>) -> Self {
        Self { http, base: base.into().trim_end_matches('/').to_string(), hashes: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn http(&self) -> &Http {
        &self.http
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    /// Sends the request built by `make` (rebuilt for retries) and decodes the JSON answer.
    async fn send<T: DeserializeOwned>(&self, url: &str, what: &str, make: impl Fn() -> reqwest::RequestBuilder) -> Result<T> {
        let mut attempt = 0;
        loop {
            let response = make()
                .header(reqwest::header::USER_AGENT, USER_AGENT)
                .send()
                .await
                .map_err(|e| Error::http(url, e))?;
            let status = response.status().as_u16();
            if status == 429 && attempt < RATE_LIMIT_RETRIES {
                attempt += 1;
                let wait = response
                    .headers()
                    .get("x-ratelimit-reset")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .unwrap_or(2)
                    .clamp(1, 15);
                tracing::debug!("Modrinth rate limit reached, retrying in {wait} s");
                tokio::time::sleep(Duration::from_secs(wait)).await;
                continue;
            }
            if status == 404 {
                return Err(Error::NotFound(what.to_string()));
            }
            if !response.status().is_success() {
                return Err(Error::HttpStatus { url: url.to_string(), status });
            }
            let bytes = response.bytes().await.map_err(|e| Error::http(url, e))?;
            return serde_json::from_slice(&bytes).map_err(|e| Error::json(url, e));
        }
    }

    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)], what: &str) -> Result<T> {
        let url = self.url(path);
        self.send(&url, what, || self.http.client().get(&url).query(query)).await
    }

    pub(crate) async fn post<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B, what: &str) -> Result<T> {
        let url = self.url(path);
        self.send(&url, what, || self.http.client().post(&url).json(body)).await
    }

    // endpoints

    pub(crate) async fn search_raw(&self, params: &[(&str, String)]) -> Result<api::SearchResponse> {
        self.get("/search", params, "Modrinth search").await
    }

    pub(crate) async fn project_raw(&self, id_or_slug: &str) -> Result<api::Project> {
        check_id(id_or_slug)?;
        self.get(&format!("/project/{id_or_slug}"), &[], &format!("Modrinth project {id_or_slug}")).await
    }

    pub(crate) async fn members_raw(&self, id_or_slug: &str) -> Result<Vec<api::TeamMember>> {
        check_id(id_or_slug)?;
        self.get(&format!("/project/{id_or_slug}/members"), &[], &format!("members of Modrinth project {id_or_slug}")).await
    }

    /// Projects by id or slug, in batches. Unknown ids are left out.
    pub(crate) async fn projects_raw(&self, ids: &[String]) -> Result<Vec<api::Project>> {
        let mut out = Vec::new();
        for chunk in dedup(ids).chunks(IDS_PER_REQUEST) {
            chunk.iter().try_for_each(|id| check_id(id))?;
            let ids = serde_json::to_string(chunk).map_err(|e| Error::Other(e.to_string()))?;
            let mut part: Vec<api::Project> = self.get("/projects", &[("ids", ids)], "Modrinth projects").await?;
            out.append(&mut part);
        }
        Ok(out)
    }

    /// Versions by id, in batches. Unknown ids are left out.
    pub(crate) async fn versions_raw(&self, ids: &[String]) -> Result<Vec<api::Version>> {
        let mut out = Vec::new();
        for chunk in dedup(ids).chunks(IDS_PER_REQUEST) {
            chunk.iter().try_for_each(|id| check_id(id))?;
            let ids = serde_json::to_string(chunk).map_err(|e| Error::Other(e.to_string()))?;
            let mut part: Vec<api::Version> = self.get("/versions", &[("ids", ids)], "Modrinth versions").await?;
            out.append(&mut part);
        }
        Ok(out)
    }

    pub(crate) async fn version_raw(&self, id: &str) -> Result<api::Version> {
        check_id(id)?;
        self.get(&format!("/version/{id}"), &[], &format!("Modrinth version {id}")).await
    }

    /// Versions of a project; empty filters are not sent.
    pub(crate) async fn project_versions_raw(&self, id_or_slug: &str, loaders: &[String], game_versions: &[String]) -> Result<Vec<api::Version>> {
        check_id(id_or_slug)?;
        let mut query = Vec::new();
        if !loaders.is_empty() {
            query.push(("loaders", serde_json::to_string(loaders).map_err(|e| Error::Other(e.to_string()))?));
        }
        if !game_versions.is_empty() {
            query.push(("game_versions", serde_json::to_string(game_versions).map_err(|e| Error::Other(e.to_string()))?));
        }
        self.get(&format!("/project/{id_or_slug}/version"), &query, &format!("versions of Modrinth project {id_or_slug}")).await
    }

    /// `POST /version_files`: the version each SHA-1 belongs to. Unknown hashes are left out.
    pub(crate) async fn version_files(&self, hashes: &[String]) -> Result<HashMap<String, api::Version>> {
        #[derive(Serialize)]
        struct Body<'a> {
            hashes: &'a [String],
            algorithm: &'static str,
        }
        let mut out = HashMap::new();
        for chunk in dedup(hashes).chunks(HASHES_PER_REQUEST) {
            let part: HashMap<String, api::Version> = self.post("/version_files", &Body { hashes: chunk, algorithm: "sha1" }, "Modrinth files").await?;
            out.extend(part.into_iter().map(|(k, v)| (k.to_lowercase(), v)));
        }
        Ok(out)
    }

    /// `POST /version_files/update`: for each SHA-1, the newest version of its project for these loaders
    /// and game versions.
    pub(crate) async fn latest_versions(&self, hashes: &[String], loaders: &[String], game_versions: &[String]) -> Result<HashMap<String, api::Version>> {
        #[derive(Serialize)]
        struct Body<'a> {
            hashes: &'a [String],
            algorithm: &'static str,
            loaders: &'a [String],
            game_versions: &'a [String],
        }
        let mut out = HashMap::new();
        for chunk in dedup(hashes).chunks(HASHES_PER_REQUEST) {
            let body = Body { hashes: chunk, algorithm: "sha1", loaders, game_versions };
            let part: HashMap<String, api::Version> = self.post("/version_files/update", &body, "Modrinth updates").await?;
            out.extend(part.into_iter().map(|(k, v)| (k.to_lowercase(), v)));
        }
        Ok(out)
    }
}

/// Ids, slugs and hashes only use `[A-Za-z0-9._-]`; anything else could escape the URL path.
pub(crate) fn check_id(id: &str) -> Result<()> {
    if !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) && id != "." && id != ".." {
        Ok(())
    } else {
        Err(Error::Other(format!("{id:?} isn't a valid Modrinth ID.")))
    }
}

/// Removes duplicates, keeping the first occurrence.
pub(crate) fn dedup(items: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    items.iter().filter(|i| seen.insert(i.as_str())).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_user_agent() {
        assert!(check_id("AANobbMI").is_ok());
        assert!(check_id("fabric-api").is_ok());
        assert!(check_id("a_b.c").is_ok());
        for bad in ["", "..", "a/b", "a?b", "a b", "%2e%2e", "é"] {
            assert!(check_id(bad).is_err(), "{bad}");
        }
        assert!(USER_AGENT.starts_with("benjaminbrtrd/Cubby/"));
        assert_eq!(dedup(&["a".into(), "b".into(), "a".into()]), ["a", "b"]);
    }
}

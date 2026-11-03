//! HTTP access to the FTB modpack API. Pack pages are cached for a few minutes because lists only
//! carry ids. The legacy `api.modpacks.ch` answers errors with HTTP 200 and `{"status":"error"}`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use cubby_common::net::Http;
use cubby_common::{Error, Result};

use crate::api;

pub const API_BASE: &str = "https://api.feed-the-beast.com/v1/modpacks/public/modpack";
/// Older host of the same API (same JSON, fewer packs, no slugs).
pub const LEGACY_API_BASE: &str = "https://api.modpacks.ch/public/modpack";

const CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const RATE_LIMIT_RETRIES: usize = 2;

type PackCache = Arc<Mutex<HashMap<u64, (Instant, Arc<api::Pack>)>>>;

/// FTB modpack API client. Cheap to clone; clones share the pack cache.
#[derive(Clone)]
pub struct FtbClient {
    pub(crate) http: Http,
    base: String,
    cache: PackCache,
}

impl FtbClient {
    /// Client for [`API_BASE`] over the shared HTTP client (and its proxy settings).
    pub fn new(http: Http) -> Self {
        Self::with_base_url(http, API_BASE)
    }

    /// Client for another API root (legacy host, tests).
    pub fn with_base_url(http: Http, base: impl Into<String>) -> Self {
        Self { http, base: base.into().trim_end_matches('/').to_string(), cache: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn http(&self) -> &Http {
        &self.http
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)], what: &str) -> Result<T> {
        let url = self.url(path);
        let mut attempt = 0;
        loop {
            let response = self.http.client().get(&url).query(query).send().await.map_err(|e| Error::http(&url, e))?;
            let status = response.status().as_u16();
            if status == 429 && attempt < RATE_LIMIT_RETRIES {
                attempt += 1;
                let wait = response.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(2).clamp(1, 15);
                tracing::debug!("FTB rate limit reached, retrying in {wait} s");
                tokio::time::sleep(Duration::from_secs(wait)).await;
                continue;
            }
            if status == 404 {
                return Err(Error::NotFound(what.to_string()));
            }
            if !response.status().is_success() {
                return Err(Error::HttpStatus { url, status });
            }
            let bytes = response.bytes().await.map_err(|e| Error::http(&url, e))?;
            return decode(&bytes, &url, what);
        }
    }

    // endpoints

    pub(crate) async fn pack_raw(&self, id: u64) -> Result<Arc<api::Pack>> {
        if let Some((at, pack)) = self.cache.lock().unwrap_or_else(|e| e.into_inner()).get(&id) {
            if at.elapsed() < CACHE_TTL {
                return Ok(pack.clone());
            }
        }
        let pack: api::Pack = self.get(&format!("/{id}"), &[], &format!("FTB modpack {id}")).await?;
        let pack = Arc::new(pack);
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).insert(id, (Instant::now(), pack.clone()));
        Ok(pack)
    }

    pub(crate) async fn version_raw(&self, pack: u64, version: u64) -> Result<api::VersionDetail> {
        self.get(&format!("/{pack}/{version}"), &[], &format!("version {version} of FTB modpack {pack}")).await
    }

    /// `/popular/installs/{n}`: most installed first.
    pub(crate) async fn popular_ids(&self, n: usize) -> Result<Vec<u64>> {
        let ids: api::PackIds = self.get(&format!("/popular/installs/{}", n.max(1)), &[], "popular FTB modpacks").await?;
        Ok(ids.packs)
    }

    /// `/featured/{n}`
    pub(crate) async fn featured_ids(&self, n: usize) -> Result<Vec<u64>> {
        let ids: api::PackIds = self.get(&format!("/featured/{}", n.max(1)), &[], "featured FTB modpacks").await?;
        Ok(ids.packs)
    }

    /// `/all`: every public pack id (about a hundred), newest first.
    pub(crate) async fn all_ids(&self) -> Result<Vec<u64>> {
        let ids: api::PackIds = self.get("/all", &[], "the list of FTB modpacks").await?;
        Ok(ids.packs)
    }

    /// `/search/{n}?term=`: pack ids in relevance order.
    pub(crate) async fn search_ids(&self, term: &str, n: usize) -> Result<Vec<u64>> {
        let ids: api::PackIds = self.get(&format!("/search/{}", n.max(1)), &[("term", term.to_string())], "FTB modpack search").await?;
        Ok(ids.packs)
    }
}

/// Decodes an answer, turning the legacy `{"status":"error"}` body into an error.
fn decode<T: DeserializeOwned>(bytes: &[u8], url: &str, what: &str) -> Result<T> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| Error::json(url, e))?;
    if value.get("status").and_then(|s| s.as_str()) == Some("error") {
        let message = value.get("message").or_else(|| value.get("error")).and_then(|m| m.as_str()).unwrap_or("unknown error");
        let lower = message.to_lowercase();
        return Err(if lower.contains("not found") || lower.contains("not valid") {
            Error::NotFound(what.to_string())
        } else {
            Error::Other(format!("FTB says: {message}"))
        });
    }
    serde_json::from_value(value).map_err(|e| Error::json(url, e))
}

/// Pack and version ids are numbers; anything else could escape the URL path.
pub(crate) fn parse_id(id: &str, what: &str) -> Result<u64> {
    id.trim().parse::<u64>().map_err(|_| Error::Other(format!("{id:?} isn't a valid FTB {what} ID.")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_legacy_errors() {
        assert_eq!(parse_id("100", "modpack").unwrap(), 100);
        for bad in ["", "..", "1/2", "a", "-1", "1?x"] {
            assert!(parse_id(bad, "modpack").is_err(), "{bad}");
        }
        let err = decode::<api::Pack>(br#"{"status":"error","message":"Modpack id not valid.","target":""}"#, "u", "pack 9").unwrap_err();
        assert!(matches!(err, Error::NotFound(_)), "{err}");
        let err = decode::<api::PackIds>(br#"{"status":"error","message":"Search term too short."}"#, "u", "search").unwrap_err();
        assert!(err.to_string().contains("too short"));
        assert!(decode::<api::PackIds>(br#"{"status":"success","packs":[1,2]}"#, "u", "x").unwrap().packs == [1, 2]);
    }
}

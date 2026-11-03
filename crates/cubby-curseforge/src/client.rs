//! HTTP access to the CurseForge API: every request carries the `x-api-key` header, 429 answers are
//! retried, 404 becomes `Error::NotFound`, 401/403 an `Error::Auth` about the key.

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;
use cubby_common::net::Http;
use cubby_common::{Error, Result};

pub const API_BASE: &str = "https://api.curseforge.com";
/// Minecraft.
pub const GAME_ID: u32 = 432;
/// "Modpacks" class of Minecraft.
pub const MODPACK_CLASS_ID: u32 = 4471;

const RATE_LIMIT_RETRIES: usize = 2;
const KEY_VARIABLE: &str = "CUBBY_CURSEFORGE_API_KEY";

/// Where [`api_key_with_source`] found the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeySource {
    /// The `CUBBY_CURSEFORGE_API_KEY` environment variable when the app runs.
    Environment,
    /// Compiled in by build.rs, from `CUBBY_CURSEFORGE_API_KEY` at build time or `secrets/curseforge-api-key.txt`.
    BuiltIn,
}

fn clean(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// The CurseForge API key and where it comes from: the environment when the app runs, else the one
/// compiled in by build.rs, else `None`. Empty values are ignored.
pub fn api_key_with_source() -> Option<(String, ApiKeySource)> {
    if let Some(key) = std::env::var(KEY_VARIABLE).ok().as_deref().and_then(clean) {
        return Some((key, ApiKeySource::Environment));
    }
    clean(&builtin_key()).map(|key| (key, ApiKeySource::BuiltIn))
}

include!(concat!(env!("OUT_DIR"), "/builtin_key.rs"));

/// The key compiled in by build.rs, unmasked; empty when the build had none.
fn builtin_key() -> String {
    // black_box: without it the optimizer unmasks the two constants at compile time and the plain key
    // ends up in the executable
    let mask = std::hint::black_box(BUILTIN_MASK);
    let bytes: Vec<u8> = BUILTIN_KEY.iter().zip(mask).map(|(b, m)| b ^ m).collect();
    String::from_utf8(bytes).unwrap_or_default()
}

/// The CurseForge API key of this run or build, if any (`CUBBY_CURSEFORGE_API_KEY`).
pub fn api_key_from_env() -> Option<String> {
    api_key_with_source().map(|(key, _)| key)
}

/// CurseForge API client. Cheap to clone.
#[derive(Clone)]
pub struct CurseForgeClient {
    pub(crate) http: Http,
    base: String,
    api_key: String,
    pub(crate) cdn_fallback: bool,
}

impl std::fmt::Debug for CurseForgeClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the key.
        f.debug_struct("CurseForgeClient").field("base", &self.base).field("has_key", &!self.api_key.is_empty()).finish()
    }
}

impl CurseForgeClient {
    /// Client for `https://api.curseforge.com` with this API key. A blank key makes every request fail
    /// with an `Error::Auth` that says a key is missing.
    pub fn new(http: Http, api_key: String) -> Self {
        Self::with_base_url(http, api_key, API_BASE)
    }

    /// Client for another API root (tests).
    pub fn with_base_url(http: Http, api_key: String, base: impl Into<String>) -> Self {
        Self { http, base: base.into().trim_end_matches('/').to_string(), api_key: api_key.trim().to_string(), cdn_fallback: false }
    }

    /// When true, a file without `downloadUrl` whose author doesn't forbid distribution gets the
    /// `edge.forgecdn.net` URL instead of being reported as blocked. Off by default, which is what
    /// Prism Launcher does (see [`CurseForgeClient::plan_from_zip`]).
    pub fn with_cdn_fallback(mut self, on: bool) -> Self {
        self.cdn_fallback = on;
        self
    }

    pub fn http(&self) -> &Http {
        &self.http
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    async fn send<T: DeserializeOwned>(&self, url: &str, what: &str, make: impl Fn() -> reqwest::RequestBuilder) -> Result<T> {
        if self.api_key.is_empty() {
            return Err(Error::Auth(format!("CurseForge needs an API key to be browsed. Set {KEY_VARIABLE} and restart Cubby.")));
        }
        let mut attempt = 0;
        loop {
            let response = make()
                .header("x-api-key", &self.api_key)
                .header(reqwest::header::ACCEPT, "application/json")
                .send()
                .await
                .map_err(|e| Error::http(url, e))?;
            let status = response.status().as_u16();
            if status == 429 && attempt < RATE_LIMIT_RETRIES {
                attempt += 1;
                tracing::debug!("CurseForge rate limit reached, retrying");
                tokio::time::sleep(Duration::from_secs(2 * attempt as u64)).await;
                continue;
            }
            match status {
                401 | 403 => return Err(Error::Auth(format!("CurseForge refused the API key (HTTP {status}). Check {KEY_VARIABLE}."))),
                404 => return Err(Error::NotFound(what.to_string())),
                _ if !response.status().is_success() => return Err(Error::HttpStatus { url: url.to_string(), status }),
                _ => {}
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_ignores_blank() {
        assert_eq!(clean("  "), None);
        assert_eq!(clean(" $2a$10$abc "), Some("$2a$10$abc".into()));
    }

    #[test]
    fn debug_hides_key() {
        let client = CurseForgeClient::new(Http::new().unwrap(), "secret-key".into());
        assert!(!format!("{client:?}").contains("secret-key"));
    }
}

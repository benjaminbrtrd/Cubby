//! HTTP client and parallel, verified downloads.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::StreamExt;
use serde::de::DeserializeOwned;
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

use crate::{Error, Result};

pub const USER_AGENT: &str = concat!("Cubby/", env!("CARGO_PKG_VERSION"), " (+https://github.com/benjaminbrtrd/Cubby)");

/// Default number of simultaneous downloads.
pub const DEFAULT_CONCURRENCY: usize = 16;

const RETRIES: usize = 3;

/// Shared HTTP client. Cheap to clone.
#[derive(Clone)]
pub struct Http {
    client: reqwest::Client,
}

/// How the HTTP client reaches the network.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProxyConfig {
    /// reqwest's default behaviour (system / environment proxy settings).
    #[default]
    System,
    /// Never use a proxy.
    None,
    /// Route everything through this proxy URL (e.g. `http://127.0.0.1:8080`).
    Manual(String),
}

impl Http {
    /// Client using the system proxy settings.
    pub fn new() -> Result<Self> {
        Self::with_proxy(&ProxyConfig::System)
    }

    /// Client using the given proxy configuration. An invalid manual proxy URL is an error.
    pub fn with_proxy(proxy: &ProxyConfig) -> Result<Self> {
        let mut builder = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(60));
        match proxy {
            ProxyConfig::System => {}
            ProxyConfig::None => builder = builder.no_proxy(),
            ProxyConfig::Manual(url) => {
                let proxy = reqwest::Proxy::all(url.trim()).map_err(|e| Error::Other(format!("The proxy URL \"{url}\" isn't valid ({e}). Check the proxy settings.")))?;
                builder = builder.proxy(proxy);
            }
        }
        let client = builder.build().map_err(|e| Error::http("<client>", e))?;
        Ok(Self { client })
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let bytes = self.get_bytes(url).await?;
        serde_json::from_slice(&bytes).map_err(|e| Error::json(url, e))
    }

    pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let response = self.client.get(url).send().await.map_err(|e| Error::http(url, e))?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::HttpStatus { url: url.to_string(), status: status.as_u16() });
        }
        let bytes = response.bytes().await.map_err(|e| Error::http(url, e))?;
        Ok(bytes.to_vec())
    }
}

/// One file to download. When `sha1` is set, an existing file with the same hash is kept and a
/// downloaded file with another hash is rejected.
#[derive(Debug, Clone)]
pub struct Download {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

impl Download {
    pub fn new(url: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self { url: url.into(), path: path.into(), sha1: None, size: None }
    }

    pub fn with_sha1(mut self, sha1: impl Into<String>) -> Self {
        self.sha1 = Some(sha1.into());
        self
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }
}

/// Downloads every job that is missing or corrupted, `concurrency` at a time.
///
/// `on_progress(done, total)` is called after each file (already valid files count as done).
/// Every job is attempted; the first error is returned at the end.
pub async fn download_all(http: &Http, jobs: Vec<Download>, concurrency: usize, mut on_progress: impl FnMut(u64, u64)) -> Result<()> {
    let mut seen = HashSet::new();
    let jobs: Vec<Download> = jobs.into_iter().filter(|job| seen.insert(job.path.clone())).collect();
    let total = jobs.len() as u64;
    let mut done = 0u64;
    on_progress(done, total);

    let mut results = futures::stream::iter(jobs.into_iter().map(|job| {
        let http = http.clone();
        async move { ensure_file(&http, &job).await }
    }))
    .buffer_unordered(concurrency.max(1));

    let mut first_error = None;
    while let Some(result) = results.next().await {
        if let Err(error) = result {
            tracing::warn!("download failed: {error}");
            first_error.get_or_insert(error);
        }
        done += 1;
        on_progress(done, total);
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Downloads a single file unless it is already present and valid.
pub async fn ensure_file(http: &Http, job: &Download) -> Result<()> {
    if is_valid(&job.path, job.sha1.as_deref()).await? {
        return Ok(());
    }
    let mut last_error = None;
    for attempt in 1..=RETRIES {
        match fetch_to(http, job).await {
            Ok(()) => return Ok(()),
            Err(error) => {
                tracing::debug!("attempt {attempt}/{RETRIES} for {} failed: {error}", job.url);
                last_error = Some(error);
                tokio::time::sleep(Duration::from_millis(400 * attempt as u64)).await;
            }
        }
    }
    Err(last_error.unwrap_or(Error::Other(format!("Couldn't download {}.", job.url))))
}

async fn is_valid(path: &Path, sha1: Option<&str>) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    match sha1 {
        None => Ok(true),
        Some(expected) => Ok(sha1_file(path).await?.eq_ignore_ascii_case(expected)),
    }
}

async fn fetch_to(http: &Http, job: &Download) -> Result<()> {
    if let Some(parent) = job.path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| Error::io(parent, e))?;
    }
    let response = http.client().get(&job.url).send().await.map_err(|e| Error::http(&job.url, e))?;
    let status = response.status();
    if !status.is_success() {
        return Err(Error::HttpStatus { url: job.url.clone(), status: status.as_u16() });
    }

    let part = job.path.with_extension("part");
    let mut file = tokio::fs::File::create(&part).await.map_err(|e| Error::io(&part, e))?;
    let mut hasher = Sha1::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| Error::http(&job.url, e))?;
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(|e| Error::io(&part, e))?;
    }
    file.flush().await.map_err(|e| Error::io(&part, e))?;
    drop(file);

    if let Some(expected) = &job.sha1 {
        let actual = hex::encode(hasher.finalize());
        if !actual.eq_ignore_ascii_case(expected) {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(Error::HashMismatch { path: job.path.clone(), expected: expected.clone(), actual });
        }
    }
    tokio::fs::rename(&part, &job.path).await.map_err(|e| Error::io(&job.path, e))?;
    Ok(())
}

/// Lowercase hex SHA-1 of a file.
pub async fn sha1_file(path: &Path) -> Result<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || sha1_file_blocking(&path)).await.map_err(|e| Error::Other(e.to_string()))?
}

/// [`sha1_file`] for synchronous code.
pub fn sha1_file_blocking(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|e| Error::io(path, e))?;
    let mut hasher = Sha1::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| Error::io(path, e))?;
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod proxy_tests {
    use super::*;

    #[test]
    fn proxy_configs() {
        assert!(Http::with_proxy(&ProxyConfig::System).is_ok());
        assert!(Http::with_proxy(&ProxyConfig::None).is_ok());
        assert!(Http::with_proxy(&ProxyConfig::Manual("http://127.0.0.1:8080".into())).is_ok());
        let error = Http::with_proxy(&ProxyConfig::Manual("not a url".into())).err().expect("invalid URL must fail");
        assert!(error.to_string().contains("The proxy URL \"not a url\" isn't valid"));
    }
}

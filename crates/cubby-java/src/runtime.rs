//! Mojang's own Java runtimes (`java-runtime-delta`...), downloaded into `paths.java()/<component>/`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cubby_common::contracts::JavaInstall;
use cubby_common::events::{Event, EventSink};
use cubby_common::net::{download_all, Download, Http, DEFAULT_CONCURRENCY};
use cubby_common::paths::Paths;
use cubby_common::{Error, Result};
use serde::Deserialize;

use crate::{detect::java_in_home, probe};

const ALL_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const MARKER: &str = ".cubby-version";

/// Key of this platform in Mojang's runtime index.
pub fn platform_key() -> &'static str {
    key_for(std::env::consts::OS, std::env::consts::ARCH)
}

fn key_for(os: &str, arch: &str) -> &'static str {
    match (os, arch) {
        ("windows", "x86") => "windows-x86",
        ("windows", "aarch64") => "windows-arm64",
        ("windows", _) => "windows-x64",
        ("macos", "aarch64") => "mac-os-arm64",
        ("macos", _) => "mac-os",
        (_, "x86") => "linux-i386",
        _ => "linux",
    }
}

/// `all.json`: platform -> component -> entries (usually one).
type Index = HashMap<String, HashMap<String, Vec<IndexEntry>>>;

#[derive(Deserialize)]
struct IndexEntry {
    manifest: Raw,
    version: VersionName,
}

#[derive(Deserialize)]
struct VersionName {
    name: String,
}

#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    sha1: String,
    #[serde(default)]
    size: u64,
    url: String,
}

#[derive(Deserialize)]
struct Manifest {
    files: HashMap<String, FileEntry>,
}

#[derive(Deserialize)]
struct FileEntry {
    #[serde(rename = "type")]
    kind: String,
    downloads: Option<Downloads>,
    #[serde(default)]
    executable: bool,
    target: Option<String>,
}

#[derive(Deserialize)]
struct Downloads {
    raw: Raw,
}

/// Downloads the Mojang runtime `component` (e.g. `java-runtime-delta`) for this platform and returns
/// it (`managed: true`). Files are SHA-1 verified; nothing is downloaded when `.cubby-version`
/// already holds the current manifest version. Progress is reported as "Downloading Java".
pub async fn ensure_runtime(http: &Http, paths: &Paths, component: &str, sink: &dyn EventSink, task: &str) -> Result<JavaInstall> {
    let key = platform_key();
    let index: Index = http.get_json(ALL_URL).await?;
    let entry = index
        .get(key)
        .and_then(|components| components.get(component))
        .and_then(|entries| entries.first())
        .ok_or_else(|| Error::Java(format!("Mojang doesn't provide the Java runtime \"{component}\" for {key}.")))?;

    let dir = paths.java().join(component);
    let marker = dir.join(MARKER);
    if tokio::fs::read_to_string(&marker).await.is_ok_and(|v| v.trim() == entry.version.name) {
        if let Some(exe) = java_in_home(&dir) {
            if let Ok(install) = probe(&exe).await {
                return Ok(JavaInstall { managed: true, ..install });
            }
        }
    }

    let manifest: Manifest = http.get_json(&entry.manifest.url).await?;
    tokio::fs::create_dir_all(&dir).await.map_err(|e| Error::io(&dir, e))?;

    let mut jobs = Vec::new();
    let mut executables = Vec::new();
    let mut links = Vec::new();
    for (name, file) in &manifest.files {
        let target = safe_join(&dir, name).ok_or_else(|| Error::Java(format!("The Java runtime manifest contains an unsafe path: \"{name}\".")))?;
        match file.kind.as_str() {
            "directory" => tokio::fs::create_dir_all(&target).await.map_err(|e| Error::io(&target, e))?,
            "file" => {
                let raw = &file.downloads.as_ref().ok_or_else(|| Error::Java(format!("The Java runtime file \"{name}\" has no download.")))?.raw;
                let mut job = Download::new(&raw.url, &target).with_size(raw.size);
                if !raw.sha1.is_empty() {
                    job = job.with_sha1(&raw.sha1);
                }
                jobs.push(job);
                if file.executable {
                    executables.push(target);
                }
            }
            "link" => {
                if let Some(link_target) = &file.target {
                    links.push((target, link_target.clone()));
                }
            }
            other => tracing::warn!("ignoring runtime entry \"{name}\" of unknown type \"{other}\""),
        }
    }

    download_all(http, jobs, DEFAULT_CONCURRENCY, |done, total| {
        sink.emit(Event::Progress { task: task.to_string(), label: "Downloading Java".into(), done, total });
    })
    .await?;

    for (link, target) in links {
        make_link(&link, &target).await?;
    }
    for exe in executables {
        make_executable(&exe).await?;
    }
    tokio::fs::write(&marker, &entry.version.name).await.map_err(|e| Error::io(&marker, e))?;

    let exe = java_in_home(&dir).ok_or_else(|| Error::Java(format!("The downloaded Java runtime {component} has no java executable.")))?;
    Ok(JavaInstall { managed: true, ..probe(&exe).await? })
}

/// `base/relative`, refusing absolute paths and `..`.
fn safe_join(base: &Path, relative: &str) -> Option<PathBuf> {
    cubby_common::packs::safe_relative_path(relative).map(|rel| base.join(rel))
}

#[cfg(unix)]
async fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).await.map_err(|e| Error::io(path, e))
}

#[cfg(not(unix))]
async fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
async fn make_link(link: &Path, target: &str) -> Result<()> {
    let _ = tokio::fs::remove_file(link).await;
    tokio::fs::symlink(target, link).await.map_err(|e| Error::io(link, e))
}

#[cfg(not(unix))]
async fn make_link(link: &Path, target: &str) -> Result<()> {
    tracing::warn!("symbolic link {} -> {target} skipped on this platform", link.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_keys() {
        assert_eq!(key_for("windows", "x86_64"), "windows-x64");
        assert_eq!(key_for("windows", "x86"), "windows-x86");
        assert_eq!(key_for("windows", "aarch64"), "windows-arm64");
        assert_eq!(key_for("linux", "x86_64"), "linux");
        assert_eq!(key_for("linux", "x86"), "linux-i386");
        assert_eq!(key_for("macos", "x86_64"), "mac-os");
        assert_eq!(key_for("macos", "aarch64"), "mac-os-arm64");
    }

    #[test]
    fn manifest_paths_are_confined() {
        let base = Path::new("base");
        assert!(safe_join(base, "bin/java.exe").is_some());
        assert!(safe_join(base, "../evil").is_none());
        assert!(safe_join(base, "/abs").is_none());
    }

    #[test]
    fn parses_index_and_manifest() {
        let index: Index = serde_json::from_str(r#"{"linux":{"java-runtime-delta":[{"manifest":{"sha1":"a","size":1,"url":"u"},"version":{"name":"21.0.7","released":"x"}}],"minecraft-java-exe":[]}}"#).unwrap();
        assert_eq!(index["linux"]["java-runtime-delta"][0].version.name, "21.0.7");
        let manifest: Manifest = serde_json::from_str(r#"{"files":{"bin":{"type":"directory"},"bin/java":{"type":"file","executable":true,"downloads":{"lzma":{"sha1":"z","size":1,"url":"l"},"raw":{"sha1":"s","size":2,"url":"r"}}},"lib/x":{"type":"link","target":"../y"}}}"#).unwrap();
        assert!(manifest.files["bin/java"].executable);
        assert_eq!(manifest.files["lib/x"].target.as_deref(), Some("../y"));
    }

    #[tokio::test]
    #[ignore = "downloads about 50 MB from Mojang"]
    async fn download_delta_runtime() {
        let root = std::env::var("CARGO_TARGET_DIR").map_or_else(|_| PathBuf::from("target/java/testdata"), |t| PathBuf::from(t).join("testdata"));
        let paths = Paths::new(root);
        let http = Http::new().unwrap();
        let sink = |event: Event| {
            if let Event::Progress { done, total, .. } = event {
                if done == total || done % 50 == 0 {
                    println!("progress {done}/{total}");
                }
            }
        };
        let install = ensure_runtime(&http, &paths, "java-runtime-delta", &sink, "test").await.unwrap();
        println!("{install:?}");
        assert!(install.managed);
        assert_eq!(install.major, 21);
        // Second call must hit the marker and download nothing.
        let again = ensure_runtime(&http, &paths, "java-runtime-delta", &sink, "test").await.unwrap();
        assert_eq!(again, install);
        let probed = probe(&install.executable).await.unwrap();
        assert_eq!(probed.major, 21);
    }
}

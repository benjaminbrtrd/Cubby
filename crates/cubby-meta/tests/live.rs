//! Tests hitting the real Mojang / Fabric / Quilt endpoints. Run with `-- --ignored`.

use std::path::PathBuf;
use std::sync::Mutex;

use cubby_common::events::Event;
use cubby_common::net::Http;
use cubby_common::paths::Paths;
use cubby_meta::{fetch_manifest, fetch_version, game_versions_supported, install, loader_versions, resolve, LoaderKind, LoaderSpec};

fn data_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/meta/testdata").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

#[tokio::test]
#[ignore = "needs network"]
async fn manifest_and_version() {
    let http = Http::new().unwrap();
    let paths = Paths::new(data_dir("manifest"));
    let manifest = fetch_manifest(&http, &paths).await.unwrap();
    assert!(manifest.find("1.21.1").is_some());
    assert!(paths.meta().join("version_manifest_v2.json").exists());
    let v = fetch_version(&http, &paths, "1.21.1").await.unwrap();
    assert_eq!(v.id, "1.21.1");
    assert!(paths.meta().join("versions/1.21.1.json").exists());
    // Second call is served from the cache.
    fetch_version(&http, &paths, "1.21.1").await.unwrap();
    assert!(fetch_version(&http, &paths, "nope-0").await.is_err());
}

#[tokio::test]
#[ignore = "needs network"]
async fn loader_listings() {
    let http = Http::new().unwrap();
    for kind in [LoaderKind::Fabric, LoaderKind::Quilt] {
        let versions = loader_versions(&http, kind, "1.21.1").await.unwrap();
        assert!(versions.iter().any(|v| v.stable), "{kind:?}");
        let games = game_versions_supported(&http, kind).await.unwrap();
        assert!(games.iter().any(|g| g == "1.21.1"), "{kind:?}");
    }
}

async fn install_and_check(name: &str, loader: LoaderSpec) {
    let http = Http::new().unwrap();
    let dir = data_dir(name);
    let paths = Paths::new(&dir);
    let game_dir = dir.join("game");
    let version = resolve(&http, &paths, "1.21.1", &loader).await.unwrap();

    let events = Mutex::new(Vec::new());
    let sink = |e: Event| events.lock().unwrap().push(e);
    let installed = install(&http, &paths, &version, &game_dir, cubby_common::net::DEFAULT_CONCURRENCY, &sink, "install:test").await.unwrap();

    assert!(installed.client_jar.ends_with(PathBuf::from("versions").join("1.21.1").join("1.21.1.jar")));
    assert!(installed.client_jar.exists());
    assert!(!installed.libraries.is_empty());
    assert!(installed.libraries.iter().all(|p| p.exists()));
    assert!(installed.natives.is_empty());
    let arg = installed.logging_argument.as_deref().unwrap();
    assert!(arg.starts_with("-Dlog4j.configurationFile=") && !arg.contains("${path}"));
    assert_eq!(installed.assets_root, paths.assets());
    assert_eq!(installed.game_assets_dir, paths.assets());
    assert!(paths.assets().join("indexes").join(format!("{}.json", installed.assets_index_id)).exists());
    let objects = std::fs::read_dir(paths.assets().join("objects")).unwrap().count();
    assert!(objects > 100, "only {objects} object folders");

    let events = events.into_inner().unwrap();
    assert!(events.iter().all(|e| matches!(e, Event::Progress { .. })));
    let labels: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            Event::Progress { task, label, .. } => {
                assert_eq!(task, "install:test");
                Some(label.clone())
            }
            _ => None,
        })
        .collect();
    for expected in ["Checking the version", "Downloading libraries", "Downloading assets"] {
        assert!(labels.iter().any(|l| l == expected), "missing {expected}");
    }

    if let LoaderSpec::Fabric(v) = &loader {
        assert_eq!(installed.version.id, format!("fabric-loader-{v}-1.21.1"));
        assert!(installed.libraries.iter().any(|p| p.to_string_lossy().contains("fabric-loader")));
    }
}

#[tokio::test]
#[ignore = "needs network"]
async fn install_vanilla_1_21_1() {
    install_and_check("vanilla", LoaderSpec::Vanilla).await;
}

#[tokio::test]
#[ignore = "needs network"]
async fn install_fabric_1_21_1() {
    let http = Http::new().unwrap();
    let latest = loader_versions(&http, LoaderKind::Fabric, "1.21.1").await.unwrap().into_iter().find(|v| v.stable).unwrap();
    install_and_check("fabric", LoaderSpec::Fabric(latest.version)).await;
}

#[tokio::test]
#[ignore = "needs network"]
async fn resolve_quilt_1_21_1() {
    let http = Http::new().unwrap();
    let paths = Paths::new(data_dir("quilt"));
    let latest = loader_versions(&http, LoaderKind::Quilt, "1.21.1").await.unwrap().into_iter().find(|v| v.stable).unwrap();
    let v = resolve(&http, &paths, "1.21.1", &LoaderSpec::Quilt(latest.version.clone())).await.unwrap();
    assert!(v.id.contains(&latest.version));
    assert!(v.main_class.unwrap().contains("quiltmc"));
    assert!(v.downloads.unwrap().client.is_some());
}

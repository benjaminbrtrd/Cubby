//! End-to-end Forge / NeoForge installs against the real servers. Run with `-- --ignored --nocapture`.
//!
//! `CUBBY_E2E_ROOT` is the data directory to install into (never the real one) and `CUBBY_E2E_JAVA<major>`
//! the Java executable for that major version (8, 17 or 21).

use std::path::{Path, PathBuf};
use std::time::Instant;

use cubby_common::contracts::{JavaInstall, LaunchCredentials};
use cubby_common::net::Http;
use cubby_common::paths::Paths;
use cubby_launch::{build_command, LaunchOptions};
use cubby_meta::{install_loader, loader_versions, resolve, LoaderKind, LoaderSpec};

fn root() -> Paths {
    Paths::new(PathBuf::from(std::env::var("CUBBY_E2E_ROOT").expect("CUBBY_E2E_ROOT")))
}

fn java(major: u32) -> PathBuf {
    PathBuf::from(std::env::var(format!("CUBBY_E2E_JAVA{major}")).unwrap_or_else(|_| panic!("CUBBY_E2E_JAVA{major}")))
}

async fn run(kind: LoaderKind, game: &str, pick: impl Fn(&[cubby_meta::LoaderVersion]) -> String, java_major: u32) {
    let http = Http::new().unwrap();
    let paths = root();
    let versions = loader_versions(&http, kind, game).await.unwrap();
    println!("{} versions for {game}: {} (first: {:?}, stable: {})", kind.as_str(), versions.len(), versions.first(), versions.iter().filter(|v| v.stable).count());
    let version = pick(&versions);
    let spec = match kind {
        LoaderKind::Forge => LoaderSpec::Forge(version.clone()),
        _ => LoaderSpec::NeoForge(version.clone()),
    };
    println!("installing {spec:?} with Java {java_major}");
    let merged = resolve(&http, &paths, game, &spec).await.unwrap();
    let game_dir = paths.instance("e2e").join(".minecraft");
    std::fs::create_dir_all(&game_dir).unwrap();
    let java_exe = java(java_major);
    let sink = |e: cubby_common::events::Event| {
        if let cubby_common::events::Event::Progress { label, done, total, .. } = e {
            if label.contains("Forge") || label.contains("loader") {
                println!("  [{label}] {done}/{total}");
            }
        }
    };
    let started = Instant::now();
    let installed = install_loader(&http, &paths, &merged, &spec, Some(&java_exe), &game_dir, 16, &sink, "t").await.unwrap();
    println!("first install: {:?}", started.elapsed());
    let started = Instant::now();
    install_loader(&http, &paths, &merged, &spec, Some(&java_exe), &game_dir, 16, &sink, "t").await.unwrap();
    println!("second install: {:?}", started.elapsed());
    for lib in &installed.libraries {
        assert!(lib.exists(), "missing {}", lib.display());
    }

    let creds = LaunchCredentials { username: "Tester".into(), uuid: "00000000-0000-0000-0000-000000000000".into(), access_token: "0".into(), user_type: "legacy".into(), xuid: String::new() };
    let java_install = JavaInstall { executable: java_exe, major: java_major, version: java_major.to_string(), arch: "x86_64".into(), vendor: None, managed: false };
    let opts = LaunchOptions { instance_id: "e2e".into(), game_dir: game_dir.clone(), java: java_install, min_memory_mb: 512, max_memory_mb: 2048, extra_jvm_args: vec![], window: None, fullscreen: false, quick_play: None, demo: false };
    let natives = paths.natives("e2e");
    cubby_launch::extract_natives(&installed, &natives).unwrap();
    let command = build_command(&installed, &creds, &opts, &natives).unwrap();
    let out = Path::new(&std::env::var("CUBBY_E2E_ROOT").unwrap()).join(format!("command-{}-{game}.txt", kind.as_str()));
    std::fs::write(&out, format!("{}\n{}\n", command.program.display(), command.args.join("\n"))).unwrap();
    println!("main class: {:?}, {} libraries, command in {}", merged.main_class, installed.libraries.len(), out.display());
}

#[tokio::test]
#[ignore = "needs network"]
async fn forge_1_20_1_recommended() {
    run(LoaderKind::Forge, "1.20.1", |v| v.iter().find(|v| v.stable).unwrap().version.clone(), 17).await;
}

#[tokio::test]
#[ignore = "needs network"]
async fn neoforge_1_21_1() {
    run(LoaderKind::NeoForge, "1.21.1", |v| v.iter().find(|v| v.stable).unwrap().version.clone(), 21).await;
}

#[tokio::test]
#[ignore = "needs network"]
async fn forge_1_12_2() {
    run(LoaderKind::Forge, "1.12.2", |v| v.iter().find(|v| v.stable).unwrap().version.clone(), 8).await;
}

#[tokio::test]
#[ignore = "needs network"]
async fn forge_1_7_10() {
    run(LoaderKind::Forge, "1.7.10", |v| v.iter().find(|v| v.stable).unwrap().version.clone(), 8).await;
}

#[tokio::test]
#[ignore = "needs network"]
async fn forge_1_6_4() {
    run(LoaderKind::Forge, "1.6.4", |v| v[0].version.clone(), 8).await;
}

#[tokio::test]
#[ignore = "needs network"]
async fn forge_1_12_2_14_23_4_2705() {
    run(LoaderKind::Forge, "1.12.2", |_| "14.23.4.2705".into(), 8).await;
}

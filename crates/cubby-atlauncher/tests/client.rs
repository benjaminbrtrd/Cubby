//! The client against a local server that plays the ATLauncher CDN with the fixtures.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::json;
use sha1::{Digest, Sha1};
use cubby_atlauncher::{optional_mods, AtLauncherClient, OptionalSelection};
use cubby_common::net::Http;
use cubby_common::packs::{PackChannel, PackLoaderKind, PackSort};
use cubby_common::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const PACKS: &str = include_str!("fixtures/packsnew.json");
const PIXELMON: &str = include_str!("fixtures/manifest_pixelmon.json");
const MOONQUEST: &str = include_str!("fixtures/manifest_moonquest.json");
const OPTIONAL: &str = include_str!("fixtures/manifest_optional.json");

/// Serves fixed bodies by path and records every request line.
struct Cdn {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
}

async fn cdn(routes: Vec<(String, Vec<u8>)>) -> Cdn {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let routes: Arc<HashMap<String, Vec<u8>>> = Arc::new(routes.into_iter().collect());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            let (routes, log) = (routes.clone(), log.clone());
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = head.lines().next().unwrap_or("").to_string();
                log.lock().unwrap().push(line.clone());
                let mut parts = line.split(' ');
                let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                let body = if method == "GET" { routes.get(path) } else { None };
                let response = match body {
                    Some(body) => {
                        let mut out = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
                        out.extend_from_slice(body);
                        out
                    }
                    None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
                };
                let _ = stream.write_all(&response).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    Cdn { base, requests }
}

fn client(cdn: &Cdn) -> AtLauncherClient {
    AtLauncherClient::with_base_url(Http::new().unwrap(), cdn.base.clone())
}

fn zip_bytes(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn sha1(bytes: &[u8]) -> String {
    hex::encode(Sha1::digest(bytes))
}

fn route(path: &str, body: impl Into<Vec<u8>>) -> (String, Vec<u8>) {
    (path.to_string(), body.into())
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cubby-atl-it-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn zip_names(path: &Path) -> Vec<String> {
    let zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut names: Vec<String> = zip.file_names().map(str::to_string).collect();
    names.sort();
    names
}

/// The manifest with its configs entry pointing at `zip`.
fn manifest_with_configs(text: &str, zip: &[u8]) -> String {
    let mut value: serde_json::Value = serde_json::from_str(text).unwrap();
    value["configs"] = json!({ "filesize": zip.len(), "sha1": sha1(zip) });
    value.to_string()
}

#[tokio::test]
async fn lists_searches_and_versions() {
    let server = cdn(vec![route("/launcher/json/packsnew.json", PACKS)]).await;
    let atl = client(&server);

    let packs = atl.packs().await.unwrap();
    let ids: Vec<_> = packs.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["PixelmonMod", "MoonQuest", "DigitalReality"]);
    assert_eq!(packs[0].icon_url.as_deref(), Some(format!("{}/launcher/images/pixelmonmod.png", server.base).as_str()));

    let page = atl.search("pokemon", PackSort::Relevance).await.unwrap();
    assert_eq!((page.total, page.hits.len(), page.hits[0].id.as_str()), (Some(1), 1, "PixelmonMod"));
    assert_eq!(atl.search("", PackSort::Name).await.unwrap().hits[0].name, "Digital Reality");

    let versions = atl.versions("DigitalReality").await.unwrap();
    assert_eq!(versions.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(), ["5.0.1", "4.0.6", "4.0.5"]);
    assert_eq!(versions[0].channel, PackChannel::Release);
    assert_eq!(versions[1].channel, PackChannel::Beta);
    assert!(matches!(atl.versions("Nope").await, Err(Error::NotFound(_))));
    // The list is fetched once, then served from memory; only GET is ever sent.
    let requests = server.requests.lock().unwrap().clone();
    assert_eq!(requests, ["GET /launcher/json/packsnew.json HTTP/1.1"]);
}

#[tokio::test]
async fn manifest_and_errors() {
    let server = cdn(vec![route("/packs/OptionalPack/versions/2.0/Configs.json", OPTIONAL), route("/packs/Weird/versions/1.0%20beta/Configs.json", "{ not json")]).await;
    let atl = client(&server);

    let manifest = atl.manifest("OptionalPack", "2.0").await.unwrap();
    assert_eq!(manifest.minecraft, "1.12.2");
    assert_eq!(optional_mods(&manifest).len(), 5);
    assert!(manifest.unsupported_reasons().is_empty());
    assert!(matches!(atl.manifest("OptionalPack", "9.9").await, Err(Error::NotFound(_))));
    assert!(matches!(atl.manifest("Weird", "1.0 beta").await, Err(Error::Json { .. })));
    assert!(atl.manifest("../evil", "1").await.is_err());
}

#[tokio::test]
async fn plans_a_neoforge_pack() {
    let configs = zip_bytes(&[("config/pixelmon.toml", "x = 1")]);
    let server = cdn(vec![
        route("/launcher/json/packsnew.json", PACKS),
        route("/packs/PixelmonMod/versions/Pixelmon-1.21.1-9.4.2/Configs.json", manifest_with_configs(PIXELMON, &configs)),
        route("/packs/PixelmonMod/versions/Pixelmon-1.21.1-9.4.2/Configs.zip", configs.clone()),
    ])
    .await;
    let cache = scratch("pixelmon");
    let result = client(&server).plan_detailed("PixelmonMod", "Pixelmon-1.21.1-9.4.2", &[], &cache).await.unwrap();
    let plan = &result.plan;

    assert_eq!(plan.provider, "atlauncher");
    assert_eq!((plan.pack_id.as_str(), plan.version_id.as_str()), ("PixelmonMod", "Pixelmon-1.21.1-9.4.2"));
    assert_eq!((plan.title.as_str(), plan.version_name.as_str()), ("Pixelmon Mod", "Pixelmon-1.21.1-9.4.2"));
    assert_eq!(plan.minecraft, "1.21.1");
    assert_eq!(plan.loader, Some((PackLoaderKind::NeoForge, "21.1.248".to_string())));
    assert!(plan.icon_url.as_deref().unwrap().ends_with("/launcher/images/pixelmonmod.png"));
    assert_eq!(plan.files.len(), 14);
    assert!(plan.files.iter().all(|f| f.sha1.is_none() && f.urls.len() == 1));
    assert!(plan.files.iter().any(|f| f.urls[0].starts_with(&format!("{}/packs/PixelmonMod/files/", server.base))));
    assert!(plan.blocked.is_empty());
    assert_eq!(result.md5.len(), 14);

    // The configs zip was downloaded, verified against the manifest's SHA-1, and is the only override.
    assert_eq!(plan.overrides.len(), 1);
    assert_eq!(plan.overrides[0].prefix, "");
    assert!(plan.overrides[0].archive.starts_with(&cache));
    assert_eq!(std::fs::read(&plan.overrides[0].archive).unwrap(), configs);
    // Nothing but GET reached the server.
    assert!(server.requests.lock().unwrap().iter().all(|r| r.starts_with("GET ")));
    let _ = std::fs::remove_dir_all(&cache);
}

#[tokio::test]
async fn a_damaged_configs_zip_is_rejected() {
    let configs = zip_bytes(&[("config/a.toml", "x = 1")]);
    let server = cdn(vec![
        route("/packs/PixelmonMod/versions/v1/Configs.json", manifest_with_configs(PIXELMON, &configs)),
        route("/packs/PixelmonMod/versions/v1/Configs.zip", zip_bytes(&[("config/a.toml", "x = 2")])),
    ])
    .await;
    let cache = scratch("damaged");
    let error = client(&server).plan("PixelmonMod", "v1", &[], &cache).await.expect_err("hash mismatch");
    assert!(matches!(error, Error::HashMismatch { .. }), "{error}");
    let left: Vec<_> = std::fs::read_dir(&cache).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert!(left.is_empty(), "{left:?}");
    let _ = std::fs::remove_dir_all(&cache);
}

#[tokio::test]
async fn plans_an_old_forge_pack_with_extract_and_optional_mods() {
    let configs = zip_bytes(&[("config/x.cfg", "1")]);
    let options = zip_bytes(&[("optionsof.txt", "ofFastRender:true")]);
    let server = cdn(vec![
        route("/packs/MoonQuest/versions/0.17.0/Configs.json", manifest_with_configs(MOONQUEST, &configs)),
        route("/packs/MoonQuest/versions/0.17.0/Configs.zip", configs.clone()),
        route("/packs/MoonQuest/files/optionsoptifine.zip", options.clone()),
    ])
    .await;
    let cache = scratch("moonquest");
    let atl = client(&server);
    let result = atl.plan_detailed("MoonQuest", "0.17.0", &["Zan's Minimap".to_string()], &cache).await.unwrap();
    let plan = &result.plan;

    assert_eq!(plan.title, "MoonQuest"); // not in this CDN's list: the id is the fallback
    assert_eq!(plan.loader, Some((PackLoaderKind::Forge, "9.11.1.953".to_string())));
    assert_eq!(plan.recommended_memory_mib, Some(2048));
    let paths: Vec<String> = plan.files.iter().map(|f| f.path.to_string_lossy().replace('\\', "/")).collect();
    assert_eq!(paths, ["mods/ZansMinimap1.6.4.zip", "mods/1.6.4/CodeChickenLib-universal-1.6.4-1.0.0.62.jar", "mods/CodeChickenCore 0.9.0.9.jar", "mods/NotEnoughItems 1.6.1.9.jar"]);
    // The URL with a space is encoded by the HTTP client.
    assert!(plan.files[2].urls[0].ends_with("CodeChickenCore 0.9.0.9.jar"));
    // Configs first, then the extract zip, which goes to the game directory untouched.
    assert_eq!(plan.overrides.len(), 2);
    assert!(plan.overrides[0].archive.to_string_lossy().ends_with("-configs.zip"));
    assert_eq!(std::fs::read(&plan.overrides[1].archive).unwrap(), options);
    assert_eq!(plan.overrides[1].prefix, "");
    assert!(result.warnings.iter().any(|w| w.contains("libraries")));
    let _ = std::fs::remove_dir_all(&cache);
}

#[tokio::test]
async fn extract_into_a_folder_is_repacked() {
    let manifest = json!({
        "version": "1", "minecraft": "1.12.2", "noConfigs": true,
        "loader": {"type": "fabric", "metadata": {"loader": "0.15.7"}},
        "mods": [
            {"name": "Textures", "type": "resourcepackextract", "download": "server", "url": "packs/P/files/tex.zip", "file": "tex.zip", "client": true},
            {"name": "Libs", "type": "extract", "extractTo": "mods", "extractFolder": "libs%s%", "download": "server", "url": "packs/P/files/libs.zip", "file": "libs.zip", "client": true},
            {"name": "Manual", "type": "mods", "download": "browser", "url": "http://forum.example.test/x", "file": "manual.jar", "md5": "ABC", "client": true}
        ]
    });
    let server = cdn(vec![
        route("/packs/P/versions/1/Configs.json", manifest.to_string()),
        route("/packs/P/files/tex.zip", zip_bytes(&[("pack.mcmeta", "{}"), ("assets/a.png", "png")])),
        route("/packs/P/files/libs.zip", zip_bytes(&[("libs/one.jar", "1"), ("libs/two.jar", "2"), ("readme.txt", "no")])),
    ])
    .await;
    let cache = scratch("repack");
    let result = client(&server).plan_detailed("P", "1", &[], &cache).await.unwrap();
    let plan = &result.plan;
    assert_eq!(plan.loader, Some((PackLoaderKind::Fabric, "0.15.7".to_string())));
    assert!(plan.files.is_empty());
    assert_eq!(plan.blocked.len(), 1);
    assert_eq!(result.md5[&plan.blocked[0].path], "abc");
    assert_eq!(plan.overrides.len(), 2);
    assert!(plan.overrides.iter().all(|o| o.prefix.is_empty()));
    assert_eq!(zip_names(&plan.overrides[0].archive), ["resourcepacks/extracted/assets/a.png", "resourcepacks/extracted/pack.mcmeta"]);
    assert_eq!(zip_names(&plan.overrides[1].archive), ["mods/one.jar", "mods/two.jar"]);
    // The downloaded originals are gone; only the repacked archives remain.
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 2);
    let _ = std::fs::remove_dir_all(&cache);
}

#[tokio::test]
async fn unsupported_packs_fail_with_the_reasons() {
    let manifest = json!({
        "version": "1", "minecraft": "1.7.10",
        "mods": [{"name": "Patch", "type": "jar", "download": "server", "url": "p/x.jar", "file": "x.jar", "client": true}]
    });
    let server = cdn(vec![route("/packs/Old/versions/1/Configs.json", manifest.to_string())]).await;
    let atl = client(&server);
    let error = atl.plan("Old", "1", &[], &scratch("unsupported")).await.err().unwrap().to_string();
    assert!(error.contains("jar mod") && error.contains("Patch"), "{error}");
    let reasons = atl.manifest("Old", "1").await.unwrap().unsupported_reasons();
    assert_eq!(reasons.len(), 1);
}

#[tokio::test]
async fn optional_selection_feeds_the_plan() {
    let server = cdn(vec![route("/packs/OptionalPack/versions/2.0/Configs.json", OPTIONAL.replace("\"configs\"", "\"noConfigsHere\""))]).await;
    let atl = client(&server);
    let manifest = atl.manifest("OptionalPack", "2.0").await.unwrap();
    let mut picker = OptionalSelection::new(&manifest);
    picker.set("Minimap", true);
    let plan = atl.plan("OptionalPack", "2.0", &picker.names(), &scratch("optional")).await.unwrap();
    let names: Vec<String> = plan.files.iter().map(|f| f.path.file_name().unwrap().to_string_lossy().to_string()).collect();
    // Base, the default-selected shaders, Minimap and the library it pulled in; the server-only mod is skipped.
    assert_eq!(names, ["base.jar", "shaders.jar", "minimap.jar", "minimaplib.jar"]);
    assert!(plan.overrides.is_empty());
}

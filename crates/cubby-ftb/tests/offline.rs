//! Client tests against real API answers saved in `tests/fixtures`, served by a tiny local HTTP server.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use cubby_common::net::Http;
use cubby_common::packs::{PackChannel, PackLoaderKind, PackSort};
use cubby_common::Error;
use cubby_ftb::FtbClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap()
}

fn fixture_json(name: &str) -> Value {
    serde_json::from_str(&fixture(name)).unwrap()
}

/// Serves `route(path)` (path without the query string) as JSON; `None` is a 404. Returns the base URL.
async fn serve(route: impl Fn(&str) -> Option<String> + Send + Sync + 'static) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let route = Arc::new(route);
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { return };
            let route = route.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                let target = request.split_whitespace().nth(1).unwrap_or("/").to_string();
                let path = target.split('?').next().unwrap_or("/").to_string();
                let (status, body) = match route(&path) {
                    Some(body) => ("200 OK", body),
                    None => ("404 Not Found", r#"{"status":"error","message":"Modpack not found"}"#.to_string()),
                };
                let head = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(body.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    base
}

fn client(base: String) -> FtbClient {
    FtbClient::with_base_url(Http::new().unwrap(), base)
}

/// StoneBlock 3 (100) and Unstable 1.20 Fabric (109) with a configurable version 12632 of the former.
async fn catalogue(version_12632: Value) -> FtbClient {
    let version_12632 = version_12632.to_string();
    client(
        serve(move |path| match path {
            "/100" => Some(fixture("pack_100.json")),
            "/109" => Some(fixture("pack_109_fabric.json")),
            "/100/12632" => Some(version_12632.clone()),
            "/114" => Some(fixture("pack_100.json").replace("\"id\": 100,", "\"id\": 114,")),
            "/114/100071" => Some(fixture("version_114_serveronly.json")),
            "/popular/installs/12" => Some(fixture("popular.json")),
            "/featured/12" => Some(r#"{"status":"success","packs":[109],"total":1}"#.into()),
            "/all" => Some(r#"{"status":"success","packs":[109,100],"total":2}"#.into()),
            p if p.starts_with("/popular/installs/") => Some(r#"{"status":"success","packs":[100,109]}"#.into()),
            p if p.starts_with("/search/") => Some(r#"{"status":"success","packs":[109,100],"curseforge":[1,2],"count":4}"#.into()),
            _ => None,
        })
        .await,
    )
}

async fn standard() -> FtbClient {
    catalogue(fixture_json("version_100_12632.json")).await
}

fn cache() -> PathBuf {
    std::env::temp_dir()
}

#[tokio::test]
async fn pack_summary_and_details() {
    let ftb = standard().await;
    let pack = ftb.pack("100").await.unwrap();
    assert_eq!(pack.id, "100");
    assert_eq!(pack.name, "FTB StoneBlock 3");
    assert!(pack.summary.starts_with("In a world surrounded by stone"));
    assert_eq!(pack.authors, ["FTB Team"]);
    assert_eq!(pack.website_url.as_deref(), Some("https://www.feed-the-beast.com/modpacks/100-ftb-stoneblock-3"));
    assert!(pack.icon_url.as_deref().unwrap().starts_with("https://") && pack.icon_url.as_deref().unwrap().ends_with(".png"));
    assert!(pack.downloads.unwrap() > 1_000_000);
    assert_eq!(pack.updated.as_deref(), Some("2022-10-31T19:07:24Z"));
    assert_eq!(pack.minecraft, ["1.18.2"]);
    assert_eq!(pack.loaders, [PackLoaderKind::Forge]);

    let details = ftb.details("100").await.unwrap();
    assert_eq!(details.summary, pack);
    assert!(details.description.starts_with("![FTB StoneBlock 3 Logo]"));
    assert!(details.links.iter().any(|(name, url)| name == "Wiki" && url == "https://docs.feed-the-beast.com"));
    assert!(details.tags.contains(&"Magic".to_string()));
    assert!(details.splash_url.is_some() && details.plays.is_some() && details.released.is_some());

    let fabric = ftb.pack("109").await.unwrap();
    assert_eq!(fabric.loaders, [PackLoaderKind::Fabric]);
    assert_eq!(fabric.minecraft, ["1.20.1"]);
}

#[tokio::test]
async fn versions_newest_first_without_archived() {
    let versions = standard().await.versions("100").await.unwrap();
    let ids: Vec<&str> = versions.iter().map(|v| v.id.as_str()).collect();
    // 2293 is archived and left out; ids grow with time.
    assert_eq!(ids, ["12632", "12492", "2327", "2275"]);
    assert_eq!(versions[0].name, "1.11.1");
    assert_eq!(versions[0].channel, PackChannel::Release);
    assert_eq!(versions[0].minecraft.as_deref(), Some("1.18.2"));
    assert_eq!(versions[0].loader, Some(PackLoaderKind::Forge));
    assert!(versions[0].released.is_some());
    assert_eq!(versions[2].channel, PackChannel::Alpha);
    assert_eq!(versions[3].channel, PackChannel::Beta);
}

#[tokio::test]
async fn plan_from_a_real_version() {
    let plan = standard().await.plan("100", "12632", &cache()).await.unwrap();
    assert_eq!(plan.provider, "ftb");
    assert_eq!((plan.pack_id.as_str(), plan.version_id.as_str()), ("100", "12632"));
    assert_eq!((plan.title.as_str(), plan.version_name.as_str()), ("FTB StoneBlock 3", "1.11.1"));
    assert_eq!(plan.minecraft, "1.18.2");
    assert_eq!(plan.loader, Some((PackLoaderKind::Forge, "40.2.14".to_string())));
    assert_eq!(plan.recommended_memory_mib, Some(6144));
    assert!(plan.icon_url.is_some() && plan.overrides.is_empty() && plan.blocked.is_empty());

    assert_eq!(plan.files.len(), 8);
    let config = plan.files.iter().find(|f| f.path.ends_with("general.toml")).unwrap();
    assert_eq!(config.path, PathBuf::from("config").join("Advancedperipherals").join("general.toml"));
    assert!(config.urls[0].starts_with("https://") && config.size.unwrap() > 0);
    let sha1 = config.sha1.as_deref().unwrap();
    assert!(sha1.len() == 40 && sha1.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    let jar = plan.files.iter().find(|f| f.path.ends_with("Applied-Botanics-1.0.3.jar")).unwrap();
    assert_eq!(jar.path, PathBuf::from("mods").join("Applied-Botanics-1.0.3.jar"));
    assert!(jar.path.is_relative());
}

#[tokio::test]
async fn plan_skips_server_only_files() {
    let ftb = standard().await;
    // Pack 114 is served with the StoneBlock page; only its version file matters here.
    let plan = ftb.plan("114", "100071", &cache()).await.unwrap();
    // 7 files: 2 client-only (kept), 2 server-only (dropped), 3 plain.
    assert_eq!(plan.files.len(), 5);
    let names: Vec<String> = plan.files.iter().map(|f| f.path.file_name().unwrap().to_string_lossy().to_string()).collect();
    assert!(names.contains(&"ftbpromoter-19.2.52.jar".to_string()));
    assert!(!names.iter().any(|n| n.contains("fullstackwatchdog") || n.contains("default-server-properties")), "{names:?}");
    assert_eq!(plan.loader.as_ref().map(|l| l.0), Some(PackLoaderKind::Forge));
    // FTB lists the CurseForge file and its own mirror: both are tried.
    let promoter = plan.files.iter().find(|f| f.path.ends_with("ftbpromoter-19.2.52.jar")).unwrap();
    assert!(!promoter.urls.is_empty());
}

fn with_file(mutate: impl FnOnce(&mut Vec<Value>)) -> Value {
    let mut version = fixture_json("version_100_12632.json");
    mutate(version["files"].as_array_mut().unwrap());
    version
}

#[tokio::test]
async fn files_without_url_but_from_curseforge_are_blocked() {
    let version = with_file(|files| {
        let cf = files.iter_mut().find(|f| f.get("curseforge").is_some()).unwrap();
        cf["url"] = json!("");
        cf["mirrors"] = json!([]);
    });
    let plan = catalogue(version).await.plan("100", "12632", &cache()).await.unwrap();
    assert_eq!(plan.files.len(), 7);
    assert_eq!(plan.blocked.len(), 1);
    let blocked = &plan.blocked[0];
    assert_eq!(blocked.file_name, "AdvancedPeripherals-1.18.2-0.7.31r.jar");
    assert_eq!(blocked.page_url, "https://www.curseforge.com/projects/431725");
    assert_eq!(blocked.path, PathBuf::from("mods").join("AdvancedPeripherals-1.18.2-0.7.31r.jar"));
    assert_eq!(blocked.sha1.as_deref(), Some("7a3abf64867b90007f5b7ca7f7c4530547599353"));
    assert_eq!(blocked.size, Some(854834));
}

#[tokio::test]
async fn mirrors_are_fallback_urls() {
    let version = with_file(|files| {
        let cf = files.iter_mut().find(|f| f.get("curseforge").is_some()).unwrap();
        cf["mirrors"] = json!(["https://files.example.test/a.jar", "https://edge.forgecdn.net/files/4684/137/AdvancedPeripherals-1.18.2-0.7.31r.jar", "ftp://nope"]);
    });
    let plan = catalogue(version).await.plan("100", "12632", &cache()).await.unwrap();
    let file = plan.files.iter().find(|f| f.path.ends_with("AdvancedPeripherals-1.18.2-0.7.31r.jar")).unwrap();
    assert_eq!(file.urls, ["https://edge.forgecdn.net/files/4684/137/AdvancedPeripherals-1.18.2-0.7.31r.jar", "https://files.example.test/a.jar"]);
}

#[tokio::test]
async fn a_file_nobody_can_serve_is_an_error() {
    let version = with_file(|files| {
        let plain = files.iter_mut().find(|f| f.get("curseforge").is_none()).unwrap();
        plain["url"] = json!("");
    });
    let error = catalogue(version).await.plan("100", "12632", &cache()).await.unwrap_err();
    assert!(error.to_string().contains("no download address"), "{error}");
}

#[tokio::test]
async fn unsafe_paths_are_refused() {
    for (path, name) in [("./mods", "../../evil.jar"), ("../outside", "a.jar"), ("/etc", "passwd"), ("C:/Windows", "x.dll"), ("./mods", "sub/x.jar"), ("./mods", "..\\x.jar"), ("./mods", "")] {
        let version = with_file(|files| {
            files[0]["path"] = json!(path);
            files[0]["name"] = json!(name);
        });
        let error = catalogue(version).await.plan("100", "12632", &cache()).await.unwrap_err();
        assert!(error.to_string().contains("unsafe file path"), "{path} {name}: {error}");
    }
    // A server-only file is never installed, so its path doesn't matter.
    let version = with_file(|files| {
        files[0]["path"] = json!("../outside");
        files[0]["serveronly"] = json!(true);
    });
    let plan = catalogue(version).await.plan("100", "12632", &cache()).await.unwrap();
    assert_eq!(plan.files.len(), 7);
}

#[tokio::test]
async fn vanilla_version_has_no_loader() {
    let version = with_file(|_| {});
    let mut version = version;
    version["targets"] = json!([{"name": "minecraft", "type": "game", "version": "1.20.1"}]);
    version["specs"] = json!({"minimum": 2048, "recommended": 0});
    let plan = catalogue(version).await.plan("100", "12632", &cache()).await.unwrap();
    assert_eq!(plan.loader, None);
    assert_eq!(plan.recommended_memory_mib, Some(2048));

    let mut broken = fixture_json("version_100_12632.json");
    broken["targets"] = json!([]);
    assert!(catalogue(broken).await.plan("100", "12632", &cache()).await.is_err());
}

#[tokio::test]
async fn featured_and_search() {
    let ftb = standard().await;
    // Featured (109) first, then the popular ones that exist (100); unknown packs (35, 91...) are skipped.
    let featured = ftb.featured().await.unwrap();
    let ids: Vec<&str> = featured.hits.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["109", "100"]);

    // Relevance keeps the server order; the CurseForge ids of a search are ignored.
    let page = ftb.search("stone", PackSort::Relevance, 0, 10).await.unwrap();
    assert_eq!(page.hits.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["109", "100"]);
    assert_eq!(page.total, Some(2));
    let page = ftb.search("stone", PackSort::Name, 0, 10).await.unwrap();
    assert_eq!(page.hits.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["100", "109"]);
    let page = ftb.search("stone", PackSort::Downloads, 0, 10).await.unwrap();
    assert_eq!(page.hits[0].id, "100");
    let page = ftb.search("stone", PackSort::Updated, 1, 10).await.unwrap();
    assert_eq!(page.hits.len(), 1);
    assert_eq!(page.hits[0].id, "100", "109 was updated more recently");

    // No query: the most installed packs; other sorts go through the full list.
    let page = ftb.search("  ", PackSort::Relevance, 0, 1).await.unwrap();
    assert_eq!((page.hits.len(), page.total, page.hits[0].id.as_str()), (1, Some(2), "100"));
    let page = ftb.search("", PackSort::Newest, 0, 10).await.unwrap();
    assert_eq!(page.hits[0].id, "109");
}

#[tokio::test]
async fn errors() {
    let ftb = standard().await;
    assert!(matches!(ftb.pack("99999").await, Err(Error::NotFound(_))));
    assert!(matches!(ftb.plan("100", "1", &cache()).await, Err(Error::NotFound(_))));
    for bad in ["", "..", "1/2", "abc", "-3"] {
        assert!(ftb.pack(bad).await.is_err(), "{bad}");
        assert!(ftb.versions(bad).await.is_err(), "{bad}");
        assert!(ftb.plan("100", bad, &cache()).await.is_err(), "{bad}");
    }
    // The legacy host answers errors with HTTP 200.
    let legacy = client(serve(|_| Some(r#"{"status":"error","message":"Modpack id not valid.","target":""}"#.into())).await);
    assert!(matches!(legacy.pack("5").await, Err(Error::NotFound(_))));
}

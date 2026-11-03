//! The client against a tiny local HTTP server that answers with hand-written fixtures (shapes taken from
//! <https://docs.curseforge.com/rest-api/>; never recorded from the live API).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use cubby_common::net::{sha1_file, Http};
use cubby_common::packs::{PackChannel, PackLoaderKind, PackSort};
use cubby_common::Error;
use cubby_curseforge::{read_manifest, CurseForgeClient};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap()
}

#[derive(Debug, Clone)]
struct Seen {
    method: String,
    target: String,
    api_key: Option<String>,
    body: String,
}

struct Server {
    base: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

/// `routes(method, path, base) -> Option<(content_type, body)>`
async fn serve(zip_path: Option<PathBuf>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (seen2, base2) = (seen.clone(), base.clone());
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { return };
            let (seen, base, zip_path) = (seen2.clone(), base2.clone(), zip_path.clone());
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let (head_end, content_length) = loop {
                    let n = socket.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..pos]).to_ascii_lowercase();
                        let len = head.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
                        break (pos + 4, len);
                    }
                };
                while buf.len() < head_end + content_length {
                    let n = socket.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
                let mut lines = head.lines();
                let mut first = lines.next().unwrap_or("").split(' ');
                let (method, target) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
                let api_key = lines.find_map(|l| l.to_ascii_lowercase().starts_with("x-api-key:").then(|| l["x-api-key:".len()..].trim().to_string()));
                let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
                seen.lock().unwrap().push(Seen { method: method.clone(), target: target.clone(), api_key, body });

                let path = target.split('?').next().unwrap_or("");
                let search: serde_json::Value = serde_json::from_slice(&fixture("search.json")).unwrap();
                let files: serde_json::Value = serde_json::from_slice(&fixture("pack_files.json")).unwrap();
                let (status, content_type, payload): (u16, &str, Vec<u8>) = match (method.as_str(), path) {
                    ("GET", "/v1/mods/search") => (200, "application/json", fixture("search.json")),
                    ("GET", "/v1/mods/925200") => (200, "application/json", serde_json::json!({ "data": search["data"][0] }).to_string().into_bytes()),
                    ("GET", "/v1/mods/925200/description") => (200, "application/json", br#"{"data":"<p>Hello</p>"}"#.to_vec()),
                    ("GET", "/v1/mods/925200/files") => (200, "application/json", fixture("pack_files.json")),
                    ("GET", "/v1/mods/925200/files/5000001") => {
                        let mut file = files["data"][0].clone();
                        file["downloadUrl"] = serde_json::json!(format!("{base}/dl/pack.zip"));
                        if let Some(zip) = &zip_path {
                            let hash = sha1_file(zip).await.unwrap();
                            file["hashes"] = serde_json::json!([{ "value": hash, "algo": 1 }]);
                        }
                        (200, "application/json", serde_json::json!({ "data": file }).to_string().into_bytes())
                    }
                    ("GET", "/v1/mods/925200/files/4000001") => (200, "application/json", serde_json::json!({ "data": files["data"][2] }).to_string().into_bytes()),
                    ("GET", "/dl/pack.zip") => (200, "application/zip", std::fs::read(zip_path.as_ref().unwrap()).unwrap()),
                    ("POST", "/v1/mods/files") => (200, "application/json", fixture("files_resolve.json")),
                    ("POST", "/v1/mods") => (200, "application/json", fixture("mods_resolve.json")),
                    ("GET", "/v1/mods/403") => (403, "text/plain", b"forbidden".to_vec()),
                    _ => (404, "text/plain", b"not found".to_vec()),
                };
                let header = format!("HTTP/1.1 {status} X\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", payload.len());
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(&payload).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    Server { base, seen }
}

fn make_zip(dir: &Path) -> PathBuf {
    let path = dir.join("pack.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(&fixture("manifest_forge.json")).unwrap();
    zip.start_file("overrides/config/pack.toml", options).unwrap();
    zip.write_all(b"x = 1\n").unwrap();
    zip.finish().unwrap();
    path
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cubby-curseforge-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn client(base: &str) -> CurseForgeClient {
    CurseForgeClient::with_base_url(Http::new().unwrap(), "test-key".into(), base)
}

#[tokio::test]
async fn browse_sends_key_and_maps_answers() {
    let server = serve(None).await;
    let cf = client(&server.base);

    let page = cf.search("all the", PackSort::Downloads, 0, 20).await.unwrap();
    assert_eq!(page.total, Some(1234));
    assert_eq!(page.hits[0].id, "925200");

    let featured = cf.featured().await.unwrap();
    assert_eq!(featured.hits.len(), 1);

    let project = cf.pack(925200).await.unwrap();
    assert_eq!(project.description_html, "<p>Hello</p>");
    assert_eq!(project.summary.name, "Example Forge Pack");

    let versions = cf.versions(925200).await.unwrap();
    assert_eq!(versions.len(), 3);
    // newest first, whatever the API order
    assert_eq!(versions[0].id, "5000001");
    assert_eq!(versions[0].channel, PackChannel::Release);
    assert_eq!(versions[0].loader, Some(PackLoaderKind::Forge));
    assert_eq!(versions[2].id, "4000001");

    let seen = server.seen.lock().unwrap().clone();
    assert!(seen.iter().all(|s| s.api_key.as_deref() == Some("test-key")), "{seen:?}");
    let search = seen.iter().find(|s| s.target.starts_with("/v1/mods/search?")).unwrap();
    for part in ["gameId=432", "classId=4471", "searchFilter=all+the", "sortField=6", "sortOrder=desc", "pageSize=20"] {
        assert!(search.target.contains(part), "{part} missing in {}", search.target);
    }
}

#[tokio::test]
async fn auth_and_missing_errors() {
    let server = serve(None).await;
    // a refused key says so
    assert!(matches!(client(&server.base).pack(403).await, Err(Error::Auth(_))));
    // unknown project
    assert!(matches!(client(&server.base).pack(1).await, Err(Error::NotFound(_))));
    // blank key: no request is made
    let blank = CurseForgeClient::with_base_url(Http::new().unwrap(), "  ".into(), &server.base);
    let before = server.seen.lock().unwrap().len();
    assert!(matches!(blank.search("", PackSort::Relevance, 0, 10).await, Err(Error::Auth(_))));
    assert_eq!(server.seen.lock().unwrap().len(), before);
}

#[tokio::test]
async fn plan_from_zip_resolves_files() {
    let dir = temp_dir("zip");
    let zip = make_zip(&dir);
    let server = serve(Some(zip.clone())).await;
    let cf = client(&server.base);

    // readable without any API call
    assert_eq!(read_manifest(&zip).unwrap().files.len(), 6);
    assert!(server.seen.lock().unwrap().is_empty());

    let plan = cf.plan_from_zip(&zip).await.unwrap();
    assert_eq!(plan.files.len(), 4);
    assert_eq!(plan.blocked.len(), 2);
    assert_eq!(plan.overrides.len(), 1);
    assert_eq!(plan.overrides[0].archive, zip);
    assert_eq!(plan.overrides[0].prefix, "overrides/");

    let seen = server.seen.lock().unwrap().clone();
    let files_call = seen.iter().find(|s| s.method == "POST" && s.target == "/v1/mods/files").unwrap();
    let body: serde_json::Value = serde_json::from_str(&files_call.body).unwrap();
    assert_eq!(body["fileIds"], serde_json::json!([2001, 2002, 2003, 4500012, 2005, 2006]));
    let mods_call = seen.iter().find(|s| s.method == "POST" && s.target == "/v1/mods").unwrap();
    let body: serde_json::Value = serde_json::from_str(&mods_call.body).unwrap();
    assert_eq!(body["modIds"], serde_json::json!([1001, 1002, 1003, 1004, 1005, 1006]));

    // the CDN fallback is opt-in
    let plan = client(&server.base).with_cdn_fallback(true).plan_from_zip(&zip).await.unwrap();
    assert_eq!(plan.blocked.len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn plan_downloads_the_pack_and_checks_its_hash() {
    let dir = temp_dir("plan");
    let source = temp_dir("plan-src");
    let zip = make_zip(&source);
    let server = serve(Some(zip.clone())).await;
    let cache = dir.join("cache");

    let plan = client(&server.base).plan(925200, 5000001, &cache).await.unwrap();
    assert_eq!(plan.pack_id, "925200");
    assert_eq!(plan.version_id, "5000001");
    assert_eq!(plan.version_name, "Example Forge Pack 1.2.0");
    assert_eq!(plan.title, "Example Forge Pack");
    assert_eq!(plan.icon_url.as_deref(), Some("https://media.forgecdn.net/avatars/thumbnails/1/2/256/256/pack.png"));
    assert_eq!(plan.overrides[0].archive, cache.join("curseforge-925200-5000001.zip"));
    assert!(plan.overrides[0].archive.exists());
    assert_eq!(plan.files.len(), 4);

    // a pack file without downloadUrl: error pointing at the website
    let error = client(&server.base).plan(925200, 4000001, &cache).await.unwrap_err().to_string();
    assert!(error.contains("https://www.curseforge.com/projects/925200/download/4000001"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(source);
}

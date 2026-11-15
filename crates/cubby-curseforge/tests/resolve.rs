//! Resolving a mod and the projects it requires, against a tiny local HTTP server with hand-written answers.

use std::sync::{Arc, Mutex};

use cubby_common::net::Http;
use cubby_common::packs::PackLoaderKind;
use cubby_curseforge::{CurseForgeClient, FileTarget};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn project(id: u64, name: &str) -> Value {
    json!({ "id": id, "name": name, "slug": name.to_lowercase().replace(' ', "-"), "classId": 6, "links": {} })
}

fn file(id: u64, mod_id: u64, date: &str, versions: &[&str], requires: &[u64]) -> Value {
    json!({
        "id": id, "modId": mod_id, "displayName": format!("file {id}"), "fileName": format!("f{id}.jar"),
        "releaseType": 1, "fileDate": date, "fileLength": 100, "downloadUrl": format!("https://cdn.example/f{id}.jar"),
        "hashes": [{ "value": format!("{id:040}"), "algo": 1 }],
        "gameVersions": versions,
        "dependencies": requires.iter().map(|m| json!({ "modId": m, "relationType": 3 })).collect::<Vec<_>>(),
    })
}

fn files_of(id: u64) -> Vec<Value> {
    match id {
        // the mod: an older NeoForge file and the Fabric one
        100 => vec![file(1001, 100, "2026-05-01", &["1.21.1", "NeoForge"], &[]), file(1002, 100, "2026-04-01", &["1.21.1", "Fabric"], &[200, 300])],
        200 => vec![file(2001, 200, "2026-05-01", &["1.21.1", "Fabric"], &[])],
        300 => vec![file(3001, 300, "2026-05-01", &["1.21.1", "Fabric"], &[400, 100])],
        // nothing for Fabric
        400 => vec![file(4001, 400, "2026-05-01", &["1.21.1", "Forge"], &[])],
        _ => Vec::new(),
    }
}

async fn serve() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { return };
            let seen = seen2.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let (head_end, len) = loop {
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
                while buf.len() < head_end + len {
                    let n = socket.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
                let mut first = head.lines().next().unwrap_or("").split(' ');
                let (method, target) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
                let body: Value = serde_json::from_slice(&buf[head_end..]).unwrap_or(Value::Null);
                seen.lock().unwrap().push(format!("{method} {target}"));
                let path = target.split('?').next().unwrap_or("");
                let answer = if method == "POST" && path == "/v1/mods" {
                    let names = [(100, "My Mod"), (200, "Fabric API"), (300, "Some Lib"), (400, "Forge Only")];
                    let ids: Vec<u64> = body["modIds"].as_array().unwrap().iter().filter_map(Value::as_u64).collect();
                    Some(json!({ "data": names.iter().filter(|(id, _)| ids.contains(id)).map(|(id, n)| project(*id, n)).collect::<Vec<_>>() }))
                } else if let Some(id) = path.strip_prefix("/v1/mods/").and_then(|r| r.strip_suffix("/files")).and_then(|id| id.parse::<u64>().ok()) {
                    let data = files_of(id);
                    Some(json!({ "data": data, "pagination": { "index": 0, "pageSize": 50, "resultCount": data.len(), "totalCount": data.len() } }))
                } else {
                    None
                };
                let (status, payload) = match answer {
                    Some(v) => (200, v.to_string()),
                    None => (404, "not found".to_string()),
                };
                let header = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", payload.len());
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(payload.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    (base, seen)
}

#[tokio::test]
async fn resolves_required_projects_for_the_instance() {
    let (base, seen) = serve().await;
    let cf = CurseForgeClient::with_base_url(Http::new().unwrap(), "k".into(), base);
    let target = FileTarget { game_version: Some("1.21.1".into()), loaders: vec![PackLoaderKind::Fabric] };

    // Fabric API is already in the instance
    let plan = cf.resolve(100, None, &target, |p| p.slug == "fabric-api").await.unwrap();
    let ids: Vec<(u64, Option<u64>, bool)> = plan.iter().map(|r| (r.project.id, r.file.as_ref().map(|f| f.id), r.dependency)).collect();
    assert_eq!(ids, vec![(100, Some(1002), false), (300, Some(3001), true), (400, None, true)]);
    assert_eq!(plan[0].file.as_ref().unwrap().download_url.as_deref(), Some("https://cdn.example/f1002.jar"));
    assert_eq!(plan[2].project.name, "Forge Only");

    // the files are asked for the instance's Minecraft version
    assert!(seen.lock().unwrap().iter().any(|r| r.starts_with("GET /v1/mods/100/files?") && r.contains("gameVersion=1.21.1")));

    // a chosen file is used as it is
    let unknown = cf.resolve(100, Some(1001), &target, |_| false).await;
    // `/v1/mods/100/files/1001` isn't served: the error says so
    assert!(unknown.is_err());

    // no suitable file: the requested project still comes back, without a file
    let none = cf.resolve(400, None, &target, |_| false).await.unwrap();
    assert_eq!(none.len(), 1);
    assert!(none[0].file.is_none());
}

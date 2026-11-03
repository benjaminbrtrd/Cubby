//! The client against a local HTTP server serving the saved responses of the real API (see
//! `tests/fixtures`) and synthetic zips.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use cubby_common::net::Http;
use cubby_common::packs::{PackChannel, PackLoaderKind};
use cubby_common::Error;
use cubby_technic::TechnicClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const SEARCH_HEXXIT: &str = include_str!("fixtures/search_hexxit.json");
const TRENDING: &str = include_str!("fixtures/trending.json");
const PACK_ZIP: &str = include_str!("fixtures/pack_zip.json");
const PACK_SOLDER: &str = include_str!("fixtures/pack_solder.json");
const PACK_TEKKIT: &str = include_str!("fixtures/pack_tekkit.json");
const SOLDER_PACK_1122: &str = include_str!("fixtures/solder_pack_1122.json");
const SOLDER_PACK_TEKKIT: &str = include_str!("fixtures/solder_pack_tekkit.json");
const SOLDER_BUILD_1122: &str = include_str!("fixtures/solder_build_1122.json");
const SOLDER_BUILD_TEKKIT: &str = include_str!("fixtures/solder_build_tekkit.json");

const TECHNIC_SOLDER: &str = "https://solder.technicpack.net/api/";
const MIRROR: &str = "https://mirror-mods.technicpack.net";

// mock server

struct Server {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl Server {
    /// Serves `routes` (path without query -> body); anything else is a 404. Returns the server once the
    /// routes can reference its own address: `routes` receives the base URL.
    async fn start(routes: impl FnOnce(&str) -> HashMap<String, Vec<u8>>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let routes = Arc::new(routes(&base));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else { return };
                let (routes, log) = (routes.clone(), log.clone());
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 4096];
                    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        match socket.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        }
                    }
                    let head = String::from_utf8_lossy(&buf).to_string();
                    let target = head.lines().next().unwrap_or_default().split(' ').nth(1).unwrap_or("/").to_string();
                    log.lock().unwrap().push(target.clone());
                    let path = target.split('?').next().unwrap_or("/");
                    let (status, body) = match routes.get(path) {
                        Some(body) => ("200 OK", body.as_slice()),
                        None => ("404 Not Found", br#"{"error":"nope"}"#.as_slice()),
                    };
                    let header = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(body).await;
                    let _ = socket.shutdown().await;
                });
            }
        });
        Server { base, requests }
    }

    fn client(&self) -> TechnicClient {
        TechnicClient::with_base_url(Http::new().unwrap(), format!("{}/api", self.base))
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut out);
    for (name, content) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(content).unwrap();
    }
    zip.finish().unwrap();
    out.into_inner()
}

fn entry_names(archive: &Path) -> Vec<String> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive).unwrap()).unwrap();
    let mut names: Vec<String> = (0..zip.len()).map(|i| zip.by_index_raw(i).unwrap().name().to_string()).collect();
    names.sort();
    names
}

fn read_entry(archive: &Path, name: &str) -> String {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive).unwrap()).unwrap();
    let mut s = String::new();
    zip.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

const FORGE_VERSION_JSON: &str = r#"{"id":"1.12.2-forge-14.23.5.2860","inheritsFrom":"1.12.2","libraries":[{"name":"net.minecraftforge:forge:1.12.2-14.23.5.2860"}]}"#;

fn routes(entries: Vec<(&str, Vec<u8>)>) -> HashMap<String, Vec<u8>> {
    entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

fn text(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

// browsing

#[tokio::test]
async fn featured_and_search() {
    let server = Server::start(|_| routes(vec![("/api/trending", text(TRENDING)), ("/api/search", text(SEARCH_HEXXIT))])).await;
    let client = server.client();

    let featured = client.featured().await.unwrap();
    assert_eq!(featured.hits.len(), 5);
    assert_eq!(featured.hits[1].id, "shivaxi-rlcraft");
    assert_eq!(featured.hits[1].name, "RLCraft");
    assert_eq!(featured.hits[1].website_url.as_deref(), Some("https://www.technicpack.net/modpack/shivaxi-rlcraft.1530385"));
    assert!(featured.hits[1].icon_url.as_deref().unwrap().starts_with("https://cdn.technicpack.net/platform2/pack-icons/1530385.png"));

    let found = client.search("  hexxit ").await.unwrap();
    assert_eq!(found.hits[0].id, "hexxit");
    assert_eq!(found.hits[0].name, "Hexxit");
    assert!(found.hits.len() > 3 && found.total.is_none());

    // The API refuses an empty query: it becomes the trending list.
    assert_eq!(client.search("").await.unwrap(), featured);

    let requests = server.requests();
    assert!(requests.contains(&"/api/trending?build=multimc".to_string()), "{requests:?}");
    assert!(requests.contains(&"/api/search?build=multimc&q=hexxit".to_string()), "{requests:?}");
}

#[tokio::test]
async fn pack_pages() {
    let server = Server::start(|_| routes(vec![("/api/modpack/shivaxi-rlcraft", text(PACK_ZIP)), ("/api/modpack/the-1122-pack", text(PACK_SOLDER))])).await;
    let client = server.client();

    let zip = client.pack("shivaxi-rlcraft").await.unwrap();
    assert!(!zip.is_solder());
    assert_eq!(zip.pack_url.as_deref(), Some("https://bhrepo.com/packs/rlcraft/modpack.zip"));
    assert_eq!(zip.solder_url, None);
    assert_eq!(zip.name, "shivaxi-rlcraft");
    assert!(zip.minecraft.is_some() && zip.version.is_some() && !zip.display_name.is_empty());
    assert!(zip.installs.unwrap() > 0 && zip.icon_url.is_some() && zip.logo_url.is_some() && zip.background_url.is_some());
    assert!(zip.platform_url.unwrap().starts_with("https://www.technicpack.net/modpack/"));

    let solder = client.pack("the-1122-pack").await.unwrap();
    assert!(solder.is_solder());
    assert_eq!(solder.pack_url, None);
    // No trailing slash, whatever the API sends.
    assert_eq!(solder.solder_url.as_deref(), Some("https://solder.technicpack.net/api"));
    assert_eq!(solder.display_name, "The 1.12.2 Pack");
    assert_eq!(solder.user, "xJon");
    assert_eq!(solder.minecraft.as_deref(), Some("1.12.2"));
    assert_eq!(solder.version.as_deref(), Some("1.6.6"));
    assert!(solder.tags.contains(&"kitchen sink".to_string()));
    assert!(solder.server_pack_url.is_some());

    assert!(matches!(client.pack("missing-pack").await, Err(Error::NotFound(_))));
    // Slugs can't change the path.
    assert!(client.pack("../trending").await.is_err());
    assert!(client.pack("").await.is_err());
}

#[tokio::test]
async fn solder_versions_are_ordered_and_marked() {
    let server = Server::start(|base| {
        let solder = format!("{base}/solder/");
        routes(vec![
            ("/api/modpack/the-1122-pack", text(&PACK_SOLDER.replace(TECHNIC_SOLDER, &solder))),
            ("/api/modpack/tekkit", text(&PACK_TEKKIT.replace(TECHNIC_SOLDER, &solder))),
            ("/api/modpack/shivaxi-rlcraft", text(PACK_ZIP)),
            ("/solder/modpack/the-1122-pack", text(SOLDER_PACK_1122)),
            ("/solder/modpack/tekkit", text(SOLDER_PACK_TEKKIT)),
        ])
    })
    .await;
    let client = server.client();

    // Oldest-first listing is reversed; the recommended build comes first.
    let versions = client.versions("the-1122-pack").await.unwrap();
    assert_eq!(versions.len(), 36);
    assert_eq!((versions[0].id.as_str(), versions[0].name.as_str(), versions[0].channel), ("1.6.6", "1.6.6 (recommended)", PackChannel::Release));
    assert_eq!(versions[1].id, "1.6.5");
    assert_eq!(versions[35].id, "1.0.0");

    // Newest-first listing: latest 3.1.3 is newer than recommended 3.1.2.
    let versions = client.versions("tekkit").await.unwrap();
    let ids: Vec<&str> = versions.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(ids[..4], ["3.1.2", "3.1.3", "3.1.1", "3.1.0"]);
    assert_eq!(versions[0].channel, PackChannel::Release);
    assert_eq!((versions[1].channel, versions[1].name.as_str()), (PackChannel::Beta, "3.1.3 (latest)"));
    assert!(versions[2..].iter().all(|v| v.channel == PackChannel::Release));

    // A zip pack has its single current version.
    let versions = client.versions("shivaxi-rlcraft").await.unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].channel, PackChannel::Release);
    assert!(versions[0].minecraft.is_some() && !versions[0].id.is_empty());
}

// plans

#[tokio::test]
async fn plans_a_zip_pack() {
    let inner_jar = zip_bytes(&[("version.json", FORGE_VERSION_JSON.as_bytes()), ("fmlversion.properties", b"fmlbuild.mcversion=1.12.2")]);
    let pack_zip = zip_bytes(&[
        ("bin/modpack.jar", &inner_jar),
        ("bin/extra.txt", b"x"),
        ("mods/a.jar", b"jar a"),
        ("config/x.cfg", b"cfg"),
        ("../evil.txt", b"evil"),
        ("C:/evil2.txt", b"evil"),
    ]);
    let server = Server::start(|base| routes(vec![("/api/modpack/shivaxi-rlcraft", text(&PACK_ZIP.replace("https://bhrepo.com/packs/rlcraft/modpack.zip", &format!("{base}/dl/modpack.zip")))), ("/dl/modpack.zip", pack_zip)])).await;
    let cache = tempfile::tempdir().unwrap();

    let planned = server.client().plan_detailed("shivaxi-rlcraft", "anything", cache.path()).await.unwrap();
    let plan = &planned.plan;
    assert_eq!(plan.provider, "technic");
    assert_eq!(plan.pack_id, "shivaxi-rlcraft");
    assert_eq!(plan.version_id, "anything");
    assert_eq!(plan.title, "RLCraft");
    assert_eq!(plan.minecraft, "1.12.2");
    assert_eq!(plan.loader, Some((PackLoaderKind::Forge, "14.23.5.2860".to_string())));
    assert!(plan.icon_url.is_some() && plan.files.is_empty() && plan.blocked.is_empty());
    assert_eq!(plan.recommended_memory_mib, None);
    assert!(!planned.legacy_jar_mod && planned.jar_mod.is_none());
    assert!(planned.notes.iter().any(|n| n.contains("2 file(s) with an unsafe path")), "{:?}", planned.notes);

    // One archive for the whole pack, with no bin/ and no unsafe path.
    assert_eq!(plan.overrides.len(), 1);
    let archive = &plan.overrides[0].archive;
    assert_eq!(plan.overrides[0].prefix, "");
    assert!(archive.starts_with(cache.path()));
    assert_eq!(entry_names(archive), ["config/x.cfg", "mods/a.jar"]);
    assert_eq!(read_entry(archive, "mods/a.jar"), "jar a");
    // Only the plan's archive is left behind (no .src.zip).
    let leftovers: Vec<String> = std::fs::read_dir(archive.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert_eq!(leftovers, ["000-pack.zip"]);

    // `plan` is the same thing without the extras.
    let again = server.client().plan("shivaxi-rlcraft", "", cache.path()).await.unwrap();
    assert_eq!(again.version_id, again.version_name);
    assert_eq!(again.minecraft, "1.12.2");
}

#[tokio::test]
async fn plans_a_vanilla_zip_pack_and_rejects_bad_zips() {
    let pack_zip = zip_bytes(&[("mods/a.jar", b"jar a")]);
    let server = Server::start(|base| {
        routes(vec![
            ("/api/modpack/shivaxi-rlcraft", text(&PACK_ZIP.replace("https://bhrepo.com/packs/rlcraft/modpack.zip", &format!("{base}/dl/modpack.zip")))),
            ("/api/modpack/tekxit-3-official-1122", text(&PACK_ZIP.replace("https://bhrepo.com/packs/rlcraft/modpack.zip", &format!("{base}/dl/bad.zip")))),
            ("/dl/modpack.zip", pack_zip),
            ("/dl/bad.zip", text("this is not a zip")),
        ])
    })
    .await;
    let cache = tempfile::tempdir().unwrap();

    // No bin/: vanilla, with the Minecraft version of the pack page; the zip is kept as it is.
    let plan = server.client().plan("shivaxi-rlcraft", "1", cache.path()).await.unwrap();
    assert_eq!(plan.loader, None);
    assert_eq!(plan.minecraft, "1.12.2");
    assert_eq!(entry_names(&plan.overrides[0].archive), ["mods/a.jar"]);

    assert!(server.client().plan("tekxit-3-official-1122", "1", cache.path()).await.is_err());
}

#[tokio::test]
async fn plans_a_solder_build() {
    let forge_zip = zip_bytes(&[("bin/version.json", FORGE_VERSION_JSON.as_bytes()), ("config/forge.cfg", b"forge")]);
    let banners_zip = zip_bytes(&[("mods/additionalbanners.jar", b"banners")]);
    let pipes_zip = zip_bytes(&[("mods/additionalpipes.jar", b"pipes"), ("config/pipes.cfg", b"new")]);
    let server = Server::start(|base| {
        let solder = format!("{base}/solder/");
        let build = SOLDER_BUILD_1122.replace(MIRROR, base);
        // Three mods: the forge zip goes first, then the two mods of the fixture.
        let build = build.replacen(r#""mods":["#, &format!(r#""mods":[{{"name":"forge","version":"14.23.5","md5":"","url":"{base}/mods/forge.zip","filesize":1}},"#), 1);
        routes(vec![
            ("/api/modpack/the-1122-pack", text(&PACK_SOLDER.replace(TECHNIC_SOLDER, &solder))),
            ("/solder/modpack/the-1122-pack/1.6.6", text(&build)),
            ("/mods/forge.zip", forge_zip),
            ("/mods/additionalbanners/additionalbanners-1.12.2-1.1.77.zip", banners_zip),
            ("/mods/additionalpipes/additionalpipes-1.12.2-6.0.0.9-technic.zip", pipes_zip),
        ])
    })
    .await;
    let cache = tempfile::tempdir().unwrap();

    let planned = server.client().plan_detailed("the-1122-pack", "1.6.6", cache.path()).await.unwrap();
    let plan = &planned.plan;
    assert_eq!((plan.provider.as_str(), plan.pack_id.as_str(), plan.version_id.as_str(), plan.version_name.as_str()), ("technic", "the-1122-pack", "1.6.6", "1.6.6"));
    assert_eq!(plan.title, "The 1.12.2 Pack");
    assert_eq!(plan.minecraft, "1.12.2");
    assert_eq!(plan.loader, Some((PackLoaderKind::Forge, "14.23.5.2860".to_string())));
    assert_eq!(plan.recommended_memory_mib, Some(5120));
    assert!(planned.notes.is_empty(), "{:?}", planned.notes);

    // One archive per mod, in the order of the build, with no bin/.
    let names: Vec<Vec<String>> = plan.overrides.iter().map(|o| entry_names(&o.archive)).collect();
    assert_eq!(names, [vec!["config/forge.cfg".to_string()], vec!["mods/additionalbanners.jar".to_string()], vec!["config/pipes.cfg".to_string(), "mods/additionalpipes.jar".to_string()]]);
    assert!(plan.overrides.iter().all(|o| o.prefix.is_empty() && o.archive.starts_with(cache.path())));

    // Unknown build, unknown pack.
    assert!(matches!(server.client().plan("the-1122-pack", "9.9.9", cache.path()).await, Err(Error::NotFound(_))));
    assert!(server.client().plan("the-1122-pack", "", cache.path()).await.is_err());
}

#[tokio::test]
async fn plans_a_legacy_jar_mod_solder_build() {
    let props = "forge.major.number=3\nforge.minor.number=4\nforge.revision.number=9\nforge.build.number=164\n";
    let modpack_jar = zip_bytes(&[("forgeversion.properties", props.as_bytes()), ("net/minecraftforge/common/Forge.class", b"class")]);
    let basemods = zip_bytes(&[("bin/modpack.jar", &modpack_jar), ("mods/base.zip", b"base")]);
    let server = Server::start(|base| {
        let solder = format!("{base}/solder/");
        routes(vec![
            ("/api/modpack/tekkit", text(&PACK_TEKKIT.replace(TECHNIC_SOLDER, &solder))),
            ("/solder/modpack/tekkit/3.1.2", text(&SOLDER_BUILD_TEKKIT.replace(MIRROR, base))),
            ("/mods/balkonsweaponmod/balkonsweaponmod-v8.6.zip", zip_bytes(&[("mods/balkon.zip", b"b")])),
            ("/mods/basemods/basemods-tekkit-v3.1.2.zip", basemods),
            ("/mods/buildcraft/buildcraft-v2.2.14.zip", zip_bytes(&[("mods/buildcraft.zip", b"bc")])),
        ])
    })
    .await;
    let cache = tempfile::tempdir().unwrap();

    let planned = server.client().plan_detailed("tekkit", "3.1.2", cache.path()).await.unwrap();
    // Minecraft version of the build (1.2.5), not of the pack page (1.4.7).
    assert_eq!(planned.plan.minecraft, "1.2.5");
    assert_eq!(planned.plan.loader, Some((PackLoaderKind::Forge, "3.4.9.164".to_string())));
    assert!(planned.legacy_jar_mod);
    assert_eq!(std::fs::read(planned.jar_mod.as_ref().unwrap()).unwrap(), modpack_jar);
    assert!(planned.notes.iter().any(|n| n.contains("jar mod")));
    assert_eq!(planned.plan.recommended_memory_mib, None);
    assert_eq!(planned.plan.overrides.len(), 3);
    assert_eq!(entry_names(&planned.plan.overrides[1].archive), ["mods/base.zip"]);
}

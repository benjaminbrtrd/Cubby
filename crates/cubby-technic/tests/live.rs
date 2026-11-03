//! Tests against the real Technic API. Read-only GET requests; the plan downloads the zips of a small
//! Solder pack into a scratch folder. Run with `-- --ignored`.

use std::path::PathBuf;

use cubby_common::net::Http;
use cubby_common::packs::PackChannel;
use cubby_technic::TechnicClient;

fn client() -> TechnicClient {
    TechnicClient::new(Http::new().unwrap())
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("cubby-technic-live").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
#[ignore = "needs network"]
async fn browse_search_and_pack_pages() {
    let c = client();
    let featured = c.featured().await.unwrap();
    assert!(featured.hits.len() >= 5, "{featured:?}");
    assert!(featured.hits.iter().all(|h| !h.id.is_empty() && !h.name.is_empty()));

    let found = c.search("hexxit").await.unwrap();
    let hexxit = found.hits.iter().find(|h| h.id == "hexxit").expect("hexxit in the results");
    assert_eq!(hexxit.name, "Hexxit");

    let pack = c.pack("hexxit").await.unwrap();
    assert!(pack.is_solder());
    assert_eq!(pack.display_name, "Hexxit");
    assert!(pack.installs.unwrap() > 1_000_000);

    let versions = c.versions("hexxit").await.unwrap();
    assert!(versions.len() > 5);
    assert!(versions[0].name.contains("recommended") && versions[0].channel == PackChannel::Release);

    // A pack hosted as a single zip.
    let zip = c.pack("shivaxi-rlcraft").await.unwrap();
    assert!(!zip.is_solder() && zip.pack_url.is_some());
    assert_eq!(c.versions("shivaxi-rlcraft").await.unwrap().len(), 1);
}

/// Hexxit is a legacy (Minecraft 1.5.2) jar-mod pack of about 40 MB.
#[tokio::test]
#[ignore = "needs network"]
async fn plans_hexxit() {
    let cache = scratch("hexxit");
    let c = client();
    let found = c.search("hexxit").await.unwrap();
    let id = &found.hits.iter().find(|h| h.id == "hexxit").unwrap().id;
    let versions = c.versions(id).await.unwrap();
    let recommended = versions.iter().find(|v| v.name.contains("recommended")).unwrap();

    let planned = c.plan_detailed(id, &recommended.id, &cache).await.unwrap();
    let plan = &planned.plan;
    println!("{plan:#?}\n{:?}\n{:?}", planned.jar_mod, planned.notes);
    assert_eq!(plan.provider, "technic");
    assert_eq!(plan.title, "Hexxit");
    assert_eq!(plan.minecraft, "1.5.2");
    assert!(plan.overrides.len() > 20);
    assert!(plan.overrides.iter().all(|o| o.archive.exists() && o.prefix.is_empty()));
    assert!(planned.legacy_jar_mod && planned.jar_mod.as_ref().is_some_and(|p| p.exists()));
    let (kind, version) = plan.loader.clone().expect("Forge version from forgeversion.properties");
    assert_eq!(kind, cubby_common::packs::PackLoaderKind::Forge);
    assert!(version.split('.').count() == 4, "{version}");
}

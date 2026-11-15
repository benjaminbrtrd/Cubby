//! Lists the contents of a real CurseForge modpack file. Needs network and `CUBBY_CURSEFORGE_API_KEY`; run with
//! `-- --ignored --nocapture`.

use cubby_common::net::Http;
use cubby_common::packs::PackSort;
use cubby_curseforge::{api_key_from_env, CurseForgeClient};

#[tokio::test]
#[ignore = "needs network and a CurseForge API key"]
async fn lists_a_popular_pack() {
    let Some(key) = api_key_from_env() else {
        println!("no CurseForge API key: skipped");
        return;
    };
    let client = CurseForgeClient::new(Http::new().unwrap(), key);
    let pack = client.search("", PackSort::Downloads, 0, 1).await.unwrap().hits.remove(0);
    let version = client.versions(pack.id.parse().unwrap()).await.unwrap().remove(0);
    let dir = std::env::temp_dir().join(format!("cubby-cf-contents-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let items = client.pack_contents(pack.id.parse().unwrap(), version.id.parse().unwrap(), &dir).await.unwrap();
    println!("{} {}: {} items", pack.name, version.name, items.len());
    for i in items.iter().take(10) {
        println!("  {:?} {} {:?} {:?} optional={}", i.kind, i.name, i.version, i.page_url, i.optional);
    }
    assert!(items.len() > 5);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "the pack zip is deleted");
    let _ = std::fs::remove_dir(&dir);
}

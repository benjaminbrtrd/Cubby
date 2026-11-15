//! Lists the contents of a real ATLauncher pack version. Needs network; run with `-- --ignored --nocapture`.

use cubby_atlauncher::{manifest_contents, AtLauncherClient};
use cubby_common::net::Http;

#[tokio::test]
#[ignore = "needs network"]
async fn lists_a_pack() {
    let atl = AtLauncherClient::new(Http::new().unwrap());
    let pack = atl.packs().await.unwrap().remove(0);
    let version = atl.versions(&pack.id).await.unwrap().remove(0);
    let items = manifest_contents(&atl.manifest(&pack.id, &version.id).await.unwrap());
    println!("{} {}: {} items", pack.name, version.name, items.len());
    for i in items.iter().take(10) {
        println!("  {:?} {} {:?} {:?} {:?} optional={}", i.kind, i.name, i.version, i.project_id, i.page_url, i.optional);
    }
    assert!(!items.is_empty());
}

//! Lists the contents of a real Technic Solder build. Needs network; run with `-- --ignored --nocapture`.

use cubby_common::net::Http;
use cubby_technic::TechnicClient;

#[tokio::test]
#[ignore = "needs network"]
async fn lists_a_solder_build() {
    let technic = TechnicClient::new(Http::new().unwrap());
    let version = technic.versions("tekkitmain").await.unwrap().remove(0);
    let items = technic.contents("tekkitmain", &version.id).await.unwrap().expect("a Solder pack lists its mods");
    println!("Tekkit {}: {} items", version.name, items.len());
    for i in items.iter().take(10) {
        println!("  {} {:?} {:?}", i.name, i.version, i.page_url);
    }
    assert!(items.len() > 5);
}

//! Lists the contents of a real FTB pack version. Needs network; run with `-- --ignored --nocapture`.

use cubby_common::net::Http;
use cubby_common::packs::PackSort;
use cubby_ftb::FtbClient;

#[tokio::test]
#[ignore = "needs network"]
async fn lists_a_popular_pack() {
    let ftb = FtbClient::new(Http::new().unwrap());
    let pack = ftb.search("", PackSort::Downloads, 0, 1).await.unwrap().hits.remove(0);
    let version = ftb.versions(&pack.id).await.unwrap().remove(0);
    let items = ftb.contents(&pack.id, &version.id).await.unwrap();
    println!("{} {}: {} items", pack.name, version.name, items.len());
    for i in items.iter().take(10) {
        println!("  {:?} {} {:?} {:?} optional={}", i.kind, i.name, i.project_id, i.page_url, i.optional);
    }
    assert!(items.len() > 5);
}

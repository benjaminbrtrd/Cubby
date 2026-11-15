//! Lists the contents of a real modpack version. Needs network; run with `-- --ignored --nocapture`.

use cubby_common::net::Http;
use cubby_modrinth::Modrinth;

#[tokio::test]
#[ignore = "needs network"]
async fn lists_a_popular_modpack() {
    let modrinth = Modrinth::new(Http::new().unwrap());
    let version = modrinth.versions("fabulously-optimized", None, None).await.unwrap().remove(0);
    let dir = std::env::temp_dir().join(format!("cubby-contents-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let items = modrinth.pack_contents(&version.id, &dir).await.unwrap();
    println!("Fabulously Optimized {}: {} items", version.version_number, items.len());
    for i in &items {
        println!("  {:?} {} {:?} by {:?} cats={:?}", i.kind, i.name, i.version, i.authors, i.categories);
    }
    assert!(items.len() > 5);
    assert!(items.iter().filter(|i| !i.authors.is_empty()).count() > items.len() / 2, "most have an owner");
    assert!(items.iter().filter(|i| i.icon_url.is_some()).count() > items.len() / 2, "most are named from Modrinth");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "the .mrpack is deleted");
    let _ = std::fs::remove_dir(&dir);
}

//! Tests against the real FTB API (read-only, no file download). Run with `-- --ignored`.

use cubby_common::net::Http;
use cubby_common::packs::PackSort;
use cubby_ftb::{FtbClient, LEGACY_API_BASE};

fn client() -> FtbClient {
    FtbClient::new(Http::new().unwrap())
}

#[tokio::test]
#[ignore = "needs network"]
async fn browse_and_plan_stoneblock() {
    let ftb = client();

    let featured = ftb.featured().await.unwrap();
    assert!(featured.hits.len() >= 5, "{featured:?}");
    assert!(featured.hits.iter().all(|p| !p.name.is_empty() && p.icon_url.is_some() && !p.minecraft.is_empty()));

    let found = ftb.search("stoneblock", PackSort::Relevance, 0, 10).await.unwrap();
    assert!(found.hits.iter().any(|p| p.id == "100"), "{found:?}");
    let by_name = ftb.search("", PackSort::Name, 0, 5).await.unwrap();
    assert_eq!(by_name.hits.len(), 5);
    assert!(by_name.total.unwrap() >= 50);

    let pack = ftb.pack("100").await.unwrap();
    assert_eq!(pack.name, "FTB StoneBlock 3");
    let details = ftb.details("100").await.unwrap();
    assert!(!details.description.is_empty());

    let versions = ftb.versions("100").await.unwrap();
    assert!(versions.len() >= 10 && versions.windows(2).all(|w| w[0].id.parse::<u64>().unwrap() > w[1].id.parse::<u64>().unwrap()));

    // 12632 (1.11.1) is a fixed, old version: stable to assert on.
    let plan = ftb.plan("100", "12632", &std::env::temp_dir()).await.unwrap();
    assert_eq!(plan.minecraft, "1.18.2");
    assert_eq!(plan.loader.as_ref().map(|l| l.1.as_str()), Some("40.2.14"));
    assert!(plan.files.len() > 1000, "{}", plan.files.len());
    assert!(plan.files.iter().all(|f| f.path.is_relative() && !f.urls.is_empty()));
    println!("{} files, {} blocked, memory {:?}", plan.files.len(), plan.blocked.len(), plan.recommended_memory_mib);

    // Every version of a modern pack plans without error (neoforge, tiny sample of the catalogue).
    let latest = &ftb.versions("130").await.unwrap()[0];
    let plan = ftb.plan("130", &latest.id, &std::env::temp_dir()).await.unwrap();
    assert!(plan.loader.is_some() && !plan.files.is_empty());
}

#[tokio::test]
#[ignore = "needs network"]
async fn legacy_host_still_answers() {
    let ftb = FtbClient::with_base_url(Http::new().unwrap(), LEGACY_API_BASE);
    let pack = ftb.pack("100").await.unwrap();
    assert_eq!(pack.name, "FTB StoneBlock 3");
    assert!(matches!(ftb.pack("99999999").await, Err(cubby_common::Error::NotFound(_))));
}

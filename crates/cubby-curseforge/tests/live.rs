//! Tests against the real CurseForge API (read-only, no pack download). They need a key:
//! `CUBBY_CURSEFORGE_API_KEY=... cargo test -p cubby-curseforge --test live -- --ignored`.
//! Without the variable they pass without doing anything.

use cubby_common::net::Http;
use cubby_common::packs::PackSort;
use cubby_curseforge::{api_key_from_env, CurseForgeClient};

fn client() -> Option<CurseForgeClient> {
    let key = std::env::var("CUBBY_CURSEFORGE_API_KEY").ok().filter(|k| !k.trim().is_empty()).or_else(api_key_from_env)?;
    Some(CurseForgeClient::new(Http::new().unwrap(), key))
}

#[tokio::test]
#[ignore = "needs network"]
async fn search_featured_and_versions() {
    let Some(cf) = client() else {
        eprintln!("CUBBY_CURSEFORGE_API_KEY is not set, skipping");
        return;
    };
    let page = cf.search("all the mods", PackSort::Downloads, 0, 5).await.unwrap();
    assert!(page.total.unwrap_or(0) > 0 && !page.hits.is_empty() && page.hits.len() <= 5);

    let featured = cf.featured().await.unwrap();
    assert!(!featured.hits.is_empty());

    let first = &page.hits[0];
    let id: u64 = first.id.parse().unwrap();
    let project = cf.pack(id).await.unwrap();
    assert_eq!(project.summary.name, first.name);
    assert!(!project.description_html.is_empty());

    let versions = cf.versions(id).await.unwrap();
    assert!(!versions.is_empty());
    assert!(versions.iter().any(|v| v.minecraft.is_some()));
}

/// Plans the newest file of the first search hit (downloads one pack zip into the target directory).
#[tokio::test]
#[ignore = "needs network"]
async fn plan_newest_version() {
    let Some(cf) = client() else {
        eprintln!("CUBBY_CURSEFORGE_API_KEY is not set, skipping");
        return;
    };
    let page = cf.search("", PackSort::Downloads, 0, 1).await.unwrap();
    let id: u64 = page.hits[0].id.parse().unwrap();
    let versions = cf.versions(id).await.unwrap();
    let file_id: u64 = versions[0].id.parse().unwrap();
    let cache = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/curseforge/live");
    let plan = cf.plan(id, file_id, &cache).await.unwrap();
    assert_eq!(plan.provider, "curseforge");
    assert!(!plan.minecraft.is_empty());
    assert!(plan.files.len() + plan.blocked.len() > 0);
}

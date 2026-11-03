//! Tests against the real ATLauncher CDN (read-only; the planned packs have tiny configs zips, which are
//! the only files downloaded). Run with `-- --ignored`.

use cubby_atlauncher::{AtLauncherClient, OptionalSelection};
use cubby_common::net::Http;
use cubby_common::packs::{PackLoaderKind, PackSort};

fn cache(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("cubby-atl-live-{name}-{}", std::process::id()))
}

#[tokio::test]
#[ignore = "needs network"]
async fn lists_packs_and_plans_a_small_one() {
    let atl = AtLauncherClient::new(Http::new().unwrap());

    let packs = atl.packs().await.unwrap();
    println!("{} public packs", packs.len());
    assert!(packs.len() > 50, "{} packs", packs.len());
    assert!(packs.iter().all(|p| !p.id.is_empty() && p.icon_url.is_some()));
    let found = atl.search("lithium", PackSort::Relevance).await.unwrap();
    assert!(found.hits.iter().any(|h| h.id == "Lithium"), "{:?}", found.hits.iter().map(|h| &h.id).collect::<Vec<_>>());

    let versions = atl.versions("Lithium").await.unwrap();
    assert!(!versions.is_empty());
    let version = &versions[0];
    let manifest = atl.manifest("Lithium", &version.id).await.unwrap();
    assert_eq!(Some(manifest.minecraft.clone()), version.minecraft);
    assert!(manifest.unsupported_reasons().is_empty());
    let picked = OptionalSelection::new(&manifest).names();

    let cache = cache("lithium");
    let result = atl.plan_detailed("Lithium", &version.id, &picked, &cache).await.unwrap();
    let plan = &result.plan;
    println!(
        "{} {} on Minecraft {} with {:?}: {} files, {} overrides, {} blocked, {:?} MiB, warnings {:?}",
        plan.title,
        plan.version_name,
        plan.minecraft,
        plan.loader,
        plan.files.len(),
        plan.overrides.len(),
        plan.blocked.len(),
        plan.recommended_memory_mib,
        result.warnings
    );
    assert_eq!(plan.title, "Lithium");
    assert!(matches!(plan.loader, Some((PackLoaderKind::Fabric, _))));
    assert!(!plan.files.is_empty());
    assert!(plan.files.iter().all(|f| f.urls[0].starts_with("https://") && f.sha1.is_none()));
    assert!(plan.files.iter().all(|f| result.md5.contains_key(&f.path)));
    assert_eq!(plan.overrides.len(), 1);
    assert!(plan.overrides[0].archive.is_file());
    let _ = std::fs::remove_dir_all(&cache);
}

#[tokio::test]
#[ignore = "needs network"]
async fn plans_a_forge_pack_with_optional_mods() {
    let atl = AtLauncherClient::new(Http::new().unwrap());
    let versions = atl.versions("Economica").await.unwrap();
    let version = versions.iter().find(|v| v.id == "Dev-2021_W39_A").expect("version");
    let manifest = atl.manifest("Economica", &version.id).await.unwrap();
    let optional = cubby_atlauncher::optional_mods(&manifest);
    println!("{} optional mods: {:?}", optional.len(), optional.iter().map(|m| &m.name).collect::<Vec<_>>());
    assert!(!optional.is_empty());
    let mut picker = OptionalSelection::new(&manifest);
    picker.select_recommended();

    let cache = cache("economica");
    let plan = atl.plan("Economica", &version.id, &picker.names(), &cache).await.unwrap();
    println!("{} files, loader {:?}", plan.files.len(), plan.loader);
    assert!(matches!(plan.loader, Some((PackLoaderKind::Forge, _))));
    assert!(plan.files.len() >= manifest.mods.iter().filter(|m| m.client && !m.optional).count());
    let _ = std::fs::remove_dir_all(&cache);
}

/// Reads the newest version of every public pack: nothing may fail to parse, and the packs that can't be
/// installed are counted by reason.
#[tokio::test]
#[ignore = "needs network"]
async fn every_pack_parses() {
    let atl = AtLauncherClient::new(Http::new().unwrap());
    let packs = atl.packs().await.unwrap();
    let mut unsupported: std::collections::BTreeMap<String, usize> = Default::default();
    let (mut ok, mut failed) = (0, Vec::new());
    for pack in &packs {
        let versions = atl.versions(&pack.id).await.unwrap();
        match atl.manifest(&pack.id, &versions[0].id).await {
            Ok(manifest) => {
                ok += 1;
                for reason in manifest.unsupported_reasons() {
                    // Group by the part after the mod name.
                    let key = reason.split_once("\" ").map(|(_, rest)| rest.to_string()).unwrap_or(reason);
                    *unsupported.entry(key).or_default() += 1;
                }
            }
            Err(error) => failed.push(format!("{} {}: {error}", pack.id, versions[0].id)),
        }
    }
    println!("{ok} manifests read, {} failed: {failed:?}", failed.len());
    println!("reasons a latest version can't be installed: {unsupported:#?}");
    assert!(failed.is_empty(), "{failed:?}");
}

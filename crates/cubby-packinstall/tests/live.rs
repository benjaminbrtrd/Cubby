//! End to end against the real services: plan a pack, install the plan into a folder, look at the result.
//! Needs network and a few MiB of downloads; run with `-- --ignored --nocapture`.
//! The output folder is `CUBBY_E2E_DIR` (default: the system temp folder); it is emptied first.

use std::path::PathBuf;

use cubby_common::events::{Event, EventSink};
use cubby_common::net::Http;
use cubby_common::packs::PackSort;
use cubby_packinstall::install::{extract_overrides, install_pack_plan, plan_from_mrpack};

struct Print;

impl EventSink for Print {
    fn emit(&self, event: Event) {
        if let Event::Progress { label, done, total, .. } = event {
            if done == 0 || done == total {
                println!("  [{label}] {done}/{total}");
            }
        }
    }
}

fn out_dir(name: &str) -> PathBuf {
    let base = std::env::var_os("CUBBY_E2E_DIR").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("cubby-e2e"));
    let dir = base.join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn count_files(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir).map(|r| r.flatten().map(|e| if e.path().is_dir() { count_files(&e.path()) } else { 1 }).sum()).unwrap_or(0)
}

/// The smallest of the latest versions of the first FTB packs, cut down to a few files.
#[tokio::test]
#[ignore = "needs network"]
async fn ftb_plan_installs() {
    let http = Http::new().unwrap();
    let ftb = cubby_ftb::FtbClient::new(http.clone());
    let dir = out_dir("ftb");
    let cache = dir.join("cache");
    std::fs::create_dir_all(&cache).unwrap();

    let packs = ftb.search("", PackSort::Name, 0, 40).await.unwrap();
    let mut best: Option<cubby_common::packs::PackPlan> = None;
    for pack in packs.hits.iter().take(40) {
        let Ok(versions) = ftb.versions(&pack.id).await else { continue };
        let Some(latest) = versions.first() else { continue };
        let Ok(plan) = ftb.plan(&pack.id, &latest.id, &cache).await else { continue };
        if !plan.files.is_empty() && best.as_ref().is_none_or(|b| plan.files.len() < b.files.len()) {
            best = Some(plan);
        }
    }
    let mut plan = best.expect("no FTB pack could be planned");
    println!("FTB pack {} {}: {} files, {} blocked, {} overrides archives", plan.title, plan.version_name, plan.files.len(), plan.blocked.len(), plan.overrides.len());
    plan.files.truncate(12);

    let game = dir.join("minecraft");
    let out = install_pack_plan(&http, &plan, &game, 8, true, &Print, "modpack:e2e").await.unwrap();
    println!("installed: {out:?}");
    assert_eq!(out.files, plan.files.len());
    for file in &plan.files {
        assert!(game.join(&file.path).is_file(), "{} missing", file.path.display());
    }
    assert_eq!(out.blocked.len(), plan.blocked.len());
    for o in &plan.overrides {
        assert!(!o.archive.exists(), "cache archive not removed: {}", o.archive.display());
    }
    println!("game dir now holds {} files", count_files(&game));
}

/// A Modrinth `.mrpack` used as a file the player imported: planned from the file, installed, file kept.
#[tokio::test]
#[ignore = "needs network"]
async fn mrpack_file_installs_and_is_kept() {
    let http = Http::new().unwrap();
    let modrinth = cubby_modrinth::Modrinth::new(http.clone());
    let dir = out_dir("mrpack");

    let hits = modrinth
        .search(&cubby_modrinth::SearchQuery {
            query: "fabulously optimized".into(),
            kind: cubby_modrinth::ProjectKind::Modpack,
            game_version: None,
            loader: None,
            categories: vec![],
            environment: None,
            sort: cubby_modrinth::Sort::Relevance,
            offset: 0,
            limit: 1,
        })
        .await
        .unwrap();
    let project = &hits.hits[0];
    let version = &modrinth.versions(&project.id, None, None).await.unwrap()[0];
    let pack = modrinth.fetch_modpack(&version.id, &dir).await.unwrap();
    println!("mrpack {} {} at {}", pack.title, pack.version_number, pack.archive.display());

    let mut plan = plan_from_mrpack(&pack.archive).unwrap();
    println!("plan: mc {} loader {:?}, {} files", plan.minecraft, plan.loader, plan.files.len());
    assert_eq!(plan.minecraft, pack.minecraft);
    plan.files.truncate(6);
    let game = dir.join("minecraft");
    let out = install_pack_plan(&http, &plan, &game, 8, false, &Print, "modpack:e2e").await.unwrap();
    println!("installed: files {} overrides {}", out.files, out.overrides);
    assert!(pack.archive.is_file(), "the player's file must stay");
    for file in &plan.files {
        assert!(game.join(&file.path).is_file());
    }
    // Same extraction as the Modrinth crate's own: same files in the same places.
    let reference = dir.join("reference");
    cubby_modrinth::mrpack::extract_overrides(&pack.archive, &reference, |_, _| {}).unwrap();
    let mut mine = 0;
    for prefix in ["overrides/", "client-overrides/"] {
        let probe = dir.join(format!("probe-{}", prefix.trim_end_matches('/')));
        mine += extract_overrides(&pack.archive, prefix, &probe, |_, _| {}).unwrap();
    }
    println!("overrides files: mine {mine}, reference {}", count_files(&reference));
    assert_eq!(count_files(&reference) as u64, mine);
}

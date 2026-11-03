//! Tests against the real Modrinth API (read-only, no file download). Run with `-- --ignored`.

use std::path::PathBuf;

use cubby_common::net::Http;
use cubby_modrinth::{Environment, Folder, Modrinth, ProjectKind, Reason, SearchQuery, Sort, Target};

fn client() -> Modrinth {
    Modrinth::new(Http::new().unwrap())
}

fn empty_game_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/modrinth/live").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
#[ignore = "needs network"]
async fn search_mods_and_shaders() {
    let m = client();
    let query = SearchQuery {
        query: "sodium".into(),
        kind: ProjectKind::Mod,
        game_version: Some("1.21.1".into()),
        loader: Some("quilt".into()),
        categories: vec!["optimization".into()],
        environment: Some(Environment::Client),
        sort: Sort::Downloads,
        offset: 0,
        limit: 5,
    };
    let results = m.search(&query).await.unwrap();
    assert!(results.total > 0 && !results.hits.is_empty() && results.hits.len() <= 5);
    let sodium = results.hits.iter().find(|h| h.slug == "sodium").expect("sodium in the results");
    assert_eq!(sodium.kind, ProjectKind::Mod);
    assert!(sodium.categories.contains(&"optimization".to_string()) && !sodium.categories.contains(&"fabric".to_string()));
    assert!(sodium.loaders.contains(&"fabric".to_string()));

    let shaders = m.search(&SearchQuery { query: String::new(), kind: ProjectKind::Shader, loader: Some("fabric".into()), categories: vec![], environment: None, ..query }).await.unwrap();
    assert!(shaders.hits.iter().all(|h| h.kind == ProjectKind::Shader), "{:?}", shaders.hits.iter().map(|h| &h.slug).collect::<Vec<_>>());
}

#[tokio::test]
#[ignore = "needs network"]
async fn project_and_versions() {
    let m = client();
    let project = m.project("sodium").await.unwrap();
    assert_eq!(project.hit.id, "AANobbMI");
    assert!(!project.hit.author.is_empty());
    assert!(!project.body.is_empty() && project.version_count > 10);

    let versions = m.versions("fabric-api", Some("1.21.1"), Some("quilt")).await.unwrap();
    assert!(!versions.is_empty());
    assert!(versions.windows(2).all(|w| w[0].published >= w[1].published), "newest first");
    assert!(versions.iter().all(|v| v.game_versions.contains(&"1.21.1".to_string())));
    assert!(versions.iter().all(|v| !v.files.is_empty() && v.files.iter().all(|f| f.sha1.len() == 40)));

    // Dependencies get their project title in one lookup.
    let sodium_versions = m.versions("sodium", Some("1.21.1"), Some("fabric")).await.unwrap();
    let with_deps = sodium_versions.iter().flat_map(|v| &v.dependencies).find(|d| d.project_id.is_some());
    if let Some(dep) = with_deps {
        assert!(dep.title.is_some(), "{dep:?}");
    }
    assert!(m.project("this-project-does-not-exist-zz").await.is_err());
}

#[tokio::test]
#[ignore = "needs network"]
async fn plan_for_an_empty_instance() {
    let m = client();
    // Lithium (Fabric) for 1.21.1 has no required dependency, Iris requires Sodium.
    let iris = m.versions("iris", Some("1.21.1"), Some("fabric")).await.unwrap();
    let release = iris.iter().find(|v| v.channel == "release").unwrap_or(&iris[0]);
    let target = Target::new("1.21.1", Some("fabric"));
    let plan = m.plan_install(&release.id, &target, &empty_game_dir("plan")).await.unwrap();
    assert_eq!(plan.items[0].reason, Reason::Requested);
    assert_eq!(plan.items[0].folder, Folder::Mods);
    assert!(plan.items.iter().any(|i| i.reason == Reason::Dependency && i.title.to_lowercase().contains("sodium")), "{plan:#?}");
    assert!(plan.download_bytes > 0);
    assert!(plan.items.iter().all(|i| i.download && !i.installed));
}

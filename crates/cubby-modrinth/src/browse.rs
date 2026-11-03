//! Search, project pages and version lists.

use std::collections::{HashMap, HashSet};

use cubby_common::Result;

use crate::api;
use crate::client::Modrinth;
use crate::model::*;

/// Every loader tag Modrinth mixes into search `categories`; anything else is a real category.
const LOADER_TAGS: &[&str] = &[
    "fabric", "quilt", "forge", "neoforge", "liteloader", "modloader", "rift", "babric", "bta-babric", "legacy-fabric", "nilloader",
    "ornithe", "java-agent", "minecraft", "datapack", "iris", "optifine", "canvas", "vanilla", "bukkit", "spigot", "paper", "purpur",
    "folia", "sponge", "bungeecord", "waterfall", "velocity", "geyser",
];

/// Loaders of resource packs and shader packs: their versions are kept whatever the instance loader.
const PACK_LOADERS: &[&str] = &["minecraft", "iris", "optifine", "canvas", "vanilla"];

/// Modrinth search facets for a query: an AND of OR-groups.
pub fn facets(q: &SearchQuery) -> Vec<Vec<String>> {
    let mut out = vec![vec![format!("project_type:{}", q.kind.as_str())]];
    if let Some(version) = q.game_version.as_deref().filter(|v| !v.is_empty()) {
        out.push(vec![format!("versions:{version}")]);
    }
    if matches!(q.kind, ProjectKind::Mod | ProjectKind::Modpack) {
        // A Quilt instance runs Fabric mods too; a modpack must be for the loader itself.
        let loaders = match q.kind {
            ProjectKind::Mod => mod_loaders(q.loader.as_deref()),
            _ => mod_loaders(q.loader.as_deref()).into_iter().take(1).collect(),
        };
        if !loaders.is_empty() {
            out.push(loaders.iter().map(|l| format!("categories:{l}")).collect());
        }
    }
    for category in q.categories.iter().map(|c| c.trim()).filter(|c| !c.is_empty()) {
        out.push(vec![format!("categories:{category}")]);
    }
    match q.environment {
        Some(Environment::Client) => out.push(vec!["client_side:required".into(), "client_side:optional".into()]),
        Some(Environment::Server) => out.push(vec!["server_side:required".into(), "server_side:optional".into()]),
        None => {}
    }
    out
}

/// Query string of `GET /search`.
pub fn search_params(q: &SearchQuery) -> Vec<(&'static str, String)> {
    let facets = serde_json::to_string(&facets(q)).unwrap_or_else(|_| "[]".into());
    let mut params = vec![
        ("facets", facets),
        ("index", q.sort.as_str().to_string()),
        ("offset", q.offset.to_string()),
        ("limit", q.limit.clamp(1, 100).to_string()),
    ];
    let text = q.query.trim();
    if !text.is_empty() {
        params.insert(0, ("query", text.to_string()));
    }
    params
}

fn split_categories(all: &[String]) -> (Vec<String>, Vec<String>) {
    all.iter().cloned().partition(|c| !LOADER_TAGS.contains(&c.as_str()))
}

pub(crate) fn hit_from_search(h: api::SearchHit) -> ProjectHit {
    let (categories, loaders) = split_categories(&h.categories);
    ProjectHit {
        slug: if h.slug.is_empty() { h.project_id.clone() } else { h.slug },
        id: h.project_id,
        kind: ProjectKind::from_api(&h.project_type),
        title: h.title,
        author: h.author,
        summary: h.description,
        icon_url: h.icon_url.filter(|u| !u.is_empty()),
        categories,
        downloads: h.downloads,
        follows: h.follows,
        updated: h.date_modified,
        published: h.date_created,
        game_versions: h.versions,
        loaders,
        client_side: Side::from_api(h.client_side.as_deref()),
        server_side: Side::from_api(h.server_side.as_deref()),
        license: h.license.filter(|l| !l.is_empty()),
        featured_image: h.featured_gallery.filter(|u| !u.is_empty()).or_else(|| h.gallery.into_iter().next()),
    }
}

fn author_of(members: &[api::TeamMember]) -> String {
    let owner = members.iter().find(|m| m.is_owner == Some(true) || m.role.eq_ignore_ascii_case("owner"));
    let first = || members.iter().min_by_key(|m| m.ordering);
    owner.or_else(first).map(|m| m.user.username.clone()).unwrap_or_default()
}

pub(crate) fn project_from_api(p: api::Project, author: String) -> Project {
    let mut categories = p.categories.clone();
    categories.extend(p.additional_categories.iter().filter(|c| !p.categories.contains(c)).cloned());
    let mut gallery = p.gallery.clone();
    gallery.sort_by_key(|g| (!g.featured, g.ordering));
    let featured_image = gallery.first().map(|g| g.url.clone());
    let link = |u: &Option<String>| u.clone().filter(|u| !u.trim().is_empty());
    Project {
        hit: ProjectHit {
            slug: if p.slug.is_empty() { p.id.clone() } else { p.slug.clone() },
            id: p.id.clone(),
            kind: ProjectKind::from_api(&p.project_type),
            title: p.title.clone(),
            author,
            summary: p.description.clone(),
            icon_url: p.icon_url.clone().filter(|u| !u.is_empty()),
            categories,
            downloads: p.downloads,
            follows: p.followers,
            updated: p.updated.clone(),
            published: p.published.clone(),
            game_versions: p.game_versions.clone(),
            loaders: p.loaders.clone(),
            client_side: Side::from_api(p.client_side.as_deref()),
            server_side: Side::from_api(p.server_side.as_deref()),
            license: p.license.as_ref().map(|l| l.id.clone()).filter(|l| !l.is_empty()),
            featured_image,
        },
        gallery: gallery
            .into_iter()
            .map(|g| GalleryImage { url: g.url, title: g.title, description: g.description, featured: g.featured })
            .collect(),
        links: Links {
            source: link(&p.source_url),
            issues: link(&p.issues_url),
            wiki: link(&p.wiki_url),
            discord: link(&p.discord_url),
            donation: p
                .donation_urls
                .iter()
                .filter(|d| !d.url.is_empty())
                .map(|d| Donation { platform: if d.platform.is_empty() { d.id.clone() } else { d.platform.clone() }, url: d.url.clone() })
                .collect(),
        },
        version_count: p.versions.len(),
        body: p.body,
    }
}

/// Whether a version runs with these mod loaders (empty = no filter). Resource pack and shader versions
/// always pass: their "loaders" are not mod loaders.
pub(crate) fn matches_loaders(version: &api::Version, allowed: &[String]) -> bool {
    allowed.is_empty()
        || version.loaders.iter().any(|l| allowed.contains(l))
        || (!version.loaders.is_empty() && version.loaders.iter().all(|l| PACK_LOADERS.contains(&l.as_str())))
}

pub(crate) fn version_from_api(v: api::Version, projects: &HashMap<String, api::Project>, version_projects: &HashMap<String, String>) -> ProjectVersion {
    let dependencies = v
        .dependencies
        .iter()
        .map(|d| {
            let project_id = d.project_id.clone().or_else(|| d.version_id.as_ref().and_then(|id| version_projects.get(id).cloned()));
            let project = project_id.as_ref().and_then(|id| projects.get(id));
            DependencyInfo {
                kind: if d.dependency_type.is_empty() { "required".into() } else { d.dependency_type.clone() },
                title: project.map(|p| p.title.clone()).or_else(|| d.file_name.clone()),
                slug: project.map(|p| p.slug.clone()).filter(|s| !s.is_empty()),
                icon_url: project.and_then(|p| p.icon_url.clone()).filter(|u| !u.is_empty()),
                project_id,
                version_id: d.version_id.clone(),
            }
        })
        .collect();
    ProjectVersion {
        channel: match v.version_type.as_str() {
            "beta" => "beta".into(),
            "alpha" => "alpha".into(),
            _ => "release".into(),
        },
        files: v
            .files
            .iter()
            .map(|f| FileInfo { url: f.url.clone(), file_name: f.filename.clone(), size: f.size, primary: f.primary, sha1: f.sha1().unwrap_or_default().to_lowercase() })
            .collect(),
        dependencies,
        id: v.id,
        project_id: v.project_id,
        name: v.name,
        version_number: v.version_number,
        game_versions: v.game_versions,
        loaders: v.loaders,
        published: v.date_published,
        downloads: v.downloads,
        changelog: v.changelog,
    }
}

impl Modrinth {
    pub async fn search(&self, query: &SearchQuery) -> Result<SearchResults> {
        let response = self.search_raw(&search_params(query)).await?;
        Ok(SearchResults {
            hits: response.hits.into_iter().map(hit_from_search).collect(),
            total: response.total_hits,
            offset: response.offset,
            limit: response.limit,
        })
    }

    /// A project page; the author is the team owner, else its first member.
    pub async fn project(&self, id_or_slug: &str) -> Result<Project> {
        let (project, members) = futures::join!(self.project_raw(id_or_slug), self.members_raw(id_or_slug));
        let project = project?;
        let members = members.unwrap_or_else(|e| {
            tracing::warn!("could not read the members of {id_or_slug}: {e}");
            Vec::new()
        });
        Ok(project_from_api(project, author_of(&members)))
    }

    /// Versions of a project, newest first, optionally only those for a game version and a launcher loader
    /// key (a Quilt loader also lists Fabric versions; "vanilla" or `None` lists every loader). Dependencies
    /// get their project title, slug and icon from one batched lookup.
    pub async fn versions(&self, id_or_slug: &str, game_version: Option<&str>, loader: Option<&str>) -> Result<Vec<ProjectVersion>> {
        let allowed = mod_loaders(loader);
        let mut server_loaders = allowed.clone();
        if !server_loaders.is_empty() {
            server_loaders.extend(PACK_LOADERS.iter().map(|l| l.to_string()));
        }
        let game_versions: Vec<String> = game_version.filter(|v| !v.is_empty()).map(str::to_owned).into_iter().collect();
        let mut versions = self.project_versions_raw(id_or_slug, &server_loaders, &game_versions).await?;
        versions.retain(|v| matches_loaders(v, &allowed) && (game_versions.is_empty() || v.game_versions.iter().any(|g| game_versions.contains(g))));
        versions.sort_by_key(|v| std::cmp::Reverse(v.published_millis()));

        let version_only: Vec<String> = versions
            .iter()
            .flat_map(|v| &v.dependencies)
            .filter(|d| d.project_id.is_none())
            .filter_map(|d| d.version_id.clone())
            .collect();
        let version_projects: HashMap<String, String> = if version_only.is_empty() {
            HashMap::new()
        } else {
            self.versions_raw(&version_only).await?.into_iter().map(|v| (v.id, v.project_id)).collect()
        };
        let mut project_ids: Vec<String> = versions.iter().flat_map(|v| &v.dependencies).filter_map(|d| d.project_id.clone()).collect();
        project_ids.extend(version_projects.values().cloned());
        let unique: HashSet<String> = project_ids.into_iter().collect();
        let projects: HashMap<String, api::Project> = if unique.is_empty() {
            HashMap::new()
        } else {
            self.projects_raw(&unique.into_iter().collect::<Vec<_>>()).await?.into_iter().map(|p| (p.id.clone(), p)).collect()
        };
        Ok(versions.into_iter().map(|v| version_from_api(v, &projects, &version_projects)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(kind: ProjectKind) -> SearchQuery {
        SearchQuery {
            query: String::new(),
            kind,
            game_version: None,
            loader: None,
            categories: Vec::new(),
            environment: None,
            sort: Sort::Relevance,
            offset: 0,
            limit: 20,
        }
    }

    #[test]
    fn facets_for_each_kind() {
        assert_eq!(facets(&query(ProjectKind::Mod)), [["project_type:mod"]]);

        let mut q = query(ProjectKind::Mod);
        q.game_version = Some("1.21.1".into());
        q.loader = Some("fabric".into());
        q.categories = vec!["optimization".into(), " ".into(), "utility".into()];
        q.environment = Some(Environment::Client);
        assert_eq!(
            facets(&q),
            vec![
                vec!["project_type:mod"],
                vec!["versions:1.21.1"],
                vec!["categories:fabric"],
                vec!["categories:optimization"],
                vec!["categories:utility"],
                vec!["client_side:required", "client_side:optional"],
            ]
        );

        q.loader = Some("quilt".into());
        q.environment = Some(Environment::Server);
        let f = facets(&q);
        assert_eq!(f[2], ["categories:quilt", "categories:fabric"]);
        assert_eq!(f[5], ["server_side:required", "server_side:optional"]);

        let mut pack = query(ProjectKind::Modpack);
        pack.loader = Some("quilt".into());
        assert_eq!(facets(&pack)[1], ["categories:quilt"]);

        // No loader facet for vanilla, shaders and resource packs.
        let mut vanilla = query(ProjectKind::Mod);
        vanilla.loader = Some("vanilla".into());
        assert_eq!(facets(&vanilla).len(), 1);
        for kind in [ProjectKind::Shader, ProjectKind::ResourcePack] {
            let mut q = query(kind);
            q.loader = Some("fabric".into());
            q.game_version = Some("1.20.1".into());
            assert_eq!(facets(&q), vec![vec![if kind == ProjectKind::Shader { "project_type:shader" } else { "project_type:resourcepack" }], vec!["versions:1.20.1"]]);
        }
    }

    #[test]
    fn search_parameters() {
        let mut q = query(ProjectKind::Shader);
        q.query = "  complementary ".into();
        q.sort = Sort::parse("downloads");
        q.offset = 40;
        q.limit = 500;
        let params = search_params(&q);
        let get = |k: &str| params.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("query"), Some("complementary"));
        assert_eq!(get("facets"), Some(r#"[["project_type:shader"]]"#));
        assert_eq!(get("index"), Some("downloads"));
        assert_eq!((get("offset"), get("limit")), (Some("40"), Some("100")));
        q.query = String::new();
        q.limit = 0;
        let params = search_params(&q);
        assert!(!params.iter().any(|(n, _)| *n == "query"));
        assert_eq!(params.iter().find(|(n, _)| *n == "limit").unwrap().1, "1");
        assert_eq!(Sort::parse("whatever"), Sort::Relevance);
    }

    #[test]
    fn search_hits_and_projects() {
        let response: api::SearchResponse = serde_json::from_str(
            r#"{"hits":[{"project_id":"AANobbMI","project_type":"mod","slug":"sodium","author":"jellysquid3","title":"Sodium",
            "description":"Fast","categories":["optimization","fabric","quilt"],"versions":["1.21.1"],"downloads":10,"follows":2,
            "icon_url":"https://cdn/i.png","date_created":"2020-01-01T00:00:00Z","date_modified":"2024-01-01T00:00:00Z",
            "license":"LGPL-3.0-only","client_side":"required","server_side":"unsupported","gallery":["https://cdn/g.png"],
            "featured_gallery":null,"color":123,"slug_extra":null},
            {"project_id":"x","project_type":"datapack","slug":null,"title":"T","description":null,"categories":null,"client_side":"weird"}],
            "offset":0,"limit":10,"total_hits":2}"#,
        )
        .unwrap();
        let hits: Vec<ProjectHit> = response.hits.into_iter().map(hit_from_search).collect();
        assert_eq!(hits[0].categories, ["optimization"]);
        assert_eq!(hits[0].loaders, ["fabric", "quilt"]);
        assert_eq!(hits[0].featured_image.as_deref(), Some("https://cdn/g.png"));
        assert_eq!((hits[0].client_side, hits[0].server_side), (Side::Required, Side::Unsupported));
        assert_eq!((hits[1].slug.as_str(), hits[1].kind, hits[1].client_side), ("x", ProjectKind::Mod, Side::Unknown));

        let project: api::Project = serde_json::from_str(
            r##"{"id":"P1","slug":"pack","project_type":"modpack","title":"Pack","description":"d","body":"# Hi",
            "categories":["adventure"],"additional_categories":["magic","adventure"],"source_url":"https://src","issues_url":"",
            "donation_urls":[{"id":"patreon","platform":"Patreon","url":"https://patreon"}],"followers":5,
            "license":{"id":"MIT","name":"MIT License","url":null},"versions":["a","b","c"],
            "gallery":[{"url":"https://g1","featured":false,"ordering":0},{"url":"https://g2","featured":true,"title":"Main","ordering":1}]}"##,
        )
        .unwrap();
        let members: Vec<api::TeamMember> = serde_json::from_str(
            r#"[{"user":{"id":"1","username":"helper"},"role":"Member","ordering":1},{"user":{"id":"2","username":"boss"},"role":"Owner","ordering":2}]"#,
        )
        .unwrap();
        let p = project_from_api(project, author_of(&members));
        assert_eq!(p.hit.author, "boss");
        assert_eq!(p.hit.kind, ProjectKind::Modpack);
        assert_eq!(p.hit.categories, ["adventure", "magic"]);
        assert_eq!(p.hit.license.as_deref(), Some("MIT"));
        assert_eq!(p.version_count, 3);
        assert_eq!(p.gallery[0].url, "https://g2");
        assert_eq!(p.hit.featured_image.as_deref(), Some("https://g2"));
        assert_eq!(p.links.source.as_deref(), Some("https://src"));
        assert_eq!(p.links.issues, None);
        assert_eq!(p.links.donation, [Donation { platform: "Patreon".into(), url: "https://patreon".into() }]);
        let nobody: Vec<api::TeamMember> = serde_json::from_str(r#"[{"user":{"username":"b"},"role":"Dev","ordering":3},{"user":{"username":"a"},"role":"Dev","ordering":0}]"#).unwrap();
        assert_eq!(author_of(&nobody), "a");
        assert_eq!(author_of(&[]), "");
    }

    #[test]
    fn loader_matching() {
        let v = |loaders: &[&str]| api::Version { loaders: loaders.iter().map(|s| s.to_string()).collect(), ..Default::default() };
        let quilt = mod_loaders(Some("quilt"));
        assert!(matches_loaders(&v(&["fabric"]), &quilt));
        assert!(matches_loaders(&v(&["quilt"]), &quilt));
        assert!(!matches_loaders(&v(&["forge"]), &quilt));
        assert!(matches_loaders(&v(&["minecraft"]), &quilt), "resource packs pass");
        assert!(matches_loaders(&v(&["iris", "optifine"]), &quilt), "shaders pass");
        assert!(!matches_loaders(&v(&["datapack"]), &quilt));
        assert!(!matches_loaders(&v(&["fabric"]), &mod_loaders(Some("forge"))));
        assert!(matches_loaders(&v(&["forge"]), &mod_loaders(Some("vanilla"))), "no filter");
    }
}

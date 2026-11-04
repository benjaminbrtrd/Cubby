//! Search, project pages and version lists, mapped to the `cubby_common::packs` types.

use cubby_common::packs::{PackChannel, PackLoaderKind, PackPage, PackSort, PackSummary, PackVersion};
use cubby_common::Result;

use crate::api;
use crate::client::{CurseForgeClient, GAME_ID, MODPACK_CLASS_ID, MOD_CLASS_ID, RESOURCE_PACK_CLASS_ID, SHADER_CLASS_ID};

/// CurseForge refuses `index + pageSize` above this.
const MAX_WINDOW: u32 = 10_000;
const MAX_PAGE_SIZE: u32 = 50;
/// Pages of `/files` read at most (50 files each).
const MAX_FILE_PAGES: u64 = 40;

/// Links of a project page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CfLinks {
    pub website: Option<String>,
    pub wiki: Option<String>,
    pub issues: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfScreenshot {
    pub title: Option<String>,
    pub url: String,
    pub thumbnail_url: Option<String>,
}

/// A modpack page: the summary plus the long description and media.
#[derive(Debug, Clone, PartialEq)]
pub struct CfProject {
    pub summary: PackSummary,
    pub slug: String,
    /// HTML, from `/v1/mods/{id}/description`. Sanitize before showing.
    pub description_html: String,
    pub links: CfLinks,
    pub screenshots: Vec<CfScreenshot>,
}

/// `sortField` of the search API: 1 Featured, 2 Popularity, 3 LastUpdated, 4 Name, 6 TotalDownloads,
/// 11 ReleasedDate; and the order (names read better ascending).
pub(crate) fn sort_params(sort: PackSort) -> (u8, &'static str) {
    match sort {
        PackSort::Relevance => (1, "desc"),
        PackSort::Downloads => (6, "desc"),
        PackSort::Updated => (3, "desc"),
        PackSort::Newest => (11, "desc"),
        PackSort::Name => (4, "asc"),
    }
}

/// What a resource search looks for. Modpacks have their own search ([`CurseForgeClient::search`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Mod,
    ResourcePack,
    Shader,
}

impl ResourceKind {
    fn class_id(self) -> u32 {
        match self {
            ResourceKind::Mod => MOD_CLASS_ID,
            ResourceKind::ResourcePack => RESOURCE_PACK_CLASS_ID,
            ResourceKind::Shader => SHADER_CLASS_ID,
        }
    }
}

/// A category players can filter a resource search by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCategory {
    pub id: u64,
    pub name: String,
}

/// A search for mods, resource packs or shader packs.
#[derive(Debug, Clone)]
pub struct ResourceQuery {
    pub kind: ResourceKind,
    pub text: String,
    /// Only files for this Minecraft version.
    pub game_version: Option<String>,
    /// Only mods for this loader (resource packs and shaders have none).
    pub loader: Option<PackLoaderKind>,
    /// One category id (CurseForge filters by one at a time).
    pub category: Option<u64>,
    pub sort: PackSort,
    pub offset: u32,
    pub limit: u32,
}

fn loader_code(loader: PackLoaderKind) -> u8 {
    match loader {
        PackLoaderKind::Forge => 1,
        PackLoaderKind::Fabric => 4,
        PackLoaderKind::Quilt => 5,
        PackLoaderKind::NeoForge => 6,
    }
}

pub(crate) fn resource_params(q: &ResourceQuery) -> Vec<(&'static str, String)> {
    let mut params = search_params(&q.text, q.sort, q.offset, q.limit);
    for (name, value) in &mut params {
        if *name == "classId" {
            *value = q.kind.class_id().to_string();
        }
    }
    if let Some(version) = q.game_version.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        params.push(("gameVersion", version.to_string()));
    }
    if let Some(id) = q.category {
        params.push(("categoryId", id.to_string()));
    }
    if let (ResourceKind::Mod, Some(loader)) = (q.kind, q.loader) {
        params.push(("modLoaderType", loader_code(loader).to_string()));
    }
    params
}

pub(crate) fn search_params(query: &str, sort: PackSort, offset: u32, limit: u32) -> Vec<(&'static str, String)> {
    let (field, order) = sort_params(sort);
    let page_size = limit.clamp(1, MAX_PAGE_SIZE);
    let index = offset.min(MAX_WINDOW.saturating_sub(page_size));
    let mut params = vec![
        ("gameId", GAME_ID.to_string()),
        ("classId", MODPACK_CLASS_ID.to_string()),
        ("sortField", field.to_string()),
        ("sortOrder", order.to_string()),
        ("index", index.to_string()),
        ("pageSize", page_size.to_string()),
    ];
    let text = query.trim();
    if !text.is_empty() {
        params.insert(2, ("searchFilter", text.to_string()));
    }
    params
}

fn loader_from_index(code: Option<u8>) -> Option<PackLoaderKind> {
    match code? {
        1 => Some(PackLoaderKind::Forge),
        4 => Some(PackLoaderKind::Fabric),
        5 => Some(PackLoaderKind::Quilt),
        6 => Some(PackLoaderKind::NeoForge),
        _ => None,
    }
}

fn loader_from_name(name: &str) -> Option<PackLoaderKind> {
    match name.to_ascii_lowercase().as_str() {
        "forge" => Some(PackLoaderKind::Forge),
        "fabric" => Some(PackLoaderKind::Fabric),
        "quilt" => Some(PackLoaderKind::Quilt),
        "neoforge" => Some(PackLoaderKind::NeoForge),
        _ => None,
    }
}

/// "1.20.1" and "1.21" are Minecraft releases; "Forge", "Java 17", "Client" and snapshots aren't.
fn is_release_version(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    (2..=3).contains(&parts.len()) && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

fn version_key(s: &str) -> Vec<u32> {
    s.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// Sorted newest first, without duplicates.
fn sorted_versions(versions: impl Iterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = versions.filter(|v| is_release_version(v)).collect();
    out.sort_by_key(|v| std::cmp::Reverse(version_key(v)));
    out.dedup();
    out
}

pub(crate) fn summary_from_mod(m: &api::Mod) -> PackSummary {
    let mut loaders: Vec<PackLoaderKind> = Vec::new();
    for l in m.latest_files_indexes.iter().filter_map(|i| loader_from_index(i.mod_loader)) {
        if !loaders.contains(&l) {
            loaders.push(l);
        }
    }
    PackSummary {
        id: m.id.to_string(),
        name: m.name.clone(),
        summary: m.summary.clone(),
        authors: m.authors.iter().map(|a| a.name.clone()).filter(|n| !n.is_empty()).collect(),
        icon_url: m.logo.as_ref().and_then(|l| l.thumbnail_url.clone().or_else(|| l.url.clone())).filter(|u| !u.is_empty()),
        website_url: m.links.website_url.clone().filter(|u| !u.is_empty()),
        downloads: Some(m.download_count.max(0.0) as u64),
        updated: m.date_modified.clone(),
        minecraft: sorted_versions(m.latest_files_indexes.iter().map(|i| i.game_version.clone())),
        loaders,
    }
}

pub(crate) fn project_from_mod(m: &api::Mod, description_html: String) -> CfProject {
    CfProject {
        summary: summary_from_mod(m),
        slug: m.slug.clone(),
        description_html,
        links: CfLinks {
            website: m.links.website_url.clone().filter(|u| !u.is_empty()),
            wiki: m.links.wiki_url.clone().filter(|u| !u.is_empty()),
            issues: m.links.issues_url.clone().filter(|u| !u.is_empty()),
            source: m.links.source_url.clone().filter(|u| !u.is_empty()),
        },
        screenshots: m
            .screenshots
            .iter()
            .filter_map(|s| Some(CfScreenshot { title: s.title.clone().filter(|t| !t.is_empty()), url: s.url.clone().filter(|u| !u.is_empty())?, thumbnail_url: s.thumbnail_url.clone() }))
            .collect(),
    }
}

pub(crate) fn version_from_file(f: &api::File) -> PackVersion {
    let channel = match f.release_type {
        2 => PackChannel::Beta,
        3 => PackChannel::Alpha,
        _ => PackChannel::Release,
    };
    let minecraft = sorted_versions(f.game_versions.iter().cloned()).into_iter().next();
    let loader = f.game_versions.iter().find_map(|v| loader_from_name(v));
    PackVersion {
        id: f.id.to_string(),
        name: if f.display_name.trim().is_empty() { f.file_name.clone() } else { f.display_name.trim().to_string() },
        channel,
        minecraft,
        loader,
        released: f.file_date.clone(),
    }
}

impl CurseForgeClient {
    /// Modpacks matching `query` (all of them when empty), `limit` (at most 50) from `offset`.
    pub async fn search(&self, query: &str, sort: PackSort, offset: u32, limit: u32) -> Result<PackPage> {
        let params = search_params(query, sort, offset, limit);
        let response: api::Envelope<Vec<api::Mod>> = self.get("/v1/mods/search", &params, "CurseForge search").await?;
        Ok(PackPage { hits: response.data.iter().map(summary_from_mod).collect(), total: response.pagination.map(|p| p.total_count) })
    }

    /// Mods, resource packs or shader packs matching the query.
    pub async fn search_resources(&self, query: &ResourceQuery) -> Result<PackPage> {
        let params = resource_params(query);
        let response: api::Envelope<Vec<api::Mod>> = self.get("/v1/mods/search", &params, "CurseForge search").await?;
        Ok(PackPage { hits: response.data.iter().map(summary_from_mod).collect(), total: response.pagination.map(|p| p.total_count) })
    }

    /// The categories of mods, resource packs or shader packs, by name.
    pub async fn resource_categories(&self, kind: ResourceKind) -> Result<Vec<ResourceCategory>> {
        let class = kind.class_id();
        let query = [("gameId", GAME_ID.to_string()), ("classId", class.to_string())];
        let response: api::Envelope<Vec<api::Category>> = self.get("/v1/categories", &query, "CurseForge categories").await?;
        let mut out: Vec<ResourceCategory> = response
            .data
            .into_iter()
            .filter(|c| c.is_class != Some(true) && c.class_id.is_none_or(|id| id == u64::from(class)) && !c.name.trim().is_empty())
            .map(|c| ResourceCategory { id: c.id, name: c.name.trim().to_string() })
            .collect();
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out.dedup_by_key(|c| c.id);
        Ok(out)
    }

    /// The popular modpacks, for the screen shown before the player types anything.
    pub async fn featured(&self) -> Result<PackPage> {
        self.search("", PackSort::Downloads, 0, 20).await
    }

    /// A modpack page. `id` is the CurseForge project id.
    pub async fn pack(&self, id: u64) -> Result<CfProject> {
        let project: api::Envelope<api::Mod> = self.get(&format!("/v1/mods/{id}"), &[], &format!("CurseForge modpack {id}")).await?;
        // The description is secondary: a failure there still shows the page.
        let description = match self.get::<api::Envelope<String>>(&format!("/v1/mods/{id}/description"), &[], &format!("description of CurseForge modpack {id}")).await {
            Ok(d) => d.data,
            Err(error) => {
                tracing::debug!("no description for CurseForge modpack {id}: {error}");
                String::new()
            }
        };
        Ok(project_from_mod(&project.data, description))
    }

    /// Every file of a modpack, newest first.
    pub async fn versions(&self, id: u64) -> Result<Vec<PackVersion>> {
        let files = self.files_raw(id).await?;
        Ok(files.iter().map(version_from_file).collect())
    }

    /// Raw files of a project, newest first (read page by page).
    pub(crate) async fn files_raw(&self, id: u64) -> Result<Vec<api::File>> {
        let mut out: Vec<api::File> = Vec::new();
        for page in 0..MAX_FILE_PAGES {
            let query = [("index", (page * MAX_PAGE_SIZE as u64).to_string()), ("pageSize", MAX_PAGE_SIZE.to_string())];
            let response: api::Envelope<Vec<api::File>> = self.get(&format!("/v1/mods/{id}/files"), &query, &format!("files of CurseForge modpack {id}")).await?;
            let got = response.data.len() as u64;
            out.extend(response.data);
            let total = response.pagination.map(|p| p.total_count).unwrap_or(0);
            if got == 0 || out.len() as u64 >= total {
                break;
            }
        }
        out.sort_by(|a, b| b.file_date.cmp(&a.file_date).then(b.id.cmp(&a.id)));
        Ok(out)
    }

    /// One file of a project, with its download URL.
    pub(crate) async fn file_raw(&self, mod_id: u64, file_id: u64) -> Result<api::File> {
        let response: api::Envelope<api::File> = self.get(&format!("/v1/mods/{mod_id}/files/{file_id}"), &[], &format!("CurseForge file {file_id}")).await?;
        Ok(response.data)
    }

    pub(crate) async fn mod_raw(&self, id: u64) -> Result<api::Mod> {
        let response: api::Envelope<api::Mod> = self.get(&format!("/v1/mods/{id}"), &[], &format!("CurseForge project {id}")).await?;
        Ok(response.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope<T: serde::de::DeserializeOwned>(name: &str) -> api::Envelope<T> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn sort_mapping() {
        assert_eq!(sort_params(PackSort::Relevance), (1, "desc"));
        assert_eq!(sort_params(PackSort::Downloads), (6, "desc"));
        assert_eq!(sort_params(PackSort::Updated), (3, "desc"));
        assert_eq!(sort_params(PackSort::Name), (4, "asc"));
    }

    #[test]
    fn resource_search_params() {
        let mut q = ResourceQuery { kind: ResourceKind::Mod, text: "sodium".into(), game_version: Some(" 1.21.1 ".into()), loader: Some(PackLoaderKind::Fabric), category: Some(409), sort: PackSort::Downloads, offset: 0, limit: 20 };
        let get = |p: &[(&str, String)], k: &str| p.iter().find(|(n, _)| *n == k).map(|(_, v)| v.clone());
        let p = resource_params(&q);
        assert_eq!(get(&p, "classId").as_deref(), Some("6"));
        assert_eq!(get(&p, "gameVersion").as_deref(), Some("1.21.1"));
        assert_eq!(get(&p, "modLoaderType").as_deref(), Some("4"));
        assert_eq!(get(&p, "categoryId").as_deref(), Some("409"));
        assert_eq!(get(&p, "searchFilter").as_deref(), Some("sodium"));
        // shaders and resource packs have no loader
        q.kind = ResourceKind::Shader;
        let p = resource_params(&q);
        assert_eq!(get(&p, "classId").as_deref(), Some("6552"));
        assert!(get(&p, "modLoaderType").is_none());
        q.kind = ResourceKind::ResourcePack;
        q.game_version = None;
        let p = resource_params(&q);
        assert_eq!(get(&p, "classId").as_deref(), Some("12"));
        assert!(get(&p, "gameVersion").is_none());
    }

    #[test]
    fn search_query_string() {
        let p = search_params("  all the mods ", PackSort::Downloads, 20, 500);
        let get = |k: &str| p.iter().find(|(n, _)| *n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("gameId"), Some("432"));
        assert_eq!(get("classId"), Some("4471"));
        assert_eq!(get("searchFilter"), Some("all the mods"));
        assert_eq!(get("pageSize"), Some("50"));
        assert_eq!(get("index"), Some("20"));
        assert_eq!(get("sortField"), Some("6"));
        assert!(get("searchFilter").is_some() && search_params("", PackSort::Relevance, 0, 10).iter().all(|(k, _)| *k != "searchFilter"));
        // the API rejects windows past 10 000
        let far = search_params("", PackSort::Relevance, 99_999, 50);
        assert_eq!(far.iter().find(|(k, _)| *k == "index").unwrap().1, "9950");
    }

    #[test]
    fn search_hits_map() {
        let response: api::Envelope<Vec<api::Mod>> = envelope("search.json");
        assert_eq!(response.pagination.as_ref().unwrap().total_count, 1234);
        let hit = summary_from_mod(&response.data[0]);
        assert_eq!(hit.id, "925200");
        assert_eq!(hit.name, "Example Forge Pack");
        assert_eq!(hit.authors, vec!["Alice".to_string()]);
        assert_eq!(hit.downloads, Some(5_000_000));
        assert_eq!(hit.minecraft, vec!["1.20.1".to_string(), "1.19.2".to_string()]);
        assert_eq!(hit.loaders, vec![PackLoaderKind::Forge, PackLoaderKind::NeoForge]);
        assert_eq!(hit.icon_url.as_deref(), Some("https://media.forgecdn.net/avatars/thumbnails/1/2/256/256/pack.png"));
        assert_eq!(hit.website_url.as_deref(), Some("https://www.curseforge.com/minecraft/modpacks/example-forge-pack"));
        assert_eq!(hit.updated.as_deref(), Some("2024-05-01T10:00:00.000Z"));
    }

    #[test]
    fn project_page_maps() {
        let response: api::Envelope<Vec<api::Mod>> = envelope("search.json");
        let project = project_from_mod(&response.data[0], "<p>Hello</p>".into());
        assert_eq!(project.slug, "example-forge-pack");
        assert_eq!(project.screenshots.len(), 1);
        assert_eq!(project.links.source.as_deref(), Some("https://github.com/alice/pack"));
        assert_eq!(project.links.wiki, None);
        assert_eq!(project.description_html, "<p>Hello</p>");
    }

    #[test]
    fn versions_map() {
        let response: api::Envelope<Vec<api::File>> = envelope("pack_files.json");
        let versions: Vec<PackVersion> = response.data.iter().map(version_from_file).collect();
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].channel, PackChannel::Release);
        assert_eq!(versions[0].minecraft.as_deref(), Some("1.20.1"));
        assert_eq!(versions[0].loader, Some(PackLoaderKind::Forge));
        assert_eq!(versions[0].name, "Example Forge Pack 1.2.0");
        assert_eq!(versions[1].channel, PackChannel::Beta);
        assert_eq!(versions[1].loader, Some(PackLoaderKind::NeoForge));
        assert_eq!(versions[2].channel, PackChannel::Alpha);
        assert_eq!(versions[2].loader, None);
        // no display name: falls back to the file name
        assert_eq!(versions[2].name, "Example-Pack-0.1.zip");
    }

    #[test]
    fn minecraft_versions_sorted_numerically() {
        let v = sorted_versions(["1.9.4", "1.20.1", "Forge", "1.20", "Java 17", "1.20.1", "1.21.4"].into_iter().map(String::from));
        assert_eq!(v, vec!["1.21.4", "1.20.1", "1.20", "1.9.4"]);
    }
}

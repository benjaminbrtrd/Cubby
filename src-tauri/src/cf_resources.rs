//! CurseForge mods, resource packs and shader packs for Discover, shaped like the Modrinth results so the
//! same cards show them. Modpacks have their own commands in `packs.rs`.

use cubby_common::packs::{PackLoaderKind, PackSort};
use cubby_curseforge::{ResourceKind, ResourceQuery};
use serde::Serialize;
use tauri::State;

use crate::dto::{ProjectHitDto, ProjectQueryDto, SearchResultsDto};
use crate::state::Core;

const KEY_MESSAGE: &str = "CurseForge needs an API key. Add yours in Settings.";

fn kind_of(kind: &str) -> Result<ResourceKind, String> {
    match kind {
        "mod" => Ok(ResourceKind::Mod),
        "shader" => Ok(ResourceKind::Shader),
        "resourcepack" => Ok(ResourceKind::ResourcePack),
        "modpack" => Err("CurseForge modpacks are browsed with the modpack commands.".into()),
        other => Err(format!("Unknown project type: {other}.")),
    }
}

fn loader_of(key: &str) -> Option<PackLoaderKind> {
    match key {
        "fabric" => Some(PackLoaderKind::Fabric),
        "quilt" => Some(PackLoaderKind::Quilt),
        "forge" => Some(PackLoaderKind::Forge),
        "neoforge" => Some(PackLoaderKind::NeoForge),
        _ => None,
    }
}

fn sort_of(sort: &str) -> PackSort {
    match sort {
        "downloads" | "follows" => PackSort::Downloads,
        "updated" => PackSort::Updated,
        "newest" => PackSort::Newest,
        _ => PackSort::Relevance,
    }
}

fn loader_name(kind: PackLoaderKind) -> &'static str {
    match kind {
        PackLoaderKind::Fabric => "fabric",
        PackLoaderKind::Quilt => "quilt",
        PackLoaderKind::Forge => "forge",
        PackLoaderKind::NeoForge => "neoforge",
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDto {
    pub id: String,
    pub name: String,
}

/// The categories of mods, shader packs or resource packs on CurseForge.
#[tauri::command]
pub async fn curseforge_categories(core: State<'_, Core>, kind: String) -> Result<Vec<CategoryDto>, String> {
    let client = core.curseforge().ok_or_else(|| KEY_MESSAGE.to_string())?;
    let categories = client.resource_categories(kind_of(&kind)?).await.map_err(crate::dto::message)?;
    Ok(categories.into_iter().map(|c| CategoryDto { id: c.id.to_string(), name: c.name }).collect())
}

/// Searches CurseForge with the same query the Modrinth search takes. Categories and the client/server
/// filter are Modrinth's: only the first category (a CurseForge category id) is used.
#[tauri::command]
pub async fn curseforge_search(core: State<'_, Core>, query: ProjectQueryDto) -> Result<SearchResultsDto, String> {
    let client = core.curseforge().ok_or_else(|| KEY_MESSAGE.to_string())?;
    let kind = kind_of(&query.kind)?;
    let wanted = ResourceQuery {
        kind,
        text: query.query.clone(),
        game_version: query.game_version.clone(),
        loader: query.loader.as_deref().and_then(loader_of),
        category: query.categories.first().and_then(|c| c.trim().parse().ok()),
        sort: sort_of(&query.sort),
        offset: query.offset,
        limit: query.limit,
    };
    let page = client.search_resources(&wanted).await.map_err(|e| crate::dto::message(e))?;
    let kind_name: &'static str = match kind {
        ResourceKind::Mod => "mod",
        ResourceKind::Shader => "shader",
        ResourceKind::ResourcePack => "resourcepack",
    };
    let hits: Vec<ProjectHitDto> = page
        .hits
        .into_iter()
        .map(|h| {
            let updated = h.updated.clone().unwrap_or_default();
            ProjectHitDto {
                id: h.id,
                slug: h.website_url.as_deref().and_then(|u| u.rsplit('/').next()).unwrap_or_default().to_string(),
                kind: kind_name,
                title: h.name,
                author: h.authors.first().cloned().unwrap_or_default(),
                summary: h.summary,
                icon_url: h.icon_url,
                categories: Vec::new(),
                downloads: h.downloads.unwrap_or(0),
                follows: 0,
                published: updated.clone(),
                updated,
                game_versions: h.minecraft,
                loaders: h.loaders.into_iter().map(|l| loader_name(l).to_string()).collect(),
                client_side: "unknown",
                server_side: "unknown",
                license: None,
                featured_image: None,
            }
        })
        .collect();
    let total = page.total.unwrap_or(hits.len() as u64);
    Ok(SearchResultsDto { hits, total, offset: query.offset, limit: query.limit })
}

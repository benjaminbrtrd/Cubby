//! Datapacks: they go into a world (`saves/<world>/datapacks`), not into the instance, so they have their
//! own search and download instead of the install plans of mods and packs.

use cubby_common::{Error, Result};

use crate::browse::hit_from_search;
use crate::client::Modrinth;
use crate::model::{FileInfo, SearchResults};

/// Search facets for datapacks made for `game_version` (any version when empty).
pub fn datapack_facets(game_version: &str) -> Vec<Vec<String>> {
    let mut facets = vec![vec!["project_type:datapack".to_string()]];
    let version = game_version.trim();
    if !version.is_empty() {
        facets.push(vec![format!("versions:{version}")]);
    }
    facets
}

impl Modrinth {
    /// Datapacks for a game version: by relevance with a query, else the most downloaded. 20 per page.
    pub async fn search_datapacks(&self, query: &str, game_version: &str, offset: u32) -> Result<SearchResults> {
        let facets = serde_json::to_string(&datapack_facets(game_version)).map_err(|e| Error::Other(e.to_string()))?;
        let text = query.trim();
        let mut params = vec![
            ("facets", facets),
            ("index", if text.is_empty() { "downloads" } else { "relevance" }.to_string()),
            ("offset", offset.to_string()),
            ("limit", "20".to_string()),
        ];
        if !text.is_empty() {
            params.insert(0, ("query", text.to_string()));
        }
        let response = self.search_raw(&params).await?;
        Ok(SearchResults {
            hits: response.hits.into_iter().map(hit_from_search).collect(),
            total: response.total_hits,
            offset: response.offset,
            limit: response.limit,
        })
    }

    /// The `.zip` of the newest datapack version of a project for `game_version`.
    pub async fn datapack_file(&self, id_or_slug: &str, game_version: &str) -> Result<FileInfo> {
        let versions = self.project_versions_raw(id_or_slug, &["datapack".to_string()], &[game_version.to_string()]).await?;
        let newest = versions
            .into_iter()
            .filter(|v| v.loaders.iter().any(|l| l == "datapack"))
            .max_by_key(|v| v.published_millis())
            .ok_or_else(|| Error::NotFound(format!("a datapack version of {id_or_slug} for Minecraft {game_version}")))?;
        let file = newest
            .files
            .iter()
            .find(|f| f.primary && f.filename.to_lowercase().ends_with(".zip"))
            .or_else(|| newest.files.iter().find(|f| f.filename.to_lowercase().ends_with(".zip")))
            .ok_or_else(|| Error::Other(format!("{} has no datapack .zip to download.", newest.name)))?;
        Ok(FileInfo {
            url: file.url.clone(),
            file_name: file.filename.clone(),
            size: file.size,
            primary: file.primary,
            sha1: file.hashes.get("sha1").cloned().unwrap_or_default().to_lowercase(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facets_for_a_version() {
        assert_eq!(datapack_facets("1.21.1"), vec![vec!["project_type:datapack".to_string()], vec!["versions:1.21.1".to_string()]]);
        assert_eq!(datapack_facets(" "), vec![vec!["project_type:datapack".to_string()]]);
    }
}

//! Datapacks: they go into a world (`saves/<world>/datapacks`), not into the instance, so they have their
//! own download instead of the install plans of mods and packs. Discover searches them like other projects.

use cubby_common::{Error, Result};

use std::collections::HashMap;
use std::path::PathBuf;

use crate::client::Modrinth;
use crate::model::FileInfo;

/// The Modrinth project and version a file on disk is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileProject {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub version_id: String,
    pub version_number: String,
}

impl Modrinth {
    /// The `.zip` of a datapack version: `version_id`, else the newest version of the project for `game_version`.
    pub async fn datapack_file(&self, id_or_slug: &str, version_id: Option<&str>, game_version: &str) -> Result<FileInfo> {
        let newest = match version_id {
            Some(id) => self.version_raw(id).await?,
            None => self
                .project_versions_raw(id_or_slug, &["datapack".to_string()], &[game_version.to_string()])
                .await?
                .into_iter()
                .filter(|v| v.loaders.iter().any(|l| l == "datapack"))
                .max_by_key(|v| v.published_millis())
                .ok_or_else(|| Error::NotFound(format!("a datapack version of {id_or_slug} for Minecraft {game_version}")))?,
        };
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

    /// The Modrinth project of each file, by SHA-1, in the order given; `None` for a file Modrinth doesn't know
    /// or that can't be read. For files outside the instance folders, like a world's datapacks.
    pub async fn identify_paths(&self, paths: &[PathBuf]) -> Result<Vec<Option<FileProject>>> {
        let mut hashes = Vec::with_capacity(paths.len());
        for path in paths {
            hashes.push(self.sha1_of(path).await.ok());
        }
        let known: Vec<String> = hashes.iter().flatten().cloned().collect();
        if known.is_empty() {
            return Ok(vec![None; paths.len()]);
        }
        let versions = self.version_files(&known).await?;
        let ids: Vec<String> = versions.values().map(|v| v.project_id.clone()).collect();
        let projects: HashMap<String, crate::api::Project> = self.projects_raw(&ids).await?.into_iter().map(|p| (p.id.clone(), p)).collect();
        Ok(hashes
            .iter()
            .map(|h| {
                let v = versions.get(h.as_ref()?)?;
                let p = projects.get(&v.project_id);
                Some(FileProject {
                    project_id: v.project_id.clone(),
                    slug: p.map(|p| p.slug.clone()).unwrap_or_default(),
                    title: p.map(|p| p.title.clone()).unwrap_or_else(|| v.name.clone()),
                    version_id: v.id.clone(),
                    version_number: v.version_number.clone(),
                })
            })
            .collect())
    }
}

//! Datapacks: they go into a world (`saves/<world>/datapacks`), not into the instance, so they have their
//! own download instead of the install plans of mods and packs. Discover searches them like other projects.

use cubby_common::{Error, Result};

use crate::client::Modrinth;
use crate::model::FileInfo;

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
}

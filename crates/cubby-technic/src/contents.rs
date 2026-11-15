//! What a build of a Technic pack contains. A Solder build lists its mods (with display names and pages when
//! the server answers `?include=mods`); a pack that is a single zip has no list short of downloading it.

use cubby_common::packs::{sort_contents, PackContentItem, PackContentKind};
use cubby_common::{Error, Result};

use crate::client::{check_segment, make_url, TechnicClient};
use crate::model::SolderBuild;

/// Solder entries that are the loader rather than a mod.
const LOADER_SLUGS: [&str; 8] = ["forge", "minecraftforge", "neoforge", "fabric", "fabric-loader", "fabricloader", "quilt", "quilt-loader"];

/// The mods of a Solder build, the loader left out.
pub fn build_contents(build: &SolderBuild) -> Vec<PackContentItem> {
    let mut out = Vec::new();
    for m in build.mods.iter().filter(|m| !m.name.trim().is_empty()) {
        if LOADER_SLUGS.contains(&m.name.trim().to_ascii_lowercase().as_str()) {
            continue;
        }
        let file = m.url.rsplit('/').next().unwrap_or_default();
        let mut item = PackContentItem::from_file(file, PackContentKind::Mod);
        item.name = m.pretty_name.as_deref().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(m.name.trim()).to_string();
        item.version = Some(m.version.trim().to_string()).filter(|v| !v.is_empty());
        item.page_url = m.link.clone().filter(|l| l.starts_with("https://") || l.starts_with("http://"));
        item.description = m.description.as_deref().map(str::trim).filter(|d| !d.is_empty()).map(str::to_string);
        item.authors = m.author.as_deref().map(str::trim).filter(|a| !a.is_empty()).map(str::to_string).into_iter().collect();
        out.push(item);
    }
    sort_contents(&mut out);
    out
}

impl TechnicClient {
    /// The mods of build `version_id` of a pack; `None` for a pack that is a single zip, which doesn't list them.
    pub async fn contents(&self, slug: &str, version_id: &str) -> Result<Option<Vec<PackContentItem>>> {
        let pack = self.pack(slug).await?;
        if !pack.is_solder() {
            return Ok(None);
        }
        let version_id = version_id.trim();
        if version_id.is_empty() {
            return Err(Error::Other(format!("Choose a version of {}.", pack.display_name)));
        }
        let root = pack.solder_url.as_deref().unwrap_or_default();
        // Display names come with `include=mods`; a server that doesn't take it still answers the plain build.
        let url = make_url(root, &["modpack", check_segment(&pack.name)?, check_segment(version_id)?], &[("include", "mods")])?;
        let detailed = self.get::<SolderBuild>(&url, &format!("build {version_id} of {}", pack.display_name)).await;
        let build = match detailed {
            Ok(build) => build,
            Err(e) => {
                tracing::debug!("Solder build {version_id} of {slug} with include=mods: {e}");
                self.solder_build(&pack, version_id).await?
            }
        };
        Ok(Some(build_contents(&build)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn build(name: &str) -> SolderBuild {
        let bytes = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn lists_the_mods_of_a_build() {
        let items = build_contents(&build("solder_build_1122.json"));
        let names: Vec<(&str, Option<&str>)> = items.iter().map(|i| (i.name.as_str(), i.version.as_deref())).collect();
        assert_eq!(names, [("additionalbanners", Some("1.12.2-1.1.77")), ("additionalpipes", Some("1.12.2-6.0.0.9-technic"))]);
        assert_eq!(items[0].file_name.as_deref(), Some("additionalbanners-1.12.2-1.1.77.zip"));
        assert!(items.iter().all(|i| i.site.is_none() && !i.optional));
    }

    #[test]
    fn uses_display_names_and_leaves_out_the_loader() {
        let items = build_contents(&build("solder_build_mods.json"));
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["Applied Energistics 2", "jei"]);
        assert_eq!(items[0].page_url.as_deref(), Some("https://appliedenergistics.org"));
        assert_eq!(items[0].authors, ["AlgorithmX2"]);
        assert!(items[0].description.as_deref().unwrap().starts_with("A mod about matter"));
        assert_eq!(items[1].page_url, None, "not a web address");
    }
}

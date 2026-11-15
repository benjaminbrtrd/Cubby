//! What a version of an FTB pack contains: its file list, which names CurseForge projects for most mods.

use cubby_common::packs::{sort_contents, ContentSite, PackContentItem, PackContentKind};
use cubby_common::Result;

use crate::api;
use crate::client::{parse_id, FtbClient};
use crate::plan::curseforge_page_url;

/// The mods, resource packs and shader packs of a version, by file name (server-only files left out). A file
/// that comes from CurseForge carries its project id, for the app to name it.
pub fn version_contents(version: &api::VersionDetail) -> Vec<PackContentItem> {
    let mut out = Vec::new();
    for file in version.files.iter().filter(|f| !f.serveronly && !f.name.is_empty()) {
        let path = format!("{}/{}", file.path.trim_end_matches(['/', '\\']), file.name);
        let kind = match PackContentKind::of_path(&path) {
            Some(kind) => kind,
            None if file.kind.eq_ignore_ascii_case("mod") => PackContentKind::Mod,
            None => continue,
        };
        let mut item = PackContentItem::from_file(&file.name, kind);
        item.optional = file.optional;
        if let Some(cf) = &file.curseforge {
            let project = cf.project.clone().into_string();
            if !project.is_empty() && project != "0" {
                item.page_url = Some(curseforge_page_url(&project));
                item.site = Some(ContentSite::Curseforge);
                item.project_id = Some(project);
            }
        }
        out.push(item);
    }
    sort_contents(&mut out);
    out
}

impl FtbClient {
    /// The mods, resource packs and shader packs of version `version_id` of pack `pack_id`.
    pub async fn contents(&self, pack_id: &str, version_id: &str) -> Result<Vec<PackContentItem>> {
        let version = self.version_raw(parse_id(pack_id, "modpack")?, parse_id(version_id, "version")?).await?;
        Ok(version_contents(&version))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn version(name: &str) -> api::VersionDetail {
        let bytes = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn lists_mods_not_configs() {
        let items = version_contents(&version("version_100_12632.json"));
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["AdvancedPeripherals-1.18.2-0.7.31r", "Applied-Botanics-1.0.3", "Applied-Mekanistics-1.2.2"]);
        assert_eq!(items[0].file_name.as_deref(), Some("AdvancedPeripherals-1.18.2-0.7.31r.jar"));
        assert_eq!(items[0].site, Some(ContentSite::Curseforge));
        assert_eq!(items[0].project_id.as_deref(), Some("431725"));
        assert_eq!(items[0].page_url.as_deref(), Some("https://www.curseforge.com/projects/431725"));
        assert!(items.iter().all(|i| i.kind == PackContentKind::Mod && !i.optional));
    }

    #[test]
    fn leaves_out_server_only_files() {
        let items = version_contents(&version("version_114_serveronly.json"));
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["extremesoundmuffler-3.39-forge-1.19.2", "ftbpromoter-19.2.52"]);
    }
}

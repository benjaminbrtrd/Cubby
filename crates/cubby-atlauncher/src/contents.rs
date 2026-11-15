//! What a version of an ATLauncher pack contains, from its manifest (`Configs.json`): the mods with their
//! names and versions, which optional, and the CurseForge project of those that come from there.

use cubby_common::packs::{sort_contents, ContentSite, PackContentItem, PackContentKind};

use crate::model::{AtlManifest, AtlMod};
use crate::modtypes::ModKind;

/// What a mod type installs, as listed content; `None` for configs, loaders and server files.
fn content_kind(m: &AtlMod) -> Option<PackContentKind> {
    let kind = ModKind::parse(&m.kind);
    let kind = match kind {
        ModKind::Extract => ModKind::parse(m.extract_to.as_deref().unwrap_or_default()),
        ModKind::Decomp => ModKind::parse(m.decomp_type.as_deref().unwrap_or_default()),
        other => other,
    };
    match kind {
        ModKind::Mods | ModKind::Flan | ModKind::Dependency | ModKind::Ic2Lib | ModKind::DenLib | ModKind::Coremods | ModKind::Jar => Some(PackContentKind::Mod),
        ModKind::TexturePack | ModKind::ResourcePack | ModKind::TexturePackExtract | ModKind::ResourcePackExtract => Some(PackContentKind::ResourcePack),
        ModKind::ShaderPack => Some(PackContentKind::Shader),
        _ => None,
    }
}

/// The mods, resource packs and shader packs a client gets from this manifest (optional ones marked), whether
/// or not the player would pick the optional ones.
pub fn manifest_contents(manifest: &AtlManifest) -> Vec<PackContentItem> {
    let mut out = Vec::new();
    for m in manifest.mods.iter().filter(|m| m.client && !m.name.trim().is_empty()) {
        let Some(kind) = content_kind(m) else { continue };
        let name = m.name.trim().to_string();
        let mut item = PackContentItem::from_file(&m.file, kind);
        item.name = name;
        item.file_name = Some(m.file.trim().to_string()).filter(|f| !f.is_empty());
        item.version = Some(m.version.trim().to_string()).filter(|v| !v.is_empty());
        item.optional = m.optional;
        item.description = Some(m.description.trim().to_string()).filter(|d| !d.is_empty());
        // ATLauncher writes "Unknown" when it doesn't know
        item.authors = m.authors.iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty() && !a.eq_ignore_ascii_case("unknown")).collect();
        item.page_url = m.website.clone().filter(|u| u.starts_with("https://") || u.starts_with("http://"));
        if let Some(id) = m.curse_id {
            item.site = Some(ContentSite::Curseforge);
            item.project_id = Some(id.to_string());
        }
        out.push(item);
    }
    sort_contents(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn manifest(name: &str) -> AtlManifest {
        let bytes = std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn lists_client_mods_with_their_projects() {
        let items = manifest_contents(&manifest("manifest_pixelmon.json"));
        assert!(!items.iter().any(|i| i.name == "Pixelmon Server"), "server-only");
        let jei = items.iter().find(|i| i.name == "Just Enough Items (JEI)").unwrap();
        assert_eq!(jei.site, Some(ContentSite::Curseforge));
        assert_eq!(jei.project_id.as_deref(), Some("238222"));
        assert_eq!(jei.kind, PackContentKind::Mod);
        let pixelmon = items.iter().find(|i| i.name == "Pixelmon").unwrap();
        assert_eq!(pixelmon.version.as_deref(), Some("9.4.2"));
        assert_eq!(pixelmon.site, None);
        assert_eq!(pixelmon.page_url.as_deref(), Some("https://www.pixelmonmod.com/"));
        let raw = manifest("manifest_pixelmon.json");
        let raw_jei = raw.mods.iter().find(|m| m.name == "Just Enough Items (JEI)").unwrap();
        assert_eq!(jei.authors, raw_jei.authors.iter().filter(|a| *a != "Unknown").cloned().collect::<Vec<_>>());
        assert_eq!(jei.description.as_deref(), Some(raw_jei.description.trim()).filter(|d| !d.is_empty()));
        let ost = items.iter().find(|i| i.name == "The Pixelmon Mod OST").unwrap();
        assert_eq!(ost.kind, PackContentKind::ResourcePack);
        assert_eq!(items.last().unwrap().kind, PackContentKind::ResourcePack, "mods come first");
        assert_eq!(items.len(), 14);
    }

    #[test]
    fn marks_optional_mods() {
        let m = manifest("manifest_optional.json");
        let items = manifest_contents(&m);
        let optional: Vec<&str> = items.iter().filter(|i| i.optional).map(|i| i.name.as_str()).collect();
        let expected: Vec<&str> = m.mods.iter().filter(|x| x.client && x.optional && content_kind(x).is_some()).map(|x| x.name.as_str()).collect();
        assert!(!optional.is_empty());
        assert_eq!(optional.len(), expected.len());
        assert!(items.iter().any(|i| i.name == "Base Mod" && !i.optional));
    }

    #[test]
    fn reads_curse_ids_as_numbers_or_text() {
        let m: AtlMod = serde_json::from_str(r#"{"name":"A","curse_id":"238222","curse_file_id":0}"#).unwrap();
        assert_eq!(m.curse_id, Some(238222));
        assert_eq!(m.curse_file_id, None);
        let m: AtlMod = serde_json::from_str(r#"{"name":"A","curseForgeProjectId":12,"curse_file_id":null}"#).unwrap();
        assert_eq!(m.curse_id, Some(12));
    }
}

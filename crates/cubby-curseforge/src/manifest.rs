//! The CurseForge modpack format: a zip with `manifest.json` (game and loader versions, the files as
//! project id + file id pairs) and an `overrides` folder copied into the game directory.

use std::io::Read;
use std::path::Path;

use serde::Deserialize;
use cubby_common::packs::{safe_relative_path, PackLoaderKind};
use cubby_common::{Error, Result};

pub const MANIFEST_FILE: &str = "manifest.json";
const DEFAULT_OVERRIDES: &str = "overrides";
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;

/// `manifest.json` of a CurseForge modpack.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CfManifest {
    pub minecraft: CfMinecraft,
    pub manifest_type: String,
    pub manifest_version: u32,
    pub name: String,
    pub version: String,
    pub author: String,
    pub files: Vec<CfManifestFile>,
    /// Folder of the zip copied into the game directory; "overrides" when missing or empty.
    #[serde(deserialize_with = "overrides_or_default")]
    pub overrides: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CfMinecraft {
    pub version: String,
    pub mod_loaders: Vec<CfModLoader>,
    /// Memory the pack asks for, in MiB.
    pub recommended_ram: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CfModLoader {
    /// "forge-47.2.0", "neoforge-21.1.5", "fabric-0.15.11", "quilt-0.26.0".
    pub id: String,
    pub primary: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CfManifestFile {
    #[serde(rename = "projectID")]
    pub project_id: u64,
    #[serde(rename = "fileID")]
    pub file_id: u64,
    /// `false` for optional files.
    #[serde(default = "yes")]
    pub required: bool,
}

fn yes() -> bool {
    true
}

fn overrides_or_default<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    let value = Option::<String>::deserialize(d)?.unwrap_or_default();
    Ok(if value.trim().is_empty() { DEFAULT_OVERRIDES.to_string() } else { value })
}

/// Reads `manifest.json` bytes. Works without an API key.
pub fn parse_manifest(bytes: &[u8]) -> Result<CfManifest> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let mut manifest: CfManifest = serde_json::from_slice(bytes).map_err(|e| Error::json(MANIFEST_FILE, e))?;
    if manifest.minecraft.version.trim().is_empty() {
        return Err(Error::Other("This isn't a CurseForge modpack: manifest.json doesn't say which Minecraft version it needs.".into()));
    }
    if !manifest.manifest_type.is_empty() && manifest.manifest_type != "minecraftModpack" {
        return Err(Error::Other(format!("manifest.json describes a \"{}\", not a modpack.", manifest.manifest_type)));
    }
    if manifest.overrides.trim().is_empty() {
        // The field is missing (serde's container default skips `overrides_or_default`).
        manifest.overrides = DEFAULT_OVERRIDES.to_string();
    }
    if safe_relative_path(&manifest.overrides).is_none() {
        return Err(Error::Other(format!("The overrides folder \"{}\" of the pack isn't a safe path.", manifest.overrides)));
    }
    manifest.minecraft.version = manifest.minecraft.version.trim().to_string();
    Ok(manifest)
}

/// Reads and parses `manifest.json` of a modpack `.zip`. Works without an API key.
pub fn read_manifest(archive: &Path) -> Result<CfManifest> {
    let file = std::fs::File::open(archive).map_err(|e| Error::io(archive, e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| Error::Other(format!("{} isn't a valid zip file ({e}).", archive.display())))?;
    let name = zip
        .file_names()
        .find(|n| n.eq_ignore_ascii_case(MANIFEST_FILE))
        .map(str::to_string)
        .ok_or_else(|| Error::Other("This zip isn't a CurseForge modpack: it has no manifest.json (Modrinth packs use .mrpack).".into()))?;
    let entry = zip.by_name(&name).map_err(|e| Error::Other(format!("Couldn't read manifest.json ({e}).")))?;
    if entry.size() > MAX_MANIFEST_BYTES {
        return Err(Error::Other("manifest.json is unreasonably large.".into()));
    }
    let mut bytes = Vec::new();
    entry.take(MAX_MANIFEST_BYTES).read_to_end(&mut bytes).map_err(|e| Error::io(archive, e))?;
    parse_manifest(&bytes)
}

/// True when the zip has entries under the `prefix` folder (e.g. "overrides/").
pub(crate) fn zip_has_folder(archive: &Path, prefix: &str) -> Result<bool> {
    let file = std::fs::File::open(archive).map_err(|e| Error::io(archive, e))?;
    let zip = zip::ZipArchive::new(file).map_err(|e| Error::Other(format!("{} isn't a valid zip file ({e}).", archive.display())))?;
    let found = zip.file_names().any(|n| n.replace('\\', "/").starts_with(prefix) && n.len() > prefix.len());
    Ok(found)
}

impl CfManifest {
    /// The loader the pack runs on: the one flagged `primary`, else the first. `None` for a vanilla pack.
    /// Ids look like "forge-47.2.0"; NeoForge ids of 1.20.1 look like "neoforge-1.20.1-47.1.106", where
    /// the Minecraft prefix isn't part of the loader version.
    pub fn loader(&self) -> Result<Option<(PackLoaderKind, String)>> {
        let chosen = self.minecraft.mod_loaders.iter().find(|l| l.primary).or_else(|| self.minecraft.mod_loaders.first());
        match chosen {
            None => Ok(None),
            Some(l) => parse_loader_id(&l.id, &self.minecraft.version).map(Some),
        }
    }

    /// `overrides/` style prefix for `PackOverrides`, always ending with a slash.
    pub fn overrides_prefix(&self) -> String {
        let clean = safe_relative_path(&self.overrides).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_else(|| DEFAULT_OVERRIDES.into());
        format!("{clean}/")
    }
}

pub(crate) fn parse_loader_id(id: &str, minecraft: &str) -> Result<(PackLoaderKind, String)> {
    let id = id.trim();
    let (kind, rest) = [("neoforge-", PackLoaderKind::NeoForge), ("forge-", PackLoaderKind::Forge), ("fabric-", PackLoaderKind::Fabric), ("quilt-", PackLoaderKind::Quilt)]
        .into_iter()
        .find_map(|(prefix, kind)| strip_prefix_ci(id, prefix).map(|rest| (kind, rest)))
        .ok_or_else(|| Error::Other(format!("This pack uses the mod loader \"{id}\", which Cubby doesn't support.")))?;
    let version = rest.strip_prefix(&format!("{minecraft}-")).unwrap_or(rest).trim();
    if version.is_empty() || version.eq_ignore_ascii_case("recommended") {
        return Err(Error::Other(format!("The mod loader \"{id}\" of this pack has no exact version.")));
    }
    Ok((kind, version.to_string()))
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    s.get(..prefix.len()).filter(|head| head.eq_ignore_ascii_case(prefix)).map(|_| &s[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> CfManifest {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
        parse_manifest(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn forge_manifest() {
        let m = fixture("manifest_forge.json");
        assert_eq!(m.name, "Example Forge Pack");
        assert_eq!(m.minecraft.version, "1.20.1");
        assert_eq!(m.minecraft.recommended_ram, Some(6144));
        assert_eq!(m.files.len(), 6);
        assert!(m.files[0].required && !m.files[4].required);
        assert_eq!(m.loader().unwrap(), Some((PackLoaderKind::Forge, "47.2.0".into())));
        assert_eq!(m.overrides_prefix(), "overrides/");
    }

    #[test]
    fn neoforge_fabric_quilt_prefixes() {
        assert_eq!(fixture("manifest_neoforge.json").loader().unwrap(), Some((PackLoaderKind::NeoForge, "21.1.5".into())));
        assert_eq!(fixture("manifest_fabric.json").loader().unwrap(), Some((PackLoaderKind::Fabric, "0.15.11".into())));
        assert_eq!(fixture("manifest_fabric.json").overrides_prefix(), "my-overrides/");
        assert_eq!(fixture("manifest_quilt.json").loader().unwrap(), Some((PackLoaderKind::Quilt, "0.26.0".into())));
    }

    #[test]
    fn loader_id_edge_cases() {
        assert_eq!(parse_loader_id("neoforge-1.20.1-47.1.106", "1.20.1").unwrap(), (PackLoaderKind::NeoForge, "47.1.106".into()));
        assert_eq!(parse_loader_id("forge-1.12.2-14.23.5.2860", "1.12.2").unwrap().1, "14.23.5.2860");
        assert!(parse_loader_id("forge-recommended", "1.20.1").is_err());
        assert!(parse_loader_id("liteloader-1.0", "1.12.2").is_err());
        assert!(parse_loader_id("forge-", "1.20.1").is_err());
    }

    #[test]
    fn primary_loader_wins_and_vanilla() {
        let m = parse_manifest(br#"{"minecraft":{"version":"1.20.1","modLoaders":[{"id":"fabric-0.1","primary":false},{"id":"quilt-0.2","primary":true}]},"files":[]}"#).unwrap();
        assert_eq!(m.loader().unwrap().unwrap().0, PackLoaderKind::Quilt);
        let v = parse_manifest(br#"{"minecraft":{"version":"1.20.1","modLoaders":[]},"files":[]}"#).unwrap();
        assert_eq!(v.loader().unwrap(), None);
    }

    #[test]
    fn rejects_bad_manifests() {
        assert!(parse_manifest(b"not json").is_err());
        assert!(parse_manifest(br#"{"minecraft":{"version":""}}"#).is_err());
        assert!(parse_manifest(br#"{"manifestType":"other","minecraft":{"version":"1.20.1"}}"#).is_err());
        assert!(parse_manifest(br#"{"minecraft":{"version":"1.20.1"},"overrides":"../x"}"#).is_err());
        assert!(parse_manifest(br#"{"minecraft":{"version":"1.20.1"},"overrides":"C:/x"}"#).is_err());
        // empty overrides falls back to the default, a BOM is tolerated
        let m = parse_manifest(b"\xEF\xBB\xBF{\"minecraft\":{\"version\":\"1.20.1\"},\"overrides\":\"\"}").unwrap();
        assert_eq!(m.overrides_prefix(), "overrides/");
    }
}

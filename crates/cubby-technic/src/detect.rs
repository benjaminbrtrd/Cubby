//! Working out the Minecraft version and mod loader of a Technic pack from its `bin/` folder, like
//! Prism Launcher's `TechnicPackProcessor.cpp`.
//!
//! `bin/modpack.jar` wins: with a `version.json` inside it is a modern pack, without one a legacy jar mod
//! (1.5.2 and older) whose Minecraft version comes from the pack page. Otherwise `bin/version.json` is read,
//! and without either the pack is vanilla.

use std::io::{Cursor, Read};

use serde_json::Value;
use cubby_common::packs::PackLoaderKind;
use cubby_common::{Error, Result};

/// What a pack runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    pub minecraft: String,
    /// `None` for a vanilla pack, or when a legacy jar mod doesn't tell the Forge version.
    pub loader: Option<(PackLoaderKind, String)>,
    /// `bin/modpack.jar` has no `version.json`: it has to be merged into `minecraft.jar` (jar mod), a
    /// regular loader install doesn't apply.
    pub legacy_jar_mod: bool,
}

/// Detects the Minecraft version and loader from the content of `bin/modpack.jar` and `bin/version.json`
/// (whichever the pack has) and the Minecraft version of the pack page (`pack_minecraft`).
pub fn detect(modpack_jar: Option<&[u8]>, version_json: Option<&[u8]>, pack_minecraft: Option<&str>) -> Result<Detected> {
    let pack_minecraft = pack_minecraft.map(str::trim).filter(|v| !v.is_empty());
    let mut fml_minecraft = None;
    let json: Vec<u8> = if let Some(jar) = modpack_jar {
        let mut zip = zip::ZipArchive::new(Cursor::new(jar)).map_err(|e| Error::Other(format!("bin/modpack.jar isn't a valid archive ({e}).")))?;
        match read_entry(&mut zip, "version.json")? {
            Some(json) => {
                if let Some(props) = read_entry(&mut zip, "fmlversion.properties")? {
                    fml_minecraft = property(&props, "fmlbuild.mcversion");
                }
                json
            }
            None => return legacy(&mut zip, pack_minecraft),
        }
    } else if let Some(json) = version_json {
        json.to_vec()
    } else {
        let minecraft = pack_minecraft.ok_or_else(|| Error::Other("The pack has no bin/ folder and doesn't say its Minecraft version.".into()))?;
        return Ok(Detected { minecraft: minecraft.to_string(), loader: None, legacy_jar_mod: false });
    };

    let root: Value = serde_json::from_slice(&json).map_err(|e| Error::json("the pack's version.json", e))?;
    let minecraft = root
        .get("inheritsFrom")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .or(fml_minecraft)
        .ok_or_else(|| Error::Other("The pack's version.json doesn't say its Minecraft version (inheritsFrom is missing).".into()))?;
    Ok(Detected { minecraft, loader: loader_of(&root), legacy_jar_mod: false })
}

fn legacy(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, pack_minecraft: Option<&str>) -> Result<Detected> {
    let minecraft = pack_minecraft
        .ok_or_else(|| Error::Other("bin/modpack.jar has no version.json and the pack doesn't say its Minecraft version.".to_string()))?
        .to_string();
    let mut loader = None;
    if let Some(props) = read_entry(zip, "forgeversion.properties")? {
        let part = |key: &str| property(&props, key);
        let (Some(major), Some(minor), Some(revision), Some(build)) = (part("forge.major.number"), part("forge.minor.number"), part("forge.revision.number"), part("forge.build.number")) else {
            return Err(Error::Other("bin/modpack.jar has an invalid forgeversion.properties.".into()));
        };
        loader = Some((PackLoaderKind::Forge, format!("{major}.{minor}.{revision}.{build}")));
    }
    Ok(Detected { minecraft, loader, legacy_jar_mod: true })
}

fn loader_of(root: &Value) -> Option<(PackLoaderKind, String)> {
    let libraries = root.get("libraries").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    for library in libraries {
        let Some(name) = library.get("name").and_then(Value::as_str) else { continue };

        if name.starts_with("net.neoforged.fancymodloader:") {
            // No easy way to get the version from the libraries: it is in the game arguments.
            let args: Vec<&str> = root.pointer("/arguments/game").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            let version = args
                .windows(2)
                .find(|w| w[0] == "--fml.neoForgeVersion" || w[0] == "--fml.forgeVersion")
                .map(|w| w[1].to_string())
                .filter(|v| !v.is_empty());
            return version.map(|v| (PackLoaderKind::NeoForge, v));
        }
        if let Some(version) = name.strip_prefix("net.neoforged:neoforge:") {
            return Some((PackLoaderKind::NeoForge, version.to_string()));
        }
        if name.starts_with("net.minecraftforge:forge:") || name.starts_with("net.minecraftforge:fmlloader:") {
            let version = name.splitn(3, ':').nth(2).unwrap_or_default();
            if let Some((_, after)) = version.split_once('-') {
                // 1.7.10 versions sometimes look like 1.7.10-10.13.4.1614-1.7.10: keep 10.13.4.1614.
                let forge = if version.starts_with("1.7.10-") { after.split('-').next().unwrap_or(after) } else { after };
                return Some((PackLoaderKind::Forge, forge.to_string()));
            }
        }
        let after_colon = |prefix: &str| name.strip_prefix(prefix).map(str::to_string);
        if let Some(v) = after_colon("net.minecraftforge:minecraftforge:") {
            return Some((PackLoaderKind::Forge, v));
        }
        if let Some(v) = after_colon("net.fabricmc:fabric-loader:") {
            return Some((PackLoaderKind::Fabric, v));
        }
        if let Some(v) = after_colon("org.quiltmc:quilt-loader:") {
            return Some((PackLoaderKind::Quilt, v));
        }
    }
    // Last resort: the version id, "1.12.2-forge-14.23.5.2860" or "1.20.1-forge-47.2.0".
    let id = root.get("id").and_then(Value::as_str)?;
    id.split_once("-forge-").map(|(_, v)| (PackLoaderKind::Forge, v.to_string())).filter(|(_, v)| !v.is_empty())
}

fn read_entry(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Option<Vec<u8>>> {
    let mut file = match zip.by_name(name) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(Error::Other(format!("Couldn't read {name} in bin/modpack.jar ({e})."))),
    };
    let mut out = Vec::new();
    // These files are a few KiB; refuse anything absurd.
    file.by_ref().take(16 * 1024 * 1024).read_to_end(&mut out).map_err(|e| Error::Other(format!("Couldn't read {name} in bin/modpack.jar ({e}).")))?;
    Ok(Some(out))
}

/// `key=value` / `key: value` lines of a `.properties` file.
fn property(data: &[u8], key: &str) -> Option<String> {
    String::from_utf8_lossy(data).lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with('!') {
            return None;
        }
        let (k, v) = line.split_once(['=', ':'])?;
        (k.trim() == key).then(|| v.trim().to_string()).filter(|v| !v.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn jar(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut out);
        for (name, content) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(content.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        out.into_inner()
    }

    const FORGE_JSON: &str = r#"{"id":"1.12.2-forge-14.23.5.2860","inheritsFrom":"1.12.2","libraries":[{"name":"net.minecraft:launchwrapper:1.12"},{"name":"net.minecraftforge:forge:1.12.2-14.23.5.2860"}]}"#;

    #[test]
    fn modern_forge_from_version_json() {
        let d = detect(None, Some(FORGE_JSON.as_bytes()), Some("1.7.10")).unwrap();
        assert_eq!(d, Detected { minecraft: "1.12.2".into(), loader: Some((PackLoaderKind::Forge, "14.23.5.2860".into())), legacy_jar_mod: false });
    }

    #[test]
    fn version_json_inside_modpack_jar_wins_over_bin() {
        let modpack = jar(&[("version.json", FORGE_JSON), ("fmlversion.properties", "fmlbuild.mcversion=9.9.9")]);
        let other = r#"{"inheritsFrom":"1.0","libraries":[]}"#;
        let d = detect(Some(&modpack), Some(other.as_bytes()), None).unwrap();
        assert_eq!(d.minecraft, "1.12.2");
        assert_eq!(d.loader, Some((PackLoaderKind::Forge, "14.23.5.2860".into())));
    }

    #[test]
    fn fml_version_gives_minecraft_when_inherits_from_is_missing() {
        let json = r#"{"libraries":[{"name":"net.minecraftforge:minecraftforge:9.11.1.1345"}]}"#;
        let modpack = jar(&[("version.json", json), ("fmlversion.properties", "#comment\nfmlbuild.major.number=7\nfmlbuild.mcversion=1.6.4\n")]);
        let d = detect(Some(&modpack), None, None).unwrap();
        assert_eq!(d.minecraft, "1.6.4");
        assert_eq!(d.loader, Some((PackLoaderKind::Forge, "9.11.1.1345".into())));
        assert!(detect(None, Some(json.as_bytes()), Some("1.6.4")).is_err());
    }

    #[test]
    fn forge_1_7_10_versions() {
        let json = r#"{"inheritsFrom":"1.7.10","libraries":[{"name":"net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10"}]}"#;
        assert_eq!(detect(None, Some(json.as_bytes()), None).unwrap().loader, Some((PackLoaderKind::Forge, "10.13.4.1614".into())));
        let json = r#"{"inheritsFrom":"1.16.5","libraries":[{"name":"net.minecraftforge:fmlloader:1.16.5-36.2.39"}]}"#;
        assert_eq!(detect(None, Some(json.as_bytes()), None).unwrap().loader, Some((PackLoaderKind::Forge, "36.2.39".into())));
    }

    #[test]
    fn fabric_quilt_neoforge() {
        let json = r#"{"inheritsFrom":"1.20.1","libraries":[{"name":"net.fabricmc:intermediary:1.20.1"},{"name":"net.fabricmc:fabric-loader:0.15.7"}]}"#;
        assert_eq!(detect(None, Some(json.as_bytes()), None).unwrap().loader, Some((PackLoaderKind::Fabric, "0.15.7".into())));
        let json = r#"{"inheritsFrom":"1.20.1","libraries":[{"name":"org.quiltmc:quilt-loader:0.23.0"}]}"#;
        assert_eq!(detect(None, Some(json.as_bytes()), None).unwrap().loader, Some((PackLoaderKind::Quilt, "0.23.0".into())));
        let json = r#"{"inheritsFrom":"1.21.1","libraries":[{"name":"net.neoforged.fancymodloader:loader:4.0.0"}],"arguments":{"game":["--launchTarget","forgeclient","--fml.neoForgeVersion","21.1.77","--fml.mcVersion","1.21.1"]}}"#;
        let d = detect(None, Some(json.as_bytes()), None).unwrap();
        assert_eq!((d.minecraft.as_str(), d.loader), ("1.21.1", Some((PackLoaderKind::NeoForge, "21.1.77".into()))));
    }

    #[test]
    fn id_fallback_and_missing_loader() {
        let json = r#"{"id":"1.20.1-forge-47.2.0","inheritsFrom":"1.20.1","libraries":[]}"#;
        assert_eq!(detect(None, Some(json.as_bytes()), None).unwrap().loader, Some((PackLoaderKind::Forge, "47.2.0".into())));
        let json = r#"{"inheritsFrom":"1.20.1","libraries":[{"name":"com.google.guava:guava:31"}]}"#;
        assert_eq!(detect(None, Some(json.as_bytes()), None).unwrap().loader, None);
    }

    #[test]
    fn legacy_jar_mod() {
        let props = "forge.major.number=4\nforge.minor.number=3\nforge.revision.number=5\nforge.build.number=318\n";
        let modpack = jar(&[("forgeversion.properties", props), ("net/minecraftforge/Forge.class", "x")]);
        let d = detect(Some(&modpack), None, Some("1.4.7")).unwrap();
        assert_eq!(d, Detected { minecraft: "1.4.7".into(), loader: Some((PackLoaderKind::Forge, "4.3.5.318".into())), legacy_jar_mod: true });
        // Without the pack's Minecraft version there is nothing to go on.
        assert!(detect(Some(&modpack), None, None).is_err());
        // No properties: still a jar mod, loader unknown.
        let bare = jar(&[("Foo.class", "x")]);
        let d = detect(Some(&bare), None, Some("1.5.2")).unwrap();
        assert!(d.legacy_jar_mod && d.loader.is_none());
        let broken = jar(&[("forgeversion.properties", "forge.major.number=4\n"), ("a", "b")]);
        assert!(detect(Some(&broken), None, Some("1.5.2")).is_err());
    }

    #[test]
    fn vanilla_pack_and_bad_input() {
        let d = detect(None, None, Some("1.12.2")).unwrap();
        assert_eq!(d, Detected { minecraft: "1.12.2".into(), loader: None, legacy_jar_mod: false });
        assert!(detect(None, None, None).is_err());
        assert!(detect(Some(b"not a zip"), None, Some("1.12.2")).is_err());
        assert!(detect(None, Some(b"{nope"), None).is_err());
    }
}

//! Tests of the pure half of the plan, on trimmed real manifests and small synthetic ones.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::json;
use cubby_common::packs::PackLoaderKind;

use crate::model::AtlManifest;
use crate::plan::{build, memory_mib, repack, ArchiveJob};

const SERVER: &str = "https://cdn.test/atl";

fn load(text: &str) -> AtlManifest {
    serde_json::from_str(text).unwrap()
}

fn pixelmon() -> AtlManifest {
    load(include_str!("../tests/fixtures/manifest_pixelmon.json"))
}

fn moonquest() -> AtlManifest {
    load(include_str!("../tests/fixtures/manifest_moonquest.json"))
}

/// A Forge 1.12.2 manifest with the given mods (each a JSON object; `client` defaults to true).
fn with_mods(mods: serde_json::Value) -> AtlManifest {
    let mods: Vec<serde_json::Value> = mods
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            let mut m = m.clone();
            m.as_object_mut().unwrap().entry("client").or_insert(json!(true));
            m
        })
        .collect();
    load(&json!({ "version": "1", "minecraft": "1.12.2", "loader": {"type": "forge", "metadata": {"version": "14.23.5.2860"}}, "mods": mods }).to_string())
}

fn m(name: &str, kind: &str, download: &str, url: &str, file: &str) -> serde_json::Value {
    json!({ "name": name, "type": kind, "download": download, "url": url, "file": file, "md5": "ABCDEF" })
}

#[test]
fn server_direct_and_client_only_mods() {
    let built = build(&pixelmon(), &[], SERVER);
    assert!(built.fatal.is_empty(), "{:?}", built.fatal);
    assert_eq!(built.loader, Some((PackLoaderKind::NeoForge, "21.1.248".into())));
    assert_eq!(built.memory_mib, None);
    // 15 mods; the server jar is client:false.
    assert_eq!(built.files.len(), 14);
    let pixelmon_jar = built.files.iter().find(|f| f.path == PathBuf::from("mods").join("Pixelmon-1.21.1-9.4.2-universal.jar")).unwrap();
    assert_eq!(pixelmon_jar.urls, ["https://download.nodecdn.net/containers/reforged/universal/release/Pixelmon-1.21.1-9.4.2-universal.jar"]);
    assert_eq!(pixelmon_jar.sha1, None);
    assert!(!built.files.iter().any(|f| f.path.to_string_lossy().contains("server")));
    let server_mod = built.files.iter().find(|f| f.urls[0].contains("/packs/")).unwrap();
    assert!(server_mod.urls[0].starts_with("https://cdn.test/atl/packs/PixelmonMod/files/"), "{}", server_mod.urls[0]);
    assert!(server_mod.size.is_some());
    // The resource pack goes to its own folder.
    assert!(built.files.iter().any(|f| f.path.starts_with("resourcepacks")));
    // MD5 is kept on the side.
    assert_eq!(built.md5[&pixelmon_jar.path], "c99d1f6a35af384fc49e03ceabafc413");
}

#[test]
fn moonquest_old_pack() {
    let built = build(&moonquest(), &[], SERVER);
    assert!(built.fatal.is_empty(), "{:?}", built.fatal);
    // No loader block: Forge comes from the forge mod, without the Minecraft version prefix.
    assert_eq!(built.loader, Some((PackLoaderKind::Forge, "9.11.1.953".into())));
    assert_eq!(built.memory_mib, Some(2048));
    // The forge jar itself isn't installed; the optional minimaps aren't selected.
    let paths: Vec<String> = built.files.iter().map(|f| f.path.to_string_lossy().replace('\\', "/")).collect();
    assert_eq!(paths, ["mods/1.6.4/CodeChickenLib-universal-1.6.4-1.0.0.62.jar", "mods/CodeChickenCore 0.9.0.9.jar", "mods/NotEnoughItems 1.6.1.9.jar"]);
    // The options zip is extracted into the game directory.
    assert_eq!(built.archives.len(), 1);
    assert_eq!(built.archives[0].dest, "");
    assert_eq!(built.archives[0].prefix, "");
    assert!(built.archives[0].url.ends_with("/packs/MoonQuest/files/optionsoptifine.zip"));
    // Libraries are reported, not installed.
    assert!(built.warnings.iter().any(|w| w.contains("2 extra libraries")), "{:?}", built.warnings);
    assert!(built.warnings.iter().any(|w| w.contains("Forge 9.11.1.953")));
}

#[test]
fn optional_mods_and_special_characters() {
    let built = build(&moonquest(), &["Rei's Minimap".into(), "Unknown Mod".into()], SERVER);
    let rei = built.files.iter().find(|f| f.path.ends_with("[1.6.4]ReiMinimap_v3.4_01.zip")).expect("selected optional mod");
    assert_eq!(rei.urls, ["https://cdn.test/atl/packs/MoonQuest/files/[1.6.4]ReiMinimap_v3.4_01.zip"]);
    assert!(!built.files.iter().any(|f| f.path.to_string_lossy().contains("ZansMinimap")));
    assert!(built.warnings.iter().any(|w| w.contains("Unknown Mod")));
}

#[test]
fn browser_mods_are_blocked() {
    let manifest = with_mods(json!([m("FastCraft", "mods", "browser", "http://forum.example.test/thread/1", "fastcraft.jar"), m("Other", "ic2lib", "server", "packs/X/o.jar", "o.jar")]));
    let built = build(&manifest, &[], SERVER);
    assert_eq!(built.blocked.len(), 1);
    let b = &built.blocked[0];
    assert_eq!((b.file_name.as_str(), b.page_url.as_str()), ("fastcraft.jar", "http://forum.example.test/thread/1"));
    assert_eq!(b.path, PathBuf::from("mods").join("fastcraft.jar"));
    assert_eq!(b.sha1, None);
    assert_eq!(built.md5[&b.path], "abcdef");
    assert_eq!(built.files.len(), 1);
    assert_eq!(built.files[0].path, PathBuf::from("mods").join("ic2").join("o.jar"));
}

#[test]
fn folders_of_every_type() {
    let kinds = [
        ("mods", "mods"),
        ("flan", "Flan"),
        ("dependency", "mods/1.12.2"),
        ("ic2lib", "mods/ic2"),
        ("denlib", "mods/denlib"),
        ("coremods", "coremods"),
        ("plugins", "plugins"),
        ("texturepack", "texturepacks"),
        ("resourcepack", "resourcepacks"),
        ("shaderpack", "shaderpacks"),
        ("root", ""),
    ];
    for (kind, folder) in kinds {
        let built = build(&with_mods(json!([m("X", kind, "server", "p/x.jar", "x.jar")])), &[], SERVER);
        let expected = if folder.is_empty() { PathBuf::from("x.jar") } else { PathBuf::from(folder).join("x.jar") };
        assert_eq!(built.files[0].path, expected, "{kind}");
    }
}

#[test]
fn unsafe_paths_and_urls_are_refused() {
    for (url, file) in [("p/x.jar", "../x.jar"), ("p/x.jar", "/etc/x.jar"), ("p/x.jar", "C:/x.jar"), ("p/../../x.jar", "x.jar"), ("file:///etc/passwd", "x.jar")] {
        let download = if url.starts_with("file") { "direct" } else { "server" };
        let built = build(&with_mods(json!([m("Evil", "mods", download, url, file)])), &[], SERVER);
        assert!(built.files.is_empty(), "{url} {file}");
        assert_eq!(built.fatal.len(), 1, "{url} {file}");
    }
    // Unknown download type and unknown mod type are fatal, like in Prism.
    assert_eq!(build(&with_mods(json!([m("X", "mods", "ftp", "p", "x.jar")])), &[], SERVER).fatal.len(), 1);
    assert_eq!(build(&with_mods(json!([m("X", "nonsense", "server", "p", "x.jar")])), &[], SERVER).fatal.len(), 1);
}

#[test]
fn unsupported_types() {
    // Jar mods patch minecraft.jar.
    let built = build(&with_mods(json!([m("Some Jar", "jar", "server", "p/j.jar", "j.jar")])), &[], SERVER);
    assert!(built.fatal[0].contains("jar mod"), "{:?}", built.fatal);
    // ...unless they are optional and left out.
    let mut optional = m("Some Jar", "jar", "server", "p/j.jar", "j.jar");
    optional["optional"] = json!(true);
    assert!(build(&with_mods(json!([optional])), &[], SERVER).fatal.is_empty());
    // Forge fixed up from a "jar" named "Minecraft Forge" (no loader block).
    let mut manifest = with_mods(json!([{ "name": "Minecraft Forge", "version": "1.9.4-12.17.0.1968", "type": "jar", "download": "server", "url": "p/f.jar", "file": "f.jar" }]));
    manifest.loader = None;
    manifest.minecraft = "1.9.4".into();
    let built = build(&manifest, &[], SERVER);
    assert!(built.fatal.is_empty() && built.files.is_empty());
    assert_eq!(built.loader, Some((PackLoaderKind::Forge, "12.17.0.1968".into())));
    // mcpc and millenaire are skipped with a warning.
    let built = build(&with_mods(json!([m("Srv", "mcpc", "server", "p/s.jar", "s.jar"), m("Mill", "millenaire", "server", "p/m.zip", "m.zip")])), &[], SERVER);
    assert!(built.fatal.is_empty() && built.files.is_empty());
    assert_eq!(built.warnings.len(), 2);
}

#[test]
fn loaders() {
    let loader = |block: serde_json::Value| {
        let mut manifest = with_mods(json!([]));
        manifest.loader = serde_json::from_value(block).unwrap();
        build(&manifest, &[], SERVER)
    };
    assert_eq!(loader(json!({"type": "fabric", "metadata": {"loader": "0.15.7", "yarn": "x"}})).loader, Some((PackLoaderKind::Fabric, "0.15.7".into())));
    assert_eq!(loader(json!({"type": "quilt", "metadata": {"loader": "0.26.0"}})).loader, Some((PackLoaderKind::Quilt, "0.26.0".into())));
    assert_eq!(loader(json!({"type": "forge", "metadata": {"version": "1.12.2-14.23.5.2860"}})).loader, Some((PackLoaderKind::Forge, "14.23.5.2860".into())));
    assert_eq!(loader(json!({"type": "", "metadata": {}})).loader, None);
    // Versions the launcher would have to look up, a loader we don't know.
    for block in [json!({"type": "forge", "metadata": {"latest": true}}), json!({"type": "forge", "choose": true, "metadata": {}}), json!({"type": "liteloader", "metadata": {}})] {
        assert_eq!(loader(block.clone()).fatal.len(), 1, "{block}");
    }
}

#[test]
fn memory_units() {
    assert_eq!(memory_mib(Some(4096)), Some(4096));
    assert_eq!(memory_mib(Some(4)), Some(4096));
    assert_eq!(memory_mib(Some(0)), None);
    assert_eq!(memory_mib(None), None);
}

#[test]
fn archive_jobs() {
    let mut extract = m("Pack Files", "extract", "server", "p/a.zip", "a.zip");
    extract["extractTo"] = json!("mods");
    extract["extractFolder"] = json!("/");
    let mut folder = m("Folder", "extract", "server", "p/b.zip", "b.zip");
    folder["extractTo"] = json!("root");
    folder["extractFolder"] = json!("Copy these%s%here/");
    let mut decomp = m("Decomp", "decomp", "server", "p/c.zip", "c.zip");
    decomp["decompType"] = json!("mods");
    decomp["decompFile"] = json!("mods/Multi.jar");
    let rp = m("RP", "resourcepackextract", "server", "p/d.zip", "d.zip");
    let built = build(&with_mods(json!([extract, folder, decomp, rp])), &[], SERVER);
    assert!(built.files.is_empty() && built.fatal.is_empty(), "{:?}", built.fatal);
    let jobs: Vec<(String, String, Option<String>)> = built.archives.iter().map(|a| (a.dest.clone(), a.prefix.clone(), a.file.clone())).collect();
    assert_eq!(
        jobs,
        [
            ("mods".into(), "".into(), None),
            ("".into(), "Copy these/here".into(), None),
            ("mods".into(), "".into(), Some("mods/Multi.jar".into())),
            ("resourcepacks/extracted".into(), "".into(), None),
        ]
    );
    // Unsafe or unsupported targets are left out with a warning, not installed.
    let mut evil = m("Evil", "extract", "server", "p/e.zip", "e.zip");
    evil["extractTo"] = json!("mods");
    evil["extractFolder"] = json!("../..");
    let mut odd = m("Odd", "extract", "server", "p/f.zip", "f.zip");
    odd["extractTo"] = json!("jar");
    let blocked = m("Manual", "extract", "browser", "http://x.test/g", "g.zip");
    let built = build(&with_mods(json!([evil, odd, blocked])), &[], SERVER);
    assert!(built.archives.is_empty() && built.blocked.is_empty() && built.fatal.is_empty());
    assert_eq!(built.warnings.len(), 3, "{:?}", built.warnings);
}

fn write_zip(path: &Path, entries: &[(&str, &str)]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, body) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(body.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

fn read_zip(path: &Path) -> Vec<(String, String)> {
    let mut zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).unwrap();
        let mut body = String::new();
        io::Read::read_to_string(&mut entry, &mut body).unwrap();
        out.push((entry.name().to_string(), body));
    }
    out.sort();
    out
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cubby-atl-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn job(dest: &str, prefix: &str, file: Option<&str>) -> ArchiveJob {
    ArchiveJob { name: "t".into(), url: String::new(), md5: None, dest: dest.into(), prefix: prefix.into(), file: file.map(Into::into) }
}

#[test]
fn repacking() {
    let dir = scratch("repack");
    let src = dir.join("src.zip");
    write_zip(&src, &[("a.txt", "A"), ("sub/b.txt", "B"), ("sub/deep/c.txt", "C"), ("other/d.txt", "D"), ("sub/../../evil.txt", "E")]);

    let dst = dir.join("whole.zip");
    assert_eq!(repack(&src, &dst, &job("mods", "", None)).unwrap(), 4);
    let names: Vec<String> = read_zip(&dst).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["mods/a.txt", "mods/other/d.txt", "mods/sub/b.txt", "mods/sub/deep/c.txt"]);

    let dst = dir.join("folder.zip");
    assert_eq!(repack(&src, &dst, &job("resourcepacks/extracted", "sub", None)).unwrap(), 2);
    assert_eq!(read_zip(&dst), [("resourcepacks/extracted/b.txt".to_string(), "B".to_string()), ("resourcepacks/extracted/deep/c.txt".to_string(), "C".to_string())]);

    let dst = dir.join("file.zip");
    assert_eq!(repack(&src, &dst, &job("mods", "", Some("sub/b.txt"))).unwrap(), 1);
    assert_eq!(read_zip(&dst), [("mods/sub/b.txt".to_string(), "B".to_string())]);

    assert_eq!(repack(&src, &dir.join("none.zip"), &job("mods", "missing", None)).unwrap(), 0);
    assert!(repack(&dir.join("absent.zip"), &dir.join("x.zip"), &job("mods", "", None)).is_err());
    let _ = fs::remove_dir_all(&dir);
}

use std::collections::HashMap;
use std::path::Path;

use cubby_common::version::{RuleEnv, VersionJson};
use cubby_meta::install::plan_libraries;
use cubby_meta::profile::merge;

fn fixture(name: &str) -> VersionJson {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn env(os: &'static str, arch: &'static str) -> RuleEnv {
    RuleEnv { os_name: os, arch, features: HashMap::new() }
}

fn unix(paths: &[std::path::PathBuf]) -> Vec<String> {
    paths.iter().map(|p| p.to_string_lossy().replace('\\', "/")).collect()
}

#[test]
fn vanilla_1_21_1_on_windows() {
    let v = fixture("1.21.1.json");
    let plan = plan_libraries(&v, Path::new("/libs"), &env("windows", "x86_64"));
    let cp = unix(&plan.classpath);
    assert!(cp.contains(&"/libs/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar".to_string()));
    assert!(cp.contains(&"/libs/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar".to_string()));
    assert!(!cp.iter().any(|p| p.contains("natives-linux") || p.contains("natives-macos")), "{cp:?}");
    assert!(plan.natives.is_empty());
    assert_eq!(plan.downloads.len(), plan.classpath.len());
    assert!(plan.downloads.iter().all(|d| d.sha1.is_some() && d.url.starts_with("https://")));
}

#[test]
fn vanilla_1_21_1_on_linux_has_no_windows_natives() {
    let v = fixture("1.21.1.json");
    let plan = plan_libraries(&v, Path::new("/libs"), &env("linux", "x86_64"));
    let cp = unix(&plan.classpath);
    assert!(cp.iter().any(|p| p.contains("natives-linux")));
    assert!(!cp.iter().any(|p| p.contains("natives-windows")));
}

#[test]
fn fabric_merge_uses_maven_urls_and_keeps_vanilla_downloads() {
    let merged = merge(fixture("1.21.1.json"), fixture("fabric-1.21.1-profile.json"));
    assert_eq!(merged.id, "fabric-loader-0.20.1-1.21.1");
    assert_eq!(merged.inherits_from.as_deref(), Some("1.21.1"));
    assert!(merged.downloads.as_ref().unwrap().client.is_some());
    assert_eq!(merged.main_class.as_deref(), Some("net.fabricmc.loader.impl.launch.knot.KnotClient"));
    let plan = plan_libraries(&merged, Path::new("/libs"), &env("windows", "x86_64"));
    let loader = plan.downloads.iter().find(|d| d.url.contains("fabric-loader-0.20.1.jar")).expect("fabric loader");
    assert!(loader.url.starts_with("https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.20.1/"));
    // Fabric's ASM replaces the vanilla one (if any): a single asm entry.
    let cp = unix(&plan.classpath);
    assert_eq!(cp.iter().filter(|p| p.contains("/org/ow2/asm/asm/")).count(), 1);
    // Child libraries come first on the classpath.
    assert!(cp[0].contains("asm") || cp[0].contains("fabric"), "{}", cp[0]);
}

#[test]
fn legacy_natives_go_to_natives_not_classpath() {
    let json = r#"{"id":"1.12.2","libraries":[
      {"name":"org.lwjgl.lwjgl:lwjgl-platform:2.9.4-nightly-20150209",
       "natives":{"linux":"natives-linux","windows":"natives-windows","osx":"natives-osx"},
       "extract":{"exclude":["META-INF/"]},
       "downloads":{"classifiers":{
          "natives-windows":{"path":"w.jar","sha1":"aa","size":1,"url":"https://libraries.minecraft.net/w.jar"},
          "natives-linux":{"path":"l.jar","sha1":"bb","size":1,"url":"https://libraries.minecraft.net/l.jar"}}}},
      {"name":"com.mojang:text2speech:1.10.3","rules":[{"action":"allow","os":{"name":"osx"}}],
       "downloads":{"artifact":{"path":"t.jar","sha1":"cc","size":1,"url":"https://libraries.minecraft.net/t.jar"}}},
      {"name":"com.google.guava:guava:21.0","downloads":{"artifact":{"path":"g.jar","sha1":"dd","size":1,"url":"https://libraries.minecraft.net/g.jar"}}},
      {"name":"org.old:nourl:1.0"}
    ]}"#;
    let v: VersionJson = serde_json::from_str(json).unwrap();
    let plan = plan_libraries(&v, Path::new("/libs"), &env("windows", "x86_64"));
    assert_eq!(plan.natives.len(), 1);
    assert!(plan.natives[0].path.to_string_lossy().ends_with("w.jar"));
    assert_eq!(plan.natives[0].exclude, vec!["META-INF/".to_string()]);
    assert_eq!(unix(&plan.classpath), vec!["/libs/g.jar".to_string(), "/libs/org/old/nourl/1.0/nourl-1.0.jar".to_string()]);
    let nourl = plan.downloads.iter().find(|d| d.url.contains("nourl")).unwrap();
    assert_eq!(nourl.url, "https://libraries.minecraft.net/org/old/nourl/1.0/nourl-1.0.jar");
}

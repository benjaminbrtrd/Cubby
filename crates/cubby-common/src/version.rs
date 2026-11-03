//! Types of the Mojang version JSON (`piston-meta.mojang.com/v1/packages/.../<id>.json`), also used by
//! Fabric and Quilt profiles (partial documents with `inheritsFrom`), plus rule evaluation.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherits_from: Option<String>,
    /// "release", "snapshot", "old_beta", "old_alpha"
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_class: Option<String>,
    /// Modern (1.13+) arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Arguments>,
    /// Legacy (< 1.13) space-separated game arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<VersionDownloads>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_version: Option<JavaVersionReq>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logging: Option<Logging>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_time: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

impl ArgValue {
    pub fn values(&self) -> Vec<&str> {
        match self {
            ArgValue::One(v) => vec![v.as_str()],
            ArgValue::Many(vs) => vs.iter().map(String::as_str).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<OsRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OsRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    /// A regex on the OS version. Not evaluated (always considered matching).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Library {
    /// Maven coordinates: `group:artifact:version[:classifier][@extension]`.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<LibraryDownloads>,
    /// Maven repository base URL (Fabric/Quilt style libraries without `downloads`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<Rule>>,
    /// Legacy natives: OS name -> classifier, e.g. `"windows": "natives-windows-${arch}"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extract: Option<Extract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryDownloads {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<Artifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Artifact {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VersionDownloads {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersionReq {
    /// Mojang runtime component, e.g. "java-runtime-delta".
    pub component: String,
    pub major_version: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Logging {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<LoggingConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// e.g. "-Dlog4j.configurationFile=${path}"
    pub argument: String,
    pub file: LoggingFile,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// Asset index document (`assets/indexes/<id>.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssetIndex {
    #[serde(default)]
    pub objects: HashMap<String, AssetObject>,
    /// Pre-1.7 indexes: assets are copied to `assets/virtual/legacy`.
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    /// Very old indexes: assets are copied to `<game dir>/resources`.
    #[serde(default)]
    pub map_to_resources: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

impl AssetObject {
    /// `<hash prefix>/<hash>`, relative to `assets/objects` and to `resources.download.minecraft.net`.
    pub fn relative_path(&self) -> String {
        format!("{}/{}", &self.hash[..2], self.hash)
    }
}

pub const ASSETS_BASE_URL: &str = "https://resources.download.minecraft.net";
pub const LIBRARIES_BASE_URL: &str = "https://libraries.minecraft.net";

impl Library {
    /// Relative Maven path of the main artifact, e.g. `org/ow2/asm/asm/9.6/asm-9.6.jar`.
    pub fn maven_path(&self) -> Option<String> {
        maven_path(&self.name, None)
    }

    /// Whether the library applies to this environment.
    pub fn applies(&self, env: &RuleEnv) -> bool {
        self.rules.as_deref().is_none_or(|rules| rules_allow(rules, env))
    }

    /// Classifier of the legacy natives jar for this environment, if any (`${arch}` resolved).
    pub fn natives_classifier(&self, env: &RuleEnv) -> Option<String> {
        let natives = self.natives.as_ref()?;
        let classifier = natives.get(env.os_name)?;
        Some(classifier.replace("${arch}", if env.arch == "x86" { "32" } else { "64" }))
    }
}

/// Converts Maven coordinates (`group:artifact:version[:classifier][@ext]`) to a relative path.
/// `classifier` overrides the one in the coordinates.
pub fn maven_path(coordinates: &str, classifier: Option<&str>) -> Option<String> {
    let (coords, extension) = match coordinates.split_once('@') {
        Some((c, ext)) => (c, ext),
        None => (coordinates, "jar"),
    };
    let mut parts = coords.split(':');
    let group = parts.next()?;
    let artifact = parts.next()?;
    let version = parts.next()?;
    let own_classifier = parts.next();
    let classifier = classifier.or(own_classifier);
    let file = match classifier {
        Some(c) => format!("{artifact}-{version}-{c}.{extension}"),
        None => format!("{artifact}-{version}.{extension}"),
    };
    Some(format!("{}/{artifact}/{version}/{file}", group.replace('.', "/")))
}

/// What rules are evaluated against.
#[derive(Debug, Clone)]
pub struct RuleEnv {
    /// Mojang OS name: "windows", "osx" or "linux".
    pub os_name: &'static str,
    /// "x86", "x86_64" or "arm64".
    pub arch: &'static str,
    /// Launcher features, e.g. `is_demo_user`, `has_custom_resolution`, `is_quick_play_singleplayer`.
    pub features: HashMap<String, bool>,
}

impl RuleEnv {
    pub fn current() -> Self {
        let os_name = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "osx"
        } else {
            "linux"
        };
        let arch = if cfg!(target_arch = "x86") {
            "x86"
        } else if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86_64"
        };
        Self { os_name, arch, features: HashMap::new() }
    }

    pub fn with_feature(mut self, name: &str, enabled: bool) -> Self {
        self.features.insert(name.to_string(), enabled);
        self
    }
}

/// Mojang semantics: no rules allows; otherwise start from "disallow" and let the last matching rule win.
pub fn rules_allow(rules: &[Rule], env: &RuleEnv) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule, env) {
            allowed = rule.action == RuleAction::Allow;
        }
    }
    allowed
}

fn rule_matches(rule: &Rule, env: &RuleEnv) -> bool {
    if let Some(os) = &rule.os {
        if let Some(name) = &os.name {
            if name != env.os_name {
                return false;
            }
        }
        if let Some(arch) = &os.arch {
            if arch != env.arch {
                return false;
            }
        }
    }
    if let Some(features) = &rule.features {
        for (name, expected) in features {
            if env.features.get(name).copied().unwrap_or(false) != *expected {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_paths() {
        assert_eq!(maven_path("org.ow2.asm:asm:9.6", None).unwrap(), "org/ow2/asm/asm/9.6/asm-9.6.jar");
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-windows", None).unwrap(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );
        assert_eq!(maven_path("a.b:c:1@zip", Some("x")).unwrap(), "a/b/c/1/c-1-x.zip");
    }

    #[test]
    fn rules() {
        let env = RuleEnv { os_name: "windows", arch: "x86_64", features: HashMap::new() };
        let allow_osx = Rule { action: RuleAction::Allow, os: Some(OsRule { name: Some("osx".into()), ..Default::default() }), features: None };
        let allow_all = Rule { action: RuleAction::Allow, os: None, features: None };
        let disallow_osx = Rule { action: RuleAction::Disallow, ..allow_osx.clone() };
        assert!(rules_allow(&[], &env));
        assert!(!rules_allow(std::slice::from_ref(&allow_osx), &env));
        assert!(rules_allow(&[allow_all, disallow_osx], &env));
    }
}

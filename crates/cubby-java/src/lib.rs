//! Finding, choosing and downloading Java runtimes.

mod detect;
mod probe;
mod runtime;

pub use detect::detect;
pub use probe::probe;
pub use runtime::{ensure_runtime, platform_key};

use cubby_common::contracts::JavaInstall;
use cubby_common::events::EventSink;
use cubby_common::net::Http;
use cubby_common::paths::Paths;
use cubby_common::version::VersionJson;
use cubby_common::{Error, Result};

/// Major Java version a Minecraft version asks for (`javaVersion.majorVersion`), 8 when absent.
pub fn required_major(version: &VersionJson) -> u32 {
    version.java_version.as_ref().map_or(8, |j| j.major_version)
}

/// Numeric sort key of a version string: `"17.0.12"` -> `[17, 0, 12]`, `"1.8.0_402"` -> `[1, 8, 0, 402]`.
pub(crate) fn version_key(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap_or(0))
        .collect()
}

/// Chooses the install for `major`.
///
/// An exact major wins (managed first, then 64-bit, then the newest). Otherwise the smallest higher
/// major is accepted, but only when `major >= 17`: older Minecraft needs exactly its Java.
pub fn pick(installs: &[JavaInstall], major: u32) -> Option<JavaInstall> {
    let best = |target: u32| {
        installs
            .iter()
            .filter(|i| i.major == target)
            .max_by_key(|i| (i.managed, i.arch != "x86", version_key(&i.version)))
            .cloned()
    };
    best(major).or_else(|| {
        if major < 17 {
            return None;
        }
        let next = installs.iter().map(|i| i.major).filter(|m| *m > major).min()?;
        best(next)
    })
}

/// Returns a usable Java for `version`: an installed one when [`pick`] finds it, else the Mojang
/// runtime named by the version (downloaded into `paths.java()`).
pub async fn find_or_download(http: &Http, paths: &Paths, version: &VersionJson, sink: &dyn EventSink, task: &str) -> Result<JavaInstall> {
    let major = required_major(version);
    let installs = detect(paths).await;
    if let Some(found) = pick(&installs, major) {
        return Ok(found);
    }
    match &version.java_version {
        Some(req) if !req.component.is_empty() => ensure_runtime(http, paths, &req.component, sink, task).await,
        _ => Err(Error::Java(format!(
            "Java {major} is required but isn't installed, and this version doesn't name a Java runtime to download. Install Java {major}, then try again."
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn install(major: u32, version: &str, arch: &str, managed: bool) -> JavaInstall {
        JavaInstall { executable: PathBuf::from(format!("{major}-{version}-{arch}-{managed}")), major, version: version.into(), arch: arch.into(), vendor: None, managed }
    }

    #[test]
    fn pick_exact_prefers_managed_then_64bit_then_newest() {
        let list = [
            install(17, "17.0.12", "x86_64", false),
            install(17, "17.0.2", "x86_64", true),
            install(17, "17.0.14", "x86", false),
            install(8, "1.8.0_402", "x86_64", false),
        ];
        assert_eq!(pick(&list, 17).unwrap().version, "17.0.2");
        let list = [install(17, "17.0.14", "x86", false), install(17, "17.0.2", "x86_64", false), install(17, "17.0.12", "x86_64", false)];
        assert_eq!(pick(&list, 17).unwrap().version, "17.0.12");
    }

    #[test]
    fn pick_falls_back_upward_only_from_17() {
        let list = [install(21, "21.0.5", "x86_64", false), install(25, "25", "x86_64", false)];
        assert_eq!(pick(&list, 17).unwrap().major, 21);
        assert!(pick(&list, 8).is_none());
        assert!(pick(&list, 16).is_none());
        assert!(pick(&list, 26).is_none());
        assert!(pick(&[], 17).is_none());
    }

    #[test]
    fn version_key_orders_numerically() {
        assert!(version_key("17.0.12") > version_key("17.0.9"));
        assert!(version_key("1.8.0_402") > version_key("1.8.0_51"));
    }
}

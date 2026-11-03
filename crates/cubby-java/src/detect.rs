//! Finding the Java installations of the machine.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use cubby_common::contracts::JavaInstall;
use cubby_common::paths::Paths;
use futures::StreamExt;

use crate::{probe, version_key};

/// How many `java -version` processes run at the same time.
const PROBE_CONCURRENCY: usize = 8;
/// How deep a root folder is searched for JVM homes (`<root>/<vendor>/<jdk>/bin/java`).
const SCAN_DEPTH: usize = 3;

#[cfg(windows)]
const EXE: &str = "java.exe";
#[cfg(not(windows))]
const EXE: &str = "java";

/// Finds every working Java: registry (Windows), usual folders, `JAVA_HOME`, `PATH` and Cubby's own
/// `paths.java()`. Duplicates are merged, failures dropped; the result is sorted by major then version.
/// Runtimes under `paths.java()` are flagged `managed`.
pub async fn detect(paths: &Paths) -> Vec<JavaInstall> {
    let java_dir = paths.java();
    let candidates = {
        let java_dir = java_dir.clone();
        tokio::task::spawn_blocking(move || candidates(&java_dir)).await.unwrap_or_default()
    };
    let managed_root = std::fs::canonicalize(&java_dir).unwrap_or(java_dir);

    let mut installs: Vec<JavaInstall> = futures::stream::iter(candidates)
        .map(|exe| async move {
            match probe(&exe).await {
                Ok(install) => Some(install),
                Err(error) => {
                    tracing::debug!("{} is not usable: {error}", exe.display());
                    None
                }
            }
        })
        .buffer_unordered(PROBE_CONCURRENCY)
        .filter_map(|x| async move { x })
        .collect()
        .await;

    // Two candidates can resolve to the same runtime (java.exe shim vs real folder): dedupe again.
    let mut seen = HashSet::new();
    installs.retain(|i| seen.insert(canonical(&i.executable)));
    for install in &mut installs {
        install.managed = canonical(&install.executable).starts_with(&managed_root);
    }
    installs.sort_by(|a, b| (a.major, version_key(&a.version), &a.executable).cmp(&(b.major, version_key(&b.version), &b.executable)));
    installs
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Executables to probe, deduplicated by canonical path, in discovery order.
fn candidates(java_dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();

    #[cfg(windows)]
    for home in registry_homes() {
        found.extend(java_in_home(&home));
    }
    for root in scan_roots() {
        scan(&root, SCAN_DEPTH, &mut found);
    }
    scan(java_dir, SCAN_DEPTH, &mut found);
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        found.extend(java_in_home(Path::new(&home)));
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let exe = dir.join(EXE);
            if exe.is_file() {
                found.push(exe);
            }
        }
    }

    let mut seen = HashSet::new();
    found.retain(|p| seen.insert(canonical(p)));
    found
}

/// The `java` executable of a JVM home (`<home>/bin/java`, or the layouts of old JDKs and macOS bundles).
pub(crate) fn java_in_home(home: &Path) -> Option<PathBuf> {
    ["bin", "jre/bin", "Contents/Home/bin", "jre.bundle/Contents/Home/bin"]
        .iter()
        .map(|sub| home.join(sub).join(EXE))
        .find(|p| p.is_file())
}

/// Adds the Java of `dir` if it is a JVM home, else searches its sub-folders down to `depth`.
fn scan(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if let Some(exe) = java_in_home(dir) {
        out.push(exe);
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|t| t.is_dir() || t.is_symlink()) {
            scan(&entry.path(), depth - 1, out);
        }
    }
}

#[cfg(windows)]
fn scan_roots() -> Vec<PathBuf> {
    const VENDORS: &[&str] = &["Java", "Eclipse Adoptium", "Eclipse Foundation", "AdoptOpenJDK", "Zulu", "Microsoft", "BellSoft", "Amazon Corretto"];
    let mut roots = Vec::new();
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(base) = std::env::var_os(var) {
            roots.extend(VENDORS.iter().map(|v| Path::new(&base).join(v)));
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let appdata = Path::new(&appdata);
        roots.push(appdata.join(".minecraft").join("runtime"));
        roots.push(appdata.join("PrismLauncher").join("java"));
    }
    roots
}

#[cfg(not(windows))]
fn scan_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = [
        "/usr/lib/jvm",
        "/usr/lib64/jvm",
        "/usr/java",
        "/opt/java",
        "/opt/jdk",
        "/opt/jdks",
        "/Library/Java/JavaVirtualMachines",
        "/usr/local/opt",
        "/opt/homebrew/opt",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    if let Some(home) = dirs_home() {
        roots.push(home.join(".sdkman/candidates/java"));
        roots.push(home.join(".jdks"));
        roots.push(home.join(".minecraft/runtime"));
        roots.push(home.join("Library/Java/JavaVirtualMachines"));
        roots.push(home.join(".local/share/PrismLauncher/java"));
        roots.push(home.join("Library/Application Support/PrismLauncher/java"));
    }
    roots
}

#[cfg(not(windows))]
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// JVM homes registered by installers in the Windows registry (64 and 32-bit views, HKLM and HKCU).
#[cfg(windows)]
fn registry_homes() -> Vec<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
    use winreg::RegKey;

    const KEYS: &[&str] = &[
        r"SOFTWARE\JavaSoft\Java Runtime Environment",
        r"SOFTWARE\JavaSoft\Java Development Kit",
        r"SOFTWARE\JavaSoft\JRE",
        r"SOFTWARE\JavaSoft\JDK",
        r"SOFTWARE\Eclipse Adoptium\JRE",
        r"SOFTWARE\Eclipse Adoptium\JDK",
        r"SOFTWARE\Eclipse Foundation\JRE",
        r"SOFTWARE\Eclipse Foundation\JDK",
        r"SOFTWARE\Microsoft\JDK",
        r"SOFTWARE\Azul Systems\Zulu",
        r"SOFTWARE\BellSoft\Liberica",
    ];
    /// Values holding a home folder: Oracle `JavaHome`, Adoptium/Microsoft `Path`, Zulu/Liberica `InstallationPath`.
    const VALUES: &[&str] = &["JavaHome", "Path", "InstallationPath"];

    fn walk(key: &RegKey, view: u32, depth: usize, out: &mut Vec<PathBuf>) {
        for name in VALUES {
            if let Ok(value) = key.get_value::<String, _>(name) {
                out.push(PathBuf::from(value));
            }
        }
        if depth == 0 {
            return;
        }
        for sub in key.enum_keys().flatten() {
            if let Ok(child) = key.open_subkey_with_flags(&sub, KEY_READ | view) {
                walk(&child, view, depth - 1, out);
            }
        }
    }

    let mut homes = Vec::new();
    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            let root = RegKey::predef(hive);
            for path in KEYS {
                if let Ok(key) = root.open_subkey_with_flags(path, KEY_READ | view) {
                    walk(&key, view, 4, &mut homes);
                }
            }
        }
    }
    homes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_finds_nested_homes() {
        let root = std::env::temp_dir().join(format!("cubby-java-scan-{}", std::process::id()));
        let home = root.join("vendor").join("jdk-17");
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::write(home.join("bin").join(EXE), b"").unwrap();
        let mut out = Vec::new();
        scan(&root, SCAN_DEPTH, &mut out);
        assert_eq!(out, vec![home.join("bin").join(EXE)]);
        assert_eq!(java_in_home(&home), Some(home.join("bin").join(EXE)));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[tokio::test]
    #[ignore = "inspects the Javas installed on this machine"]
    async fn detect_this_machine() {
        let data = std::env::var("CARGO_TARGET_DIR").map_or_else(|_| PathBuf::from("target/java/testdata"), |t| PathBuf::from(t).join("testdata"));
        let installs = detect(&Paths::new(data)).await;
        for install in &installs {
            println!("{:>2} {:<14} {:<7} managed={} vendor={:?} {}", install.major, install.version, install.arch, install.managed, install.vendor, install.executable.display());
        }
        assert!(!installs.is_empty());
    }
}

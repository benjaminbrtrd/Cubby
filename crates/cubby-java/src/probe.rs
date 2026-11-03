//! Running `java -XshowSettings:properties -version` and reading the answer.

use std::path::{Path, PathBuf};
use std::time::Duration;

use cubby_common::contracts::JavaInstall;
use cubby_common::{Error, Result};
use tokio::process::Command;

const TIMEOUT: Duration = Duration::from_secs(10);

/// Runs `executable` and describes it. On Windows the returned executable is `javaw.exe` when it
/// sits next to `java.exe`, so the game does not open a console. `managed` is always false.
pub async fn probe(executable: &Path) -> Result<JavaInstall> {
    let console = console_variant(executable);
    let mut command = Command::new(&console);
    command.arg("-XshowSettings:properties").arg("-version").stdin(std::process::Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

    let output = tokio::time::timeout(TIMEOUT, command.output())
        .await
        .map_err(|_| Error::Java(format!("{} didn't respond within {} seconds.", console.display(), TIMEOUT.as_secs())))?
        .map_err(|e| Error::io(&console, e))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let info = parse_settings(&stderr).ok_or_else(|| Error::Java(format!("{} isn't a working Java (no java.version in its output).", console.display())))?;

    Ok(JavaInstall {
        executable: windowed_variant(&console),
        major: parse_major(&info.version).ok_or_else(|| Error::Java(format!("Couldn't read the Java version \"{}\".", info.version)))?,
        version: info.version,
        arch: normalize_arch(&info.arch),
        vendor: info.vendor,
        managed: false,
    })
}

/// `javaw.exe` -> `java.exe` (it is the one that prints to stderr); other names are unchanged.
fn console_variant(path: &Path) -> PathBuf {
    match path.file_name().and_then(|n| n.to_str()) {
        Some(name) if name.eq_ignore_ascii_case("javaw.exe") => path.with_file_name("java.exe"),
        _ => path.to_path_buf(),
    }
}

/// `java.exe` -> `javaw.exe` when it exists.
fn windowed_variant(path: &Path) -> PathBuf {
    if cfg!(windows) && path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case("java.exe")) {
        let w = path.with_file_name("javaw.exe");
        if w.is_file() {
            return w;
        }
    }
    path.to_path_buf()
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Settings {
    pub version: String,
    pub arch: String,
    pub vendor: Option<String>,
}

/// Reads `java.version`, `os.arch` and `java.vendor` from the `-XshowSettings:properties` output.
pub(crate) fn parse_settings(stderr: &str) -> Option<Settings> {
    let (mut version, mut arch, mut vendor) = (None, None, None);
    for line in stderr.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim().to_string();
        match key.trim() {
            "java.version" if version.is_none() => version = Some(value),
            "os.arch" if arch.is_none() => arch = Some(value),
            "java.vendor" if vendor.is_none() && !value.is_empty() => vendor = Some(value),
            _ => {}
        }
    }
    Some(Settings { version: version?, arch: arch.unwrap_or_default(), vendor })
}

/// Major version of a `java.version`: `1.8.0_402` -> 8, `17.0.12` -> 17, `21-ea` -> 21.
pub(crate) fn parse_major(version: &str) -> Option<u32> {
    let mut parts = version.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty());
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

/// Maps `os.arch` values to `x86_64`, `x86` or `aarch64` (others are kept as they are).
pub(crate) fn normalize_arch(arch: &str) -> String {
    match arch {
        "amd64" | "x86_64" | "x64" => "x86_64",
        "x86" | "i386" | "i486" | "i586" | "i686" => "x86",
        "aarch64" | "arm64" => "aarch64",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn majors() {
        assert_eq!(parse_major("1.8.0_402"), Some(8));
        assert_eq!(parse_major("17.0.12"), Some(17));
        assert_eq!(parse_major("21"), Some(21));
        assert_eq!(parse_major("21-ea"), Some(21));
        assert_eq!(parse_major("9-ea"), Some(9));
        assert_eq!(parse_major("1.7.0_80"), Some(7));
        assert_eq!(parse_major("nonsense"), None);
    }

    #[test]
    fn settings_output() {
        let out = "Property settings:\n    java.vendor = Eclipse Adoptium\n    java.version = 17.0.12\n    os.arch = amd64\n    java.vendor.url = https://adoptium.net/\n\nopenjdk version \"17.0.12\"";
        let s = parse_settings(out).unwrap();
        assert_eq!(s, Settings { version: "17.0.12".into(), arch: "amd64".into(), vendor: Some("Eclipse Adoptium".into()) });
        assert_eq!(normalize_arch(&s.arch), "x86_64");
        assert!(parse_settings("Error: could not find java").is_none());
    }
}

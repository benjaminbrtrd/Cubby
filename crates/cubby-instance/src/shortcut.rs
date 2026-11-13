//! Desktop shortcuts that start an instance: they open `cubby://play/<id>`, which Cubby handles by
//! starting the game (and Cubby itself, when it isn't running).
//!
//! Windows gets an Internet shortcut (`.url`), Linux a `.desktop` file, macOS a tiny app bundle whose
//! script opens the link (a `.webloc` would open the browser first).

use std::fs;
use std::path::{Path, PathBuf};

use cubby_common::{Error, Result};

/// The link a shortcut opens.
pub fn play_link(id: &str) -> String {
    format!("cubby://play/{id}")
}

/// A name usable as a file name on every system.
pub fn shortcut_file_stem(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { ' ' } else { c })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned.trim_matches('.').to_string();
    if cleaned.is_empty() { "Minecraft".into() } else { cleaned }
}

/// Contents of a Windows `.url` file. `icon` is an `.exe` or `.ico` to show.
pub fn windows_url_file(id: &str, icon: Option<&Path>) -> String {
    let mut text = format!("[InternetShortcut]\r\nURL={}\r\n", play_link(id));
    if let Some(icon) = icon {
        text.push_str(&format!("IconFile={}\r\nIconIndex=0\r\n", icon.display()));
    }
    text
}

/// Contents of a Linux `.desktop` file.
pub fn desktop_entry(id: &str, name: &str, icon: Option<&Path>) -> String {
    let mut text = format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment=Play {} with Cubby\nExec=xdg-open {}\nTerminal=false\nCategories=Game;\n",
        name.replace('\n', " "),
        name.replace('\n', " "),
        play_link(id)
    );
    if let Some(icon) = icon {
        text.push_str(&format!("Icon={}\n", icon.display()));
    }
    text
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// `Info.plist` of the macOS shortcut app.
pub fn macos_info_plist(id: &str, name: &str, has_icon: bool) -> String {
    let icon = if has_icon { "\t<key>CFBundleIconFile</key>\n\t<string>AppIcon</string>\n" } else { "" };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n<dict>\n\t<key>CFBundleExecutable</key>\n\t<string>play</string>\n{icon}\
\t<key>CFBundleIdentifier</key>\n\t<string>com.cubbylauncher.shortcut.{}</string>\n\t<key>CFBundleName</key>\n\t<string>{}</string>\n\
\t<key>CFBundlePackageType</key>\n\t<string>APPL</string>\n\t<key>LSUIElement</key>\n\t<true/>\n</dict>\n</plist>\n",
        xml_escape(&id.replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "-")),
        xml_escape(name)
    )
}

/// A path in `dir` named `stem` + `ext` that doesn't exist yet ("Pack 2.url" when "Pack.url" does).
fn free_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut path = dir.join(format!("{stem}{ext}"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stem} {n}{ext}"));
        n += 1;
    }
    path
}

/// Writes a shortcut for the current system into `dir` (the Desktop). `icon`: Cubby's icon file when
/// known (`.exe` or `.ico` on Windows, `.icns` on macOS, `.png` on Linux). Returns the shortcut's path.
pub fn create_shortcut(dir: &Path, id: &str, name: &str, icon: Option<&Path>) -> Result<PathBuf> {
    fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    let stem = shortcut_file_stem(name);
    if cfg!(target_os = "windows") {
        let path = free_path(dir, &stem, ".url");
        fs::write(&path, windows_url_file(id, icon)).map_err(|e| Error::io(&path, e))?;
        Ok(path)
    } else if cfg!(target_os = "macos") {
        let app = free_path(dir, &stem, ".app");
        let contents = app.join("Contents");
        let macos = contents.join("MacOS");
        fs::create_dir_all(&macos).map_err(|e| Error::io(&macos, e))?;
        let has_icon = match icon {
            Some(icon) if icon.is_file() => {
                let resources = contents.join("Resources");
                fs::create_dir_all(&resources).map_err(|e| Error::io(&resources, e))?;
                fs::copy(icon, resources.join("AppIcon.icns")).is_ok()
            }
            _ => false,
        };
        fs::write(contents.join("Info.plist"), macos_info_plist(id, name, has_icon)).map_err(|e| Error::io(&contents, e))?;
        let script = macos.join("play");
        fs::write(&script, format!("#!/bin/sh\nexec /usr/bin/open '{}'\n", play_link(id).replace('\'', ""))).map_err(|e| Error::io(&script, e))?;
        set_executable(&script)?;
        Ok(app)
    } else {
        let path = free_path(dir, &stem, ".desktop");
        fs::write(&path, desktop_entry(id, name, icon)).map_err(|e| Error::io(&path, e))?;
        set_executable(&path)?;
        Ok(path)
    }
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|e| Error::io(path, e))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    #[test]
    fn file_names_and_contents() {
        assert_eq!(shortcut_file_stem("  All: the / Mods? "), "All the Mods");
        assert_eq!(shortcut_file_stem("..."), "Minecraft");
        assert!(windows_url_file("my-pack", Some(Path::new("C:/Cubby/cubby.exe"))).contains("URL=cubby://play/my-pack\r\nIconFile=C:/Cubby/cubby.exe"));
        assert!(desktop_entry("my-pack", "My Pack", None).contains("Exec=xdg-open cubby://play/my-pack\n"));
        let plist = macos_info_plist("my-pack", "Tom & Jerry", true);
        assert!(plist.contains("<string>Tom &amp; Jerry</string>") && plist.contains("AppIcon"));
    }

    #[test]
    fn writes_one_per_call() {
        let dir = temp_dir("shortcut");
        let a = create_shortcut(&dir, "my-pack", "My Pack", None).unwrap();
        let b = create_shortcut(&dir, "my-pack", "My Pack", None).unwrap();
        assert_ne!(a, b);
        assert!(a.exists() && b.exists());
    }
}

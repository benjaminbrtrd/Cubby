use std::fs;
use std::io;
use std::path::Path;

use cubby_common::contracts::InstalledVersion;
use cubby_common::{Error, Result};

/// Recreates `natives_dir` and extracts every native jar of `installed` into it.
///
/// Entries under `META-INF/` and under any of the jar's `exclude` prefixes are skipped, as are entries
/// whose path would escape the directory.
pub fn extract_natives(installed: &InstalledVersion, natives_dir: &Path) -> Result<()> {
    if natives_dir.exists() {
        fs::remove_dir_all(natives_dir).map_err(|e| Error::io(natives_dir, e))?;
    }
    fs::create_dir_all(natives_dir).map_err(|e| Error::io(natives_dir, e))?;

    for jar in &installed.natives {
        let file = fs::File::open(&jar.path).map_err(|e| Error::io(&jar.path, e))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| Error::Launch(format!("{} isn't a valid natives jar ({e}). Try again.", jar.path.display())))?;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| Error::Launch(format!("{} isn't a valid natives jar ({e}). Try again.", jar.path.display())))?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_string();
            if name.starts_with("META-INF/") || jar.exclude.iter().any(|p| name.starts_with(p.as_str())) {
                continue;
            }
            let Some(relative) = entry.enclosed_name() else { continue };
            let target = natives_dir.join(relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
            }
            let mut out = fs::File::create(&target).map_err(|e| Error::io(&target, e))?;
            io::copy(&mut entry, &mut out).map_err(|e| Error::io(&target, e))?;
        }
    }
    Ok(())
}

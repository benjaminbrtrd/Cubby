//! Small filesystem helpers.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use cubby_common::{Error, Result};

/// Writes `data` to `path` through a temporary file in the same folder, then renames it over the target.
pub(crate) fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| Error::Other(format!("{} has no parent folder.", path.display())))?;
    fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".tmp");
    let tmp = parent.join(tmp_name);
    let write = || -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()
    };
    if let Err(e) = write() {
        let _ = fs::remove_file(&tmp);
        return Err(Error::io(&tmp, e));
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        Error::io(path, e)
    })
}

/// Turns a display name into a folder-safe slug (`[a-z0-9-]`, never empty, never a Windows device name).
pub(crate) fn slugify(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let mut out = out.trim_matches('-').to_string();
    out.truncate(48);
    let mut out = out.trim_end_matches('-').to_string();
    let is_device = ["con", "prn", "aux", "nul"].contains(&out.as_str())
        || ((out.starts_with("com") || out.starts_with("lpt")) && out.len() == 4 && out.as_bytes()[3].is_ascii_digit());
    if out.is_empty() {
        out = "instance".into();
    } else if is_device {
        out = format!("instance-{out}");
    }
    out
}

/// Rejects anything that is not a plain file or folder name (no separators, no `..`).
pub(crate) fn check_plain_name(name: &str) -> Result<()> {
    let bad = name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':', '\0'])
        || Path::new(name).components().count() != 1;
    if bad {
        Err(Error::Other(format!("{name:?} isn't a valid file name.")))
    } else {
        Ok(())
    }
}

/// Recursively copies a folder (symlinks are skipped).
pub(crate) fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).map_err(|e| Error::io(dst, e))?;
    for entry in fs::read_dir(src).map_err(|e| Error::io(src, e))? {
        let entry = entry.map_err(|e| Error::io(src, e))?;
        let ty = entry.file_type().map_err(|e| Error::io(entry.path(), e))?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), &target).map_err(|e| Error::io(entry.path(), e))?;
        }
    }
    Ok(())
}

/// Total size in bytes of a file or folder tree; unreadable entries count as zero.
pub(crate) fn tree_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else { return 0 };
    if meta.is_file() {
        return meta.len();
    }
    let mut total = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let Ok(ty) = entry.file_type() else { continue };
            if ty.is_dir() {
                stack.push(entry.path());
            } else if ty.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

/// Modification time of a path, if available.
pub(crate) fn mtime(path: &Path) -> Option<DateTime<Utc>> {
    fs::metadata(path).and_then(|m| m.modified()).ok().map(DateTime::<Utc>::from)
}

/// Moves `src` into `trash_dir` under a unique name `<name>-<timestamp>`; returns the new location.
pub(crate) fn move_to_trash(src: &Path, trash_dir: &Path, name: &str) -> Result<PathBuf> {
    fs::create_dir_all(trash_dir).map_err(|e| Error::io(trash_dir, e))?;
    let stamp = Utc::now().format("%Y%m%d-%H%M%S");
    let mut dest = trash_dir.join(format!("{name}-{stamp}"));
    let mut n = 1;
    while dest.exists() {
        n += 1;
        dest = trash_dir.join(format!("{name}-{stamp}-{n}"));
    }
    fs::rename(src, &dest).map_err(|e| Error::io(src, e))?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slugify("Mon Super Modpack!"), "mon-super-modpack");
        assert_eq!(slugify("  Été  --  1.20.1 "), "ete-1-20-1");
        assert_eq!(slugify("???"), "instance");
        assert_eq!(slugify("CON"), "instance-con");
        assert_eq!(slugify("lpt1"), "instance-lpt1");
    }

    #[test]
    fn plain_names() {
        assert!(check_plain_name("a.jar").is_ok());
        assert!(check_plain_name("../a.jar").is_err());
        assert!(check_plain_name("a/b").is_err());
        assert!(check_plain_name("..").is_err());
    }
}

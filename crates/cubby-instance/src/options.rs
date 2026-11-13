//! Sharing the game's options between instances: video and sound options, key bindings and the server list.

use std::fs;
use std::path::Path;

use cubby_common::{Error, Result};
use serde::Deserialize;

use crate::util::atomic_write;

/// What to copy from one instance to another.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OptionsCopy {
    /// Everything in `options.txt` but the key bindings (video, sound, language, resource packs...).
    pub options: bool,
    /// The `key_*` lines of `options.txt`.
    pub keys: bool,
    /// `servers.dat`.
    pub servers: bool,
}

/// Lines that belong to the target: the format version (the game upgrades old options from it) and the
/// resource packs, which name files only the source has.
fn kept_by_target(key: &str) -> bool {
    matches!(key, "version" | "resourcePacks" | "incompatibleResourcePacks")
}

fn is_key(key: &str) -> bool {
    key.starts_with("key_")
}

fn parse(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect()
}

/// `target`'s options with the chosen part of `source` laid over them. Keys `source` doesn't have stay as
/// they are; keys only `source` has are added at the end.
pub fn merge_options(target: &str, source: &str, options: bool, keys: bool) -> String {
    let wanted = |k: &str| !kept_by_target(k) && if is_key(k) { keys } else { options };
    let from: Vec<(String, String)> = parse(source).into_iter().filter(|(k, _)| wanted(k)).collect();
    let mut out: Vec<(String, String)> = parse(target);
    for (k, v) in from {
        match out.iter_mut().find(|(ok, _)| *ok == k) {
            Some(slot) => slot.1 = v,
            None => out.push((k, v)),
        }
    }
    let mut text: String = out.into_iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    if text.is_empty() {
        text.push('\n');
    }
    text
}

/// Copies options, key bindings and/or the server list from one game folder to another.
/// Returns what was copied, so the caller can say when the source had nothing.
pub fn copy_options(source_game: &Path, target_game: &Path, what: OptionsCopy) -> Result<OptionsCopy> {
    let mut done = OptionsCopy::default();
    if what.options || what.keys {
        let src = source_game.join("options.txt");
        if let Ok(source) = fs::read_to_string(&src) {
            let dest = target_game.join("options.txt");
            let target = fs::read_to_string(&dest).unwrap_or_default();
            fs::create_dir_all(target_game).map_err(|e| Error::io(target_game, e))?;
            atomic_write(&dest, merge_options(&target, &source, what.options, what.keys).as_bytes())?;
            done.options = what.options;
            done.keys = what.keys;
        }
    }
    if what.servers {
        let src = source_game.join("servers.dat");
        if src.is_file() {
            fs::create_dir_all(target_game).map_err(|e| Error::io(target_game, e))?;
            let data = fs::read(&src).map_err(|e| Error::io(&src, e))?;
            atomic_write(&target_game.join("servers.dat"), &data)?;
            done.servers = true;
        }
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    const SOURCE: &str = "version:3955\nfov:1.0\nguiScale:3\nresourcePacks:[\"file/Faithful.zip\"]\nkey_key.jump:key.keyboard.space\nkey_key.sprint:key.keyboard.left.control\nkey_key.zoom:key.keyboard.c\n";
    const TARGET: &str = "version:3700\nfov:0.0\nresourcePacks:[\"vanilla\"]\nkey_key.jump:key.keyboard.j\nrenderDistance:8\n";

    #[test]
    fn only_keys() {
        let out = merge_options(TARGET, SOURCE, false, true);
        assert_eq!(
            out,
            "version:3700\nfov:0.0\nresourcePacks:[\"vanilla\"]\nkey_key.jump:key.keyboard.space\nrenderDistance:8\nkey_key.sprint:key.keyboard.left.control\nkey_key.zoom:key.keyboard.c\n"
        );
    }

    #[test]
    fn only_options_keeps_version_and_packs() {
        let out = merge_options(TARGET, SOURCE, true, false);
        assert!(out.starts_with("version:3700\nfov:1.0\nresourcePacks:[\"vanilla\"]\nkey_key.jump:key.keyboard.j\n"), "{out}");
        assert!(out.contains("guiScale:3\n"));
        assert!(!out.contains("key.keyboard.space"));
    }

    #[test]
    fn copies_files() {
        let dir = temp_dir("options-copy");
        let (a, b) = (dir.join("a"), dir.join("b"));
        fs::create_dir_all(&a).unwrap();
        fs::write(a.join("options.txt"), SOURCE).unwrap();
        fs::write(a.join("servers.dat"), [10u8, 0, 0, 0]).unwrap();
        let all = OptionsCopy { options: true, keys: true, servers: true };
        assert_eq!(copy_options(&a, &b, all).unwrap(), all);
        assert!(fs::read_to_string(b.join("options.txt")).unwrap().contains("key_key.zoom"));
        assert_eq!(fs::read(b.join("servers.dat")).unwrap(), [10, 0, 0, 0]);
        // A source without options copies nothing.
        assert_eq!(copy_options(&dir.join("empty"), &b, all).unwrap(), OptionsCopy::default());
    }
}

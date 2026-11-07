//! The default skins of Minecraft (Steve, Alex and the seven characters added in 1.19.3), read from a client
//! jar Cubby already downloaded: nothing is bundled with Cubby, and the textures are the game's own.

use std::fs;
use std::io::Read;
use std::path::Path;

use cubby_common::{Error, Result};
use serde::Serialize;

/// The characters, in the order the game lists them.
const CHARACTERS: [(&str, &str); 9] = [
    ("steve", "Steve"),
    ("alex", "Alex"),
    ("sunny", "Sunny"),
    ("noor", "Noor"),
    ("makena", "Makena"),
    ("kai", "Kai"),
    ("efe", "Efe"),
    ("zuri", "Zuri"),
    ("ari", "Ari"),
];

/// One default skin, in both arm models (`wide` = classic, `slim`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DefaultSkin {
    pub id: String,
    pub name: String,
    pub wide: Vec<u8>,
    pub slim: Vec<u8>,
}

fn read_entry(zip: &mut zip::ZipArchive<fs::File>, name: &str) -> Option<Vec<u8>> {
    let entry = zip.by_name(name).ok()?;
    if entry.size() > 64 * 1024 {
        return None;
    }
    let mut buf = Vec::new();
    entry.take(64 * 1024).read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// The default skins found in one client jar, or `None` when it has none (versions before 1.19.3 only have
/// Steve and Alex, in another place).
pub fn read_default_skins(jar: &Path) -> Option<Vec<DefaultSkin>> {
    let mut zip = zip::ZipArchive::new(fs::File::open(jar).ok()?).ok()?;
    let mut out = Vec::new();
    for (id, name) in CHARACTERS {
        let wide = read_entry(&mut zip, &format!("assets/minecraft/textures/entity/player/wide/{id}.png"))?;
        let slim = read_entry(&mut zip, &format!("assets/minecraft/textures/entity/player/slim/{id}.png"))?;
        out.push(DefaultSkin { id: id.to_string(), name: name.to_string(), wide, slim });
    }
    Some(out)
}

/// The default skins, from the newest client jar in `versions_dir` that has them (`versions/<id>/<id>.jar`).
pub fn default_skins(versions_dir: &Path) -> Result<Vec<DefaultSkin>> {
    let mut jars: Vec<_> = fs::read_dir(versions_dir)
        .map_err(|e| Error::io(versions_dir, e))?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            let jar = e.path().join(format!("{id}.jar"));
            jar.is_file().then_some((e.metadata().and_then(|m| m.modified()).ok(), jar))
        })
        .collect();
    // newest first: a recent version has the nine characters
    jars.sort_by_key(|a| std::cmp::Reverse(a.0));
    jars.iter()
        .find_map(|(_, jar)| read_default_skins(jar))
        .ok_or_else(|| Error::Other("Cubby reads the default skins from the game files. Create an instance and start it once, then come back.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;
    use std::io::Write;

    fn jar_with(dir: &Path, id: &str, characters: &[&str]) {
        let folder = dir.join(id);
        fs::create_dir_all(&folder).unwrap();
        let mut zip = zip::ZipWriter::new(fs::File::create(folder.join(format!("{id}.jar"))).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        for c in characters {
            for model in ["wide", "slim"] {
                zip.start_file(format!("assets/minecraft/textures/entity/player/{model}/{c}.png"), opts).unwrap();
                zip.write_all(format!("{model}-{c}").as_bytes()).unwrap();
            }
        }
        zip.finish().unwrap();
    }

    #[test]
    fn reads_the_nine_characters_from_a_jar() {
        let dir = temp_dir("default-skins");
        let all: Vec<&str> = CHARACTERS.iter().map(|(id, _)| *id).collect();
        jar_with(&dir, "26.3", &all);
        let skins = default_skins(&dir).unwrap();
        assert_eq!(skins.len(), 9);
        assert_eq!((skins[0].id.as_str(), skins[0].name.as_str()), ("steve", "Steve"));
        assert_eq!(skins[1].wide, b"wide-alex");
        assert_eq!(skins[1].slim, b"slim-alex");
    }

    #[test]
    fn an_old_jar_without_them_is_skipped_and_none_is_an_error() {
        let dir = temp_dir("default-skins-old");
        jar_with(&dir, "1.8.9", &["steve", "alex"]);
        let err = default_skins(&dir).unwrap_err().to_string();
        assert!(err.contains("start it once"), "{err}");
        let all: Vec<&str> = CHARACTERS.iter().map(|(id, _)| *id).collect();
        jar_with(&dir, "1.21.1", &all);
        assert_eq!(default_skins(&dir).unwrap().len(), 9);
        assert!(default_skins(&dir.join("missing")).is_err());
    }

    /// With CUBBY_TEST_JAR=<a recent client jar>: the real textures are PNGs of 64×64.
    #[test]
    #[ignore = "needs a real client jar"]
    fn real_jar() {
        let jar = std::env::var("CUBBY_TEST_JAR").expect("set CUBBY_TEST_JAR");
        let skins = read_default_skins(Path::new(&jar)).expect("a jar with the default skins");
        assert_eq!(skins.len(), 9);
        for s in &skins {
            assert_eq!(&s.wide[..4], &[0x89, b'P', b'N', b'G']);
            assert_eq!(&s.slim[..4], &[0x89, b'P', b'N', b'G']);
        }
    }
}

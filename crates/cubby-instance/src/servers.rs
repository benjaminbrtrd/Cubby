//! The game's multiplayer server list (`servers.dat`, uncompressed NBT).

use std::path::Path;

use serde::{Deserialize, Serialize};
use cubby_common::Result;

use crate::nbt::{self, Tag};
use crate::util::atomic_write;

/// One entry of the multiplayer list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerEntry {
    pub name: String,
    pub address: String,
    /// Base64 PNG icon.
    pub icon: Option<String>,
    /// `Some(true)` accept server resource packs, `Some(false)` refuse, `None` ask.
    pub accept_textures: Option<bool>,
}

/// Reads `<game_dir>/servers.dat`; a missing or corrupt file yields an empty list.
pub fn read_servers(game_dir: &Path) -> Vec<ServerEntry> {
    let Some(root) = nbt::read_file(&game_dir.join("servers.dat")) else { return Vec::new() };
    let Some(Tag::List(items)) = root.get("servers") else { return Vec::new() };
    items
        .iter()
        .filter_map(|t| {
            Some(ServerEntry {
                name: t.get("name").and_then(Tag::as_str).unwrap_or_default().to_string(),
                address: t.get("ip").and_then(Tag::as_str)?.to_string(),
                icon: t.get("icon").and_then(Tag::as_str).filter(|s| !s.is_empty()).map(String::from),
                accept_textures: t.get("acceptTextures").and_then(Tag::as_i64).map(|v| v != 0),
            })
        })
        .collect()
}

/// Writes `<game_dir>/servers.dat` (uncompressed NBT) atomically, in the given order.
pub fn write_servers(game_dir: &Path, servers: &[ServerEntry]) -> Result<()> {
    let list = servers
        .iter()
        .map(|s| {
            let mut c = vec![
                ("name".to_string(), Tag::String(s.name.clone())),
                ("ip".to_string(), Tag::String(s.address.clone())),
            ];
            if let Some(icon) = &s.icon {
                c.push(("icon".into(), Tag::String(icon.clone())));
            }
            if let Some(a) = s.accept_textures {
                c.push(("acceptTextures".into(), Tag::Byte(a as i8)));
            }
            Tag::Compound(c)
        })
        .collect();
    let root = Tag::Compound(vec![("servers".into(), Tag::List(list))]);
    atomic_write(&game_dir.join("servers.dat"), &nbt::write_root(&root))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    fn e(name: &str, ip: &str, icon: Option<&str>, a: Option<bool>) -> ServerEntry {
        ServerEntry { name: name.into(), address: ip.into(), icon: icon.map(String::from), accept_textures: a }
    }

    #[test]
    fn round_trip() {
        let game = temp_dir("servers");
        let list = vec![
            e("Hypixel", "mc.hypixel.net", Some("iVBOR"), Some(true)),
            e("Local é", "localhost:25566", None, Some(false)),
            e("X", "x.y", None, None),
        ];
        write_servers(&game, &list).unwrap();
        assert_eq!(read_servers(&game), list);
        write_servers(&game, &[]).unwrap();
        assert!(read_servers(&game).is_empty());
    }

    #[test]
    fn reads_extra_tags() {
        let game = temp_dir("servers-extra");
        let root = Tag::Compound(vec![
            ("other".into(), Tag::Int(3)),
            (
                "servers".into(),
                Tag::List(vec![Tag::Compound(vec![
                    ("hidden".into(), Tag::Byte(0)),
                    ("ip".into(), Tag::String("a.b".into())),
                    ("name".into(), Tag::String("A".into())),
                    ("acceptTextures".into(), Tag::Byte(1)),
                ])]),
            ),
        ]);
        std::fs::write(game.join("servers.dat"), nbt::write_root(&root)).unwrap();
        assert_eq!(read_servers(&game), vec![e("A", "a.b", None, Some(true))]);
    }

    #[test]
    fn missing_or_corrupt() {
        let game = temp_dir("servers-none");
        assert!(read_servers(&game).is_empty());
        std::fs::write(game.join("servers.dat"), "junk").unwrap();
        assert!(read_servers(&game).is_empty());
    }
}

//! Compiles in the CurseForge API key of this build, if one is given, masked so it doesn't show up as
//! plain text in the executable (`strings Cubby.exe`). Masking is not encryption: anyone determined can
//! still recover it from the binary, as with every launcher that ships a key.
//!
//! The key comes from `CUBBY_CURSEFORGE_API_KEY` at build time, else from
//! `<workspace>/secrets/curseforge-api-key.txt` (git-ignored).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::PathBuf;

fn main() {
    let file = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../secrets/curseforge-api-key.txt");
    println!("cargo:rerun-if-env-changed=CUBBY_CURSEFORGE_API_KEY");
    println!("cargo:rerun-if-changed={}", file.display());

    let key = std::env::var("CUBBY_CURSEFORGE_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .or_else(|| std::fs::read_to_string(&file).ok())
        .map(|k| k.trim().to_string())
        .unwrap_or_default();

    // a fresh random mask for every build
    let mask: Vec<u8> = (0..key.len().max(1))
        .map(|i| {
            let mut h = RandomState::new().build_hasher();
            h.write_usize(i);
            (h.finish() & 0xff) as u8
        })
        .collect();
    let masked: Vec<u8> = key.bytes().zip(&mask).map(|(b, m)| b ^ m).collect();

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("builtin_key.rs");
    std::fs::write(out, format!("const BUILTIN_MASK: &[u8] = &{mask:?};\nconst BUILTIN_KEY: &[u8] = &{masked:?};\n")).unwrap();
}

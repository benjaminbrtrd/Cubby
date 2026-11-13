//! How much memory to give the game when the player lets Cubby choose.

use std::path::Path;

const STEP_MB: u32 = 512;

/// Number of mods the game will load: `.jar` files in `<game dir>/mods` (turned-off `.jar.disabled` don't count).
pub fn count_enabled_mods(game_dir: &Path) -> usize {
    let Ok(rd) = std::fs::read_dir(game_dir.join("mods")) else {
        return 0;
    };
    rd.flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .to_lowercase()
                .ends_with(".jar")
        })
        .count()
}

/// Maximum heap for an instance with `mods` mods on a computer with `total_mb` of memory.
///
/// The steps follow what modpack authors usually ask for: Vanilla runs well in 2 GB, a few dozen mods
/// in 3 to 4 GB, a big pack in 6 to 8 GB. More than that only makes garbage collection pauses longer.
/// The game never gets more than half of the computer's memory, so the system and Cubby keep enough.
pub fn recommended_memory_mb(total_mb: u64, mods: usize) -> u32 {
    let wanted: u32 = match mods {
        0 => 2048,
        1..=30 => 3072,
        31..=100 => 4096,
        101..=200 => 6144,
        _ => 8192,
    };
    if total_mb == 0 {
        return wanted;
    }
    let half = u32::try_from(total_mb / 2).unwrap_or(u32::MAX);
    let cap = (half / STEP_MB * STEP_MB).max(1024);
    wanted.min(cap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    #[test]
    fn grows_with_mods_and_stays_under_half() {
        assert_eq!(recommended_memory_mb(16_384, 0), 2048);
        assert_eq!(recommended_memory_mb(16_384, 12), 3072);
        assert_eq!(recommended_memory_mb(16_384, 80), 4096);
        assert_eq!(recommended_memory_mb(16_384, 150), 6144);
        assert_eq!(recommended_memory_mb(32_768, 400), 8192);
        assert_eq!(recommended_memory_mb(8_192, 400), 4096);
        assert_eq!(recommended_memory_mb(6_000, 400), 2560);
        assert_eq!(recommended_memory_mb(1_500, 0), 1024);
        assert_eq!(recommended_memory_mb(0, 150), 6144);
    }

    #[test]
    fn counts_enabled_jars_only() {
        let dir = temp_dir("memory-mods");
        let mods = dir.join("mods");
        std::fs::create_dir_all(mods.join("sub.jar")).unwrap();
        for name in ["a.jar", "B.JAR", "c.jar.disabled", "readme.txt"] {
            std::fs::write(mods.join(name), "x").unwrap();
        }
        assert_eq!(count_enabled_mods(&dir), 2);
        assert_eq!(count_enabled_mods(&dir.join("nope")), 0);
    }
}

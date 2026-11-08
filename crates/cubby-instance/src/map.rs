//! A top-down map of a world around its spawn, read from the region files (`region/r.X.Z.mca`), as a PNG.
//!
//! Each pixel is the top block of a column (from the chunk's `WORLD_SURFACE` heightmap), coloured by block name
//! and shaded by height like the game's own maps. Worlds from Minecraft 1.13 on are read (block palettes);
//! older ones have numeric block ids and give an error. Ungenerated chunks stay transparent.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use cubby_common::{Error, Result};
use serde::Serialize;

use crate::nbt::{self, Tag};
use crate::util::check_plain_name;

/// Blocks on each side of the spawn: the map is twice this wide.
pub const MAP_RADIUS: i32 = 384;
/// The chunk format with sections at the root and `block_states` (1.18).
const DATA_VERSION_1_18: i64 = 2844;
/// Heightmaps and block states stopped spanning two longs (1.16).
const DATA_VERSION_1_16: i64 = 2526;
const MAX_CHUNK_BYTES: usize = 8 * 1024 * 1024;

/// A rendered map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldMap {
    pub path: PathBuf,
    /// Block coordinates of the top-left pixel.
    pub min_x: i32,
    pub min_z: i32,
    pub size: u32,
    /// Chunks drawn (the others aren't generated yet).
    pub chunks: u32,
}

/// RGB of a block, by its name without `minecraft:`.
fn block_color(name: &str) -> [u8; 3] {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    let exact: Option<[u8; 3]> = match n {
        "grass_block" => Some([118, 168, 70]),
        "water" | "bubble_column" => Some([52, 92, 196]),
        "lava" => Some([220, 100, 20]),
        "sand" | "sandstone" | "cut_sandstone" | "smooth_sandstone" => Some([219, 207, 163]),
        "red_sand" | "red_sandstone" => Some([190, 102, 33]),
        "gravel" => Some([136, 126, 126]),
        "dirt" | "coarse_dirt" | "rooted_dirt" | "dirt_path" | "farmland" => Some([134, 96, 67]),
        "podzol" => Some([91, 63, 24]),
        "mycelium" => Some([111, 99, 105]),
        "snow" | "snow_block" | "powder_snow" => Some([248, 254, 254]),
        "ice" | "packed_ice" | "blue_ice" | "frosted_ice" => Some([160, 188, 255]),
        "clay" => Some([160, 166, 179]),
        "mud" | "mangrove_roots" | "muddy_mangrove_roots" => Some([60, 57, 61]),
        "moss_block" | "moss_carpet" => Some([89, 109, 45]),
        "terracotta" => Some([152, 94, 67]),
        "netherrack" => Some([111, 54, 52]),
        "end_stone" => Some([219, 222, 158]),
        "obsidian" | "crying_obsidian" => Some([20, 18, 29]),
        "bedrock" => Some([85, 85, 85]),
        "pumpkin" | "carved_pumpkin" => Some([198, 118, 24]),
        "melon" => Some([111, 145, 30]),
        "cactus" => Some([87, 130, 41]),
        "sugar_cane" | "bamboo" | "kelp" | "kelp_plant" | "seagrass" | "tall_seagrass" => Some([96, 150, 60]),
        "lily_pad" => Some([32, 128, 48]),
        _ => None,
    };
    if let Some(c) = exact {
        return c;
    }
    let has = |s: &str| n.contains(s);
    if has("leaves") || has("vine") || has("azalea") {
        return [55, 110, 35];
    }
    if has("short_grass") || n == "grass" || has("tall_grass") || has("fern") || has("bush") || has("sapling") {
        return [100, 155, 60];
    }
    if has("flower") || has("tulip") || has("poppy") || has("dandelion") || has("orchid") || has("allium") || has("daisy") || has("rose") {
        return [180, 160, 70];
    }
    if has("cherry") {
        return [228, 170, 190];
    }
    if has("birch") {
        return [216, 202, 145];
    }
    if has("spruce") || has("dark_oak") || has("mangrove") {
        return [102, 76, 50];
    }
    if has("log") || has("wood") || has("planks") || has("stairs") && has("oak") || has("fence") || has("door") {
        return [143, 119, 72];
    }
    if has("deepslate") || has("blackstone") || has("basalt") {
        return [72, 72, 78];
    }
    if has("stone") || has("cobble") || has("andesite") || has("tuff") || has("ore") {
        return [125, 125, 125];
    }
    if has("granite") {
        return [149, 103, 85];
    }
    if has("diorite") || has("calcite") || has("quartz") {
        return [222, 222, 214];
    }
    if has("brick") {
        return [150, 74, 60];
    }
    if has("glass") {
        return [200, 220, 230];
    }
    if has("wool") || has("carpet") || has("concrete") || has("terracotta") {
        return [200, 200, 200];
    }
    if has("snow") {
        return [248, 254, 254];
    }
    if has("nylium") || has("wart") {
        return [130, 30, 40];
    }
    if has("coral") {
        return [200, 80, 140];
    }
    [140, 140, 120]
}

/// Values of `count` entries of `bits` bits packed in longs: spanning two longs (before 1.16) or not.
fn unpack(longs: &[i64], bits: usize, count: usize, spanning: bool) -> Option<Vec<u32>> {
    if bits == 0 || bits > 32 {
        return None;
    }
    let mask = (1u64 << bits) - 1;
    let mut out = Vec::with_capacity(count);
    if spanning {
        for i in 0..count {
            let bit = i * bits;
            let (idx, off) = (bit / 64, bit % 64);
            let mut v = (*longs.get(idx)? as u64) >> off;
            if off + bits > 64 {
                v |= (*longs.get(idx + 1)? as u64) << (64 - off);
            }
            out.push((v & mask) as u32);
        }
    } else {
        let per = 64 / bits;
        for i in 0..count {
            let long = *longs.get(i / per)? as u64;
            out.push(((long >> ((i % per) * bits)) & mask) as u32);
        }
    }
    Some(out)
}

/// The top blocks of one chunk: 256 (name, y), x fastest, `None` where nothing is there.
fn chunk_surface(chunk: &Tag) -> Result<Vec<Option<(String, i32)>>> {
    let version = chunk.get("DataVersion").and_then(Tag::as_i64).unwrap_or(0);
    if version < 1451 {
        return Err(Error::Other("This world is from before Minecraft 1.13: its map can't be drawn yet.".into()));
    }
    let level = if version >= DATA_VERSION_1_18 { chunk } else { chunk.get("Level").unwrap_or(chunk) };
    let spanning = version < DATA_VERSION_1_16;
    let min_y = if version >= DATA_VERSION_1_18 { -64 } else { 0 };
    let none = || vec![None; 256];
    let Some(heights) = level.get("Heightmaps").and_then(|h| h.get("WORLD_SURFACE").or_else(|| h.get("MOTION_BLOCKING"))).and_then(Tag::as_longs) else {
        return Ok(none());
    };
    // 9 bits since 1.18 (384 blocks), 9 before too (256 + 1 values).
    let Some(heights) = unpack(heights, 9, 256, spanning) else { return Ok(none()) };

    // Sections by Y: (palette names, block indices), decoded when a column needs them.
    let sections_tag = level.get("sections").or_else(|| level.get("Sections")).and_then(Tag::as_list).unwrap_or(&[]);
    let mut sections: HashMap<i32, (&Tag, Option<Vec<u32>>)> = HashMap::new();
    for s in sections_tag {
        let Some(y) = s.get("Y").and_then(Tag::as_i64) else { continue };
        sections.insert(y as i32, (s, None));
    }
    let mut out = Vec::with_capacity(256);
    for (i, h) in heights.iter().enumerate() {
        if *h == 0 {
            out.push(None);
            continue;
        }
        let y = min_y + *h as i32 - 1;
        let sy = y.div_euclid(16);
        let Some((section, decoded)) = sections.get_mut(&sy) else {
            out.push(None);
            continue;
        };
        let (palette, data) = if version >= DATA_VERSION_1_18 {
            let states = section.get("block_states");
            (states.and_then(|s| s.get("palette")).and_then(Tag::as_list), states.and_then(|s| s.get("data")).and_then(Tag::as_longs))
        } else {
            (section.get("Palette").and_then(Tag::as_list), section.get("BlockStates").and_then(Tag::as_longs))
        };
        let Some(palette) = palette.filter(|p| !p.is_empty()) else {
            out.push(None);
            continue;
        };
        let index = if palette.len() == 1 {
            0
        } else {
            if decoded.is_none() {
                let bits = (usize::BITS - (palette.len() - 1).leading_zeros()).max(4) as usize;
                *decoded = data.and_then(|d| unpack(d, bits, 4096, spanning));
            }
            let (x, z, ly) = (i % 16, i / 16, y.rem_euclid(16) as usize);
            decoded.as_ref().and_then(|d| d.get(ly * 256 + z * 16 + x)).copied().unwrap_or(0) as usize
        };
        let name = palette.get(index).and_then(|p| p.get("Name")).and_then(Tag::as_str).unwrap_or("minecraft:air");
        out.push((!name.ends_with(":air") && !name.ends_with("cave_air")).then(|| (name.to_string(), y)));
    }
    Ok(out)
}

/// The NBT of the chunk at (`cx`, `cz`) inside a region file (coordinates within the region, 0..32).
fn read_chunk(file: &mut fs::File, cx: usize, cz: usize) -> Option<Tag> {
    let mut header = [0u8; 4];
    file.seek(SeekFrom::Start(((cx + cz * 32) * 4) as u64)).ok()?;
    file.read_exact(&mut header).ok()?;
    let offset = u32::from_be_bytes([0, header[0], header[1], header[2]]) as u64 * 4096;
    if offset == 0 {
        return None;
    }
    let mut head = [0u8; 5];
    file.seek(SeekFrom::Start(offset)).ok()?;
    file.read_exact(&mut head).ok()?;
    let len = (u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as usize).checked_sub(1)?;
    if len > MAX_CHUNK_BYTES {
        return None;
    }
    let mut raw = vec![0u8; len];
    file.read_exact(&mut raw).ok()?;
    let mut data = Vec::new();
    match head[4] {
        1 => flate2::read::GzDecoder::new(&raw[..]).take(32 * 1024 * 1024).read_to_end(&mut data).ok()?,
        2 => flate2::read::ZlibDecoder::new(&raw[..]).take(32 * 1024 * 1024).read_to_end(&mut data).ok()?,
        3 => {
            data = raw;
            data.len()
        }
        _ => return None,
    };
    nbt::parse_root(&data)
}

/// The spawn point of a world, from `level.dat` (0, 0 when it can't be read).
fn spawn_of(world: &Path) -> (i32, i32) {
    let data = nbt::read_file(&world.join("level.dat")).and_then(|r| r.get("Data").cloned());
    let get = |k: &str| data.as_ref().and_then(|d| d.get(k)).and_then(Tag::as_i64).unwrap_or(0) as i32;
    (get("SpawnX"), get("SpawnZ"))
}

fn region_files(world: &Path, min: (i32, i32), max: (i32, i32)) -> Vec<(i32, i32, PathBuf)> {
    let mut out = Vec::new();
    for rz in min.1.div_euclid(512)..=max.1.div_euclid(512) {
        for rx in min.0.div_euclid(512)..=max.0.div_euclid(512) {
            let path = world.join("region").join(format!("r.{rx}.{rz}.mca"));
            if path.is_file() {
                out.push((rx, rz, path));
            }
        }
    }
    out
}

fn newest_change(files: &[(i32, i32, PathBuf)]) -> Option<SystemTime> {
    files.iter().filter_map(|(_, _, p)| fs::metadata(p).and_then(|m| m.modified()).ok()).max()
}

/// Draws the map of `saves/<world>` around its spawn into `cache_dir/<world>.png`, unless that file is newer than
/// the region files. `on_progress(done, total)` counts region files.
pub fn render_map(game_dir: &Path, world: &str, cache_dir: &Path, mut on_progress: impl FnMut(u64, u64)) -> Result<WorldMap> {
    check_plain_name(world)?;
    let dir = game_dir.join("saves").join(world);
    if !dir.is_dir() {
        return Err(Error::NotFound(format!("world {world}")));
    }
    let (sx, sz) = spawn_of(&dir);
    // Snap to chunks, so the map lines up with them.
    let (min_x, min_z) = ((sx - MAP_RADIUS).div_euclid(16) * 16, (sz - MAP_RADIUS).div_euclid(16) * 16);
    let size = (MAP_RADIUS * 2) as u32;
    let (max_x, max_z) = (min_x + size as i32 - 1, min_z + size as i32 - 1);
    let regions = region_files(&dir, (min_x, min_z), (max_x, max_z));
    if regions.is_empty() {
        return Err(Error::Other("This world has no map yet: play in it once, then draw it.".into()));
    }
    fs::create_dir_all(cache_dir).map_err(|e| Error::io(cache_dir, e))?;
    let out_path = cache_dir.join(format!("{world}.png"));
    let fresh = match (fs::metadata(&out_path).and_then(|m| m.modified()).ok(), newest_change(&regions)) {
        (Some(png), Some(regions)) => png >= regions,
        _ => false,
    };
    let meta_path = cache_dir.join(format!("{world}.json"));
    if fresh {
        if let Some(map) = fs::read(&meta_path).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()).and_then(|v| v.get("chunks")?.as_u64()) {
            return Ok(WorldMap { path: out_path, min_x, min_z, size, chunks: map as u32 });
        }
    }

    let (w, h) = (size as usize, size as usize);
    let mut heights = vec![i32::MIN; w * h];
    let mut colors = vec![[0u8; 3]; w * h];
    let mut chunks = 0u32;
    let total = regions.len() as u64;
    let mut error = None;
    for (n, (rx, rz, path)) in regions.iter().enumerate() {
        on_progress(n as u64, total);
        let Ok(mut file) = fs::File::open(path) else { continue };
        for cz in 0..32 {
            for cx in 0..32 {
                let (bx, bz) = (rx * 512 + cx as i32 * 16, rz * 512 + cz as i32 * 16);
                if bx + 15 < min_x || bz + 15 < min_z || bx > max_x || bz > max_z {
                    continue;
                }
                let Some(chunk) = read_chunk(&mut file, cx, cz) else { continue };
                let surface = match chunk_surface(&chunk) {
                    Ok(s) => s,
                    Err(e) => {
                        error.get_or_insert(e);
                        continue;
                    }
                };
                chunks += 1;
                for (i, top) in surface.into_iter().enumerate() {
                    let Some((name, y)) = top else { continue };
                    let (px, pz) = (bx + (i % 16) as i32 - min_x, bz + (i / 16) as i32 - min_z);
                    if px < 0 || pz < 0 || px >= w as i32 || pz >= h as i32 {
                        continue;
                    }
                    let at = pz as usize * w + px as usize;
                    heights[at] = y;
                    colors[at] = block_color(&name);
                }
            }
        }
    }
    on_progress(total, total);
    if chunks == 0 {
        return Err(error.unwrap_or_else(|| Error::Other("This world has no map yet: play in it once, then draw it.".into())));
    }

    // Shade by the height difference with the block to the north, like the game's maps.
    let mut rgba = vec![0u8; w * h * 4];
    for z in 0..h {
        for x in 0..w {
            let at = z * w + x;
            if heights[at] == i32::MIN {
                continue;
            }
            let north = if z > 0 && heights[at - w] != i32::MIN { heights[at - w] } else { heights[at] };
            let shade = match heights[at].cmp(&north) {
                std::cmp::Ordering::Greater => 1.12,
                std::cmp::Ordering::Less => 0.82,
                std::cmp::Ordering::Equal => 1.0,
            };
            let c = colors[at];
            for k in 0..3 {
                rgba[at * 4 + k] = (c[k] as f32 * shade).min(255.0) as u8;
            }
            rgba[at * 4 + 3] = 255;
        }
    }
    let tmp = out_path.with_extension("part");
    {
        let file = fs::File::create(&tmp).map_err(|e| Error::io(&tmp, e))?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let png_err = |e: png::EncodingError| Error::Other(format!("Couldn't write the map ({e})."));
        let mut writer = encoder.write_header().map_err(png_err)?;
        writer.write_image_data(&rgba).map_err(png_err)?;
        writer.finish().map_err(png_err)?;
    }
    fs::rename(&tmp, &out_path).map_err(|e| Error::io(&out_path, e))?;
    let _ = fs::write(&meta_path, format!("{{\"chunks\":{chunks}}}"));
    Ok(WorldMap { path: out_path, min_x, min_z, size, chunks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;
    use std::io::Write;

    fn pack(values: &[u32], bits: usize) -> Vec<i64> {
        let per = 64 / bits;
        let mut out = vec![0u64; values.len().div_ceil(per)];
        for (i, v) in values.iter().enumerate() {
            out[i / per] |= (*v as u64) << ((i % per) * bits);
        }
        out.into_iter().map(|v| v as i64).collect()
    }

    fn compound(items: Vec<(&str, Tag)>) -> Tag {
        Tag::Compound(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    /// A 1.21 chunk: grass at y=64 everywhere but one column of water at y=62.
    fn chunk() -> Tag {
        let mut heights = vec![64 + 64 + 1; 256];
        heights[17] = 62 + 64 + 1;
        // Section 4 holds y 64..79: index 0 grass on its bottom layer, 1 air elsewhere.
        let mut grass_layer = vec![1u32; 4096];
        grass_layer[..256].iter_mut().for_each(|v| *v = 0);
        let palette = |names: &[&str]| Tag::List(names.iter().map(|n| compound(vec![("Name", Tag::String(n.to_string()))])).collect());
        let mut water_layer = vec![0u32; 4096];
        water_layer[14 * 256 + 17] = 1;
        compound(vec![
            ("DataVersion", Tag::Int(3955)),
            ("Heightmaps", compound(vec![("WORLD_SURFACE", Tag::LongArray(pack(&heights, 9)))])),
            (
                "sections",
                Tag::List(vec![
                    compound(vec![
                        ("Y", Tag::Byte(4)),
                        ("block_states", compound(vec![("palette", palette(&["minecraft:grass_block", "minecraft:air"])), ("data", Tag::LongArray(pack(&grass_layer, 4)))])),
                    ]),
                    compound(vec![
                        ("Y", Tag::Byte(3)),
                        ("block_states", compound(vec![("palette", palette(&["minecraft:stone", "minecraft:water"])), ("data", Tag::LongArray(pack(&water_layer, 4)))])),
                    ]),
                ]),
            ),
        ])
    }

    #[test]
    fn surface_of_a_chunk() {
        let surface = chunk_surface(&chunk()).unwrap();
        assert_eq!(surface[0], Some(("minecraft:grass_block".to_string(), 64)));
        assert_eq!(surface[17], Some(("minecraft:water".to_string(), 62)));
        assert!(chunk_surface(&compound(vec![("DataVersion", Tag::Int(100))])).is_err());
    }

    #[test]
    fn unpacks_both_ways() {
        let values: Vec<u32> = (0..256).map(|i| i % 300).collect();
        assert_eq!(unpack(&pack(&values, 9), 9, 256, false).unwrap(), values);
        // Spanning: 9-bit values laid end to end.
        let mut longs = vec![0u64; (256 * 9usize).div_ceil(64)];
        for (i, v) in values.iter().enumerate() {
            let bit = i * 9;
            longs[bit / 64] |= (*v as u64) << (bit % 64);
            if bit % 64 + 9 > 64 {
                longs[bit / 64 + 1] |= (*v as u64) >> (64 - bit % 64);
            }
        }
        let longs: Vec<i64> = longs.into_iter().map(|v| v as i64).collect();
        assert_eq!(unpack(&longs, 9, 256, true).unwrap(), values);
    }

    #[test]
    fn renders_a_region() {
        let game = temp_dir("map-render");
        let world = game.join("saves/World");
        fs::create_dir_all(world.join("region")).unwrap();
        // One chunk at (0, 0) in r.0.0.mca, zlib-compressed.
        let nbt = nbt::write_root(&chunk());
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        z.write_all(&nbt).unwrap();
        let body = z.finish().unwrap();
        let mut region = vec![0u8; 8192];
        region[0..4].copy_from_slice(&[0, 0, 2, 1]);
        region.extend(((body.len() + 1) as u32).to_be_bytes());
        region.push(2);
        region.extend(&body);
        region.resize(region.len().div_ceil(4096) * 4096, 0);
        fs::write(world.join("region/r.0.0.mca"), region).unwrap();

        let cache = game.join("maps");
        let map = render_map(&game, "World", &cache, |_, _| {}).unwrap();
        assert_eq!((map.chunks, map.size), (1, 768));
        assert_eq!((map.min_x, map.min_z), (-384, -384));
        let decoder = png::Decoder::new(std::io::BufReader::new(fs::File::open(&map.path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut buf).unwrap();
        let px = |x: usize, z: usize| &buf[(z * 768 + x) * 4..(z * 768 + x) * 4 + 4];
        assert_eq!(px(384, 384)[3], 255, "block (0, 0) is drawn");
        assert_eq!(px(0, 0)[3], 0, "ungenerated chunks stay transparent");
        // The second call reads the cache.
        assert_eq!(render_map(&game, "World", &cache, |_, _| {}).unwrap().chunks, 1);
        assert!(render_map(&game, "Nope", &cache, |_, _| {}).is_err());
    }
}

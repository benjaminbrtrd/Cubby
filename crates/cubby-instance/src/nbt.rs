//! A tiny, defensive NBT reader, just enough to read `level.dat`.

use std::io::Read;
use std::path::Path;

const MAX_DEPTH: u32 = 32;
const MAX_UNCOMPRESSED: u64 = 32 * 1024 * 1024;

/// A parsed NBT tag. Array payloads are skipped, only their presence is kept.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    List(Vec<Tag>),
    Compound(Vec<(String, Tag)>),
    /// Byte, int or long array.
    Array,
}

impl Tag {
    pub(crate) fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Tag::String(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn as_i64(&self) -> Option<i64> {
        match self {
            Tag::Byte(v) => Some(*v as i64),
            Tag::Short(v) => Some(*v as i64),
            Tag::Int(v) => Some(*v as i64),
            Tag::Long(v) => Some(*v),
            _ => None,
        }
    }
}

/// Reads a (usually gzip-compressed) NBT file and returns its root compound.
pub(crate) fn read_file(path: &Path) -> Option<Tag> {
    let file = std::fs::File::open(path).ok()?;
    let mut raw = Vec::new();
    file.take(MAX_UNCOMPRESSED).read_to_end(&mut raw).ok()?;
    let data = if raw.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(&raw[..]).take(MAX_UNCOMPRESSED).read_to_end(&mut out).ok()?;
        out
    } else {
        raw
    };
    parse_root(&data)
}

/// Parses uncompressed NBT bytes: a named root compound.
pub(crate) fn parse_root(data: &[u8]) -> Option<Tag> {
    let mut r = Reader { data, pos: 0 };
    if r.u8()? != 10 {
        return None;
    }
    r.string()?;
    r.payload(10, 0)
}

/// Serialises `root` (which must be a compound) as uncompressed NBT with an empty root name.
///
/// Strings are written as plain UTF-8, which equals Java's modified UTF-8 for text without NUL or
/// supplementary characters; longer strings are truncated to the 65535-byte limit on a char boundary.
/// Arrays (which the reader does not keep) cannot be written and are emitted as empty arrays.
pub(crate) fn write_root(root: &Tag) -> Vec<u8> {
    let mut out = vec![10];
    write_name(&mut out, "");
    write_payload(&mut out, root);
    out
}

fn tag_id(tag: &Tag) -> u8 {
    match tag {
        Tag::Byte(_) => 1,
        Tag::Short(_) => 2,
        Tag::Int(_) => 3,
        Tag::Long(_) => 4,
        Tag::Float(_) => 5,
        Tag::Double(_) => 6,
        Tag::Array => 7,
        Tag::String(_) => 8,
        Tag::List(_) => 9,
        Tag::Compound(_) => 10,
    }
}

fn write_name(out: &mut Vec<u8>, s: &str) {
    let mut end = s.len().min(u16::MAX as usize);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    out.extend((end as u16).to_be_bytes());
    out.extend(&s.as_bytes()[..end]);
}

fn write_payload(out: &mut Vec<u8>, tag: &Tag) {
    match tag {
        Tag::Byte(v) => out.push(*v as u8),
        Tag::Short(v) => out.extend(v.to_be_bytes()),
        Tag::Int(v) => out.extend(v.to_be_bytes()),
        Tag::Long(v) => out.extend(v.to_be_bytes()),
        Tag::Float(v) => out.extend(v.to_be_bytes()),
        Tag::Double(v) => out.extend(v.to_be_bytes()),
        Tag::Array => out.extend(0i32.to_be_bytes()),
        Tag::String(s) => write_name(out, s),
        Tag::List(items) => {
            out.push(items.first().map(tag_id).unwrap_or(0));
            out.extend((items.len() as i32).to_be_bytes());
            for item in items {
                write_payload(out, item);
            }
        }
        Tag::Compound(items) => {
            for (k, v) in items {
                out.push(tag_id(v));
                write_name(out, k);
                write_payload(out, v);
            }
            out.push(0);
        }
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let end = self.pos.checked_add(n)?;
        let s = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(s)
    }

    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    fn len(&mut self) -> Option<usize> {
        let n = i32::from_be_bytes(self.array()?);
        usize::try_from(n).ok()
    }

    fn string(&mut self) -> Option<String> {
        let n = u16::from_be_bytes(self.array()?) as usize;
        Some(String::from_utf8_lossy(self.take(n)?).into_owned())
    }

    fn payload(&mut self, ty: u8, depth: u32) -> Option<Tag> {
        if depth > MAX_DEPTH {
            return None;
        }
        Some(match ty {
            1 => Tag::Byte(self.u8()? as i8),
            2 => Tag::Short(i16::from_be_bytes(self.array()?)),
            3 => Tag::Int(i32::from_be_bytes(self.array()?)),
            4 => Tag::Long(i64::from_be_bytes(self.array()?)),
            5 => Tag::Float(f32::from_be_bytes(self.array()?)),
            6 => Tag::Double(f64::from_be_bytes(self.array()?)),
            7 => {
                let n = self.len()?;
                self.take(n)?;
                Tag::Array
            }
            8 => Tag::String(self.string()?),
            9 => {
                let elem = self.u8()?;
                let n = self.len()?;
                if n > self.data.len() - self.pos {
                    return None;
                }
                let mut items = Vec::new();
                if elem == 0 {
                    return Some(Tag::List(items));
                }
                for _ in 0..n {
                    items.push(self.payload(elem, depth + 1)?);
                }
                Tag::List(items)
            }
            10 => {
                let mut items = Vec::new();
                loop {
                    let t = self.u8()?;
                    if t == 0 {
                        break;
                    }
                    let name = self.string()?;
                    items.push((name, self.payload(t, depth + 1)?));
                }
                Tag::Compound(items)
            }
            11 | 12 => {
                let n = self.len()?;
                self.take(n.checked_mul(if ty == 11 { 4 } else { 8 })?)?;
                Tag::Array
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn name(out: &mut Vec<u8>, s: &str) {
        out.extend((s.len() as u16).to_be_bytes());
        out.extend(s.as_bytes());
    }

    /// Builds a minimal `level.dat` payload: `{Data: {LevelName, LastPlayed, Seed: [I; 1, 2]}}`.
    pub(crate) fn level_dat_bytes(level_name: &str, last_played: i64) -> Vec<u8> {
        let mut b = vec![10];
        name(&mut b, "");
        b.push(10);
        name(&mut b, "Data");
        b.push(8);
        name(&mut b, "LevelName");
        name(&mut b, level_name);
        b.push(4);
        name(&mut b, "LastPlayed");
        b.extend(last_played.to_be_bytes());
        b.push(11);
        name(&mut b, "Ints");
        b.extend(2i32.to_be_bytes());
        b.extend(1i32.to_be_bytes());
        b.extend(2i32.to_be_bytes());
        b.push(0); // end Data
        b.push(0); // end root
        b
    }

    #[test]
    fn parses_level_name() {
        let tag = parse_root(&level_dat_bytes("Mon monde", 1_700_000_000_000)).unwrap();
        let data = tag.get("Data").unwrap();
        assert_eq!(data.get("LevelName").and_then(Tag::as_str), Some("Mon monde"));
        assert_eq!(data.get("LastPlayed").and_then(Tag::as_i64), Some(1_700_000_000_000));
    }

    #[test]
    fn writer_round_trip() {
        let root = Tag::Compound(vec![(
            "l".into(),
            Tag::List(vec![Tag::Compound(vec![
                ("s".into(), Tag::String("héllo".into())),
                ("b".into(), Tag::Byte(1)),
                ("n".into(), Tag::Long(-5)),
            ])]),
        )]);
        assert_eq!(parse_root(&write_root(&root)), Some(root));
    }

    #[test]
    fn rejects_garbage_and_truncation() {
        assert!(parse_root(b"hello").is_none());
        let good = level_dat_bytes("x", 1);
        for cut in 0..good.len() {
            assert!(parse_root(&good[..cut]).is_none());
        }
        // A list claiming a huge length must not allocate or panic.
        let mut b = vec![10, 0, 0, 9, 0, 1, b'a', 3, 0x7f, 0xff, 0xff, 0xff];
        b.extend([0, 0]);
        assert!(parse_root(&b).is_none());
    }
}

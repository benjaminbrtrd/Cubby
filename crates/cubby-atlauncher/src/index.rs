//! The pack list (`launcher/json/packsnew.json`): which packs Cubby shows, and how they are searched.

use cubby_common::packs::{PackChannel, PackPage, PackSort, PackSummary, PackVersion};
use cubby_common::{Error, Result};

use crate::model::{IndexPack, IndexVersion};

/// Pack id used everywhere in this crate: the name without anything but ASCII letters and digits, which
/// is how the CDN names the pack's folder ("Digital Reality" is "DigitalReality").
pub fn safe_name(name: &str) -> String {
    name.chars().filter(char::is_ascii_alphanumeric).collect()
}

/// Whether the pack is listed: public (Prism skips private and system packs, which need a share code or
/// an account) and installable (it has at least one released version; development builds have no manifest).
pub(crate) fn is_listed(pack: &IndexPack) -> bool {
    pack.kind.eq_ignore_ascii_case("public") && !pack.system && !pack.versions.is_empty() && !safe_name(&pack.name).is_empty()
}

/// Reads `packsnew.json`, keeping the listed packs in the CDN's order (its `position`, best first).
pub(crate) fn parse_index(bytes: &[u8], what: &str) -> Result<Vec<IndexPack>> {
    let mut packs: Vec<IndexPack> = serde_json::from_slice(bytes).map_err(|e| Error::json(what, e))?;
    packs.retain(is_listed);
    packs.sort_by_key(|p| p.position);
    Ok(packs)
}

pub(crate) fn summary(pack: &IndexPack, base: &str) -> PackSummary {
    let safe = safe_name(&pack.name);
    let mut minecraft: Vec<String> = Vec::new();
    for v in pack.versions.iter().filter(|v| !v.minecraft.is_empty()) {
        if !minecraft.contains(&v.minecraft) {
            minecraft.push(v.minecraft.clone());
        }
    }
    PackSummary {
        icon_url: Some(format!("{base}/launcher/images/{}.png", safe.to_ascii_lowercase())),
        id: safe,
        name: pack.name.clone(),
        summary: pack.description.trim().to_string(),
        authors: Vec::new(),
        website_url: pack.website_url.clone().filter(|u| !u.trim().is_empty()),
        downloads: None,
        updated: None,
        minecraft,
        // The list only says whether a version has a loader, not which.
        loaders: Vec::new(),
    }
}

pub(crate) fn version(v: &IndexVersion) -> PackVersion {
    PackVersion {
        id: v.version.clone(),
        name: v.version.clone(),
        channel: if v.is_recommended { PackChannel::Release } else { PackChannel::Beta },
        minecraft: Some(v.minecraft.clone()).filter(|m| !m.is_empty()),
        loader: None,
        released: None,
    }
}

/// Filters and orders the packs for a search. The list has no download counts or dates, so
/// `Downloads`, `Updated` and `Newest` keep the CDN's order; `Relevance` ranks name matches first.
pub(crate) fn search(packs: &[IndexPack], query: &str, sort: PackSort, base: &str) -> PackPage {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut hits: Vec<(u32, &IndexPack)> = packs
        .iter()
        .filter_map(|p| {
            let name = p.name.to_lowercase();
            let description = p.description.to_lowercase();
            let mut score = 0;
            for term in &terms {
                score += if name == *term {
                    100
                } else if name.starts_with(term.as_str()) {
                    60
                } else if name.contains(term.as_str()) {
                    40
                } else if description.contains(term.as_str()) {
                    10
                } else {
                    return None;
                };
            }
            Some((score, p))
        })
        .collect();
    match sort {
        PackSort::Relevance if !terms.is_empty() => hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.position.cmp(&b.1.position))),
        PackSort::Name => hits.sort_by_key(|(_, p)| p.name.to_lowercase()),
        _ => hits.sort_by_key(|(_, p)| p.position),
    }
    let total = hits.len() as u64;
    PackPage { hits: hits.into_iter().map(|(_, p)| summary(p, base)).collect(), total: Some(total) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> Vec<IndexPack> {
        parse_index(include_bytes!("../tests/fixtures/packsnew.json"), "fixture").unwrap()
    }

    #[test]
    fn safe_names() {
        assert_eq!(safe_name("Digital Reality"), "DigitalReality");
        assert_eq!(safe_name("Jurassic World: Revelations®"), "JurassicWorldRevelations");
        assert_eq!(safe_name("Mech & Magic"), "MechMagic");
    }

    #[test]
    fn only_public_installable_packs_are_listed() {
        let names: Vec<_> = index().iter().map(|p| p.name.clone()).collect();
        // Private packs, system packs and packs without a release are gone; the rest follows `position`.
        assert_eq!(names, ["Pixelmon Mod", "MoonQuest", "Digital Reality"]);
    }

    #[test]
    fn summaries() {
        let packs = index();
        let s = summary(&packs[2], "https://cdn.test/atl");
        assert_eq!(s.id, "DigitalReality");
        assert_eq!(s.name, "Digital Reality");
        assert_eq!(s.icon_url.as_deref(), Some("https://cdn.test/atl/launcher/images/digitalreality.png"));
        assert_eq!(s.minecraft, ["1.21.1", "1.12.2"]);
        assert_eq!(s.website_url.as_deref(), Some("http://digitalreality.enjin.com/home"));
        let moon = summary(&packs[1], "https://cdn.test/atl");
        assert_eq!(moon.website_url, None);
        assert_eq!(moon.minecraft, ["1.6.4"]);
    }

    #[test]
    fn versions_and_channels() {
        let packs = index();
        let versions: Vec<_> = packs[2].versions.iter().map(version).collect();
        assert_eq!(versions[0].id, "5.0.1");
        assert_eq!(versions[0].channel, PackChannel::Release);
        assert_eq!(versions[0].minecraft.as_deref(), Some("1.21.1"));
        assert_eq!(versions[1].channel, PackChannel::Beta);
    }

    #[test]
    fn searching() {
        let packs = index();
        let base = "https://cdn.test/atl";
        assert_eq!(search(&packs, "", PackSort::Relevance, base).total, Some(3));
        let names = |page: PackPage| page.hits.into_iter().map(|h| h.name).collect::<Vec<_>>();
        // Name matches rank above description matches.
        assert_eq!(names(search(&packs, "reality", PackSort::Relevance, base)), ["Digital Reality"]);
        assert_eq!(names(search(&packs, "pokemon", PackSort::Relevance, base)), ["Pixelmon Mod"]);
        assert_eq!(names(search(&packs, "MOON quest", PackSort::Relevance, base)), ["MoonQuest"]);
        assert!(search(&packs, "zzz", PackSort::Relevance, base).hits.is_empty());
        assert_eq!(names(search(&packs, "", PackSort::Name, base)), ["Digital Reality", "MoonQuest", "Pixelmon Mod"]);
        assert_eq!(names(search(&packs, "", PackSort::Downloads, base)), ["Pixelmon Mod", "MoonQuest", "Digital Reality"]);
    }
}

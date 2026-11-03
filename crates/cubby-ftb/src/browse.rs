//! Featured packs, search, pack pages and version lists.

use std::cmp::Reverse;

use futures::stream::{self, StreamExt};
use cubby_common::packs::{PackChannel, PackLoaderKind, PackPage, PackSort, PackSummary, PackVersion};
use cubby_common::Result;

use crate::api;
use crate::client::{parse_id, FtbClient};

/// Packs fetched at the same time when a list of ids is turned into summaries.
const PACK_FETCH_CONCURRENCY: usize = 8;
/// Packs on the "featured" screen.
const FEATURED_COUNT: usize = 12;
/// Most ids a search asks for (the API caps it itself).
const MAX_IDS: usize = 100;
const MAX_LIMIT: u32 = 50;

/// A pack page: the summary plus what only the detail view shows.
#[derive(Debug, Clone, PartialEq)]
pub struct PackDetails {
    pub summary: PackSummary,
    /// Markdown (FTB writes it with images and headings).
    pub description: String,
    /// Wiki, issue tracker... as (name, URL).
    pub links: Vec<(String, String)>,
    pub tags: Vec<String>,
    /// Times the pack was played, when known.
    pub plays: Option<u64>,
    /// RFC 3339, first release of the pack.
    pub released: Option<String>,
    /// Splash artwork, when there is some.
    pub splash_url: Option<String>,
}

impl FtbClient {
    /// Packs to show before the player searches: the featured ones, then the most installed.
    pub async fn featured(&self) -> Result<PackPage> {
        let (featured, popular) = futures::join!(self.featured_ids(FEATURED_COUNT), self.popular_ids(FEATURED_COUNT));
        // Featured is a nicety: the popular list alone is a fine answer.
        let mut ids = featured.unwrap_or_default();
        ids.extend(popular?);
        let mut seen = std::collections::HashSet::new();
        ids.retain(|id| seen.insert(*id));
        ids.truncate(FEATURED_COUNT);
        let hits = self.summaries(&ids).await;
        Ok(PackPage { total: Some(hits.len() as u64), hits })
    }

    /// Searches by name (an empty `query` lists every pack). `Relevance` keeps the server's order
    /// (most installed first when there is no query), `Downloads` ranks by installs; the other sorts
    /// fetch every match to order them (about a hundred small requests, cached for ten minutes).
    pub async fn search(&self, query: &str, sort: PackSort, offset: u32, limit: u32) -> Result<PackPage> {
        let limit = limit.clamp(1, MAX_LIMIT) as usize;
        let offset = offset as usize;
        let query = query.trim();
        let wanted = (offset + limit).min(MAX_IDS);

        // Candidate ids, how many packs match, and whether the server's order is the one to show.
        let (ids, total, server_order) = if query.is_empty() {
            match sort {
                PackSort::Relevance | PackSort::Downloads => {
                    let (popular, all) = futures::join!(self.popular_ids(wanted), self.all_ids());
                    let popular = popular?;
                    let total = all.map(|a| a.len()).unwrap_or(popular.len());
                    (popular, total, true)
                }
                _ => {
                    let all = self.all_ids().await?;
                    let total = all.len();
                    (all, total, false)
                }
            }
        } else {
            let ids = self.search_ids(query, MAX_IDS).await?;
            let total = ids.len();
            (ids, total, matches!(sort, PackSort::Relevance))
        };

        let hits = if server_order {
            let page: Vec<u64> = ids.iter().copied().skip(offset).take(limit).collect();
            self.summaries(&page).await
        } else {
            let mut all = self.summaries(&ids).await;
            sort_summaries(&mut all, sort);
            all.into_iter().skip(offset).take(limit).collect()
        };
        Ok(PackPage { hits, total: Some(total as u64) })
    }

    /// One pack as shown in lists.
    pub async fn pack(&self, id: &str) -> Result<PackSummary> {
        let pack = self.pack_raw(parse_id(id, "modpack")?).await?;
        Ok(summary(&pack))
    }

    /// One pack with its description and links.
    pub async fn details(&self, id: &str) -> Result<PackDetails> {
        let pack = self.pack_raw(parse_id(id, "modpack")?).await?;
        Ok(details(&pack))
    }

    /// Installable versions of a pack, newest first. Archived and private versions are left out.
    pub async fn versions(&self, id: &str) -> Result<Vec<PackVersion>> {
        let pack = self.pack_raw(parse_id(id, "modpack")?).await?;
        Ok(versions(&pack))
    }

    /// Summaries of these packs, in order; packs that can't be read or are private are left out.
    async fn summaries(&self, ids: &[u64]) -> Vec<PackSummary> {
        let packs: Vec<_> = stream::iter(ids.iter().copied().map(|id| async move { (id, self.pack_raw(id).await) }))
            .buffered(PACK_FETCH_CONCURRENCY)
            .collect()
            .await;
        packs
            .into_iter()
            .filter_map(|(id, pack)| match pack {
                Ok(pack) if !pack.private => Some(summary(&pack)),
                Ok(_) => None,
                Err(error) => {
                    tracing::warn!("skipping FTB modpack {id}: {error}");
                    None
                }
            })
            .collect()
    }
}

pub(crate) fn sort_summaries(packs: &mut [PackSummary], sort: PackSort) {
    match sort {
        PackSort::Relevance => {}
        PackSort::Downloads => packs.sort_by_key(|p| Reverse(p.downloads.unwrap_or(0))),
        // RFC 3339 in UTC sorts as text.
        PackSort::Updated => packs.sort_by(|a, b| b.updated.cmp(&a.updated)),
        // Ids grow with creation time.
        PackSort::Newest => packs.sort_by_key(|p| Reverse(p.id.parse::<u64>().unwrap_or(0))),
        PackSort::Name => packs.sort_by_key(|p| p.name.to_lowercase()),
    }
}

pub(crate) fn rfc3339(seconds: Option<i64>) -> Option<String> {
    let dt = chrono::DateTime::from_timestamp(seconds.filter(|s| *s > 0)?, 0)?;
    Some(dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// Versions a player can install, newest first (ids grow with time).
fn visible_versions(pack: &api::Pack) -> Vec<&api::VersionStub> {
    let mut versions: Vec<_> = pack.versions.iter().filter(|v| !v.private && v.kind != "archived").collect();
    versions.sort_by_key(|v| Reverse(v.id));
    versions
}

pub(crate) fn target_version<'a>(targets: &'a [api::Target], name: &str) -> Option<&'a str> {
    targets.iter().find(|t| t.name.eq_ignore_ascii_case(name) && !t.version.is_empty()).map(|t| t.version.as_str())
}

/// The mod loader of a version and its version, e.g. `(NeoForge, "21.1.262")`.
pub(crate) fn loader_of(targets: &[api::Target]) -> Option<(PackLoaderKind, String)> {
    [("neoforge", PackLoaderKind::NeoForge), ("forge", PackLoaderKind::Forge), ("fabric", PackLoaderKind::Fabric), ("quilt", PackLoaderKind::Quilt)]
        .into_iter()
        .find_map(|(name, kind)| target_version(targets, name).map(|v| (kind, v.to_string())))
}

pub(crate) fn channel(kind: &str) -> PackChannel {
    match kind {
        "beta" => PackChannel::Beta,
        "alpha" => PackChannel::Alpha,
        _ => PackChannel::Release,
    }
}

/// Square artwork if there is some, else any non-splash image, else the first one.
pub(crate) fn icon_of(art: &[api::Art]) -> Option<String> {
    let usable = || art.iter().filter(|a| a.url.starts_with("http"));
    usable()
        .find(|a| a.kind == "square")
        .or_else(|| usable().find(|a| a.kind != "splash" && a.kind != "banner"))
        .or_else(|| usable().next())
        .map(|a| a.url.clone())
}

fn website_url(pack: &api::Pack) -> String {
    let slug = pack.slug.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| slugify(&pack.name));
    if slug.is_empty() {
        format!("https://www.feed-the-beast.com/modpacks/{}", pack.id)
    } else {
        format!("https://www.feed-the-beast.com/modpacks/{}-{slug}", pack.id)
    }
}

fn slugify(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

pub(crate) fn summary(pack: &api::Pack) -> PackSummary {
    let versions = visible_versions(pack);
    let mut minecraft: Vec<String> = Vec::new();
    let mut loaders: Vec<PackLoaderKind> = Vec::new();
    for v in &versions {
        if let Some(mc) = target_version(&v.targets, "minecraft") {
            if !minecraft.iter().any(|m| m == mc) {
                minecraft.push(mc.to_string());
            }
        }
        if let Some((kind, _)) = loader_of(&v.targets) {
            if !loaders.contains(&kind) {
                loaders.push(kind);
            }
        }
    }
    PackSummary {
        id: pack.id.to_string(),
        name: pack.name.clone(),
        summary: pack.synopsis.trim().to_string(),
        authors: pack.authors.iter().map(|a| a.name.clone()).filter(|n| !n.is_empty()).collect(),
        icon_url: icon_of(&pack.art),
        website_url: Some(website_url(pack)),
        downloads: pack.installs,
        updated: rfc3339(pack.updated),
        minecraft,
        loaders,
    }
}

pub(crate) fn details(pack: &api::Pack) -> PackDetails {
    PackDetails {
        summary: summary(pack),
        description: pack.description.clone(),
        links: pack.links.iter().filter(|l| l.link.starts_with("http")).map(|l| (l.name.clone(), l.link.clone())).collect(),
        tags: pack.tags.iter().map(|t| t.name.clone()).filter(|n| !n.is_empty()).collect(),
        plays: pack.plays,
        released: rfc3339(pack.released),
        splash_url: pack.art.iter().find(|a| a.kind == "splash" && a.url.starts_with("http")).map(|a| a.url.clone()),
    }
}

pub(crate) fn versions(pack: &api::Pack) -> Vec<PackVersion> {
    visible_versions(pack)
        .into_iter()
        .map(|v| PackVersion {
            id: v.id.to_string(),
            name: v.name.clone(),
            channel: channel(&v.kind),
            minecraft: target_version(&v.targets, "minecraft").map(str::to_string),
            loader: loader_of(&v.targets).map(|(kind, _)| kind),
            released: rfc3339(v.updated),
        })
        .collect()
}

//! Feed The Beast modpacks from the public FTB App API: browse packs and turn a version into a
//! [`PackPlan`](cubby_common::packs::PackPlan).

pub mod api;
mod browse;
mod client;
mod contents;
mod plan;

pub use browse::PackDetails;
pub use contents::version_contents;
pub use client::{FtbClient, API_BASE, LEGACY_API_BASE};
pub use plan::curseforge_page_url;

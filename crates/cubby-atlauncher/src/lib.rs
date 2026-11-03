//! ATLauncher modpacks: browse the public packs on the ATLauncher CDN, read a version's manifest
//! and its optional mods, and turn it into a [`PackPlan`](cubby_common::packs::PackPlan).

mod client;
mod index;
mod model;
mod modtypes;
mod optional;
mod plan;
#[cfg(test)]
mod plan_tests;

pub use client::{AtLauncherClient, AtlPlan, ATL_BASE};
pub use index::safe_name;
pub use model::{AtlLibrary, AtlManifest, AtlMod, LoaderMetadata, ManifestConfigs, ManifestLoader, ManifestMessages};
pub use modtypes::ModKind;
pub use optional::{optional_mods, OptionalMod, OptionalSelection};

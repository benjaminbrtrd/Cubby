//! CurseForge: browse the catalogue, plan modpack installs, read a modpack `.zip`, and install mods,
//! resource packs, shader packs and datapacks one at a time.
//!
//! The API needs a key, which is not in the sources; see [`api_key_from_env`].

mod api;
mod browse;
mod client;
mod content;
mod export;
mod manifest;
mod pack_contents;
mod plan;

pub use browse::{CfLinks, CfProject, CfScreenshot, ResourceCategory, ResourceKind, ResourceQuery, MAX_SEARCH_CATEGORIES, MAX_SEARCH_LOADERS, MAX_SEARCH_VERSIONS};
pub use content::{best_file, CfFile, CfProjectBrief, FileTarget, Resolved};
pub use client::{api_key_from_env, api_key_with_source, ApiKeySource, CurseForgeClient, API_BASE, GAME_ID, MODPACK_CLASS_ID, MOD_CLASS_ID, RESOURCE_PACK_CLASS_ID, SHADER_CLASS_ID, DATAPACK_CLASS_ID};
pub use export::{export_curseforge, fingerprint, CfExportSpec};
pub use pack_contents::manifest_contents;
pub use manifest::{parse_manifest, read_manifest, CfManifest, CfManifestFile, CfMinecraft, CfModLoader};

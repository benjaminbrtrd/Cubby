//! CurseForge modpacks: browse the catalogue, plan an install and read a modpack `.zip`.
//!
//! The API needs a key, which is not in the sources; see [`api_key_from_env`].

mod api;
mod browse;
mod client;
mod manifest;
mod plan;

pub use browse::{CfLinks, CfProject, CfScreenshot, ResourceCategory, ResourceKind, ResourceQuery};
pub use client::{api_key_from_env, api_key_with_source, ApiKeySource, CurseForgeClient, API_BASE, GAME_ID, MODPACK_CLASS_ID, MOD_CLASS_ID, RESOURCE_PACK_CLASS_ID, SHADER_CLASS_ID};
pub use manifest::{parse_manifest, read_manifest, CfManifest, CfManifestFile, CfMinecraft, CfModLoader};

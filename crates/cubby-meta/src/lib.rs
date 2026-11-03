//! Version metadata (Mojang, Fabric, Quilt, Forge, NeoForge) and installing a version on disk.

pub mod forge;
pub mod install;
pub mod loaders;
pub mod mojang;
pub mod profile;

pub use install::{install, install_loader, resolve, LoaderSpec};
pub use loaders::{game_versions_supported, loader_profile, loader_versions, LoaderKind, LoaderVersion};
pub use mojang::{fetch_manifest, fetch_version, ManifestEntry, VersionManifest};

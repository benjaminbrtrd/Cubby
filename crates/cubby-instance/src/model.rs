//! Data model of an instance.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The mod loader of an instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Loader {
    Vanilla,
    Fabric { version: String },
    Quilt { version: String },
    NeoForge { version: String },
    Forge { version: String },
}

/// Per-instance Java overrides; `None` means "use the global settings".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JavaOverrides {
    pub executable: Option<PathBuf>,
    pub min_memory_mb: Option<u32>,
    pub max_memory_mb: Option<u32>,
    pub jvm_args: Option<String>,
}

/// Game window size override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

/// A game instance, stored as `<instances>/<id>/instance.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    /// Folder name, a unique slug derived from the name at creation. Never changes.
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub group: Option<String>,
    /// A built-in icon key or a file name inside the instance folder.
    #[serde(default)]
    pub icon: Option<String>,
    pub minecraft: String,
    pub loader: Loader,
    pub created: DateTime<Utc>,
    #[serde(default)]
    pub last_played: Option<DateTime<Utc>>,
    #[serde(default)]
    pub total_played_secs: u64,
    #[serde(default)]
    pub java: JavaOverrides,
    #[serde(default)]
    pub window: Option<WindowSize>,
    /// Free text the player keeps about this instance.
    #[serde(default)]
    pub notes: Option<String>,
    /// The Modrinth modpack this instance was installed from, if any.
    #[serde(default)]
    pub modpack: Option<ModpackInfo>,
}

/// Where a modpack instance comes from (a Modrinth project version).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackInfo {
    /// "modrinth", "curseforge", "ftb", "atlauncher" or "technic"; `None` in older files means Modrinth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub project_id: String,
    pub version_id: String,
    /// Project title at install time.
    pub title: String,
    pub version_number: String,
}

/// Parameters of [`crate::InstanceStore::create`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewInstance {
    pub name: String,
    #[serde(default)]
    pub group: Option<String>,
    pub minecraft: String,
    pub loader: Loader,
    #[serde(default)]
    pub icon: Option<String>,
}

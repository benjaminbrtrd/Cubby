//! On-disk layout of the launcher data directory.
//!
//! ```text
//! <root>/                     %APPDATA%/Cubby on Windows, ~/.local/share/cubby on Linux
//!   settings.toml             global settings
//!   accounts.json             accounts and tokens
//!   instances/<id>/           one folder per instance (instance.toml + .minecraft/ + backups/)
//!   meta/                     cached metadata (version manifests, loader profiles, modrinth/ packs)
//!   libraries/                shared Maven-layout libraries
//!   assets/indexes|objects/   shared assets
//!   versions/<id>/<id>.jar    client jars
//!   natives/<instance-id>/    extracted natives, recreated at each launch
//!   java/<component>/         Java runtimes downloaded by Cubby
//!   logs/                     launcher logs
//!   trash/                    deleted instances and worlds
//! ```

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// Moves the data folder of the old name to the new one when only the old one exists. A failed move
/// (another program has a file open) leaves both untouched: the launcher starts on an empty folder and
/// tries again next time, since the new folder is then not created by the move.
fn migrate_from(old: &Path, new: &Path) {
    if new.exists() || !old.is_dir() {
        return;
    }
    if let Err(e) = std::fs::rename(old, new) {
        tracing::warn!("could not move {} to {}: {e}", old.display(), new.display());
    }
}

#[derive(Debug, Clone)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    /// Uses the platform data directory, or `CUBBY_DATA_DIR` when set.
    pub fn from_env() -> Result<Self> {
        if let Some(dir) = std::env::var_os("CUBBY_DATA_DIR") {
            return Ok(Self::new(dir));
        }
        let base = dirs::data_dir().ok_or_else(|| Error::NotFound("the user data directory".into()))?;
        let root = base.join(if cfg!(windows) { "Cubby" } else { "cubby" });
        // Cubby used to be called Shard; move that folder once.
        let old = base.join(if cfg!(windows) { "Shard" } else { "shard" });
        migrate_from(&old, &root);
        Ok(Self::new(root))
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.toml")
    }

    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }

    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }

    pub fn instance(&self, id: &str) -> PathBuf {
        self.instances().join(id)
    }

    pub fn meta(&self) -> PathBuf {
        self.root.join("meta")
    }

    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }

    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }

    pub fn natives(&self, instance_id: &str) -> PathBuf {
        self.root.join("natives").join(instance_id)
    }

    pub fn java(&self) -> PathBuf {
        self.root.join("java")
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// Creates the top-level folders.
    pub fn ensure(&self) -> Result<()> {
        for dir in [self.root.clone(), self.instances(), self.meta(), self.libraries(), self.assets(), self.versions(), self.java(), self.logs()] {
            std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        }
        Ok(())
    }
}

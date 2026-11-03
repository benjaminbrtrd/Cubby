//! Instances and their content, world backups, Prism Launcher import and launcher settings.

mod backups;
pub mod content;
mod model;
mod nbt;
pub mod prism;
mod servers;
mod settings;
mod store;
mod util;

pub use content::{
    delete_screenshot, list_mods, list_packs, list_screenshots, list_worlds, remove_mod, set_mod_enabled,
    set_resource_packs, set_shader_pack, LoaderKind, ModFile, ModMeta, PackFile, PackKind, ScreenshotFile, WorldInfo,
};
pub use servers::{read_servers, write_servers, ServerEntry};
pub use backups::{backup_stamp, WorldBackup};
pub use model::{Instance, JavaOverrides, Loader, ModpackInfo, NewInstance, WindowSize};
pub use prism::{import_prism, list_prism_instances, PrismInstanceInfo};
pub use settings::{FrameMode, GameSettings, JavaSettings, OnLaunch, ProxyMode, ProxySettings, Settings};
pub use store::{GroupInfo, InstanceStore};

#[cfg(test)]
pub(crate) mod testutil {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A fresh empty directory under `target/instance/testdata`.
    pub fn temp_dir(name: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/instance/testdata")
            .join(format!("{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Writes a zip archive with the given `(entry name, contents)` pairs.
    pub fn write_zip(path: &std::path::Path, entries: &[(&str, &str)]) {
        use std::io::Write;
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (name, body) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
}

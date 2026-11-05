//! App info and links.

use super::*;

// App

#[tauri::command]
pub fn app_info(core: State<'_, Core>) -> AppInfoDto {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    AppInfoDto {
        version: env!("CARGO_PKG_VERSION"),
        data_dir: core.paths.root().to_string_lossy().into_owned(),
        msa_configured: core.msa_client_id().is_some(),
        msa_source: core.msa_client_id_with_source().map(|(_, source)| source),
        curseforge_key: core.curseforge_key_with_source().map(|(_, source)| source),
        total_memory_mb: sys.total_memory() / (1024 * 1024),
        dev_build: cfg!(debug_assertions),
    }
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> CmdResult<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only web links can be opened.".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

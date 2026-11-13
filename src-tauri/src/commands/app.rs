//! App info and links.

use super::*;

// App

#[tauri::command]
pub fn app_info(core: State<'_, Core>) -> AppInfoDto {
    AppInfoDto {
        version: env!("CARGO_PKG_VERSION"),
        data_dir: core.paths.root().to_string_lossy().into_owned(),
        msa_configured: core.msa_client_id().is_some(),
        msa_source: core.msa_client_id_with_source().map(|(_, source)| source),
        curseforge_key: core.curseforge_key_with_source().map(|(_, source)| source),
        total_memory_mb: core.total_memory_mb,
        dev_build: cfg!(debug_assertions),
        discord: core.discord.is_some(),
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

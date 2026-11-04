//! Tauri glue between the web UI and the `cubby-*` crates.

mod commands;
mod dto;
mod events;
mod packs;
mod state;
mod update;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(if cfg!(debug_assertions) { log::LevelFilter::Info } else { log::LevelFilter::Warn })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(update::PendingState::default())
        .setup(|app| {
            let core = state::Core::new(app.handle().clone())?;
            // Lets the webview load instance icons, world icons and screenshots, wherever the data lives.
            app.asset_protocol_scope().allow_directory(core.paths.root(), true)?;
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::open_url,
            commands::list_instances,
            commands::create_instance,
            commands::update_instance,
            commands::duplicate_instance,
            commands::delete_instance,
            commands::open_folder,
            commands::list_versions,
            commands::list_loader_versions,
            commands::list_mods,
            commands::set_mod_enabled,
            commands::remove_mod,
            commands::list_worlds,
            commands::list_screenshots,
            commands::running_games,
            commands::game_log,
            commands::kill_instance,
            commands::launch_instance,
            commands::list_accounts,
            commands::start_microsoft_login,
            commands::finish_microsoft_login,
            commands::cancel_microsoft_login,
            commands::remove_account,
            commands::set_default_account,
            commands::get_settings,
            commands::save_settings,
            commands::list_java,
            commands::list_prism_instances,
            commands::import_prism,
            commands::repair_instance,
            commands::delete_screenshot,
            commands::list_packs,
            commands::set_resource_packs,
            commands::set_shader_pack,
            commands::list_servers,
            commands::save_servers,
            commands::ping_server,
            commands::data_folders,
            commands::open_data_folder,
            commands::clear_cache,
            commands::search_projects,
            commands::get_project,
            commands::list_project_versions,
            commands::plan_install,
            commands::install_version,
            commands::install_modpack,
            commands::content_sources,
            commands::check_updates,
            commands::apply_updates,
            update::check_update,
            update::download_update,
            update::install_update,
            commands::backup_world,
            commands::list_backups,
            commands::restore_backup,
            commands::delete_world,
            packs::pack_providers,
            packs::pack_search,
            packs::pack_details,
            packs::pack_versions,
            packs::pack_optional_mods,
            packs::install_pack,
            packs::pick_pack_file,
            packs::inspect_pack_file,
            packs::import_pack_file,
            packs::import_pack_url,
            packs::blocked_files,
            packs::find_blocked_files,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Cubby")
        .run(|app, event| {
            // A downloaded update goes in when Cubby closes.
            if let tauri::RunEvent::Exit = event {
                update::install_on_exit(app);
            }
        });
}

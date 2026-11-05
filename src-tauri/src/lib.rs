//! Tauri glue between the web UI and the `cubby-*` crates.

mod cf_resources;
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
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Info
                } else {
                    log::LevelFilter::Warn
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(update::PendingState::default())
        .setup(|app| {
            let core = state::Core::new(app.handle().clone())?;
            // Lets the webview load instance icons, world icons and screenshots, wherever the data lives.
            app.asset_protocol_scope()
                .allow_directory(core.paths.root(), true)?;
            app.manage(core);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::open_url,
            commands::instances::list_instances,
            commands::instances::create_instance,
            commands::instances::update_instance,
            commands::instances::duplicate_instance,
            commands::instances::delete_instance,
            commands::instances::open_folder,
            commands::instances::list_versions,
            commands::instances::list_loader_versions,
            commands::content::list_mods,
            commands::content::set_mod_enabled,
            commands::content::remove_mod,
            commands::content::list_worlds,
            commands::content::list_screenshots,
            commands::launch::running_games,
            commands::launch::game_log,
            commands::launch::kill_instance,
            commands::launch::launch_instance,
            commands::accounts::list_accounts,
            commands::accounts::start_microsoft_login,
            commands::accounts::finish_microsoft_login,
            commands::accounts::cancel_microsoft_login,
            commands::accounts::remove_account,
            commands::accounts::set_default_account,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::list_java,
            commands::prism::list_prism_instances,
            commands::prism::import_prism,
            commands::launch::repair_instance,
            commands::content::delete_screenshot,
            commands::content::list_packs,
            commands::content::set_resource_packs,
            commands::content::set_shader_pack,
            commands::content::list_servers,
            commands::content::save_servers,
            commands::content::ping_server,
            commands::settings::data_folders,
            commands::settings::open_data_folder,
            commands::settings::clear_cache,
            commands::discover::search_projects,
            commands::discover::get_project,
            commands::discover::list_project_versions,
            commands::discover::plan_install,
            commands::discover::install_version,
            commands::discover::install_modpack,
            commands::discover::content_sources,
            commands::discover::check_updates,
            commands::discover::apply_updates,
            cf_resources::curseforge_search,
            cf_resources::curseforge_categories,
            update::check_update,
            update::download_update,
            update::install_update,
            commands::content::backup_world,
            commands::content::list_backups,
            commands::content::restore_backup,
            commands::content::delete_world,
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

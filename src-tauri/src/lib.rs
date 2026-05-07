mod ai;
mod archive;
mod audio;
mod config;
mod file_access;
mod shortcuts;
mod weather;
mod wallpaper;

pub fn run() {
    tauri::Builder::default()
        .plugin(shortcuts::plugin())
        .invoke_handler(tauri::generate_handler![
            config::load_config,
            config::save_config,
            ai::chat_with_deepseek,
            ai::create_archive_card,
            archive::archive_save_message,
            archive::archive_save_card,
            archive::archive_list_cards,
            archive::archive_read_project_memory,
            archive::archive_write_project_memory,
            archive::memory_compact_day,
            archive::memory_read_project_memory,
            archive::memory_write_project_memory,
            file_access::file_read_policy,
            file_access::file_list_authorized_roots,
            file_access::file_add_authorized_root,
            file_access::file_remove_authorized_root,
            file_access::file_scan_authorized_root,
            file_access::file_read_text,
            weather::refresh_weather,
            audio::start_audio_spectrum,
            audio::stop_audio_spectrum,
            audio::audio_meter_snapshot,
            audio::audio_diagnostics,
            wallpaper::next_wallpaper,
            wallpaper::switch_wallpaper_folder,
            shortcuts::quit_app,
            shortcuts::set_overlay_interactive
        ])
        .setup(|app| {
            let config = match config::load_config() {
                Ok(config) => config,
                Err(err) => {
                    eprintln!("[jingzhuo-config] load failed: {err}");
                    config::AppConfig::default()
                }
            };
            if let Err(err) = wallpaper::initialize_state(&config) {
                eprintln!("[jingzhuo-wallpaper] {err}");
            }

            shortcuts::register(app.handle());
            shortcuts::enable_default_click_through(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run 静桌");
}

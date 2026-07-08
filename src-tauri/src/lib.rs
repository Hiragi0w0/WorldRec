#![cfg_attr(test, allow(dead_code))]

mod ai;
mod db;
mod log_watcher;

#[cfg(not(test))]
mod autostart;

#[cfg(not(test))]
mod commands;

#[cfg(not(test))]
mod tray;

mod settings;
mod vrchat_api;
mod vrchat_auth;

#[cfg(not(test))]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::Manager;

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![autostart::BACKGROUND_START_ARG]),
        ))
        .plugin(tauri_plugin_opener::init())
        .manage(log_watcher::state::LogWatcherState::new())
        .manage(vrchat_api::VrchatApiState::new())
        .manage(vrchat_auth::VrchatAuthState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::get_runtime_status,
            commands::get_log_watcher_status,
            commands::list_library_worlds,
            commands::get_library_world_detail,
            commands::list_recent_visits,
            commands::list_visits,
            commands::get_vrchat_world_detail,
            commands::start_log_watcher,
            commands::sync_latest_vrchat_log,
            commands::sync_latest_vrchat_log_before_exit,
            commands::stop_log_watcher,
            commands::get_visit_transition_graph,
            commands::get_stats_date_range,
            commands::delete_all_history,
            commands::delete_visit_history,
            autostart::get_app_autostart_status,
            autostart::set_app_autostart_enabled,
            ai::get_ai_status,
            ai::save_gemini_api_key,
            ai::clear_gemini_api_key,
            ai::recommend_worlds,
            vrchat_auth::get_vrchat_auth_status,
            vrchat_auth::vrchat_login,
            vrchat_auth::vrchat_complete_email_2fa,
            vrchat_auth::clear_vrchat_login_data,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            tray::create_tray(app)?;
            autostart::sync_settings_with_autostart_state(app.handle());

            // main window は tauri.conf.json で visible: false としており、
            // 自動起動（--background）時は非表示のまま通知領域に常駐する。
            if !autostart::is_background_start() {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

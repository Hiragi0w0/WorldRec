use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Emitter, Manager,
};

use crate::log_watcher::state::LogWatcherState;

pub const OPEN_SETTINGS_EVENT: &str = "open_settings";

const MENU_ID_OPEN: &str = "open";
const MENU_ID_SETTINGS: &str = "settings";
const MENU_ID_QUIT: &str = "quit";

pub fn create_tray(app: &App) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, MENU_ID_OPEN, "WorldRec を開く", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, MENU_ID_SETTINGS, "設定を開く", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, MENU_ID_QUIT, "終了", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &settings_item, &quit_item])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("WorldRec")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_ID_OPEN => show_main_window(app),
            MENU_ID_SETTINGS => open_settings(app),
            MENU_ID_QUIT => quit_app(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => show_main_window(tray.app_handle()),
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;

    Ok(())
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        if window.is_minimized().unwrap_or(false) {
            let _ = window.unminimize();
        }
        let _ = window.set_focus();
    }
}

fn open_settings(app: &AppHandle) {
    show_main_window(app);
    let _ = app.emit(OPEN_SETTINGS_EVENT, ());
}

fn quit_app(app: &AppHandle) {
    // 終了前に最新ログを同期する。watcher の停止 command は既存方針どおり呼ばない。
    let settings = crate::commands::get_settings();
    let state = app.state::<LogWatcherState>();
    if let Err(error) = state.sync_latest_log_before_exit_if_vrchat_stopped(
        &settings.log_dir,
        &settings.db_path,
        Some(app.clone()),
    ) {
        log::warn!("終了前のログ同期に失敗しました: {error}");
    }

    app.exit(0);
}

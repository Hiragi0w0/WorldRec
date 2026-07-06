use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::settings;

pub const BACKGROUND_START_ARG: &str = "--background";

#[derive(Debug, Clone, Serialize)]
pub struct AppAutostartStatusDto {
    pub enabled: bool,
}

/// OS のログイン時自動起動（--background 付き）で起動されたかどうか。
pub fn is_background_start() -> bool {
    std::env::args().any(|arg| arg == BACKGROUND_START_ARG)
}

fn os_autostart_enabled(app: &AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|error| format!("自動起動の登録状態を確認できませんでした: {error}"))
}

/// settings.json と OS の自動起動登録状態がズレている場合、OS 側の状態を優先して settings を更新する。
pub fn sync_settings_with_autostart_state(app: &AppHandle) {
    let Ok(os_enabled) = os_autostart_enabled(app) else {
        return;
    };

    let mut settings = settings::load_settings();
    if settings.app_autostart_enabled != os_enabled {
        settings.app_autostart_enabled = os_enabled;
        if let Err(error) = settings::save_settings(settings) {
            log::warn!("autostart 設定の同期保存に失敗しました: {error}");
        }
    }
}

#[tauri::command]
pub fn get_app_autostart_status(app: AppHandle) -> Result<AppAutostartStatusDto, String> {
    let enabled = os_autostart_enabled(&app)?;

    Ok(AppAutostartStatusDto { enabled })
}

#[tauri::command]
pub fn set_app_autostart_enabled(
    app: AppHandle,
    enabled: bool,
) -> Result<AppAutostartStatusDto, String> {
    let autolaunch = app.autolaunch();

    if enabled {
        autolaunch
            .enable()
            .map_err(|error| format!("自動起動を有効化できませんでした: {error}"))?;
    } else {
        autolaunch
            .disable()
            .map_err(|error| format!("自動起動を無効化できませんでした: {error}"))?;
    }

    let mut settings = settings::load_settings();
    settings.app_autostart_enabled = enabled;
    settings::save_settings(settings)?;

    Ok(AppAutostartStatusDto { enabled })
}

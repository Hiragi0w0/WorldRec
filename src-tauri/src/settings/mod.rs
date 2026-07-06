use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{env, fs};

const MIN_BATCH_FLUSH_SECONDS: f64 = 0.5;
const MAX_BATCH_FLUSH_SECONDS: f64 = 600.0;
const MIN_BATCH_MAX_EVENTS: u32 = 1;
const MAX_BATCH_MAX_EVENTS: u32 = 500;
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontSize {
    Standard,
    Large,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TapeStyle {
    Kraft,
    Mint,
    Lavender,
    Pink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperStyle {
    Dotted,
    Lined,
    Blank,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewFormat {
    List,
    Grid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub schema_version: u32,
    pub theme: Theme,
    pub font_size: FontSize,
    pub tape_style: TapeStyle,
    pub paper_style: PaperStyle,
    pub view_format: ViewFormat,
    pub log_dir: String,
    pub db_path: String,
    pub ai_enabled: bool,
    pub has_gemini_api_key: bool,
    pub gemini_model: String,
    pub has_dify_api_key: bool,
    pub dify_enabled: bool,
    pub dify_endpoint: String,
    pub batch_flush_seconds: f64,
    pub batch_max_events: u32,
    // legacy field: VRChat 連動起動用。新しい「OSログイン時の自動起動」は app_autostart_enabled を使う。
    pub vrchat_autostart_enabled: bool,
    pub app_autostart_enabled: bool,
    pub onboarding_completed: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            theme: Theme::System,
            font_size: FontSize::Standard,
            tape_style: TapeStyle::Kraft,
            paper_style: PaperStyle::Dotted,
            view_format: ViewFormat::List,
            log_dir: String::new(),
            db_path: String::new(),
            ai_enabled: false,
            has_gemini_api_key: false,
            gemini_model: DEFAULT_GEMINI_MODEL.to_string(),
            has_dify_api_key: false,
            dify_enabled: false,
            dify_endpoint: "http://localhost/v1".to_string(),
            batch_flush_seconds: 1.0,
            batch_max_events: 50,
            vrchat_autostart_enabled: false,
            app_autostart_enabled: false,
            onboarding_completed: false,
        }
    }
}

pub fn load_settings() -> AppSettings {
    let Ok(path) = settings_path() else {
        return sanitize_settings(AppSettings::default());
    };

    let Ok(raw) = fs::read_to_string(&path) else {
        return sanitize_settings(AppSettings::default());
    };

    let settings = serde_json::from_str::<AppSettings>(&raw).unwrap_or_default();
    sanitize_settings(settings)
}

pub fn save_settings(settings: AppSettings) -> Result<AppSettings, String> {
    let sanitized = sanitize_settings(settings);
    let path = settings_path()?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "WorldRec settings directory cannot be created {}: {}",
                parent.display(),
                error
            )
        })?;
    }

    let json = serde_json::to_string_pretty(&sanitized)
        .map_err(|error| format!("WorldRec settings cannot be serialized: {error}"))?;

    fs::write(&path, json).map_err(|error| {
        format!(
            "WorldRec settings cannot be written {}: {}",
            path.display(),
            error
        )
    })?;

    Ok(sanitized)
}

fn sanitize_settings(mut settings: AppSettings) -> AppSettings {
    settings.schema_version = settings.schema_version.max(1);
    settings.batch_flush_seconds = settings
        .batch_flush_seconds
        .clamp(MIN_BATCH_FLUSH_SECONDS, MAX_BATCH_FLUSH_SECONDS);
    settings.batch_max_events = settings
        .batch_max_events
        .clamp(MIN_BATCH_MAX_EVENTS, MAX_BATCH_MAX_EVENTS);
    settings.log_dir = settings.log_dir.trim().to_string();
    settings.db_path = settings.db_path.trim().to_string();
    settings.gemini_model = settings.gemini_model.trim().to_string();
    if settings.gemini_model.is_empty() {
        settings.gemini_model = DEFAULT_GEMINI_MODEL.to_string();
    }
    settings.has_gemini_api_key = false;
    settings.has_dify_api_key = false;
    settings
}

fn settings_path() -> Result<PathBuf, String> {
    let local_app_data = env::var_os("LOCALAPPDATA").ok_or_else(|| {
        "LOCALAPPDATA is not set; WorldRec settings path cannot be resolved".to_string()
    })?;

    Ok(PathBuf::from(local_app_data)
        .join("WorldRec")
        .join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_settings_values() {
        let settings = AppSettings {
            schema_version: 0,
            log_dir: "  C:\\VRChat\\Logs  ".to_string(),
            db_path: "  C:\\WorldRec\\worldrec.db  ".to_string(),
            ai_enabled: true,
            has_gemini_api_key: true,
            gemini_model: "  gemini-test  ".to_string(),
            has_dify_api_key: true,
            onboarding_completed: true,
            batch_flush_seconds: 0.1,
            batch_max_events: 999,
            ..AppSettings::default()
        };

        let sanitized = sanitize_settings(settings);

        assert_eq!(sanitized.schema_version, 1);
        assert_eq!(sanitized.log_dir, "C:\\VRChat\\Logs");
        assert_eq!(sanitized.db_path, "C:\\WorldRec\\worldrec.db");
        assert!(sanitized.ai_enabled);
        assert!(!sanitized.has_gemini_api_key);
        assert_eq!(sanitized.gemini_model, "gemini-test");
        assert!(!sanitized.has_dify_api_key);
        assert!(sanitized.onboarding_completed);
        assert_eq!(sanitized.batch_flush_seconds, MIN_BATCH_FLUSH_SECONDS);
        assert_eq!(sanitized.batch_max_events, MAX_BATCH_MAX_EVENTS);
    }

    #[test]
    fn missing_onboarding_completed_defaults_to_false() {
        let raw = r#"{"schema_version":1}"#;

        let settings = serde_json::from_str::<AppSettings>(raw).expect("settings should parse");

        assert!(!settings.onboarding_completed);
    }

    #[test]
    fn missing_app_autostart_enabled_defaults_to_false() {
        let raw = r#"{"schema_version":1,"vrchat_autostart_enabled":true}"#;

        let settings = serde_json::from_str::<AppSettings>(raw).expect("settings should parse");

        assert!(!settings.app_autostart_enabled);
        assert!(settings.vrchat_autostart_enabled);
    }

    #[test]
    fn blank_gemini_model_falls_back_to_default() {
        let settings = AppSettings {
            gemini_model: "   ".to_string(),
            ..AppSettings::default()
        };

        let sanitized = sanitize_settings(settings);

        assert_eq!(sanitized.gemini_model, DEFAULT_GEMINI_MODEL);
    }

    #[test]
    fn invalid_enum_json_falls_back_to_default_settings() {
        let raw = r#"{"theme":"neon","batch_max_events":10}"#;

        let settings = serde_json::from_str::<AppSettings>(raw).unwrap_or_default();

        assert!(matches!(settings.theme, Theme::System));
        assert_eq!(
            settings.batch_max_events,
            AppSettings::default().batch_max_events
        );
    }
}

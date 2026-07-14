use serde::Serialize;
#[cfg(not(test))]
use tauri::AppHandle;

use crate::log_watcher::service::{
    effective_paths_equal, normalize_effective_path, resolve_db_path, resolve_log_dir,
    validate_db_path, validate_log_dir,
};
#[cfg(not(test))]
use crate::log_watcher::state::LogWatcherState;
use crate::log_watcher::LogWatcherStatus;
#[cfg(not(test))]
use crate::settings;
use crate::settings::AppSettings;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SettingsApplyOutcome {
    Applied,
    Rejected,
    RolledBack,
    RollbackFailed,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsApplyResultDto {
    pub outcome: SettingsApplyOutcome,
    pub settings: AppSettings,
    pub paths_changed: bool,
    pub watcher_was_running: bool,
    pub watcher_restarted: bool,
    pub watcher_status: LogWatcherStatus,
    pub message: Option<String>,
    pub primary_error: Option<String>,
    pub rollback_error: Option<String>,
}

trait WatcherControl {
    fn status(&self) -> LogWatcherStatus;
    fn start(&mut self, log_dir: &str, db_path: &str) -> Result<LogWatcherStatus, String>;
    fn stop(&mut self) -> Result<LogWatcherStatus, String>;
}

#[cfg(not(test))]
struct RuntimeWatcher<'a> {
    state: &'a LogWatcherState,
    app_handle: Option<AppHandle>,
}

#[cfg(not(test))]
impl WatcherControl for RuntimeWatcher<'_> {
    fn status(&self) -> LogWatcherStatus {
        self.state.status()
    }

    fn start(&mut self, log_dir: &str, db_path: &str) -> Result<LogWatcherStatus, String> {
        self.state.start(log_dir, db_path, self.app_handle.clone())
    }

    fn stop(&mut self) -> Result<LogWatcherStatus, String> {
        self.state.stop(self.app_handle.clone())
    }
}

#[cfg(not(test))]
pub fn apply_settings(
    requested: AppSettings,
    state: &LogWatcherState,
    app_handle: Option<AppHandle>,
) -> Result<SettingsApplyResultDto, String> {
    let previous = settings::load_settings();
    let mut watcher = RuntimeWatcher { state, app_handle };
    apply_settings_with_store(previous, requested, &mut watcher, |settings| {
        settings::save_settings(settings)
    })
}

fn apply_settings_with_store(
    previous: AppSettings,
    requested: AppSettings,
    watcher: &mut impl WatcherControl,
    mut save: impl FnMut(AppSettings) -> Result<AppSettings, String>,
) -> Result<SettingsApplyResultDto, String> {
    let requested_storage = normalize_default_paths_for_storage(requested)?;
    let requested_effective = resolve_settings_paths(requested_storage.clone())?;
    let current_status = watcher.status();
    let previous_effective = resolve_settings_paths(previous.clone()).ok();
    let previous_status = if current_status.running {
        current_status
    } else if let Some(previous_effective) = previous_effective.as_ref() {
        status_with_effective_paths(current_status, previous_effective)?
    } else {
        LogWatcherStatus {
            running: false,
            log_dir: previous.log_dir.clone(),
            db_path: previous.db_path.clone(),
            last_error: current_status.last_error,
        }
    };
    let watcher_was_running = previous_status.running;

    let baseline_paths = if watcher_was_running {
        Some((
            resolve_log_dir(&previous_status.log_dir)?,
            resolve_db_path(&previous_status.db_path)?,
        ))
    } else if let Some(settings) = previous_effective.as_ref() {
        Some((
            resolve_log_dir(&settings.log_dir)?,
            resolve_db_path(&settings.db_path)?,
        ))
    } else {
        None
    };
    let requested_log_dir = resolve_log_dir(&requested_effective.log_dir)?;
    let requested_db_path = resolve_db_path(&requested_effective.db_path)?;
    let paths_changed = baseline_paths.map_or(true, |(baseline_log_dir, baseline_db_path)| {
        !effective_paths_equal(&baseline_log_dir, &requested_log_dir)
            || !effective_paths_equal(&baseline_db_path, &requested_db_path)
    });

    if !paths_changed {
        let saved = save(requested_storage)?;
        let settings = resolve_settings_paths(saved)?;
        return Ok(SettingsApplyResultDto {
            outcome: SettingsApplyOutcome::Applied,
            settings: settings.clone(),
            paths_changed: false,
            watcher_was_running,
            watcher_restarted: false,
            watcher_status: status_with_effective_paths(watcher.status(), &settings)?,
            message: Some("設定を保存しました。".to_string()),
            primary_error: None,
            rollback_error: None,
        });
    }

    if let Err(error) =
        validate_log_dir(&requested_log_dir).and_then(|_| validate_db_path(&requested_db_path))
    {
        let settings = previous_effective.unwrap_or_else(|| previous.clone());
        return Ok(SettingsApplyResultDto {
            outcome: SettingsApplyOutcome::Rejected,
            settings,
            paths_changed: true,
            watcher_was_running,
            watcher_restarted: false,
            watcher_status: previous_status,
            message: Some(
                "新しい保存先を確認できなかったため、設定は変更していません。以前の監視を継続しています。"
                    .to_string(),
            ),
            primary_error: Some(error),
            rollback_error: None,
        });
    }

    if watcher_was_running {
        if let Err(error) = watcher.stop() {
            return rollback_after_failure(previous, previous_status, error, watcher, &mut save);
        }
    }

    let saved = match save(requested_storage) {
        Ok(saved) => saved,
        Err(error) => {
            return rollback_after_failure(previous, previous_status, error, watcher, &mut save);
        }
    };
    let settings = resolve_settings_paths(saved)?;

    if watcher_was_running {
        if let Err(error) = watcher.start(&settings.log_dir, &settings.db_path) {
            if watcher.status().running {
                let _ = watcher.stop();
            }
            return rollback_after_failure(previous, previous_status, error, watcher, &mut save);
        }
    }

    Ok(SettingsApplyResultDto {
        outcome: SettingsApplyOutcome::Applied,
        settings: settings.clone(),
        paths_changed: true,
        watcher_was_running,
        watcher_restarted: watcher_was_running,
        watcher_status: status_with_effective_paths(watcher.status(), &settings)?,
        message: Some(if watcher_was_running {
            "設定を保存し、新しい保存先でログ監視を再開しました。".to_string()
        } else {
            "設定を保存しました。ログ監視は停止中のままです。".to_string()
        }),
        primary_error: None,
        rollback_error: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn rollback_after_failure(
    previous: AppSettings,
    previous_status: LogWatcherStatus,
    primary_error: String,
    watcher: &mut impl WatcherControl,
    save: &mut impl FnMut(AppSettings) -> Result<AppSettings, String>,
) -> Result<SettingsApplyResultDto, String> {
    let mut rollback_errors = Vec::new();
    if watcher.status().running {
        if let Err(error) = watcher.stop() {
            rollback_errors.push(format!(
                "partially started watcher cannot be stopped: {error}"
            ));
        }
    }

    let restored_settings = match save(previous.clone()) {
        Ok(settings) => resolve_settings_paths(settings).unwrap_or(previous),
        Err(error) => {
            let mut rollback_error = format!("previous settings cannot be restored: {error}");
            if previous_status.running {
                if let Err(error) =
                    watcher.start(&previous_status.log_dir, &previous_status.db_path)
                {
                    rollback_error
                        .push_str(&format!("; previous watcher cannot be restarted: {error}"));
                }
            }
            return Err(format!(
                "settings apply failed ({primary_error}); {rollback_error}"
            ));
        }
    };

    if previous_status.running {
        if let Err(error) = watcher.start(&previous_status.log_dir, &previous_status.db_path) {
            rollback_errors.push(format!("previous watcher cannot be restarted: {error}"));
        }
    }

    let rollback_failed = !rollback_errors.is_empty();
    let watcher_status = status_with_effective_paths(watcher.status(), &restored_settings)?;

    Ok(SettingsApplyResultDto {
        outcome: if rollback_failed {
            SettingsApplyOutcome::RollbackFailed
        } else {
            SettingsApplyOutcome::RolledBack
        },
        settings: restored_settings,
        paths_changed: true,
        watcher_was_running: previous_status.running,
        watcher_restarted: previous_status.running && watcher_status.running,
        watcher_status,
        message: Some(if rollback_failed {
            "以前の設定へ戻しましたが、ログ監視を再開できませんでした。保存先を確認して「監視を開始」を押してください。"
                .to_string()
        } else {
            "新しい保存先で監視を開始できなかったため、以前の設定へ戻しました。ログ監視も以前の保存先で再開しています。"
                .to_string()
        }),
        primary_error: Some(primary_error),
        rollback_error: (!rollback_errors.is_empty()).then(|| rollback_errors.join("; ")),
    })
}

pub fn resolve_settings_paths(mut settings: AppSettings) -> Result<AppSettings, String> {
    let log_dir = normalize_effective_path(&resolve_log_dir(&settings.log_dir)?);
    let db_path = normalize_effective_path(&resolve_db_path(&settings.db_path)?);
    settings.log_dir = log_dir.to_string_lossy().to_string();
    settings.db_path = db_path.to_string_lossy().to_string();
    Ok(settings)
}

fn normalize_default_paths_for_storage(mut settings: AppSettings) -> Result<AppSettings, String> {
    let configured_log = resolve_log_dir(&settings.log_dir)?;
    let default_log = resolve_log_dir("")?;
    if effective_paths_equal(&configured_log, &default_log) {
        settings.log_dir.clear();
    }

    let configured_db = resolve_db_path(&settings.db_path)?;
    let default_db = resolve_db_path("")?;
    if effective_paths_equal(&configured_db, &default_db) {
        settings.db_path.clear();
    }

    Ok(settings)
}

pub fn status_with_effective_paths(
    status: LogWatcherStatus,
    settings: &AppSettings,
) -> Result<LogWatcherStatus, String> {
    if status.running {
        return Ok(status);
    }

    let effective = resolve_settings_paths(settings.clone())?;
    Ok(LogWatcherStatus {
        running: false,
        log_dir: effective.log_dir,
        db_path: effective.db_path,
        last_error: status.last_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, env, fs, path::PathBuf};

    struct FakeWatcher {
        status: LogWatcherStatus,
        start_failures: VecDeque<String>,
        events: Vec<String>,
    }

    impl FakeWatcher {
        fn new(settings: &AppSettings, running: bool) -> Self {
            Self {
                status: LogWatcherStatus {
                    running,
                    log_dir: settings.log_dir.clone(),
                    db_path: settings.db_path.clone(),
                    last_error: None,
                },
                start_failures: VecDeque::new(),
                events: Vec::new(),
            }
        }

        fn queue_start_failure(&mut self, error: impl Into<String>) {
            self.start_failures.push_back(error.into());
        }
    }

    impl WatcherControl for FakeWatcher {
        fn status(&self) -> LogWatcherStatus {
            self.status.clone()
        }

        fn start(&mut self, log_dir: &str, db_path: &str) -> Result<LogWatcherStatus, String> {
            self.events.push(format!("start:{log_dir}|{db_path}"));
            if let Some(error) = self.start_failures.pop_front() {
                return Err(error);
            }
            self.status.running = true;
            self.status.log_dir = log_dir.to_string();
            self.status.db_path = db_path.to_string();
            self.status.last_error = None;
            Ok(self.status())
        }

        fn stop(&mut self) -> Result<LogWatcherStatus, String> {
            self.events.push("stop".to_string());
            self.status.running = false;
            Ok(self.status())
        }
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "worldrec-settings-apply-{label}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    fn settings_for(log_dir: &std::path::Path, db_path: &std::path::Path) -> AppSettings {
        AppSettings {
            log_dir: log_dir.to_string_lossy().to_string(),
            db_path: db_path.to_string_lossy().to_string(),
            ..AppSettings::default()
        }
    }

    fn save_in_memory(settings: AppSettings) -> Result<AppSettings, String> {
        Ok(settings)
    }

    #[test]
    fn unchanged_effective_paths_keep_running_watcher() {
        let root = unique_temp_dir("unchanged");
        let log_dir = root.join("logs");
        let db_path = root.join("worldrec.db");
        fs::create_dir_all(&log_dir).expect("log directory should be created");
        let previous = settings_for(&log_dir, &db_path);
        let mut watcher = FakeWatcher::new(&previous, true);

        let result =
            apply_settings_with_store(previous.clone(), previous, &mut watcher, save_in_memory)
                .expect("settings should apply");

        assert_eq!(result.outcome, SettingsApplyOutcome::Applied);
        assert!(!result.paths_changed);
        assert!(result.watcher_status.running);
        assert!(!result.watcher_restarted);
        assert!(watcher.events.is_empty());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn explicit_default_paths_match_blank_defaults() {
        let previous = AppSettings::default();
        let requested = AppSettings {
            log_dir: resolve_log_dir("")
                .expect("default log directory should resolve")
                .to_string_lossy()
                .to_string(),
            db_path: resolve_db_path("")
                .expect("default database path should resolve")
                .to_string_lossy()
                .to_string(),
            ..AppSettings::default()
        };
        let mut watcher = FakeWatcher::new(&previous, false);

        let result = apply_settings_with_store(previous, requested, &mut watcher, save_in_memory)
            .expect("equivalent defaults should apply");

        assert!(!result.paths_changed);
        assert!(watcher.events.is_empty());
    }

    #[test]
    fn valid_requested_paths_replace_invalid_saved_paths() {
        let root = unique_temp_dir("repair-invalid-paths");
        let log_dir = root.join("logs");
        let db_path = root.join("worldrec.db");
        fs::create_dir_all(&log_dir).expect("log directory should be created");
        let previous = AppSettings {
            log_dir: "relative\\logs".to_string(),
            db_path: "relative\\worldrec.db".to_string(),
            ..AppSettings::default()
        };
        let mut watcher = FakeWatcher::new(&previous, false);

        let result = apply_settings_with_store(
            previous,
            settings_for(&log_dir, &db_path),
            &mut watcher,
            save_in_memory,
        )
        .expect("valid replacement paths should apply");

        assert_eq!(result.outcome, SettingsApplyOutcome::Applied);
        assert!(result.paths_changed);
        assert!(effective_paths_equal(
            PathBuf::from(result.settings.log_dir).as_path(),
            &log_dir
        ));
        assert!(effective_paths_equal(
            PathBuf::from(result.settings.db_path).as_path(),
            &db_path
        ));
        assert!(!result.watcher_status.running);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn running_status_keeps_actual_paths_instead_of_newer_settings_paths() {
        let root = unique_temp_dir("actual-status");
        let actual = settings_for(&root.join("actual-logs"), &root.join("actual.db"));
        let configured = settings_for(&root.join("configured-logs"), &root.join("configured.db"));
        let watcher = FakeWatcher::new(&actual, true);

        let status = status_with_effective_paths(watcher.status(), &configured)
            .expect("running status should resolve");

        assert_eq!(status.log_dir, actual.log_dir);
        assert_eq!(status.db_path, actual.db_path);
    }

    #[test]
    fn missing_log_directory_rejects_without_stopping_old_watcher() {
        let root = unique_temp_dir("missing-log");
        let old_log = root.join("old-logs");
        let old_db = root.join("old.db");
        fs::create_dir_all(&old_log).expect("old log directory should be created");
        let previous = settings_for(&old_log, &old_db);
        let mut watcher = FakeWatcher::new(&previous, true);

        let result = apply_settings_with_store(
            previous,
            settings_for(&root.join("missing"), &root.join("new.db")),
            &mut watcher,
            save_in_memory,
        )
        .expect("rejection should be returned as DTO");

        assert_eq!(result.outcome, SettingsApplyOutcome::Rejected);
        assert!(result.watcher_status.running);
        assert_eq!(result.watcher_status.db_path, old_db.to_string_lossy());
        assert!(watcher.events.is_empty());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn database_directory_rejects_without_changing_settings() {
        let root = unique_temp_dir("db-directory");
        let old_log = root.join("old-logs");
        let new_log = root.join("new-logs");
        let old_db = root.join("old.db");
        let invalid_db = root.join("database-directory");
        fs::create_dir_all(&old_log).expect("old log directory should be created");
        fs::create_dir_all(&new_log).expect("new log directory should be created");
        fs::create_dir_all(&invalid_db).expect("invalid database directory should be created");
        let previous = settings_for(&old_log, &old_db);
        let mut watcher = FakeWatcher::new(&previous, false);

        let result = apply_settings_with_store(
            previous,
            settings_for(&new_log, &invalid_db),
            &mut watcher,
            save_in_memory,
        )
        .expect("rejection should be returned as DTO");

        assert_eq!(result.outcome, SettingsApplyOutcome::Rejected);
        assert_eq!(result.settings.db_path, old_db.to_string_lossy());
        assert!(!result.watcher_status.running);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn successful_path_change_restarts_watcher_with_new_paths() {
        let root = unique_temp_dir("success");
        let old_log = root.join("old-logs");
        let new_log = root.join("new-logs");
        let old_db = root.join("old.db");
        let new_db = root.join("new.db");
        fs::create_dir_all(&old_log).expect("old log directory should be created");
        fs::create_dir_all(&new_log).expect("new log directory should be created");
        let previous = settings_for(&old_log, &old_db);
        let mut watcher = FakeWatcher::new(&previous, true);

        let result = apply_settings_with_store(
            previous,
            settings_for(&new_log, &new_db),
            &mut watcher,
            save_in_memory,
        )
        .expect("settings should apply");

        assert_eq!(result.outcome, SettingsApplyOutcome::Applied);
        assert!(result.watcher_restarted);
        assert_eq!(result.watcher_status.log_dir, new_log.to_string_lossy());
        assert_eq!(result.watcher_status.db_path, new_db.to_string_lossy());
        assert_eq!(watcher.events.first().map(String::as_str), Some("stop"));
        assert!(watcher
            .events
            .get(1)
            .is_some_and(|event| event.starts_with("start:")));

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn stopped_watcher_stays_stopped_after_path_change() {
        let root = unique_temp_dir("stopped");
        let old_log = root.join("old-logs");
        let new_log = root.join("new-logs");
        let old_db = root.join("old.db");
        let new_db = root.join("new.db");
        fs::create_dir_all(&old_log).expect("old log directory should be created");
        fs::create_dir_all(&new_log).expect("new log directory should be created");
        let previous = settings_for(&old_log, &old_db);
        let mut watcher = FakeWatcher::new(&previous, false);

        let result = apply_settings_with_store(
            previous,
            settings_for(&new_log, &new_db),
            &mut watcher,
            save_in_memory,
        )
        .expect("settings should apply");

        assert_eq!(result.outcome, SettingsApplyOutcome::Applied);
        assert!(!result.watcher_was_running);
        assert!(!result.watcher_restarted);
        assert!(!result.watcher_status.running);
        assert_eq!(result.watcher_status.db_path, new_db.to_string_lossy());
        assert!(watcher.events.is_empty());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn failed_new_start_restores_old_settings_and_watcher() {
        let root = unique_temp_dir("rollback");
        let old_log = root.join("old-logs");
        let new_log = root.join("new-logs");
        let old_db = root.join("old.db");
        let new_db = root.join("new.db");
        fs::create_dir_all(&old_log).expect("old log directory should be created");
        fs::create_dir_all(&new_log).expect("new log directory should be created");
        let previous = settings_for(&old_log, &old_db);
        let mut watcher = FakeWatcher::new(&previous, true);
        watcher.queue_start_failure("injected new watcher start failure");

        let result = apply_settings_with_store(
            previous,
            settings_for(&new_log, &new_db),
            &mut watcher,
            save_in_memory,
        )
        .expect("rollback should be returned as DTO");

        assert_eq!(result.outcome, SettingsApplyOutcome::RolledBack);
        assert_eq!(result.settings.db_path, old_db.to_string_lossy());
        assert!(result.watcher_status.running);
        assert_eq!(result.watcher_status.db_path, old_db.to_string_lossy());
        assert_eq!(watcher.events.len(), 3);
        assert_eq!(watcher.events.first().map(String::as_str), Some("stop"));

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn failed_rollback_restart_keeps_old_settings_and_stopped_status() {
        let root = unique_temp_dir("rollback-failed");
        let old_log = root.join("old-logs");
        let new_log = root.join("new-logs");
        let old_db = root.join("old.db");
        let new_db = root.join("new.db");
        fs::create_dir_all(&old_log).expect("old log directory should be created");
        fs::create_dir_all(&new_log).expect("new log directory should be created");
        let previous = settings_for(&old_log, &old_db);
        let mut watcher = FakeWatcher::new(&previous, true);
        watcher.queue_start_failure("injected new watcher start failure");
        watcher.queue_start_failure("injected rollback watcher start failure");

        let result = apply_settings_with_store(
            previous,
            settings_for(&new_log, &new_db),
            &mut watcher,
            save_in_memory,
        )
        .expect("rollback failure should be returned as DTO");

        assert_eq!(result.outcome, SettingsApplyOutcome::RollbackFailed);
        assert_eq!(result.settings.db_path, old_db.to_string_lossy());
        assert!(!result.watcher_status.running);
        assert!(result.rollback_error.is_some());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn rollback_save_failure_still_restarts_previous_watcher() {
        let root = unique_temp_dir("rollback-save-failed");
        let old_log = root.join("old-logs");
        let new_log = root.join("new-logs");
        let old_db = root.join("old.db");
        let new_db = root.join("new.db");
        fs::create_dir_all(&old_log).expect("old log directory should be created");
        fs::create_dir_all(&new_log).expect("new log directory should be created");
        let previous = settings_for(&old_log, &old_db);
        let mut watcher = FakeWatcher::new(&previous, true);
        watcher.queue_start_failure("injected new watcher start failure");
        let mut save_count = 0;

        let result = apply_settings_with_store(
            previous,
            settings_for(&new_log, &new_db),
            &mut watcher,
            |settings| {
                save_count += 1;
                if save_count == 2 {
                    Err("injected rollback save failure".to_string())
                } else {
                    Ok(settings)
                }
            },
        );

        assert!(result.is_err());
        assert!(watcher.status.running);
        assert!(effective_paths_equal(
            PathBuf::from(&watcher.status.log_dir).as_path(),
            &old_log
        ));
        assert!(effective_paths_equal(
            PathBuf::from(&watcher.status.db_path).as_path(),
            &old_db
        ));
        assert_eq!(watcher.events.len(), 3);
        assert_eq!(watcher.events.first().map(String::as_str), Some("stop"));

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }
}

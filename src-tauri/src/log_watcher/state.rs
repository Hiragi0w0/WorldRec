use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::JoinHandle;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::db::models::PendingVisit;
use crate::log_watcher::service::{
    emit_error, is_vrchat_running, resolve_db_path, resolve_log_dir, spawn_polling_watcher,
    sync_latest_log_into_database, sync_latest_log_once, validate_log_dir, ExitSyncOutcome,
    SyncLatestLogOnceResult, LOG_WATCH_STATE_CHANGED_EVENT,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LogWatcherStatus {
    pub running: bool,
    pub log_dir: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Default)]
pub struct LogWatcherState {
    inner: Mutex<LogWatcherInner>,
}

#[derive(Debug, Default)]
struct LogWatcherInner {
    running: bool,
    log_dir: String,
    last_error: Option<String>,
    current_visit: Arc<Mutex<Option<PendingVisit>>>,
    sync_mutex: Arc<Mutex<()>>,
    stop_requested: Option<Arc<AtomicBool>>,
    worker: Option<JoinHandle<()>>,
}

impl LogWatcherState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn status(&self) -> LogWatcherStatus {
        self.inner
            .lock()
            .expect("log watcher state mutex should not be poisoned")
            .status()
    }

    pub fn current_visit(&self) -> Option<PendingVisit> {
        let current_visit = {
            let inner = self
                .inner
                .lock()
                .expect("log watcher state mutex should not be poisoned");
            Arc::clone(&inner.current_visit)
        };

        let visit = current_visit
            .lock()
            .expect("current visit mutex should not be poisoned")
            .clone();

        visit
    }

    pub fn start(
        &self,
        configured_log_dir: &str,
        configured_db_path: &str,
        app_handle: Option<AppHandle>,
    ) -> Result<LogWatcherStatus, String> {
        let log_dir = resolve_log_dir(configured_log_dir)?;
        let log_dir_string = log_dir.to_string_lossy().to_string();
        let db_path = resolve_db_path(configured_db_path)?;

        {
            let inner = self
                .inner
                .lock()
                .expect("log watcher state mutex should not be poisoned");

            if inner.running {
                return Ok(inner.status());
            }
        }

        if let Err(error) = validate_log_dir(&log_dir) {
            let status = {
                let mut inner = self
                    .inner
                    .lock()
                    .expect("log watcher state mutex should not be poisoned");
                inner.running = false;
                inner.log_dir = log_dir_string;
                inner.last_error = Some(error.clone());
                inner.status()
            };

            emit_status(app_handle.as_ref(), &status);
            emit_error(app_handle.as_ref(), error.clone());

            return Err(error);
        }

        let stop_requested = Arc::new(AtomicBool::new(false));
        let current_visit = {
            let inner = self
                .inner
                .lock()
                .expect("log watcher state mutex should not be poisoned");
            Arc::clone(&inner.current_visit)
        };
        let sync_mutex = {
            let inner = self
                .inner
                .lock()
                .expect("log watcher state mutex should not be poisoned");
            Arc::clone(&inner.sync_mutex)
        };
        *current_visit
            .lock()
            .expect("current visit mutex should not be poisoned") = None;
        let worker = spawn_polling_watcher(
            log_dir,
            db_path,
            Arc::clone(&stop_requested),
            Arc::clone(&current_visit),
            sync_mutex,
            app_handle.clone(),
        );

        let status = {
            let mut inner = self
                .inner
                .lock()
                .expect("log watcher state mutex should not be poisoned");
            inner.running = true;
            inner.log_dir = log_dir_string;
            inner.last_error = None;
            inner.stop_requested = Some(stop_requested);
            inner.worker = Some(worker);
            inner.status()
        };

        emit_status(app_handle.as_ref(), &status);

        Ok(status)
    }

    pub fn sync_latest_log_once(
        &self,
        configured_log_dir: &str,
        configured_db_path: &str,
        app_handle: Option<AppHandle>,
    ) -> Result<SyncLatestLogOnceResult, String> {
        let log_dir = resolve_log_dir(configured_log_dir)?;
        let db_path = resolve_db_path(configured_db_path)?;
        let current_visit = {
            let inner = self
                .inner
                .lock()
                .expect("log watcher state mutex should not be poisoned");
            Arc::clone(&inner.current_visit)
        };
        let sync_mutex = {
            let inner = self
                .inner
                .lock()
                .expect("log watcher state mutex should not be poisoned");
            Arc::clone(&inner.sync_mutex)
        };

        let _sync_guard = sync_mutex
            .lock()
            .expect("log sync mutex should not be poisoned");
        sync_latest_log_once(&log_dir, &db_path, &current_visit, app_handle.as_ref())
    }

    pub fn sync_latest_log_before_exit_if_vrchat_stopped(
        &self,
        configured_log_dir: &str,
        configured_db_path: &str,
        app_handle: Option<AppHandle>,
    ) -> Result<ExitSyncOutcome, String> {
        self.sync_latest_log_before_exit_with_vrchat_status(
            configured_log_dir,
            configured_db_path,
            app_handle,
            is_vrchat_running,
        )
    }

    fn sync_latest_log_before_exit_with_vrchat_status(
        &self,
        configured_log_dir: &str,
        configured_db_path: &str,
        app_handle: Option<AppHandle>,
        is_vrchat_running_now: impl FnOnce() -> bool,
    ) -> Result<ExitSyncOutcome, String> {
        if is_vrchat_running_now() {
            return Ok(ExitSyncOutcome {
                synced: false,
                skipped_reason: Some("vrchat_running".to_string()),
                result: None,
            });
        }

        let log_dir = resolve_log_dir(configured_log_dir)?;
        let db_path = resolve_db_path(configured_db_path)?;
        let (current_visit, sync_mutex) = {
            let inner = self
                .inner
                .lock()
                .expect("log watcher state mutex should not be poisoned");
            (
                Arc::clone(&inner.current_visit),
                Arc::clone(&inner.sync_mutex),
            )
        };

        let _sync_guard = sync_mutex
            .lock()
            .expect("log sync mutex should not be poisoned");
        let result =
            sync_latest_log_into_database(&log_dir, &db_path, &current_visit, app_handle.as_ref())?;

        Ok(ExitSyncOutcome {
            synced: true,
            skipped_reason: None,
            result: Some(result),
        })
    }

    pub fn stop(&self, app_handle: Option<AppHandle>) -> LogWatcherStatus {
        let worker = {
            let mut inner = self
                .inner
                .lock()
                .expect("log watcher state mutex should not be poisoned");

            if let Some(stop_requested) = &inner.stop_requested {
                stop_requested.store(true, Ordering::Relaxed);
            }

            inner.running = false;
            inner.stop_requested = None;
            inner.worker.take()
        };

        if let Some(worker) = worker {
            let _ = worker.join();
        }

        {
            let current_visit = {
                let inner = self
                    .inner
                    .lock()
                    .expect("log watcher state mutex should not be poisoned");
                Arc::clone(&inner.current_visit)
            };
            *current_visit
                .lock()
                .expect("current visit mutex should not be poisoned") = None;
        }

        let status = self.status();
        emit_status(app_handle.as_ref(), &status);
        status
    }
}

impl LogWatcherInner {
    fn status(&self) -> LogWatcherStatus {
        LogWatcherStatus {
            running: self.running,
            log_dir: self.log_dir.clone(),
            last_error: self.last_error.clone(),
        }
    }
}

fn emit_status(app_handle: Option<&AppHandle>, status: &LogWatcherStatus) {
    if let Some(app_handle) = app_handle {
        let _ = app_handle.emit(LOG_WATCH_STATE_CHANGED_EVENT, status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs, path::PathBuf};

    fn unique_temp_dir() -> PathBuf {
        env::temp_dir().join(format!(
            "worldrec-log-watcher-state-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    #[test]
    fn initial_status_is_not_running() {
        let state = LogWatcherState::new();

        let status = state.status();

        assert!(!status.running);
        assert!(status.log_dir.is_empty());
        assert!(status.last_error.is_none());
    }

    #[test]
    fn start_sets_running_status() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp log dir should be created");
        let state = LogWatcherState::new();

        let status = state
            .start(
                &dir.to_string_lossy(),
                &dir.join("worldrec.db").to_string_lossy(),
                None,
            )
            .expect("existing log dir should start");

        assert!(status.running);
        assert_eq!(status.log_dir, dir.to_string_lossy());
        assert!(status.last_error.is_none());

        state.stop(None);
        fs::remove_dir_all(dir).expect("temp log dir should be removed");
    }

    #[test]
    fn starting_twice_keeps_single_running_status() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp log dir should be created");
        let state = LogWatcherState::new();

        let first_status = state
            .start(
                &dir.to_string_lossy(),
                &dir.join("worldrec.db").to_string_lossy(),
                None,
            )
            .expect("first start should succeed");
        let second_status = state
            .start(
                &dir.to_string_lossy(),
                &dir.join("worldrec.db").to_string_lossy(),
                None,
            )
            .expect("second start should be idempotent");

        assert!(first_status.running);
        assert_eq!(second_status, first_status);

        state.stop(None);
        fs::remove_dir_all(dir).expect("temp log dir should be removed");
    }

    #[test]
    fn stop_sets_running_false() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp log dir should be created");
        let state = LogWatcherState::new();

        state
            .start(
                &dir.to_string_lossy(),
                &dir.join("worldrec.db").to_string_lossy(),
                None,
            )
            .expect("start should succeed");
        let status = state.stop(None);

        assert!(!status.running);

        fs::remove_dir_all(dir).expect("temp log dir should be removed");
    }

    #[test]
    fn missing_log_dir_returns_error_without_panic() {
        let dir = unique_temp_dir();
        let state = LogWatcherState::new();

        let result = state.start(
            &dir.to_string_lossy(),
            &dir.join("worldrec.db").to_string_lossy(),
            None,
        );
        let status = state.status();

        assert!(result.is_err());
        assert!(!status.running);
        assert!(status.last_error.is_some());
    }

    #[test]
    fn sync_latest_log_before_exit_syncs_when_vrchat_is_stopped() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp log dir should be created");
        let db_path = dir.join("worldrec.db");
        fs::write(
            dir.join("output_log_2026-06-10_20-00-00.txt"),
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345\n\
2026.06.10 20:10:00 Log        -  [Behaviour] OnLeftRoom\n",
        )
        .expect("log should be written");
        let state = LogWatcherState::new();

        let outcome = state
            .sync_latest_log_before_exit_with_vrchat_status(
                &dir.to_string_lossy(),
                &db_path.to_string_lossy(),
                None,
                || false,
            )
            .expect("exit sync should succeed");

        assert!(outcome.synced);
        assert!(outcome.skipped_reason.is_none());
        let result = outcome.result.expect("sync result should be returned");
        assert!(result.processed);
        assert_eq!(result.processed_line_count, 2);
        assert_eq!(result.saved_visit_count, 2);

        fs::remove_dir_all(dir).expect("temp log dir should be removed");
    }

    #[test]
    fn sync_latest_log_before_exit_skips_when_vrchat_is_running() {
        let state = LogWatcherState::new();

        let outcome = state
            .sync_latest_log_before_exit_with_vrchat_status("", "", None, || true)
            .expect("exit sync skip should succeed");

        assert!(!outcome.synced);
        assert_eq!(outcome.skipped_reason.as_deref(), Some("vrchat_running"));
        assert!(outcome.result.is_none());
    }
}

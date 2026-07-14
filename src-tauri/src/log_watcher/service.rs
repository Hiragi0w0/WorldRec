use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::log_watcher::reader::{latest_log_snapshot, read_lines_from_offset, WatchedLogFile};
use crate::log_watcher::stay_duration::{
    calculate_stay_duration_from_timestamps, parse_timestamp_seconds,
};
use crate::{
    db::{
        connection::open_database,
        models::{PendingVisit, VisitInput},
        repository::add_visit_if_missing,
        schema::initialize_database,
    },
    log_watcher::{
        parser::{parse_log_event, ParsedLogEvent},
        processor::{process_log_lines_to_actions, ProcessedLogAction},
        visit_session::VisitSession,
    },
};

pub const LOG_WATCH_STATE_CHANGED_EVENT: &str = "log_watch_state_changed";
pub const LOG_WATCH_ERROR_EVENT: &str = "log_watch_error";
pub const VISIT_SAVED_EVENT: &str = "visit_saved";
pub const CURRENT_VISIT_CHANGED_EVENT: &str = "current_visit_changed";

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_RECOVERY_STAY_SECONDS: i64 = 24 * 60 * 60;
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VisitSavedPayload {
    pub visited_at: String,
    pub world_name: String,
    pub world_id: Option<String>,
    pub stay_duration_seconds: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SyncLatestLogOnceResult {
    pub latest_log_file: Option<String>,
    pub processed: bool,
    pub processed_line_count: usize,
    pub saved_visit_count: usize,
    pub current_visit: Option<PendingVisit>,
}

#[derive(Debug, Clone)]
pub struct ExitSyncOutcome {
    pub synced: bool,
    pub skipped_reason: Option<String>,
    pub result: Option<SyncLatestLogOnceResult>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct RecoverableVisit {
    id: i64,
    visited_at: String,
    world_name: String,
    world_id: Option<String>,
    source_log_file: Option<String>,
}

struct VisitRecorder {
    db_path: PathBuf,
    connection: Option<Connection>,
}

impl VisitRecorder {
    fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            connection: None,
        }
    }

    fn save_pending_visit_if_missing(
        &mut self,
        pending_visit: &PendingVisit,
    ) -> Result<Option<VisitSavedPayload>, String> {
        if self.update_existing_pending_visit_by_visit_key(pending_visit)? {
            return Ok(None);
        }

        if self.visit_exists_by_visit_key_including_finalized(pending_visit)? {
            return Ok(None);
        }

        let db_path = self.db_path.clone();
        let connection = self.connection(&db_path)?;

        let inserted = connection
            .execute(
                "
            INSERT OR IGNORE INTO visit_histories (
                visited_at,
                world_name,
                world_id,
                instance_id,
                instance_access_type,
                instance_nonce,
                instance_raw_tags,
                stay_duration_seconds,
                memo,
                tags,
                source_log_file,
                created_at,
                updated_at
            ) VALUES (
                ?1,
                ?2,
                ?3,
                ?4,
                ?5,
                ?6,
                ?7,
                NULL,
                NULL,
                NULL,
                ?8,
                CURRENT_TIMESTAMP,
                CURRENT_TIMESTAMP
            )
            ",
                params![
                    pending_visit.visited_at.as_str(),
                    pending_visit.world_name.as_str(),
                    pending_visit.world_id.as_deref(),
                    pending_visit.instance_id.as_deref(),
                    pending_visit.instance_access_type.as_deref(),
                    pending_visit.instance_nonce.as_deref(),
                    pending_visit.instance_raw_tags.as_deref(),
                    pending_visit.source_log_file.as_deref(),
                ],
            )
            .map_err(|error| {
                format!(
                    "Pending VRChat visit cannot be saved to database {}: {}",
                    db_path.display(),
                    error
                )
            })?;

        if inserted == 0 {
            return Ok(None);
        }

        Ok(Some(VisitSavedPayload {
            visited_at: pending_visit.visited_at.clone(),
            world_name: pending_visit.world_name.clone(),
            world_id: pending_visit.world_id.clone(),
            stay_duration_seconds: None,
        }))
    }

    fn update_existing_pending_visit_by_visit_key(
        &mut self,
        pending_visit: &PendingVisit,
    ) -> Result<bool, String> {
        let db_path = self.db_path.clone();
        let connection = self.connection(&db_path)?;

        let updated = connection
            .execute(
                "
            UPDATE visit_histories
            SET
                world_id = COALESCE(world_id, ?1),
                instance_id = COALESCE(instance_id, ?2),
                instance_access_type =
                    CASE
                        WHEN instance_access_type IS NULL THEN ?3
                        WHEN lower(instance_access_type) = 'public'
                             AND ?3 IS NOT NULL
                             AND lower(?3) <> 'public'
                        THEN ?3
                        ELSE instance_access_type
                    END,
                instance_nonce = COALESCE(instance_nonce, ?4),
                instance_raw_tags =
                    CASE
                        WHEN instance_raw_tags IS NULL THEN ?5
                        WHEN ?5 IS NOT NULL AND length(?5) > length(instance_raw_tags) THEN ?5
                        ELSE instance_raw_tags
                    END,
                source_log_file = COALESCE(source_log_file, ?6),
                updated_at = CURRENT_TIMESTAMP
            WHERE visited_at = ?7
              AND world_name = ?8
              AND stay_duration_seconds IS NULL
              AND (
                  source_log_file = ?9
                  OR source_log_file IS NULL
                  OR ?9 IS NULL
              )
            ",
                params![
                    pending_visit.world_id.as_deref(),
                    pending_visit.instance_id.as_deref(),
                    pending_visit.instance_access_type.as_deref(),
                    pending_visit.instance_nonce.as_deref(),
                    pending_visit.instance_raw_tags.as_deref(),
                    pending_visit.source_log_file.as_deref(),
                    pending_visit.visited_at.as_str(),
                    pending_visit.world_name.as_str(),
                    pending_visit.source_log_file.as_deref(),
                ],
            )
            .map_err(|error| {
                format!(
                    "Existing pending VRChat visit cannot be updated in database {}: {}",
                    db_path.display(),
                    error
                )
            })?;

        Ok(updated > 0)
    }

    fn visit_exists_by_visit_key_including_finalized(
        &mut self,
        pending_visit: &PendingVisit,
    ) -> Result<bool, String> {
        let db_path = self.db_path.clone();
        let connection = self.connection(&db_path)?;

        let exists: bool = connection
            .query_row(
                "
            SELECT EXISTS (
                SELECT 1
                FROM visit_histories
                WHERE visited_at = ?1
                  AND COALESCE(source_log_file, '') = COALESCE(?2, '')
                  AND (
                      (world_id IS NOT NULL AND ?3 IS NOT NULL AND world_id = ?3)
                      OR
                      (world_id IS NULL AND world_name = ?4)
                      OR
                      (?3 IS NULL AND world_name = ?4)
                  )
                LIMIT 1
            )
            ",
                params![
                    pending_visit.visited_at.as_str(),
                    pending_visit.source_log_file.as_deref(),
                    pending_visit.world_id.as_deref(),
                    pending_visit.world_name.as_str(),
                ],
                |row| row.get(0),
            )
            .map_err(|error| {
                format!(
                    "Existing VRChat visit cannot be checked in database {}: {}",
                    db_path.display(),
                    error
                )
            })?;

        Ok(exists)
    }

    fn save_finalized_visit(
        &mut self,
        visit_input: &VisitInput,
    ) -> Result<Option<VisitSavedPayload>, String> {
        if self.update_unfinished_visit_duration_by_visit_key(visit_input)? {
            return Ok(Some(VisitSavedPayload {
                visited_at: visit_input.visited_at.clone(),
                world_name: visit_input.world_name.clone(),
                world_id: visit_input.world_id.clone(),
                stay_duration_seconds: Some(visit_input.stay_duration_seconds),
            }));
        }

        if self.update_unfinished_visit_duration_by_identity(
            &visit_input.visited_at,
            &visit_input.world_name,
            visit_input.world_id.as_deref(),
            visit_input.source_log_file.as_deref(),
            visit_input.stay_duration_seconds,
        )? {
            return Ok(Some(VisitSavedPayload {
                visited_at: visit_input.visited_at.clone(),
                world_name: visit_input.world_name.clone(),
                world_id: visit_input.world_id.clone(),
                stay_duration_seconds: Some(visit_input.stay_duration_seconds),
            }));
        }

        let db_path = self.db_path.clone();
        let connection = self.connection(&db_path)?;

        let inserted = add_visit_if_missing(connection, visit_input).map_err(|error| {
            format!(
                "VRChat visit cannot be saved to database {}: {}",
                db_path.display(),
                error
            )
        })?;

        if !inserted {
            return Ok(None);
        }

        Ok(Some(VisitSavedPayload {
            visited_at: visit_input.visited_at.clone(),
            world_name: visit_input.world_name.clone(),
            world_id: visit_input.world_id.clone(),
            stay_duration_seconds: Some(visit_input.stay_duration_seconds),
        }))
    }

    fn update_unfinished_visit_duration_by_visit_key(
        &mut self,
        visit_input: &VisitInput,
    ) -> Result<bool, String> {
        if !(1..=MAX_RECOVERY_STAY_SECONDS).contains(&visit_input.stay_duration_seconds) {
            return Ok(false);
        }

        let db_path = self.db_path.clone();
        let connection = self.connection(&db_path)?;

        let updated = connection
            .execute(
                "
                UPDATE visit_histories
                SET
                    world_id = COALESCE(world_id, ?1),
                    instance_id = COALESCE(instance_id, ?2),
                    instance_access_type =
                        CASE
                            WHEN instance_access_type IS NULL THEN ?3
                            WHEN lower(instance_access_type) = 'public'
                                 AND ?3 IS NOT NULL
                                 AND lower(?3) <> 'public'
                            THEN ?3
                            ELSE instance_access_type
                        END,
                    instance_nonce = COALESCE(instance_nonce, ?4),
                    instance_raw_tags =
                        CASE
                            WHEN instance_raw_tags IS NULL THEN ?5
                            WHEN ?5 IS NOT NULL AND length(?5) > length(instance_raw_tags) THEN ?5
                            ELSE instance_raw_tags
                        END,
                    source_log_file = COALESCE(source_log_file, ?6),
                    stay_duration_seconds = ?7,
                    updated_at = CURRENT_TIMESTAMP
                WHERE visited_at = ?8
                  AND world_name = ?9
                  AND stay_duration_seconds IS NULL
                  AND (
                      source_log_file = ?10
                      OR source_log_file IS NULL
                      OR ?10 IS NULL
                  )
                ",
                params![
                    visit_input.world_id.as_deref(),
                    visit_input.instance_id.as_deref(),
                    visit_input.instance_access_type.as_deref(),
                    visit_input.instance_nonce.as_deref(),
                    visit_input.instance_raw_tags.as_deref(),
                    visit_input.source_log_file.as_deref(),
                    visit_input.stay_duration_seconds,
                    visit_input.visited_at.as_str(),
                    visit_input.world_name.as_str(),
                    visit_input.source_log_file.as_deref(),
                ],
            )
            .map_err(|error| {
                format!(
                    "Unfinished VRChat visit cannot be finalized by visit key in database {}: {}",
                    db_path.display(),
                    error
                )
            })?;

        Ok(updated > 0)
    }

    #[allow(dead_code)]
    fn update_pending_visit_metadata(
        &mut self,
        pending_visit: &PendingVisit,
    ) -> Result<(), String> {
        if self.update_pending_visit_metadata_by_visit_key(pending_visit)? {
            return Ok(());
        }
        let db_path = self.db_path.clone();
        let connection = self.connection(&db_path)?;

        let updated = match pending_visit.world_id.as_deref() {
            Some(world_id) => connection.execute(
                "
                UPDATE visit_histories
                SET
                    world_name = ?1,
                    instance_id = COALESCE(instance_id, ?2),
                    instance_access_type =
                        CASE
                            WHEN instance_access_type IS NULL THEN ?3
                            WHEN lower(instance_access_type) = 'public'
                                 AND ?3 IS NOT NULL
                                 AND lower(?3) <> 'public'
                            THEN ?3
                            ELSE instance_access_type
                        END,
                    instance_nonce = COALESCE(instance_nonce, ?4),
                    instance_raw_tags =
                        CASE
                            WHEN instance_raw_tags IS NULL THEN ?5
                            WHEN ?5 IS NOT NULL AND length(?5) > length(instance_raw_tags) THEN ?5
                            ELSE instance_raw_tags
                        END,
                    source_log_file = COALESCE(source_log_file, ?6),
                    updated_at = CURRENT_TIMESTAMP
                WHERE world_id = ?7
                  AND visited_at = ?8
                ",
                params![
                    pending_visit.world_name,
                    pending_visit.instance_id,
                    pending_visit.instance_access_type,
                    pending_visit.instance_nonce,
                    pending_visit.instance_raw_tags,
                    pending_visit.source_log_file,
                    world_id,
                    pending_visit.visited_at,
                ],
            ),
            None => connection.execute(
                "
                UPDATE visit_histories
                SET
                    world_id = COALESCE(world_id, ?1),
                    instance_id = COALESCE(instance_id, ?2),
                    instance_access_type =
                        CASE
                            WHEN instance_access_type IS NULL THEN ?3
                            WHEN lower(instance_access_type) = 'public'
                                 AND ?3 IS NOT NULL
                                 AND lower(?3) <> 'public'
                            THEN ?3
                            ELSE instance_access_type
                        END,
                    instance_nonce = COALESCE(instance_nonce, ?4),
                    instance_raw_tags =
                        CASE
                            WHEN instance_raw_tags IS NULL THEN ?5
                            WHEN ?5 IS NOT NULL AND length(?5) > length(instance_raw_tags) THEN ?5
                            ELSE instance_raw_tags
                        END,
                    source_log_file = COALESCE(source_log_file, ?6),
                    updated_at = CURRENT_TIMESTAMP
                WHERE world_id IS NULL
                  AND world_name = ?7
                  AND visited_at = ?8
                ",
                params![
                    pending_visit.world_id,
                    pending_visit.instance_id,
                    pending_visit.instance_access_type,
                    pending_visit.instance_nonce,
                    pending_visit.instance_raw_tags,
                    pending_visit.source_log_file,
                    pending_visit.world_name,
                    pending_visit.visited_at,
                ],
            ),
        }
        .map_err(|error| {
            format!(
                "Pending VRChat visit metadata cannot be updated in database {}: {}",
                db_path.display(),
                error
            )
        })?;

        let _ = updated;
        Ok(())
    }

    #[allow(dead_code)]
    fn update_pending_visit_metadata_by_visit_key(
        &mut self,
        pending_visit: &PendingVisit,
    ) -> Result<bool, String> {
        let db_path = self.db_path.clone();
        let connection = self.connection(&db_path)?;

        let updated = connection
            .execute(
                "
                UPDATE visit_histories
                SET
                    world_id = COALESCE(world_id, ?1),
                    instance_id = COALESCE(instance_id, ?2),
                    instance_access_type =
                        CASE
                            WHEN instance_access_type IS NULL THEN ?3
                            WHEN lower(instance_access_type) = 'public'
                                 AND ?3 IS NOT NULL
                                 AND lower(?3) <> 'public'
                            THEN ?3
                            ELSE instance_access_type
                        END,
                    instance_nonce = COALESCE(instance_nonce, ?4),
                    instance_raw_tags =
                        CASE
                            WHEN instance_raw_tags IS NULL THEN ?5
                            WHEN ?5 IS NOT NULL AND length(?5) > length(instance_raw_tags) THEN ?5
                            ELSE instance_raw_tags
                        END,
                    source_log_file = COALESCE(source_log_file, ?6),
                    updated_at = CURRENT_TIMESTAMP
                WHERE visited_at = ?7
                  AND world_name = ?8
                  AND stay_duration_seconds IS NULL
                  AND (
                      source_log_file = ?9
                      OR source_log_file IS NULL
                      OR ?9 IS NULL
                  )
                ",
                params![
                    pending_visit.world_id.as_deref(),
                    pending_visit.instance_id.as_deref(),
                    pending_visit.instance_access_type.as_deref(),
                    pending_visit.instance_nonce.as_deref(),
                    pending_visit.instance_raw_tags.as_deref(),
                    pending_visit.source_log_file.as_deref(),
                    pending_visit.visited_at.as_str(),
                    pending_visit.world_name.as_str(),
                    pending_visit.source_log_file.as_deref(),
                ],
            )
            .map_err(|error| {
                format!(
                    "Pending VRChat visit metadata cannot be updated by visit key in database {}: {}",
                    db_path.display(),
                    error
                )
            })?;

        Ok(updated > 0)
    }

    #[allow(dead_code)]
    fn list_unfinished_visits(&mut self, limit: i64) -> Result<Vec<RecoverableVisit>, String> {
        let db_path = self.db_path.clone();
        let connection = self.connection(&db_path)?;

        let mut statement = connection
            .prepare(
                "
                SELECT
                    id,
                    visited_at,
                    world_name,
                    world_id,
                    source_log_file
                FROM visit_histories
                WHERE stay_duration_seconds IS NULL
                  AND source_log_file IS NOT NULL
                  AND trim(source_log_file) <> ''
                ORDER BY visited_at DESC, id DESC
                LIMIT ?1
                ",
            )
            .map_err(|error| {
                format!(
                    "Unfinished VRChat visits cannot be queried from database {}: {}",
                    db_path.display(),
                    error
                )
            })?;

        let rows = statement
            .query_map(params![limit], |row| {
                Ok(RecoverableVisit {
                    id: row.get(0)?,
                    visited_at: row.get(1)?,
                    world_name: row.get(2)?,
                    world_id: row.get(3)?,
                    source_log_file: row.get(4)?,
                })
            })
            .map_err(|error| {
                format!(
                    "Unfinished VRChat visits cannot be read from database {}: {}",
                    db_path.display(),
                    error
                )
            })?;

        let mut visits = Vec::new();

        for row in rows {
            visits.push(row.map_err(|error| {
                format!(
                    "Unfinished VRChat visit row cannot be read from database {}: {}",
                    db_path.display(),
                    error
                )
            })?);
        }

        Ok(visits)
    }

    #[allow(dead_code)]
    fn update_unfinished_visit_duration(
        &mut self,
        visit: &RecoverableVisit,
        end_timestamp: &str,
    ) -> Result<Option<VisitSavedPayload>, String> {
        let Some(stay_duration_seconds) =
            valid_stay_duration_seconds(&visit.visited_at, end_timestamp)
        else {
            return Ok(None);
        };

        let db_path = self.db_path.clone();
        let connection = self.connection(&db_path)?;

        let updated = connection
            .execute(
                "
                UPDATE visit_histories
                SET
                    stay_duration_seconds = ?1,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = ?2
                  AND stay_duration_seconds IS NULL
                ",
                params![stay_duration_seconds, visit.id],
            )
            .map_err(|error| {
                format!(
                    "Unfinished VRChat visit cannot be finalized in database {}: {}",
                    db_path.display(),
                    error
                )
            })?;

        if updated == 0 {
            return Ok(None);
        }

        Ok(Some(VisitSavedPayload {
            visited_at: visit.visited_at.clone(),
            world_name: visit.world_name.clone(),
            world_id: visit.world_id.clone(),
            stay_duration_seconds: Some(stay_duration_seconds),
        }))
    }

    fn update_unfinished_visit_duration_by_identity(
        &mut self,
        visited_at: &str,
        world_name: &str,
        world_id: Option<&str>,
        source_log_file: Option<&str>,
        stay_duration_seconds: i64,
    ) -> Result<bool, String> {
        if !(1..=MAX_RECOVERY_STAY_SECONDS).contains(&stay_duration_seconds) {
            return Ok(false);
        }

        let db_path = self.db_path.clone();
        let connection = self.connection(&db_path)?;

        let updated = match world_id {
            Some(world_id) => connection.execute(
                "
                UPDATE visit_histories
                SET
                    stay_duration_seconds = ?1,
                    updated_at = CURRENT_TIMESTAMP
                WHERE world_id = ?2
                  AND visited_at = ?3
                  AND stay_duration_seconds IS NULL
                  AND COALESCE(source_log_file, '') = COALESCE(?4, '')
                ",
                params![stay_duration_seconds, world_id, visited_at, source_log_file],
            ),
            None => connection.execute(
                "
                UPDATE visit_histories
                SET
                    stay_duration_seconds = ?1,
                    updated_at = CURRENT_TIMESTAMP
                WHERE world_id IS NULL
                  AND world_name = ?2
                  AND visited_at = ?3
                  AND stay_duration_seconds IS NULL
                  AND COALESCE(source_log_file, '') = COALESCE(?4, '')
                ",
                params![
                    stay_duration_seconds,
                    world_name,
                    visited_at,
                    source_log_file
                ],
            ),
        }
        .map_err(|error| {
            format!(
                "Unfinished VRChat visit cannot be finalized in database {}: {}",
                db_path.display(),
                error
            )
        })?;

        Ok(updated > 0)
    }

    fn connection(&mut self, db_path: &Path) -> Result<&Connection, String> {
        if self.connection.is_none() {
            self.connection = Some(open_initialized_database(db_path)?);
        }

        Ok(self
            .connection
            .as_ref()
            .expect("initialized database connection should exist"))
    }
}

pub fn resolve_log_dir(settings_log_dir: &str) -> Result<PathBuf, String> {
    let configured_log_dir = settings_log_dir.trim();

    if configured_log_dir.is_empty() {
        return default_vrchat_log_dir();
    }

    resolve_absolute_path(PathBuf::from(configured_log_dir), "VRChat log directory")
}

pub fn validate_log_dir(log_dir: &Path) -> Result<(), String> {
    if !log_dir.exists() {
        return Err(format!(
            "VRChat log directory does not exist: {}",
            log_dir.display()
        ));
    }

    if !log_dir.is_dir() {
        return Err(format!(
            "VRChat log path is not a directory: {}",
            log_dir.display()
        ));
    }

    fs::read_dir(log_dir).map_err(|error| {
        format!(
            "VRChat log directory cannot be read {}: {}",
            log_dir.display(),
            error
        )
    })?;

    Ok(())
}

pub fn resolve_db_path(settings_db_path: &str) -> Result<PathBuf, String> {
    let configured_db_path = settings_db_path.trim();

    if !configured_db_path.is_empty() {
        return resolve_absolute_path(PathBuf::from(configured_db_path), "WorldRec database");
    }

    let db_path = default_worldrec_db_path()?;
    let db_dir = db_path.parent().ok_or_else(|| {
        format!(
            "WorldRec database directory cannot be resolved: {}",
            db_path.display()
        )
    })?;

    fs::create_dir_all(db_dir).map_err(|error| {
        format!(
            "WorldRec data directory cannot be created {}: {}",
            db_dir.display(),
            error
        )
    })?;

    Ok(db_path)
}

fn resolve_absolute_path(path: PathBuf, label: &str) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "{label} path must be absolute and cannot be resolved safely: {}",
            path.display()
        ));
    }

    Ok(path)
}

pub fn normalize_effective_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return display_safe_canonical_path(canonical);
    }

    if let (Some(parent), Some(file_name)) = (path.parent(), path.file_name()) {
        if let Ok(canonical_parent) = parent.canonicalize() {
            return display_safe_canonical_path(canonical_parent).join(file_name);
        }
    }

    path.to_path_buf()
}

#[cfg(target_os = "windows")]
fn display_safe_canonical_path(path: PathBuf) -> PathBuf {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{unc}"));
    }
    if let Some(regular) = value.strip_prefix(r"\\?\") {
        return PathBuf::from(regular);
    }
    path
}

#[cfg(not(target_os = "windows"))]
fn display_safe_canonical_path(path: PathBuf) -> PathBuf {
    path
}

pub fn effective_paths_equal(left: &Path, right: &Path) -> bool {
    let left = normalize_effective_path(left);
    let right = normalize_effective_path(right);

    #[cfg(target_os = "windows")]
    {
        normalize_windows_path_string(&left) == normalize_windows_path_string(&right)
    }

    #[cfg(not(target_os = "windows"))]
    {
        left == right
    }
}

#[cfg(target_os = "windows")]
fn normalize_windows_path_string(path: &Path) -> String {
    path.to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .replace('/', "\\")
        .to_lowercase()
}

pub fn validate_db_path(db_path: &Path) -> Result<(), String> {
    if db_path.is_dir() {
        return Err(format!(
            "WorldRec database path is a directory: {}",
            db_path.display()
        ));
    }

    let connection = open_initialized_database(db_path)?;
    let quick_check: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|error| {
            format!(
                "WorldRec database quick_check failed {}: {}",
                db_path.display(),
                error
            )
        })?;

    if !quick_check.eq_ignore_ascii_case("ok") {
        return Err(format!(
            "WorldRec database quick_check did not return ok {}: {}",
            db_path.display(),
            quick_check
        ));
    }

    connection
        .execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TEMP TABLE worldrec_path_write_check (value INTEGER NOT NULL);
             INSERT INTO worldrec_path_write_check (value) VALUES (1);
             ROLLBACK;",
        )
        .map_err(|error| {
            format!(
                "WorldRec database is not writable {}: {}",
                db_path.display(),
                error
            )
        })?;

    Ok(())
}

fn default_worldrec_db_path() -> Result<PathBuf, String> {
    let local_app_data = env::var_os("LOCALAPPDATA").ok_or_else(|| {
        "LOCALAPPDATA is not set; WorldRec database path cannot be resolved".to_string()
    })?;

    Ok(worldrec_db_path_from_local_app_data(local_app_data))
}

fn worldrec_db_path_from_local_app_data(local_app_data: impl Into<PathBuf>) -> PathBuf {
    local_app_data.into().join("WorldRec").join("worldrec.db")
}

pub fn spawn_polling_watcher(
    log_dir: PathBuf,
    db_path: PathBuf,
    stop_requested: Arc<AtomicBool>,
    current_visit: Arc<Mutex<Option<PendingVisit>>>,
    sync_mutex: Arc<Mutex<()>>,
    app_handle: Option<AppHandle>,
) -> Result<JoinHandle<()>, String> {
    thread::Builder::new()
        .name("worldrec-log-watcher".to_string())
        .spawn(move || {
            run_polling_watcher(
                log_dir,
                db_path,
                stop_requested,
                current_visit,
                sync_mutex,
                app_handle,
            )
        })
        .map_err(|error| format!("WorldRec log watcher thread cannot be started: {error}"))
}

pub fn sync_latest_log_once(
    log_dir: &Path,
    db_path: &Path,
    current_visit: &Arc<Mutex<Option<PendingVisit>>>,
    app_handle: Option<&AppHandle>,
) -> Result<SyncLatestLogOnceResult, String> {
    sync_latest_log_into_database(log_dir, db_path, current_visit, app_handle)
}

pub(crate) fn sync_latest_log_into_database(
    log_dir: &Path,
    db_path: &Path,
    current_visit: &Arc<Mutex<Option<PendingVisit>>>,
    app_handle: Option<&AppHandle>,
) -> Result<SyncLatestLogOnceResult, String> {
    validate_log_dir(log_dir)?;

    let Some(snapshot) = latest_log_snapshot(log_dir).map_err(|error| {
        format!(
            "Failed to inspect VRChat log directory {}: {}",
            log_dir.display(),
            error
        )
    })?
    else {
        return Ok(SyncLatestLogOnceResult {
            latest_log_file: None,
            processed: false,
            processed_line_count: 0,
            saved_visit_count: 0,
            current_visit: current_visit
                .lock()
                .expect("current visit mutex should not be poisoned")
                .clone(),
        });
    };

    let read_result = read_lines_from_offset(&snapshot.path, 0).map_err(|error| {
        format!(
            "VRChat log cannot be read for manual sync {}: {}",
            snapshot.path.display(),
            error
        )
    })?;
    let processed_line_count = read_result.lines.len();
    let latest_log_file = snapshot
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);

    let mut visit_session = VisitSession::new();
    let mut visit_recorder = VisitRecorder::new(db_path.to_path_buf());

    update_current_visit(current_visit, None);
    emit_current_visit_changed(app_handle);

    let saved_visits = process_log_lines_with_recorder(
        read_result.lines,
        latest_log_file.as_deref(),
        &mut visit_session,
        &mut visit_recorder,
        current_visit,
        app_handle,
    );
    let current_visit = visit_session.current_visit().cloned();

    Ok(SyncLatestLogOnceResult {
        latest_log_file,
        processed: true,
        processed_line_count,
        saved_visit_count: saved_visits.len(),
        current_visit,
    })
}

fn default_vrchat_log_dir() -> Result<PathBuf, String> {
    let user_profile = env::var_os("USERPROFILE").ok_or_else(|| {
        "USERPROFILE is not set; VRChat log directory cannot be resolved".to_string()
    })?;

    Ok(PathBuf::from(user_profile)
        .join("AppData")
        .join("LocalLow")
        .join("VRChat")
        .join("VRChat"))
}

fn run_polling_watcher(
    log_dir: PathBuf,
    db_path: PathBuf,
    stop_requested: Arc<AtomicBool>,
    current_visit: Arc<Mutex<Option<PendingVisit>>>,
    sync_mutex: Arc<Mutex<()>>,
    app_handle: Option<AppHandle>,
) {
    let mut watched_log_file = None;
    let mut visit_session = VisitSession::new();
    let mut visit_recorder = VisitRecorder::new(db_path);
    let mut previous_vrchat_running = inspect_vrchat_running();

    if let Err(error) =
        recover_unfinished_visits(&log_dir, &mut visit_recorder, app_handle.as_ref())
    {
        emit_error(
            app_handle.as_ref(),
            format!("Failed to recover unfinished VRChat visits: {}", error),
        );
    }

    let mut synced_closed_latest_log_once = false;

    while !stop_requested.load(Ordering::Relaxed) {
        // VRChatの起動確認
        let current_vrchat_running = inspect_vrchat_running();
        if did_vrchat_start(previous_vrchat_running, current_vrchat_running) {
            watched_log_file = None;
            visit_session = VisitSession::new();
            update_current_visit(&current_visit, None);
            emit_current_visit_changed(app_handle.as_ref());
        }

        // VRChatの前回状態は起動中かつ今回が未起動か確認
        if did_vrchat_stop(previous_vrchat_running, current_vrchat_running) {
            finalize_current_visit_on_vrchat_exit(
                watched_log_file.as_mut(),
                &mut visit_session,
                &mut visit_recorder,
                &current_visit,
                app_handle.as_ref(),
            );
        }

        if current_vrchat_running.is_some() {
            previous_vrchat_running = current_vrchat_running;
        }

        if !matches!(current_vrchat_running, Some(true)) {
            if !synced_closed_latest_log_once {
                let _sync_guard = sync_mutex
                    .lock()
                    .expect("log sync mutex should not be poisoned");
                match poll_latest_log_file(
                    &log_dir,
                    &mut watched_log_file,
                    &mut visit_session,
                    &mut visit_recorder,
                    &current_visit,
                    app_handle.as_ref(),
                ) {
                    Ok(()) => {}
                    Err(error) => {
                        emit_error(
                            app_handle.as_ref(),
                            format!(
                                "Failed to inspect VRChat log directory {}: {}",
                                log_dir.display(),
                                error
                            ),
                        );
                    }
                }
                synced_closed_latest_log_once = true;
            }
            thread::sleep(POLL_INTERVAL);
            continue;
        }

        synced_closed_latest_log_once = false;

        {
            let _sync_guard = sync_mutex
                .lock()
                .expect("log sync mutex should not be poisoned");
            match poll_latest_log_file(
                &log_dir,
                &mut watched_log_file,
                &mut visit_session,
                &mut visit_recorder,
                &current_visit,
                app_handle.as_ref(),
            ) {
                Ok(()) => {}
                Err(error) => {
                    emit_error(
                        app_handle.as_ref(),
                        format!(
                            "Failed to inspect VRChat log directory {}: {}",
                            log_dir.display(),
                            error
                        ),
                    );
                }
            }
        }

        thread::sleep(POLL_INTERVAL);
    }

    // アプリ終了時には current visit を無理に確定しない。
    // 入室時点で stay_duration_seconds = NULL の行は既にDBへ保存済みのため、
    // ここでは未読ログのdrainのみ行い、退出ログが見つからなければNULL行のまま残す。
    drain_watched_log_file_to_eof(
        watched_log_file.as_mut(),
        &mut visit_session,
        &mut visit_recorder,
        &current_visit,
        app_handle.as_ref(),
    );
    discard_current_visit(&mut visit_session, &current_visit, app_handle.as_ref());
}

fn poll_latest_log_file(
    log_dir: &Path,
    watched_log_file: &mut Option<WatchedLogFile>,
    visit_session: &mut VisitSession,
    visit_recorder: &mut VisitRecorder,
    current_visit: &Arc<Mutex<Option<PendingVisit>>>,
    app_handle: Option<&AppHandle>,
) -> std::io::Result<()> {
    let Some(snapshot) = latest_log_snapshot(log_dir)? else {
        return Ok(());
    };

    match watched_log_file {
        Some(current) if current.path == snapshot.path => {
            if snapshot.size < current.offset {
                current.offset = snapshot.size;
                return Ok(());
            }

            if snapshot.size == current.offset {
                return Ok(());
            }

            let read_result = read_lines_from_offset(&current.path, current.offset)?;
            current.offset = read_result.next_offset;

            process_log_lines_with_recorder(
                read_result.lines,
                current.path.file_name().and_then(|name| name.to_str()),
                visit_session,
                visit_recorder,
                current_visit,
                app_handle,
            );
        }
        Some(_) => {
            // ログ切替だけでは visit を確定しない。確定は LeaveWorld のみで行う。
            *visit_session = VisitSession::new();
            update_current_visit(current_visit, None);
            emit_current_visit_changed(app_handle);

            let read_result = read_lines_from_offset(&snapshot.path, 0)?;

            process_log_lines_with_recorder(
                read_result.lines,
                snapshot.path.file_name().and_then(|name| name.to_str()),
                visit_session,
                visit_recorder,
                current_visit,
                app_handle,
            );

            watched_log_file.replace(WatchedLogFile {
                path: snapshot.path,
                offset: read_result.next_offset,
            });
        }
        None => {
            *visit_session = VisitSession::new();
            update_current_visit(current_visit, None);

            let read_result = read_lines_from_offset(&snapshot.path, 0)?;

            process_log_lines_with_recorder(
                read_result.lines,
                snapshot.path.file_name().and_then(|name| name.to_str()),
                visit_session,
                visit_recorder,
                current_visit,
                app_handle,
            );

            watched_log_file.replace(WatchedLogFile {
                path: snapshot.path,
                offset: read_result.next_offset,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
pub fn process_log_lines(
    lines: Vec<String>,
    source_log_file: Option<&str>,
    visit_session: &mut VisitSession,
    db_path: &Path,
    app_handle: Option<&AppHandle>,
) -> Vec<VisitSavedPayload> {
    let mut visit_recorder = VisitRecorder::new(db_path.to_path_buf());
    let current_visit = Arc::new(Mutex::new(visit_session.current_visit().cloned()));

    process_log_lines_with_recorder(
        lines,
        source_log_file,
        visit_session,
        &mut visit_recorder,
        &current_visit,
        app_handle,
    )
}

fn process_log_lines_with_recorder(
    lines: Vec<String>,
    source_log_file: Option<&str>,
    visit_session: &mut VisitSession,
    visit_recorder: &mut VisitRecorder,
    current_visit: &Arc<Mutex<Option<PendingVisit>>>,
    app_handle: Option<&AppHandle>,
) -> Vec<VisitSavedPayload> {
    let mut saved_visits = Vec::new();
    let actions = process_log_lines_to_actions(lines, source_log_file, visit_session);

    for action in actions {
        match action {
            ProcessedLogAction::CurrentVisitChanged(next_current_visit) => {
                if let Some(pending_visit) = next_current_visit.as_ref() {
                    match visit_recorder.save_pending_visit_if_missing(pending_visit) {
                        Ok(Some(payload)) => {
                            emit_visit_saved(app_handle, &payload);
                            saved_visits.push(payload);
                        }
                        Ok(None) => {}
                        Err(error) => emit_error(app_handle, error),
                    }
                }

                update_current_visit(current_visit, next_current_visit);
                emit_current_visit_changed(app_handle);
            }
            ProcessedLogAction::FinalizedVisit(visit_input) => {
                match visit_recorder.save_finalized_visit(&visit_input) {
                    Ok(Some(payload)) => {
                        emit_visit_saved(app_handle, &payload);
                        saved_visits.push(payload);
                    }
                    Ok(None) => {}
                    Err(error) => emit_error(app_handle, error),
                }
            }
        }
    }

    saved_visits
}

fn drain_watched_log_file_to_eof(
    watched_log_file: Option<&mut WatchedLogFile>,
    visit_session: &mut VisitSession,
    visit_recorder: &mut VisitRecorder,
    current_visit: &Arc<Mutex<Option<PendingVisit>>>,
    app_handle: Option<&AppHandle>,
) -> Vec<VisitSavedPayload> {
    let Some(watched_log_file) = watched_log_file else {
        return Vec::new();
    };

    let read_result = match read_lines_from_offset(&watched_log_file.path, watched_log_file.offset)
    {
        Ok(read_result) => read_result,
        Err(error) => {
            emit_error(
                app_handle,
                format!(
                    "VRChat log cannot be drained to EOF {}: {}",
                    watched_log_file.path.display(),
                    error
                ),
            );
            return Vec::new();
        }
    };

    watched_log_file.offset = read_result.next_offset;

    process_log_lines_with_recorder(
        read_result.lines,
        watched_log_file
            .path
            .file_name()
            .and_then(|name| name.to_str()),
        visit_session,
        visit_recorder,
        current_visit,
        app_handle,
    )
}

fn finalize_current_visit_on_vrchat_exit(
    watched_log_file: Option<&mut WatchedLogFile>,
    visit_session: &mut VisitSession,
    visit_recorder: &mut VisitRecorder,
    current_visit: &Arc<Mutex<Option<PendingVisit>>>,
    app_handle: Option<&AppHandle>,
) -> Vec<VisitSavedPayload> {
    let Some(watched_log_file) = watched_log_file else {
        discard_current_visit(visit_session, current_visit, app_handle);
        return Vec::new();
    };

    let mut saved_visits = drain_watched_log_file_to_eof(
        Some(watched_log_file),
        visit_session,
        visit_recorder,
        current_visit,
        app_handle,
    );

    let Some(pending_visit) = visit_session.current_visit().cloned() else {
        return saved_visits;
    };

    let source_log_file = pending_visit.source_log_file.clone().or_else(|| {
        watched_log_file
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
    });

    let Some(source_log_file) = source_log_file else {
        discard_current_visit(visit_session, current_visit, app_handle);
        return saved_visits;
    };

    let read_result = match read_lines_from_offset(&watched_log_file.path, 0) {
        Ok(read_result) => read_result,
        Err(error) => {
            emit_error(
                app_handle,
                format!(
                    "VRChat log cannot be read for exit finalization {}: {}",
                    watched_log_file.path.display(),
                    error
                ),
            );
            discard_current_visit(visit_session, current_visit, app_handle);
            return saved_visits;
        }
    };

    if let Some(end_timestamp) = infer_current_visit_end_timestamp_from_log_lines(
        &pending_visit,
        &read_result.lines,
        &source_log_file,
    ) {
        if let Some(visit_input) = visit_session.leave_world(&end_timestamp) {
            match visit_recorder.save_finalized_visit(&visit_input) {
                Ok(Some(payload)) => {
                    emit_visit_saved(app_handle, &payload);
                    saved_visits.push(payload);
                }
                Ok(None) => {}
                Err(error) => emit_error(app_handle, error),
            }
        }
    }

    discard_current_visit(visit_session, current_visit, app_handle);

    saved_visits
}

fn discard_current_visit(
    visit_session: &mut VisitSession,
    current_visit: &Arc<Mutex<Option<PendingVisit>>>,
    app_handle: Option<&AppHandle>,
) {
    let has_shared_current_visit = current_visit
        .lock()
        .expect("current visit mutex should not be poisoned")
        .is_some();

    if visit_session.current_visit().is_none() && !has_shared_current_visit {
        return;
    }

    visit_session.discard_current_visit();
    update_current_visit(current_visit, None);
    emit_current_visit_changed(app_handle);
}

#[allow(dead_code)]
fn recover_unfinished_visits(
    log_dir: &Path,
    visit_recorder: &mut VisitRecorder,
    app_handle: Option<&AppHandle>,
) -> Result<(), String> {
    let unfinished_visits = visit_recorder.list_unfinished_visits(200)?;

    for visit in unfinished_visits {
        let Some(source_log_file) = visit.source_log_file.as_deref() else {
            continue;
        };

        let log_path = resolve_source_log_path(log_dir, source_log_file);

        if !log_path.exists() {
            continue;
        }

        let read_result = read_lines_from_offset(&log_path, 0).map_err(|error| {
            format!(
                "VRChat log cannot be read for unfinished visit recovery {}: {}",
                log_path.display(),
                error
            )
        })?;

        let Some(end_timestamp) =
            infer_visit_end_timestamp_from_log_lines(&visit, &read_result.lines, source_log_file)
        else {
            continue;
        };

        match visit_recorder.update_unfinished_visit_duration(&visit, &end_timestamp) {
            Ok(Some(payload)) => emit_visit_saved(app_handle, &payload),
            Ok(None) => {}
            Err(error) => emit_error(app_handle, error),
        }
    }

    Ok(())
}

fn infer_visit_end_timestamp_from_log_lines(
    visit: &RecoverableVisit,
    lines: &[String],
    source_log_file: &str,
) -> Option<String> {
    let start_seconds = parse_timestamp_seconds(&visit.visited_at)?;

    for line in lines {
        let Some(event) = parse_log_event(line, Some(source_log_file.to_string())) else {
            continue;
        };

        match event {
            ParsedLogEvent::EnterWorld(next_visit) => {
                let Some(next_seconds) = parse_timestamp_seconds(&next_visit.visited_at) else {
                    continue;
                };

                if next_seconds <= start_seconds {
                    continue;
                }

                if is_duplicate_recovery_visit_event(visit, &next_visit) {
                    continue;
                }

                return None;
            }
            ParsedLogEvent::LeaveWorld(end_timestamp) => {
                let Some(end_seconds) = parse_timestamp_seconds(&end_timestamp) else {
                    continue;
                };

                if end_seconds <= start_seconds {
                    continue;
                }

                return Some(end_timestamp);
            }
            ParsedLogEvent::VisitMetadata(_) => {}
        }
    }

    None
}

fn infer_current_visit_end_timestamp_from_log_lines(
    pending_visit: &PendingVisit,
    lines: &[String],
    source_log_file: &str,
) -> Option<String> {
    let start_seconds = parse_timestamp_seconds(&pending_visit.visited_at)?;

    for line in lines {
        let Some(ParsedLogEvent::LeaveWorld(end_timestamp)) =
            parse_log_event(line, Some(source_log_file.to_string()))
        else {
            continue;
        };

        let Some(end_seconds) = parse_timestamp_seconds(&end_timestamp) else {
            continue;
        };

        if end_seconds > start_seconds {
            return Some(end_timestamp);
        }
    }

    None
}

fn is_duplicate_recovery_visit_event(visit: &RecoverableVisit, next_visit: &PendingVisit) -> bool {
    is_same_recovery_source_log_file(visit, next_visit)
        && is_same_recovery_world(visit, next_visit)
        && timestamps_within_duplicate_window(&visit.visited_at, &next_visit.visited_at)
}

fn is_same_recovery_source_log_file(visit: &RecoverableVisit, next_visit: &PendingVisit) -> bool {
    match (&visit.source_log_file, &next_visit.source_log_file) {
        (Some(current_source), Some(next_source)) => current_source == next_source,
        _ => false,
    }
}

fn is_same_recovery_world(visit: &RecoverableVisit, next_visit: &PendingVisit) -> bool {
    let same_world_id = match (&visit.world_id, &next_visit.world_id) {
        (Some(current_world_id), Some(next_world_id)) => current_world_id == next_world_id,
        _ => false,
    };

    same_world_id || visit.world_name == next_visit.world_name
}

fn timestamps_within_duplicate_window(current_timestamp: &str, next_timestamp: &str) -> bool {
    let Some(current_seconds) = parse_timestamp_seconds(current_timestamp) else {
        return false;
    };
    let Some(next_seconds) = parse_timestamp_seconds(next_timestamp) else {
        return false;
    };

    let delta = next_seconds - current_seconds;

    (0..=5).contains(&delta)
}

fn valid_stay_duration_seconds(start_timestamp: &str, end_timestamp: &str) -> Option<i64> {
    let stay_duration_seconds =
        calculate_stay_duration_from_timestamps(start_timestamp, end_timestamp)?;

    if (1..=MAX_RECOVERY_STAY_SECONDS).contains(&stay_duration_seconds) {
        Some(stay_duration_seconds)
    } else {
        None
    }
}

#[allow(dead_code)]
fn resolve_source_log_path(log_dir: &Path, source_log_file: &str) -> PathBuf {
    let source_log_path = PathBuf::from(source_log_file);

    if source_log_path.is_absolute() {
        source_log_path
    } else {
        log_dir.join(source_log_path)
    }
}

pub fn is_vrchat_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let mut command = Command::new("tasklist");
        command.creation_flags(CREATE_NO_WINDOW);

        let output = command
            .args(["/FI", "IMAGENAME eq VRChat.exe", "/FO", "CSV", "/NH"])
            .output();

        let Ok(output) = output else {
            return false;
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();

        stdout.contains("vrchat.exe")
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

fn inspect_vrchat_running() -> Option<bool> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let mut command = Command::new("tasklist");
        command.creation_flags(CREATE_NO_WINDOW);

        let output = command
            .args(["/FI", "IMAGENAME eq VRChat.exe", "/FO", "CSV", "/NH"])
            .output();

        let Ok(output) = output else {
            return None;
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();

        Some(stdout.contains("vrchat.exe"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Some(false)
    }
}

fn did_vrchat_stop(
    previous_vrchat_running: Option<bool>,
    current_vrchat_running: Option<bool>,
) -> bool {
    matches!(
        (previous_vrchat_running, current_vrchat_running),
        (Some(true), Some(false))
    )
}

fn did_vrchat_start(
    previous_vrchat_running: Option<bool>,
    current_vrchat_running: Option<bool>,
) -> bool {
    matches!(
        (previous_vrchat_running, current_vrchat_running),
        (Some(false), Some(true))
    )
}

fn open_initialized_database(db_path: &Path) -> Result<Connection, String> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "WorldRec database directory cannot be created {}: {}",
                parent.display(),
                error
            )
        })?;
    }

    let connection = open_database(db_path).map_err(|error| {
        format!(
            "WorldRec database cannot be opened {}: {}",
            db_path.display(),
            error
        )
    })?;

    initialize_database(&connection).map_err(|error| {
        format!(
            "WorldRec database cannot be initialized {}: {}",
            db_path.display(),
            error
        )
    })?;

    Ok(connection)
}

fn emit_visit_saved(app_handle: Option<&AppHandle>, payload: &VisitSavedPayload) {
    if let Some(app_handle) = app_handle {
        let _ = app_handle.emit(VISIT_SAVED_EVENT, payload);
    }
}

fn emit_current_visit_changed(app_handle: Option<&AppHandle>) {
    if let Some(app_handle) = app_handle {
        let _ = app_handle.emit(CURRENT_VISIT_CHANGED_EVENT, ());
    }
}

fn update_current_visit(
    current_visit: &Arc<Mutex<Option<PendingVisit>>>,
    next_visit: Option<PendingVisit>,
) {
    *current_visit
        .lock()
        .expect("current visit mutex should not be poisoned") = next_visit;
}

pub fn emit_error(app_handle: Option<&AppHandle>, message: String) {
    if let Some(app_handle) = app_handle {
        let _ = app_handle.emit(LOG_WATCH_ERROR_EVENT, message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs, sync::atomic::AtomicBool};

    fn unique_temp_dir() -> PathBuf {
        env::temp_dir().join(format!(
            "worldrec-log-watcher-service-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    fn count_visits(db_path: &Path) -> i64 {
        let connection = open_initialized_database(db_path).expect("test database should open");
        connection
            .query_row("SELECT COUNT(*) FROM visit_histories", [], |row| row.get(0))
            .expect("visit count should be queried")
    }

    fn count_null_duration_visits(db_path: &Path) -> i64 {
        let connection = open_initialized_database(db_path).expect("test database should open");
        connection
            .query_row(
                "SELECT COUNT(*) FROM visit_histories WHERE stay_duration_seconds IS NULL",
                [],
                |row| row.get(0),
            )
            .expect("null duration visit count should be queried")
    }

    fn visit_duration_by_world(db_path: &Path, world_name: &str) -> Option<i64> {
        let connection = open_initialized_database(db_path).expect("test database should open");
        connection
            .query_row(
                "
                SELECT stay_duration_seconds
                FROM visit_histories
                WHERE world_name = ?1
                ORDER BY id DESC
                LIMIT 1
                ",
                params![world_name],
                |row| row.get(0),
            )
            .expect("visit duration should be queried by world")
    }

    fn insert_unfinished_visit(
        db_path: &Path,
        visited_at: &str,
        world_name: &str,
        world_id: Option<&str>,
        source_log_file: Option<&str>,
    ) {
        let connection = open_initialized_database(db_path).expect("test database should open");
        connection
            .execute(
                "
                INSERT INTO visit_histories (
                    visited_at,
                    world_name,
                    world_id,
                    stay_duration_seconds,
                    source_log_file,
                    created_at,
                    updated_at
                )
                VALUES (?1, ?2, ?3, NULL, ?4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                ",
                params![visited_at, world_name, world_id, source_log_file],
            )
            .expect("unfinished visit should be inserted");
    }

    fn visit_duration_by_source(
        db_path: &Path,
        visited_at: &str,
        world_name: &str,
        source_log_file: Option<&str>,
    ) -> Option<i64> {
        let connection = open_initialized_database(db_path).expect("test database should open");
        connection
            .query_row(
                "
                SELECT stay_duration_seconds
                FROM visit_histories
                WHERE visited_at = ?1
                  AND world_name = ?2
                  AND COALESCE(source_log_file, '') = COALESCE(?3, '')
                ",
                params![visited_at, world_name, source_log_file],
                |row| row.get(0),
            )
            .expect("visit duration should be queried by source")
    }

    fn setup_session_from_log(
        log_path: &Path,
        current_visit: &Arc<Mutex<Option<PendingVisit>>>,
        visit_session: &mut VisitSession,
        visit_recorder: &mut VisitRecorder,
    ) -> WatchedLogFile {
        let read_result = read_lines_from_offset(log_path, 0).expect("log should be read");

        process_log_lines_with_recorder(
            read_result.lines,
            log_path.file_name().and_then(|name| name.to_str()),
            visit_session,
            visit_recorder,
            current_visit,
            None,
        );

        WatchedLogFile {
            path: log_path.to_path_buf(),
            offset: read_result.next_offset,
        }
    }

    #[test]
    fn recovery_keeps_unfinished_visit_null_without_leave_event() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let source_log_file = "output_log_2026-06-10_20-00-00.txt";
        let source_log_path = dir.join(source_log_file);
        let start_line =
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345";

        fs::write(
            &source_log_path,
            format!("{start_line}\n2026.06.10 20:10:00 Log        -  Network settled\n"),
        )
        .expect("source log should be written");
        fs::write(
            dir.join("output_log_2026-06-10_21-00-00.txt"),
            "2026.06.10 21:00:00 Log        -  Latest log marker\n",
        )
        .expect("newer log should be written");

        let ParsedLogEvent::EnterWorld(visit) =
            parse_log_event(start_line, Some(source_log_file.to_string()))
                .expect("start line should parse")
        else {
            panic!("expected enter world event");
        };

        insert_unfinished_visit(
            &db_path,
            &visit.visited_at,
            &visit.world_name,
            visit.world_id.as_deref(),
            visit.source_log_file.as_deref(),
        );

        let mut visit_recorder = VisitRecorder::new(db_path.clone());
        recover_unfinished_visits(&dir, &mut visit_recorder, None)
            .expect("unfinished visits should recover");

        assert_eq!(
            visit_duration_by_source(
                &db_path,
                &visit.visited_at,
                &visit.world_name,
                Some(source_log_file)
            ),
            None
        );
        assert_eq!(count_null_duration_visits(&db_path), 1);

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn recovery_finalizes_unfinished_visit_with_application_quit_event() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let source_log_file = "output_log_2026-06-10_20-00-00.txt";
        let source_log_path = dir.join(source_log_file);
        let start_line =
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345";

        fs::write(
            &source_log_path,
            format!(
                "{start_line}\n\
2026.06.10 20:10:00 Debug      -  VRCApplication: HandleApplicationQuit at 600.000\n"
            ),
        )
        .expect("source log should be written");

        let ParsedLogEvent::EnterWorld(visit) =
            parse_log_event(start_line, Some(source_log_file.to_string()))
                .expect("start line should parse")
        else {
            panic!("expected enter world event");
        };

        insert_unfinished_visit(
            &db_path,
            &visit.visited_at,
            &visit.world_name,
            visit.world_id.as_deref(),
            visit.source_log_file.as_deref(),
        );

        let mut visit_recorder = VisitRecorder::new(db_path.clone());
        recover_unfinished_visits(&dir, &mut visit_recorder, None)
            .expect("unfinished visits should recover");

        assert_eq!(
            visit_duration_by_source(
                &db_path,
                &visit.visited_at,
                &visit.world_name,
                Some(source_log_file)
            ),
            Some(600)
        );
        assert_eq!(count_null_duration_visits(&db_path), 0);

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn recovery_does_not_finalize_unfinished_visit_from_next_enter_world() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let source_log_file = "output_log_2026-06-10_20-00-00.txt";
        let source_log_path = dir.join(source_log_file);
        let start_line =
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345";

        fs::write(
            &source_log_path,
            format!(
                "{start_line}\n\
2026.06.10 20:30:00 Log        -  [Behaviour] Entering Room: World B wrld_b:67890\n"
            ),
        )
        .expect("source log should be written");

        let ParsedLogEvent::EnterWorld(visit) =
            parse_log_event(start_line, Some(source_log_file.to_string()))
                .expect("start line should parse")
        else {
            panic!("expected enter world event");
        };

        insert_unfinished_visit(
            &db_path,
            &visit.visited_at,
            &visit.world_name,
            visit.world_id.as_deref(),
            visit.source_log_file.as_deref(),
        );

        let mut visit_recorder = VisitRecorder::new(db_path.clone());
        recover_unfinished_visits(&dir, &mut visit_recorder, None)
            .expect("unfinished visits should recover");

        assert_eq!(
            visit_duration_by_source(
                &db_path,
                &visit.visited_at,
                &visit.world_name,
                Some(source_log_file)
            ),
            None
        );
        assert_eq!(count_null_duration_visits(&db_path), 1);

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn recovery_does_not_finalize_unfinished_visit_from_later_visit_leave_world() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let source_log_file = "output_log_2026-06-10_20-00-00.txt";
        let source_log_path = dir.join(source_log_file);
        let start_line =
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345";

        fs::write(
            &source_log_path,
            format!(
                "{start_line}\n\
2026.06.10 20:30:00 Log        -  [Behaviour] Entering Room: World B wrld_b:67890\n\
2026.06.10 21:00:00 Log        -  [Behaviour] OnLeftRoom\n"
            ),
        )
        .expect("source log should be written");

        let ParsedLogEvent::EnterWorld(visit) =
            parse_log_event(start_line, Some(source_log_file.to_string()))
                .expect("start line should parse")
        else {
            panic!("expected enter world event");
        };

        insert_unfinished_visit(
            &db_path,
            &visit.visited_at,
            &visit.world_name,
            visit.world_id.as_deref(),
            visit.source_log_file.as_deref(),
        );

        let mut visit_recorder = VisitRecorder::new(db_path.clone());
        recover_unfinished_visits(&dir, &mut visit_recorder, None)
            .expect("unfinished visits should recover");

        assert_eq!(
            visit_duration_by_source(
                &db_path,
                &visit.visited_at,
                &visit.world_name,
                Some(source_log_file)
            ),
            None
        );
        assert_eq!(count_null_duration_visits(&db_path), 1);

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn recovery_does_not_use_closed_file_fallback_without_leave_event() {
        let source_log_file = "output_log_2026-06-10_20-00-00.txt";
        let start_line =
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345";
        let ParsedLogEvent::EnterWorld(visit) =
            parse_log_event(start_line, Some(source_log_file.to_string()))
                .expect("start line should parse")
        else {
            panic!("expected enter world event");
        };
        let recoverable_visit = RecoverableVisit {
            id: 1,
            visited_at: visit.visited_at,
            world_name: visit.world_name,
            world_id: visit.world_id,
            source_log_file: visit.source_log_file,
        };
        let lines = vec![
            start_line.to_string(),
            "2026.06.10 20:10:00 Log        -  Network settled".to_string(),
        ];

        let end_timestamp =
            infer_visit_end_timestamp_from_log_lines(&recoverable_visit, &lines, source_log_file);

        assert_eq!(end_timestamp, None);
    }

    #[test]
    fn poll_latest_log_file_syncs_closed_latest_log_with_leave_event() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let log_path = dir.join("output_log_2026-06-10_20-00-00.txt");
        fs::write(
            &log_path,
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345\n\
2026.06.10 20:10:00 Log        -  [Behaviour] OnLeftRoom\n",
        )
        .expect("log should be written");

        let current_visit = Arc::new(Mutex::new(None));
        let mut watched_log_file = None;
        let mut visit_session = VisitSession::new();
        let mut visit_recorder = VisitRecorder::new(db_path.clone());

        poll_latest_log_file(
            &dir,
            &mut watched_log_file,
            &mut visit_session,
            &mut visit_recorder,
            &current_visit,
            None,
        )
        .expect("latest log should be polled");

        assert_eq!(count_visits(&db_path), 1);
        assert_eq!(visit_duration_by_world(&db_path, "World A"), Some(600));
        assert_eq!(count_null_duration_visits(&db_path), 0);
        assert!(visit_session.current_visit().is_none());
        assert!(current_visit.lock().unwrap().is_none());

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn poll_latest_log_file_keeps_closed_latest_log_null_without_leave_event() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let log_path = dir.join("output_log_2026-06-10_20-00-00.txt");
        fs::write(
            &log_path,
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345\n",
        )
        .expect("log should be written");

        let current_visit = Arc::new(Mutex::new(None));
        let mut watched_log_file = None;
        let mut visit_session = VisitSession::new();
        let mut visit_recorder = VisitRecorder::new(db_path.clone());

        poll_latest_log_file(
            &dir,
            &mut watched_log_file,
            &mut visit_session,
            &mut visit_recorder,
            &current_visit,
            None,
        )
        .expect("latest log should be polled");

        assert_eq!(count_visits(&db_path), 1);
        assert_eq!(visit_duration_by_world(&db_path, "World A"), None);
        assert_eq!(count_null_duration_visits(&db_path), 1);
        assert_eq!(
            visit_session
                .current_visit()
                .expect("visit should remain current")
                .world_name,
            "World A"
        );
        assert_eq!(
            current_visit
                .lock()
                .unwrap()
                .as_ref()
                .expect("shared current visit should remain")
                .world_name,
            "World A"
        );

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn poll_latest_log_file_is_idempotent_with_same_watched_file_offset() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let log_path = dir.join("output_log_2026-06-10_20-00-00.txt");
        fs::write(
            &log_path,
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345\n\
2026.06.10 20:10:00 Log        -  [Behaviour] OnLeftRoom\n",
        )
        .expect("log should be written");

        let current_visit = Arc::new(Mutex::new(None));
        let mut watched_log_file = None;
        let mut visit_session = VisitSession::new();
        let mut visit_recorder = VisitRecorder::new(db_path.clone());

        poll_latest_log_file(
            &dir,
            &mut watched_log_file,
            &mut visit_session,
            &mut visit_recorder,
            &current_visit,
            None,
        )
        .expect("latest log should be polled first time");
        poll_latest_log_file(
            &dir,
            &mut watched_log_file,
            &mut visit_session,
            &mut visit_recorder,
            &current_visit,
            None,
        )
        .expect("latest log should be polled second time");

        assert_eq!(count_visits(&db_path), 1);
        assert_eq!(visit_duration_by_world(&db_path, "World A"), Some(600));
        assert_eq!(count_null_duration_visits(&db_path), 0);

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn sync_latest_log_once_processes_latest_log_from_start_without_duplicates() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let log_path = dir.join("output_log_2026-06-10_20-00-00.txt");
        fs::write(
            &log_path,
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345\n\
2026.06.10 20:10:00 Log        -  [Behaviour] OnLeftRoom\n",
        )
        .expect("log should be written");
        let current_visit = Arc::new(Mutex::new(None));

        let first = sync_latest_log_once(&dir, &db_path, &current_visit, None)
            .expect("first manual sync should succeed");
        let second = sync_latest_log_once(&dir, &db_path, &current_visit, None)
            .expect("second manual sync should succeed");

        assert_eq!(
            first.latest_log_file.as_deref(),
            Some("output_log_2026-06-10_20-00-00.txt")
        );
        assert!(first.processed);
        assert_eq!(first.processed_line_count, 2);
        assert_eq!(first.saved_visit_count, 2);
        assert_eq!(second.processed_line_count, 2);
        assert_eq!(second.saved_visit_count, 0);
        assert_eq!(count_visits(&db_path), 1);
        assert_eq!(visit_duration_by_world(&db_path, "World A"), Some(600));
        assert_eq!(count_null_duration_visits(&db_path), 0);
        assert!(current_visit.lock().unwrap().is_none());

        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn sync_latest_log_once_updates_current_visit_for_open_visit() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let log_path = dir.join("output_log_2026-06-10_20-00-00.txt");
        fs::write(
            &log_path,
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345\n",
        )
        .expect("log should be written");
        let current_visit = Arc::new(Mutex::new(None));

        let result = sync_latest_log_once(&dir, &db_path, &current_visit, None)
            .expect("manual sync should succeed");

        assert!(result.processed);
        assert_eq!(result.processed_line_count, 1);
        assert_eq!(result.saved_visit_count, 1);
        assert_eq!(count_visits(&db_path), 1);
        assert_eq!(count_null_duration_visits(&db_path), 1);
        assert_eq!(
            result
                .current_visit
                .as_ref()
                .expect("current visit should be returned")
                .world_name,
            "World A"
        );
        assert_eq!(
            current_visit
                .lock()
                .unwrap()
                .as_ref()
                .expect("shared current visit should be updated")
                .world_name,
            "World A"
        );

        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn identity_fallback_updates_only_matching_source_log_file_without_world_id() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let visited_at = "2026.06.10 20:00:00";

        insert_unfinished_visit(
            &db_path,
            visited_at,
            "World A",
            None,
            Some("output_log_a.txt"),
        );
        insert_unfinished_visit(
            &db_path,
            visited_at,
            "World A",
            None,
            Some("output_log_b.txt"),
        );

        let mut visit_recorder = VisitRecorder::new(db_path.clone());
        let updated = visit_recorder
            .update_unfinished_visit_duration_by_identity(
                visited_at,
                "World A",
                None,
                Some("output_log_a.txt"),
                600,
            )
            .expect("fallback update should succeed");

        assert!(updated);
        assert_eq!(
            visit_duration_by_source(&db_path, visited_at, "World A", Some("output_log_a.txt")),
            Some(600)
        );
        assert_eq!(
            visit_duration_by_source(&db_path, visited_at, "World A", Some("output_log_b.txt")),
            None
        );

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn identity_fallback_updates_only_matching_source_log_file_with_world_id() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let visited_at = "2026.06.10 20:00:00";

        insert_unfinished_visit(
            &db_path,
            visited_at,
            "World A",
            Some("wrld_a"),
            Some("output_log_a.txt"),
        );
        insert_unfinished_visit(
            &db_path,
            visited_at,
            "World A Duplicate Name",
            Some("wrld_a"),
            Some("output_log_b.txt"),
        );

        let mut visit_recorder = VisitRecorder::new(db_path.clone());
        let updated = visit_recorder
            .update_unfinished_visit_duration_by_identity(
                visited_at,
                "World A",
                Some("wrld_a"),
                Some("output_log_a.txt"),
                600,
            )
            .expect("fallback update should succeed");

        assert!(updated);
        assert_eq!(
            visit_duration_by_source(&db_path, visited_at, "World A", Some("output_log_a.txt")),
            Some(600)
        );
        assert_eq!(
            visit_duration_by_source(
                &db_path,
                visited_at,
                "World A Duplicate Name",
                Some("output_log_b.txt")
            ),
            None
        );

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn identity_fallback_matches_null_source_log_file_only_with_null_input() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let visited_at = "2026.06.10 20:00:00";
        let other_visited_at = "2026.06.10 21:00:00";

        insert_unfinished_visit(&db_path, visited_at, "World A", None, None);
        insert_unfinished_visit(
            &db_path,
            visited_at,
            "World A",
            None,
            Some("output_log_a.txt"),
        );
        insert_unfinished_visit(&db_path, other_visited_at, "World B", None, None);

        let mut visit_recorder = VisitRecorder::new(db_path.clone());
        let updated_null_source = visit_recorder
            .update_unfinished_visit_duration_by_identity(visited_at, "World A", None, None, 600)
            .expect("fallback update should succeed for null source");
        let updated_some_source = visit_recorder
            .update_unfinished_visit_duration_by_identity(
                other_visited_at,
                "World B",
                None,
                Some("output_log_b.txt"),
                900,
            )
            .expect("fallback update should succeed without matching row");

        assert!(updated_null_source);
        assert!(!updated_some_source);
        assert_eq!(
            visit_duration_by_source(&db_path, visited_at, "World A", None),
            Some(600)
        );
        assert_eq!(
            visit_duration_by_source(&db_path, visited_at, "World A", Some("output_log_a.txt")),
            None
        );
        assert_eq!(
            visit_duration_by_source(&db_path, other_visited_at, "World B", None),
            None
        );

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn vrchat_stop_transition_ignores_process_inspection_failure() {
        assert!(did_vrchat_stop(Some(true), Some(false)));
        assert!(!did_vrchat_stop(Some(true), None));
        assert!(!did_vrchat_stop(None, Some(false)));
        assert!(!did_vrchat_stop(Some(false), Some(false)));
    }

    #[test]
    fn vrchat_start_transition_ignores_process_inspection_failure() {
        assert!(did_vrchat_start(Some(false), Some(true)));
        assert!(!did_vrchat_start(Some(false), None));
        assert!(!did_vrchat_start(None, Some(true)));
        assert!(!did_vrchat_start(Some(true), Some(true)));
    }

    #[test]
    fn vrchat_exit_saves_current_visit_with_on_left_room_timestamp() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let log_path = dir.join("output_log_2026-06-10_20-00-00.txt");
        fs::write(
            &log_path,
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345\n",
        )
        .expect("log should be written");

        let current_visit = Arc::new(Mutex::new(None));
        let mut visit_session = VisitSession::new();
        let mut visit_recorder = VisitRecorder::new(db_path.clone());
        let mut watched_log_file = setup_session_from_log(
            &log_path,
            &current_visit,
            &mut visit_session,
            &mut visit_recorder,
        );
        fs::write(
            &log_path,
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345\n\
2026.06.10 20:10:00 Log        -  [Behaviour] OnLeftRoom\n",
        )
        .expect("log should be appended");

        let saved = finalize_current_visit_on_vrchat_exit(
            Some(&mut watched_log_file),
            &mut visit_session,
            &mut visit_recorder,
            &current_visit,
            None,
        );

        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].stay_duration_seconds, Some(600));
        assert_eq!(count_visits(&db_path), 1);
        assert_eq!(count_null_duration_visits(&db_path), 0);
        assert!(visit_session.current_visit().is_none());
        assert!(current_visit.lock().unwrap().is_none());

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn resyncing_same_log_does_not_duplicate_finalized_visit() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let log_path = dir.join("output_log_2026-06-10_20-00-00.txt");
        let log_contents =
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345\n\
2026.06.10 20:10:00 Log        -  [Behaviour] OnLeftRoom\n";
        fs::write(&log_path, log_contents).expect("log should be written");

        // 1回目の同期: NULL行をINSERTし、その後OnLeftRoomで確定する。
        let mut visit_session = VisitSession::new();
        let saved_first = process_log_lines(
            log_contents.lines().map(|line| line.to_string()).collect(),
            log_path.file_name().and_then(|name| name.to_str()),
            &mut visit_session,
            &db_path,
            None,
        );
        assert_eq!(saved_first.len(), 2);
        assert_eq!(
            saved_first
                .last()
                .expect("finalize event should exist")
                .stay_duration_seconds,
            Some(600)
        );
        assert_eq!(count_visits(&db_path), 1);
        assert_eq!(count_null_duration_visits(&db_path), 0);

        // 2回目の同期: 新しいVisitSession/watcher状態で同じログを再度流す。
        let mut resynced_visit_session = VisitSession::new();
        let saved_second = process_log_lines(
            log_contents.lines().map(|line| line.to_string()).collect(),
            log_path.file_name().and_then(|name| name.to_str()),
            &mut resynced_visit_session,
            &db_path,
            None,
        );

        assert!(saved_second.is_empty());
        assert_eq!(count_visits(&db_path), 1);
        assert_eq!(count_null_duration_visits(&db_path), 0);
        assert_eq!(visit_duration_by_world(&db_path, "World A"), Some(600));

        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn different_source_log_file_is_treated_as_separate_visit() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let visited_at = "2026-06-10T20:00:00";

        insert_unfinished_visit(&db_path, visited_at, "World A", None, Some("log_a.txt"));
        let connection = open_initialized_database(&db_path).expect("test database should open");
        connection
            .execute(
                "UPDATE visit_histories SET stay_duration_seconds = 600 WHERE source_log_file = ?1",
                params!["log_a.txt"],
            )
            .expect("existing visit should be finalized");
        drop(connection);

        let mut visit_recorder = VisitRecorder::new(db_path.clone());
        let pending_visit = PendingVisit {
            visited_at: visited_at.to_string(),
            world_name: "World A".to_string(),
            world_id: None,
            instance_id: None,
            instance_access_type: None,
            instance_nonce: None,
            instance_raw_tags: None,
            source_log_file: Some("log_b.txt".to_string()),
        };

        let saved = visit_recorder
            .save_pending_visit_if_missing(&pending_visit)
            .expect("save should succeed for a different source log file");

        assert!(saved.is_some());
        assert_eq!(count_visits(&db_path), 2);
        assert_eq!(count_null_duration_visits(&db_path), 1);

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn metadata_completion_still_applies_when_unfinished_row_exists() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let visited_at = "2026-06-10T20:00:00";

        insert_unfinished_visit(&db_path, visited_at, "World A", None, Some("log_a.txt"));

        let mut visit_recorder = VisitRecorder::new(db_path.clone());
        let pending_visit = PendingVisit {
            visited_at: visited_at.to_string(),
            world_name: "World A".to_string(),
            world_id: Some("wrld_a".to_string()),
            instance_id: Some("12345".to_string()),
            instance_access_type: None,
            instance_nonce: None,
            instance_raw_tags: None,
            source_log_file: Some("log_a.txt".to_string()),
        };

        let saved = visit_recorder
            .save_pending_visit_if_missing(&pending_visit)
            .expect("metadata completion should succeed");

        assert!(saved.is_none());
        assert_eq!(count_visits(&db_path), 1);
        assert_eq!(count_null_duration_visits(&db_path), 1);

        let connection = open_initialized_database(&db_path).expect("test database should open");
        let world_id: Option<String> = connection
            .query_row(
                "SELECT world_id FROM visit_histories WHERE source_log_file = ?1",
                params!["log_a.txt"],
                |row| row.get(0),
            )
            .expect("world_id should be queried");
        assert_eq!(world_id, Some("wrld_a".to_string()));

        drop(connection);
        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn vrchat_exit_keeps_current_visit_as_null_when_leave_is_missing() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let log_path = dir.join("output_log_2026-06-10_20-00-00.txt");
        fs::write(
            &log_path,
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345\n\
2026.06.10 20:05:00 Log        -  Network settled before shutdown\n",
        )
        .expect("log should be written");

        let current_visit = Arc::new(Mutex::new(None));
        let mut visit_session = VisitSession::new();
        let mut visit_recorder = VisitRecorder::new(db_path.clone());
        let mut watched_log_file = setup_session_from_log(
            &log_path,
            &current_visit,
            &mut visit_session,
            &mut visit_recorder,
        );

        let saved = finalize_current_visit_on_vrchat_exit(
            Some(&mut watched_log_file),
            &mut visit_session,
            &mut visit_recorder,
            &current_visit,
            None,
        );

        assert!(saved.is_empty());
        assert_eq!(count_visits(&db_path), 1);
        assert_eq!(count_null_duration_visits(&db_path), 1);
        assert!(visit_session.current_visit().is_none());
        assert!(current_visit.lock().unwrap().is_none());

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn worldrec_stop_drains_finalized_visits_but_discards_remaining_current_visit() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let log_path = dir.join("output_log_2026-06-10_20-00-00.txt");
        fs::write(
            &log_path,
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345\n",
        )
        .expect("log should be written");

        let current_visit = Arc::new(Mutex::new(None));
        let stop_requested = Arc::new(AtomicBool::new(false));
        let mut visit_session = VisitSession::new();
        let mut visit_recorder = VisitRecorder::new(db_path.clone());
        let mut watched_log_file = setup_session_from_log(
            &log_path,
            &current_visit,
            &mut visit_session,
            &mut visit_recorder,
        );
        fs::write(
            &log_path,
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345\n\
2026.06.10 20:30:00 Log        -  [Behaviour] OnLeftRoom\n\
2026.06.10 20:31:00 Log        -  [Behaviour] Entering Room: World B wrld_b:67890\n",
        )
        .expect("log should be appended");

        drain_watched_log_file_to_eof(
            Some(&mut watched_log_file),
            &mut visit_session,
            &mut visit_recorder,
            &current_visit,
            None,
        );
        assert_eq!(count_visits(&db_path), 2);
        assert_eq!(visit_duration_by_world(&db_path, "World A"), Some(1800));
        assert_eq!(count_null_duration_visits(&db_path), 1);
        assert_eq!(
            visit_session
                .current_visit()
                .expect("second visit should remain current")
                .world_name,
            "World B"
        );

        stop_requested.store(true, Ordering::Relaxed);
        discard_current_visit(&mut visit_session, &current_visit, None);

        assert_eq!(count_visits(&db_path), 2);
        assert_eq!(count_null_duration_visits(&db_path), 1);
        assert!(visit_session.current_visit().is_none());
        assert!(current_visit.lock().unwrap().is_none());

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn vrchat_exit_discards_current_visit_when_end_timestamp_is_missing() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let db_path = dir.join("worldrec.db");
        let log_path = dir.join("output_log_2026-06-10_20-00-00.txt");
        fs::write(
            &log_path,
            "2026.06.10 20:00:00 Log        -  [Behaviour] Entering Room: World A wrld_a:12345\n",
        )
        .expect("log should be written");

        let current_visit = Arc::new(Mutex::new(None));
        let mut visit_session = VisitSession::new();
        let mut visit_recorder = VisitRecorder::new(db_path.clone());
        let mut watched_log_file = setup_session_from_log(
            &log_path,
            &current_visit,
            &mut visit_session,
            &mut visit_recorder,
        );

        let saved = finalize_current_visit_on_vrchat_exit(
            Some(&mut watched_log_file),
            &mut visit_session,
            &mut visit_recorder,
            &current_visit,
            None,
        );

        assert!(saved.is_empty());
        assert_eq!(count_visits(&db_path), 1);
        assert_eq!(count_null_duration_visits(&db_path), 1);
        assert!(visit_session.current_visit().is_none());
        assert!(current_visit.lock().unwrap().is_none());

        drop(visit_recorder);
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[test]
    fn effective_path_comparison_ignores_trailing_separator() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let with_separator = PathBuf::from(format!("{}\\", dir.display()));

        assert!(effective_paths_equal(&dir, &with_separator));

        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn effective_path_comparison_is_case_insensitive_on_windows() {
        let left = PathBuf::from(r"C:\Users\WorldRec\worldrec.db");
        let right = PathBuf::from(r"c:\users\worldrec\WORLDREC.DB");

        assert!(effective_paths_equal(&left, &right));
    }

    #[test]
    fn relative_configured_paths_are_rejected() {
        assert!(resolve_log_dir("relative\\logs").is_err());
        assert!(resolve_db_path("relative\\worldrec.db").is_err());
    }

    #[test]
    fn database_path_with_file_parent_is_rejected_as_unwritable() {
        let dir = unique_temp_dir();
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let file_parent = dir.join("not-a-directory");
        fs::write(&file_parent, "blocker").expect("blocking file should be written");

        let result = validate_db_path(&file_parent.join("worldrec.db"));

        assert!(result.is_err());
        fs::remove_dir_all(dir).expect("temp dir should be removed");
    }
}

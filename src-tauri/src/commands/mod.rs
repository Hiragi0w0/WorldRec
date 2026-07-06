use chrono::{DateTime, Duration, Local, NaiveDate};
use rusqlite::{params, params_from_iter, types::Value, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tauri::{AppHandle, State};

use crate::db::{
    connection::open_database,
    models::{PendingVisit, VisitHistory, WorldPreviewCache, WorldPreviewUpsert},
    repository::{self, LibraryQueryCriteria, LibraryWorldRow, LibraryWorldVisitRow},
    schema::initialize_database,
};
use crate::log_watcher::service::{is_vrchat_running, resolve_db_path, resolve_log_dir};
use crate::log_watcher::state::{LogWatcherState, LogWatcherStatus};
use crate::settings::AppSettings;

const DEFAULT_RECENT_VISIT_LIMIT: i64 = 100;
const MIN_RECENT_VISIT_LIMIT: i64 = 1;
const MAX_RECENT_VISIT_LIMIT: i64 = 500;
const WORLD_PREVIEW_FAILURE_COOLDOWN_HOURS: i64 = 6;
const WORLD_PREVIEW_SUCCESS_TTL_DAYS: i64 = 30;

#[derive(Debug, Clone, Deserialize)]
pub struct VisitFilterCriteria {
    pub mode: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub world_name: Option<String>,
    pub tag: Option<String>,
    pub instance_access_type: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VisitHistoryDto {
    pub id: i64,
    pub visited_at: String,
    pub world_name: String,
    pub world_id: Option<String>,
    pub instance_id: Option<String>,
    pub instance_access_type: Option<String>,
    pub stay_duration_seconds: Option<i64>,
    pub memo: Option<String>,
    pub tags: Option<String>,
    pub source_log_file: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub thumbnail_url: Option<String>,
    pub image_url: Option<String>,
    pub world_preview_fetched_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStatusDto {
    pub db_path: String,
    pub log_dir: String,
    pub watcher_running: bool,
    pub watcher_last_error: Option<String>,
    pub vrchat_running: bool,
    pub visit_count: i64,
    pub latest_visit_at: Option<String>,
    pub latest_world_name: Option<String>,
    pub current_visit: Option<CurrentVisitDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CurrentVisitDto {
    pub visited_at: String,
    pub world_name: String,
    pub world_id: Option<String>,
    pub instance_id: Option<String>,
    pub instance_access_type: Option<String>,
    pub source_log_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncLatestLogResult {
    pub latest_log_file: Option<String>,
    pub processed: bool,
    pub processed_line_count: usize,
    pub saved_visit_count: usize,
    pub watcher_running: bool,
    pub current_visit: Option<CurrentVisitDto>,
    pub skipped_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibrarySearchCriteria {
    pub query: Option<String>,
    pub visited_from: Option<String>,
    pub visited_to: Option<String>,
    pub tags: Vec<String>,
    pub memo_query: Option<String>,
    pub sort_key: String,
    pub sort_direction: String,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryWorldDto {
    pub key: String,
    pub world_id: Option<String>,
    pub world_name: String,
    pub thumbnail_url: Option<String>,
    pub image_url: Option<String>,
    pub world_preview_fetched_at: Option<String>,
    pub visit_count: i64,
    pub first_visited_at: String,
    pub last_visited_at: String,
    pub total_stay_duration_seconds: i64,
    pub tags: Vec<String>,
    pub memo_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryWorldPageDto {
    pub items: Vec<LibraryWorldDto>,
    pub total_count: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryWorldVisitDto {
    pub id: i64,
    pub visited_at: String,
    pub stay_duration_seconds: Option<i64>,
    pub memo: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryWorldDetailDto {
    pub world: LibraryWorldDto,
    pub visits: Vec<LibraryWorldVisitDto>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransitionGraphCriteria {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransitionGraphNodeDto {
    pub key: String,
    pub world_id: Option<String>,
    pub world_name: String,
    pub visit_count: i64,
    pub total_stay_seconds: i64,
    pub last_visited_at: String,
    pub degree: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransitionGraphEdgeDto {
    pub key: String,
    pub from: String,
    pub to: String,
    pub from_world_name: String,
    pub to_world_name: String,
    pub transition_count: i64,
    pub latest_transition_at: String,
    pub transition_times: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LongestVisitDto {
    pub key: String,
    pub world_id: Option<String>,
    pub world_name: String,
    pub visited_at: String,
    pub stay_duration_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransitionGraphSummaryDto {
    pub visit_count: i64,
    pub unique_world_count: i64,
    pub transition_count: i64,
    pub top_transition: Option<TransitionGraphEdgeDto>,
    pub top_longest_visit: Option<LongestVisitDto>,
    pub hidden_node_count: i64,
    pub hidden_edge_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct VisitTransitionGraphDto {
    pub start: String,
    pub end: String,
    pub summary: TransitionGraphSummaryDto,
    pub nodes: Vec<TransitionGraphNodeDto>,
    pub edges: Vec<TransitionGraphEdgeDto>,
    pub top_worlds: Vec<TransitionGraphNodeDto>,
    pub top_transitions: Vec<TransitionGraphEdgeDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsDateRangeDto {
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteHistoryResultDto {
    pub deleted_count: usize,
}

impl From<VisitHistory> for VisitHistoryDto {
    fn from(visit: VisitHistory) -> Self {
        Self {
            id: visit.id,
            visited_at: visit.visited_at,
            world_name: visit.world_name,
            world_id: visit.world_id,
            instance_id: visit.instance_id,
            instance_access_type: visit.instance_access_type,
            stay_duration_seconds: visit.stay_duration_seconds,
            memo: visit.memo,
            tags: visit.tags,
            source_log_file: visit.source_log_file,
            created_at: visit.created_at,
            updated_at: visit.updated_at,
            thumbnail_url: None,
            image_url: None,
            world_preview_fetched_at: None,
        }
    }
}

impl From<PendingVisit> for CurrentVisitDto {
    fn from(visit: PendingVisit) -> Self {
        Self {
            visited_at: visit.visited_at,
            world_name: visit.world_name,
            world_id: visit.world_id,
            instance_id: visit.instance_id,
            instance_access_type: visit.instance_access_type,
            source_log_file: visit.source_log_file,
        }
    }
}

impl From<LibraryWorldRow> for LibraryWorldDto {
    fn from(world: LibraryWorldRow) -> Self {
        Self {
            key: world.key,
            world_id: world.world_id,
            world_name: world.world_name,
            thumbnail_url: world.thumbnail_url,
            image_url: world.image_url,
            world_preview_fetched_at: world.world_preview_fetched_at,
            visit_count: world.visit_count,
            first_visited_at: world.first_visited_at,
            last_visited_at: world.last_visited_at,
            total_stay_duration_seconds: world.total_stay_duration_seconds,
            tags: world.tags,
            memo_count: world.memo_count,
        }
    }
}

impl From<LibraryWorldVisitRow> for LibraryWorldVisitDto {
    fn from(visit: LibraryWorldVisitRow) -> Self {
        Self {
            id: visit.id,
            visited_at: visit.visited_at,
            stay_duration_seconds: visit.stay_duration_seconds,
            memo: visit.memo,
        }
    }
}

#[tauri::command]
pub fn get_settings() -> AppSettings {
    let mut settings = crate::settings::load_settings();
    resolve_settings_paths(&mut settings);
    apply_ai_secret_status(&mut settings);

    settings
}

#[tauri::command]
pub fn save_settings(mut settings: AppSettings) -> Result<AppSettings, String> {
    if let Ok(default_log_dir) = resolve_log_dir("") {
        if settings.log_dir.trim() == default_log_dir.to_string_lossy() {
            settings.log_dir.clear();
        }
    }
    if let Ok(default_db_path) = resolve_db_path("") {
        if settings.db_path.trim() == default_db_path.to_string_lossy() {
            settings.db_path.clear();
        }
    }

    let mut saved = crate::settings::save_settings(settings)?;
    resolve_settings_paths(&mut saved);
    apply_ai_secret_status(&mut saved);

    Ok(saved)
}

fn apply_ai_secret_status(settings: &mut AppSettings) {
    settings.has_gemini_api_key = crate::ai::has_gemini_api_key();
}

fn resolve_settings_paths(settings: &mut AppSettings) {
    if let Ok(log_dir) = resolve_log_dir(&settings.log_dir) {
        settings.log_dir = log_dir.to_string_lossy().to_string();
    }
    if let Ok(db_path) = resolve_db_path(&settings.db_path) {
        settings.db_path = db_path.to_string_lossy().to_string();
    }
}

#[tauri::command]
pub fn list_recent_visits(limit: Option<i64>) -> Result<Vec<VisitHistoryDto>, String> {
    list_visits(VisitFilterCriteria {
        mode: Some("recent".to_string()),
        start: None,
        end: None,
        world_name: None,
        tag: None,
        instance_access_type: None,
        limit,
    })
}

#[tauri::command]
pub fn list_visits(criteria: VisitFilterCriteria) -> Result<Vec<VisitHistoryDto>, String> {
    let settings = get_settings();
    let db_path = resolve_db_path(&settings.db_path)?;
    let connection = open_initialized_database(&db_path)?;

    query_visits(&connection, criteria)
}

#[tauri::command]
pub fn list_library_worlds(criteria: LibrarySearchCriteria) -> Result<LibraryWorldPageDto, String> {
    let settings = get_settings();
    let db_path = resolve_db_path(&settings.db_path)?;
    let connection = open_initialized_database(&db_path)?;

    let limit = normalize_library_limit(criteria.limit);
    let offset = criteria.offset.max(0);
    let sort_key = normalize_library_sort_key(&criteria.sort_key);
    let sort_direction = normalize_library_sort_direction(&criteria.sort_direction);
    let query_criteria = LibraryQueryCriteria {
        query: criteria.query,
        visited_from: normalized_range_bound(criteria.visited_from.as_deref(), false),
        visited_to: normalized_range_bound(criteria.visited_to.as_deref(), true),
        tags: criteria.tags,
        memo_query: criteria.memo_query,
        sort_key: sort_key.to_string(),
        sort_direction: sort_direction.to_string(),
        limit,
        offset,
    };

    let (items, total_count) = repository::list_library_worlds(&connection, &query_criteria)
        .map_err(|error| format!("library worlds query failed: {error}"))?;

    Ok(LibraryWorldPageDto {
        items: items.into_iter().map(LibraryWorldDto::from).collect(),
        total_count,
        limit,
        offset,
    })
}

#[tauri::command]
pub fn get_library_world_detail(
    world_id: Option<String>,
    world_name: String,
) -> Result<LibraryWorldDetailDto, String> {
    let settings = get_settings();
    let db_path = resolve_db_path(&settings.db_path)?;
    let connection = open_initialized_database(&db_path)?;

    let Some((world, visits)) =
        repository::get_library_world_group(&connection, world_id.as_deref(), &world_name)
            .map_err(|error| format!("library world detail query failed: {error}"))?
    else {
        // 詳細画面は具体的なワールド指定の結果なので、見つからない場合は
        // 空DTOよりも呼び出し側で扱いやすいnot foundエラーとして返す。
        return Err("library world was not found".to_string());
    };

    Ok(LibraryWorldDetailDto {
        world: LibraryWorldDto::from(world),
        visits: visits.into_iter().map(LibraryWorldVisitDto::from).collect(),
    })
}

#[tauri::command]
pub fn get_runtime_status(state: State<'_, LogWatcherState>) -> Result<RuntimeStatusDto, String> {
    let settings = get_settings();
    let db_path = resolve_db_path(&settings.db_path)?;
    let log_dir = resolve_log_dir(&settings.log_dir)?;
    let connection = open_initialized_database(&db_path)?;
    let watcher_status = state.status();
    let current_visit = state.current_visit();
    let visit_count = visit_count(&connection)?;
    let latest_visit = latest_visit(&connection)?;

    Ok(RuntimeStatusDto {
        db_path: db_path.to_string_lossy().to_string(),
        log_dir: log_dir.to_string_lossy().to_string(),
        watcher_running: watcher_status.running,
        watcher_last_error: watcher_status.last_error,
        vrchat_running: is_vrchat_running(),
        visit_count,
        latest_visit_at: latest_visit.as_ref().map(|visit| visit.visited_at.clone()),
        latest_world_name: latest_visit.map(|visit| visit.world_name),
        current_visit: current_visit.map(CurrentVisitDto::from),
    })
}

#[tauri::command]
pub fn start_log_watcher(
    app_handle: AppHandle,
    state: State<'_, LogWatcherState>,
) -> Result<LogWatcherStatus, String> {
    let settings = get_settings();

    state.start(&settings.log_dir, &settings.db_path, Some(app_handle))
}

#[tauri::command]
pub fn sync_latest_vrchat_log(
    app_handle: AppHandle,
    state: State<'_, LogWatcherState>,
) -> Result<SyncLatestLogResult, String> {
    let settings = get_settings();
    let sync_result =
        state.sync_latest_log_once(&settings.log_dir, &settings.db_path, Some(app_handle))?;
    let watcher_status = state.status();

    Ok(SyncLatestLogResult {
        latest_log_file: sync_result.latest_log_file,
        processed: sync_result.processed,
        processed_line_count: sync_result.processed_line_count,
        saved_visit_count: sync_result.saved_visit_count,
        watcher_running: watcher_status.running,
        current_visit: sync_result.current_visit.map(CurrentVisitDto::from),
        skipped_reason: None,
    })
}

#[tauri::command]
pub fn sync_latest_vrchat_log_before_exit(
    app_handle: AppHandle,
    state: State<'_, LogWatcherState>,
) -> Result<SyncLatestLogResult, String> {
    let settings = get_settings();
    let outcome = state.sync_latest_log_before_exit_if_vrchat_stopped(
        &settings.log_dir,
        &settings.db_path,
        Some(app_handle),
    )?;
    let watcher_status = state.status();

    if !outcome.synced {
        return Ok(SyncLatestLogResult {
            latest_log_file: None,
            processed: false,
            processed_line_count: 0,
            saved_visit_count: 0,
            watcher_running: watcher_status.running,
            current_visit: state.current_visit().map(CurrentVisitDto::from),
            skipped_reason: outcome.skipped_reason,
        });
    }

    let sync_result = outcome
        .result
        .expect("synced exit log sync should return a sync result");

    Ok(SyncLatestLogResult {
        latest_log_file: sync_result.latest_log_file,
        processed: sync_result.processed,
        processed_line_count: sync_result.processed_line_count,
        saved_visit_count: sync_result.saved_visit_count,
        watcher_running: watcher_status.running,
        current_visit: sync_result.current_visit.map(CurrentVisitDto::from),
        skipped_reason: outcome.skipped_reason,
    })
}

#[tauri::command]
pub fn stop_log_watcher(
    app_handle: AppHandle,
    state: State<'_, LogWatcherState>,
) -> Result<LogWatcherStatus, String> {
    Ok(state.stop(Some(app_handle)))
}

#[tauri::command]
pub fn get_log_watcher_status(
    state: State<'_, LogWatcherState>,
) -> Result<LogWatcherStatus, String> {
    Ok(state.status())
}

#[tauri::command]
pub async fn get_vrchat_world_detail(
    world_id: String,
    world_name: Option<String>,
    state: State<'_, crate::vrchat_api::VrchatApiState>,
    auth_state: State<'_, crate::vrchat_auth::VrchatAuthState>,
) -> Result<crate::vrchat_api::VrchatWorldDetailDto, String> {
    let cache_connection = open_current_database_for_preview_cache();
    let existing_cache =
        cache_connection
            .as_ref()
            .and_then(|connection| {
                match repository::get_world_preview_cache(connection, &world_id) {
                    Ok(cache) => cache,
                    Err(error) => {
                        eprintln!("World preview cache query failed: {error}");
                        None
                    }
                }
            });

    if let Some(cache) = existing_cache.as_ref() {
        let has_image = cache.image_url.is_some() || cache.thumbnail_image_url.is_some();
        let has_description = cache.description.is_some();
        if has_image && has_description && world_preview_success_cache_is_fresh(cache) {
            return Ok(crate::vrchat_api::VrchatWorldDetailDto {
                world_id: cache.world_id.clone(),
                name: cache.world_name.clone(),
                author_name: cache.author_name.clone(),
                description: cache.description.clone(),
                image_url: cache.image_url.clone(),
                thumbnail_image_url: cache.thumbnail_image_url.clone(),
                capacity: None,
                recommended_capacity: None,
                visits: None,
                favorites: None,
                occupants: None,
                public_occupants: None,
                private_occupants: None,
                release_status: None,
                tags: vec![],
                platforms: vec![],
            });
        }
    }

    if existing_cache
        .as_ref()
        .and_then(|cache| cache.fetch_failed_at.as_deref())
        .is_some_and(world_preview_failure_is_in_cooldown)
    {
        return Err("VRChat API request skipped: world preview recently failed".to_string());
    }

    let auth_cookie = auth_state.get_saved_cookie();
    let result =
        crate::vrchat_api::fetch_world_detail(&world_id, state.inner(), auth_cookie.as_deref())
            .await;
    let now = Local::now().to_rfc3339();

    match &result {
        Ok(detail) => {
            if let Some(connection) = cache_connection.as_ref() {
                let cache_world_name =
                    resolve_success_world_name(world_name.as_deref(), &detail.name, &world_id);
                let input = WorldPreviewUpsert {
                    world_id: world_id.clone(),
                    world_name: cache_world_name,
                    image_url: detail.image_url.clone(),
                    thumbnail_image_url: detail.thumbnail_image_url.clone(),
                    author_name: detail.author_name.clone(),
                    description: detail.description.clone(),
                };

                if let Err(error) = repository::upsert_world_preview_cache(connection, &input, &now)
                {
                    eprintln!("World preview cache upsert failed: {error}");
                }
            }
        }
        Err(_) => {
            if let Some(connection) = cache_connection.as_ref() {
                let cache_world_name = resolve_failed_world_name(
                    world_name.as_deref(),
                    existing_cache.as_ref(),
                    &world_id,
                );

                if let Err(error) = repository::mark_world_preview_fetch_failed(
                    connection,
                    &world_id,
                    &cache_world_name,
                    &now,
                ) {
                    eprintln!("World preview cache failure mark failed: {error}");
                }
            }
        }
    }

    result
}

#[tauri::command]
pub fn get_visit_transition_graph(
    criteria: TransitionGraphCriteria,
) -> Result<VisitTransitionGraphDto, String> {
    let settings = get_settings();
    let db_path = resolve_db_path(&settings.db_path)?;
    let connection = open_initialized_database(&db_path)?;

    query_visit_transition_graph(&connection, criteria)
}

#[tauri::command]
pub fn get_stats_date_range() -> Result<StatsDateRangeDto, String> {
    let settings = get_settings();
    let db_path = resolve_db_path(&settings.db_path)?;
    let connection = open_initialized_database(&db_path)?;

    query_stats_date_range(&connection)
}

#[tauri::command]
pub fn delete_all_history() -> Result<DeleteHistoryResultDto, String> {
    let settings = get_settings();
    let db_path = resolve_db_path(&settings.db_path)?;
    let connection = open_initialized_database(&db_path)?;
    let deleted_count = repository::delete_all_visit_histories(&connection)
        .map_err(|error| format!("delete all history failed: {error}"))?;
    Ok(DeleteHistoryResultDto { deleted_count })
}

#[tauri::command]
pub fn delete_visit_history(visit_id: i64) -> Result<DeleteHistoryResultDto, String> {
    if visit_id <= 0 {
        return Err("invalid visit id".to_string());
    }
    let settings = get_settings();
    let db_path = resolve_db_path(&settings.db_path)?;
    let connection = open_initialized_database(&db_path)?;
    let deleted_count = repository::delete_visit_history_by_id(&connection, visit_id)
        .map_err(|error| format!("delete visit history failed: {error}"))?;
    Ok(DeleteHistoryResultDto { deleted_count })
}

fn open_current_database_for_preview_cache() -> Option<Connection> {
    let settings = get_settings();
    let db_path = match resolve_db_path(&settings.db_path) {
        Ok(db_path) => db_path,
        Err(error) => {
            eprintln!("World preview cache db path resolve failed: {error}");
            return None;
        }
    };

    match open_initialized_database(&db_path) {
        Ok(connection) => Some(connection),
        Err(error) => {
            eprintln!("World preview cache database open failed: {error}");
            None
        }
    }
}

fn world_preview_failure_is_in_cooldown(fetch_failed_at: &str) -> bool {
    let Ok(failed_at) = DateTime::parse_from_rfc3339(fetch_failed_at) else {
        return false;
    };

    Local::now().fixed_offset() - failed_at < Duration::hours(WORLD_PREVIEW_FAILURE_COOLDOWN_HOURS)
}

fn world_preview_success_cache_is_fresh(cache: &WorldPreviewCache) -> bool {
    let Some(fetched_at_str) = cache.fetched_at.as_deref() else {
        return false;
    };
    let Ok(fetched_at) = DateTime::parse_from_rfc3339(fetched_at_str) else {
        return false;
    };
    Local::now().fixed_offset() - fetched_at < Duration::days(WORLD_PREVIEW_SUCCESS_TTL_DAYS)
}

fn resolve_success_world_name(
    requested_world_name: Option<&str>,
    detail_world_name: &str,
    world_id: &str,
) -> String {
    trimmed(requested_world_name)
        .or_else(|| trimmed(Some(detail_world_name)))
        .unwrap_or_else(|| world_id.to_string())
}

fn resolve_failed_world_name(
    requested_world_name: Option<&str>,
    existing_cache: Option<&WorldPreviewCache>,
    world_id: &str,
) -> String {
    trimmed(requested_world_name)
        .or_else(|| existing_cache.map(|cache| cache.world_name.clone()))
        .unwrap_or_else(|| world_id.to_string())
}

fn query_visits(
    connection: &Connection,
    criteria: VisitFilterCriteria,
) -> Result<Vec<VisitHistoryDto>, String> {
    let mode = criteria.mode.as_deref().unwrap_or("recent");
    let mut sql = String::from(
        "SELECT v.id, v.visited_at, v.world_name, v.world_id, v.instance_id, \
         v.instance_access_type, v.instance_nonce, v.instance_raw_tags, \
         v.stay_duration_seconds, v.memo, v.tags, v.source_log_file, v.created_at, v.updated_at, \
         w.thumbnail_image_url AS thumbnail_url, w.image_url AS image_url, \
         w.fetched_at AS world_preview_fetched_at \
         FROM visit_histories v \
         LEFT JOIN world_preview_cache w ON w.world_id = v.world_id",
    );
    let mut where_clauses: Vec<&str> = Vec::new();
    let mut params: Vec<Value> = Vec::new();

    match mode {
        "today" => {
            if let Some((start, end)) = local_day_range(0) {
                where_clauses.push("v.visited_at >= ?");
                params.push(Value::Text(start));
                where_clauses.push("v.visited_at < ?");
                params.push(Value::Text(end));
            }
        }
        "yesterday" => {
            if let Some((start, end)) = local_day_range(-1) {
                where_clauses.push("v.visited_at >= ?");
                params.push(Value::Text(start));
                where_clauses.push("v.visited_at < ?");
                params.push(Value::Text(end));
            }
        }
        "range" => {
            if let Some(start) = normalized_range_bound(criteria.start.as_deref(), false) {
                where_clauses.push("v.visited_at >= ?");
                params.push(Value::Text(start));
            }
            if let Some(end) = normalized_range_bound(criteria.end.as_deref(), true) {
                where_clauses.push("v.visited_at < ?");
                params.push(Value::Text(end));
            }
        }
        "all" | "recent" => {}
        _ => {}
    }

    if let Some(world_name) = trimmed(criteria.world_name.as_deref()) {
        where_clauses.push("v.world_name LIKE ?");
        params.push(Value::Text(format!("%{world_name}%")));
    }
    if let Some(tag) = trimmed(criteria.tag.as_deref()) {
        where_clauses.push("v.tags LIKE ?");
        params.push(Value::Text(format!("%{tag}%")));
    }
    if let Some(instance_access_type) = trimmed(criteria.instance_access_type.as_deref()) {
        where_clauses.push("v.instance_access_type = ?");
        params.push(Value::Text(instance_access_type));
    }

    if !where_clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&where_clauses.join(" AND "));
    }

    sql.push_str(" ORDER BY v.visited_at DESC, v.id DESC");

    if mode != "all" {
        sql.push_str(" LIMIT ?");
        params.push(Value::Integer(clamp_limit(criteria.limit)));
    }

    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("visit_histories query prepare failed: {error}"))?;
    let rows = statement
        .query_map(params_from_iter(params), visit_from_row)
        .map_err(|error| format!("visit_histories query failed: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("visit_histories row decode failed: {error}"))
}

#[derive(Debug, Clone)]
struct TransitionGraphVisit {
    visited_at: String,
    world_name: String,
    world_id: Option<String>,
    stay_duration_seconds: Option<i64>,
    key: String,
}

fn query_visit_transition_graph(
    connection: &Connection,
    criteria: TransitionGraphCriteria,
) -> Result<VisitTransitionGraphDto, String> {
    let start_bound = normalized_range_bound(Some(&criteria.start), false)
        .ok_or_else(|| "transition graph start is required".to_string())?;
    let end_bound = normalized_range_bound(Some(&criteria.end), true)
        .ok_or_else(|| "transition graph end is required".to_string())?;

    let visits = query_transition_graph_visits(connection, &start_bound, &end_bound)?;
    let mut node_indexes: HashMap<String, usize> = HashMap::new();
    let mut nodes: Vec<TransitionGraphNodeDto> = Vec::new();

    for visit in &visits {
        if let Some(index) = node_indexes.get(&visit.key).copied() {
            let node = &mut nodes[index];
            node.visit_count += 1;
            node.total_stay_seconds += visit.stay_duration_seconds.unwrap_or(0);
            node.last_visited_at = visit.visited_at.clone();
        } else {
            node_indexes.insert(visit.key.clone(), nodes.len());
            nodes.push(TransitionGraphNodeDto {
                key: visit.key.clone(),
                world_id: visit.world_id.clone(),
                world_name: visit.world_name.clone(),
                visit_count: 1,
                total_stay_seconds: visit.stay_duration_seconds.unwrap_or(0),
                last_visited_at: visit.visited_at.clone(),
                degree: 0,
            });
        }
    }

    let mut edges_by_key: HashMap<String, TransitionGraphEdgeDto> = HashMap::new();
    for pair in visits.windows(2) {
        let from = &pair[0];
        let to = &pair[1];
        if from.key == to.key {
            continue;
        }

        let edge_key = format!("{}→{}", from.key, to.key);
        let transition_at = to.visited_at.clone();
        edges_by_key
            .entry(edge_key.clone())
            .and_modify(|edge| {
                edge.transition_count += 1;
                edge.latest_transition_at = transition_at.clone();
                edge.transition_times.push(transition_at.clone());
            })
            .or_insert_with(|| TransitionGraphEdgeDto {
                key: edge_key,
                from: from.key.clone(),
                to: to.key.clone(),
                from_world_name: from.world_name.clone(),
                to_world_name: to.world_name.clone(),
                transition_count: 1,
                latest_transition_at: transition_at.clone(),
                transition_times: vec![transition_at],
            });
    }

    let all_edges = edges_by_key.into_values().collect::<Vec<_>>();
    for edge in &all_edges {
        if let Some(index) = node_indexes.get(&edge.from).copied() {
            nodes[index].degree += 1;
        }
        if let Some(index) = node_indexes.get(&edge.to).copied() {
            nodes[index].degree += 1;
        }
    }

    let transition_count = all_edges
        .iter()
        .map(|edge| edge.transition_count)
        .sum::<i64>();
    let mut top_transitions = all_edges.clone();
    top_transitions.sort_by(|left, right| {
        right
            .transition_count
            .cmp(&left.transition_count)
            .then_with(|| right.latest_transition_at.cmp(&left.latest_transition_at))
            .then_with(|| left.from_world_name.cmp(&right.from_world_name))
            .then_with(|| left.to_world_name.cmp(&right.to_world_name))
    });
    top_transitions.truncate(5);
    let top_transition = top_transitions.first().cloned();

    let top_longest_visit = visits
        .iter()
        .filter_map(|visit| {
            visit
                .stay_duration_seconds
                .map(|stay_seconds| (visit, stay_seconds))
        })
        .max_by(|(left, left_seconds), (right, right_seconds)| {
            left_seconds
                .cmp(right_seconds)
                .then_with(|| left.visited_at.cmp(&right.visited_at))
                .then_with(|| right.world_name.cmp(&left.world_name))
        })
        .map(|(visit, stay_seconds)| LongestVisitDto {
            key: visit.key.clone(),
            world_id: visit.world_id.clone(),
            world_name: visit.world_name.clone(),
            visited_at: visit.visited_at.clone(),
            stay_duration_seconds: stay_seconds,
        });

    let all_node_count = nodes.len();
    let mut top_worlds = nodes.clone();
    top_worlds.sort_by(|left, right| {
        right
            .visit_count
            .cmp(&left.visit_count)
            .then_with(|| right.total_stay_seconds.cmp(&left.total_stay_seconds))
            .then_with(|| right.last_visited_at.cmp(&left.last_visited_at))
            .then_with(|| left.world_name.cmp(&right.world_name))
    });
    top_worlds.truncate(5);

    let mut display_nodes = nodes;
    display_nodes.sort_by(|left, right| {
        right
            .visit_count
            .cmp(&left.visit_count)
            .then_with(|| right.last_visited_at.cmp(&left.last_visited_at))
            .then_with(|| left.world_name.cmp(&right.world_name))
    });
    display_nodes.truncate(30);

    let display_node_keys = display_nodes
        .iter()
        .map(|node| node.key.clone())
        .collect::<HashSet<_>>();
    let mut display_edge_candidates = all_edges
        .into_iter()
        .filter(|edge| {
            display_node_keys.contains(&edge.from) && display_node_keys.contains(&edge.to)
        })
        .collect::<Vec<_>>();
    let display_edge_candidate_count = display_edge_candidates.len();
    display_edge_candidates.sort_by(|left, right| {
        right
            .transition_count
            .cmp(&left.transition_count)
            .then_with(|| right.latest_transition_at.cmp(&left.latest_transition_at))
            .then_with(|| left.from_world_name.cmp(&right.from_world_name))
            .then_with(|| left.to_world_name.cmp(&right.to_world_name))
    });
    display_edge_candidates.truncate(80);

    Ok(VisitTransitionGraphDto {
        start: start_bound,
        end: end_bound,
        summary: TransitionGraphSummaryDto {
            visit_count: visits.len() as i64,
            unique_world_count: all_node_count as i64,
            transition_count,
            top_transition,
            top_longest_visit,
            hidden_node_count: (all_node_count - display_nodes.len()) as i64,
            hidden_edge_count: (display_edge_candidate_count - display_edge_candidates.len())
                as i64,
        },
        nodes: display_nodes,
        edges: display_edge_candidates,
        top_worlds,
        top_transitions,
    })
}

fn query_stats_date_range(connection: &Connection) -> Result<StatsDateRangeDto, String> {
    connection
        .query_row(
            "SELECT MIN(substr(visited_at, 1, 10)), MAX(substr(visited_at, 1, 10)) FROM visit_histories",
            [],
            |row| {
                Ok(StatsDateRangeDto {
                    start: row.get(0)?,
                    end: row.get(1)?,
                })
            },
        )
        .map_err(|error| format!("stats date range query failed: {error}"))
}

fn query_transition_graph_visits(
    connection: &Connection,
    start_bound: &str,
    end_bound: &str,
) -> Result<Vec<TransitionGraphVisit>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, visited_at, world_name, world_id, stay_duration_seconds \
             FROM visit_histories \
             WHERE visited_at >= ?1 AND visited_at < ?2 \
             ORDER BY visited_at ASC, id ASC",
        )
        .map_err(|error| format!("transition graph query prepare failed: {error}"))?;
    let rows = statement
        .query_map(params![start_bound, end_bound], |row| {
            let world_name: String = row.get("world_name")?;
            let world_id: Option<String> = row.get("world_id")?;
            Ok(TransitionGraphVisit {
                visited_at: row.get("visited_at")?,
                key: transition_graph_world_key(world_id.as_deref(), &world_name),
                world_name,
                world_id,
                stay_duration_seconds: row.get("stay_duration_seconds")?,
            })
        })
        .map_err(|error| format!("transition graph query failed: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("transition graph row decode failed: {error}"))
}

fn transition_graph_world_key(world_id: Option<&str>, world_name: &str) -> String {
    world_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| world_name.trim().to_lowercase())
}

fn open_initialized_database(db_path: &std::path::Path) -> Result<Connection, String> {
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

fn visit_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<VisitHistoryDto> {
    Ok(VisitHistoryDto {
        id: row.get("id")?,
        visited_at: row.get("visited_at")?,
        world_name: row.get("world_name")?,
        world_id: row.get("world_id")?,
        instance_id: row.get("instance_id")?,
        instance_access_type: row.get("instance_access_type")?,
        stay_duration_seconds: row.get("stay_duration_seconds")?,
        memo: row.get("memo")?,
        tags: row.get("tags")?,
        source_log_file: row.get("source_log_file")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        thumbnail_url: row.get("thumbnail_url")?,
        image_url: row.get("image_url")?,
        world_preview_fetched_at: row.get("world_preview_fetched_at")?,
    })
}

fn visit_count(connection: &Connection) -> Result<i64, String> {
    connection
        .query_row("SELECT COUNT(*) FROM visit_histories", [], |row| row.get(0))
        .map_err(|error| format!("visit count query failed: {error}"))
}

fn latest_visit(connection: &Connection) -> Result<Option<VisitHistoryDto>, String> {
    let mut statement = connection
        .prepare(
            "SELECT v.id, v.visited_at, v.world_name, v.world_id, v.instance_id, \
             v.instance_access_type, v.instance_nonce, v.instance_raw_tags, \
             v.stay_duration_seconds, v.memo, v.tags, v.source_log_file, \
             v.created_at, v.updated_at, w.thumbnail_image_url AS thumbnail_url, \
             w.image_url AS image_url, w.fetched_at AS world_preview_fetched_at \
             FROM visit_histories v \
             LEFT JOIN world_preview_cache w ON w.world_id = v.world_id \
             ORDER BY v.visited_at DESC, v.id DESC LIMIT 1",
        )
        .map_err(|error| format!("latest visit query prepare failed: {error}"))?;
    let mut rows = statement
        .query_map([], visit_from_row)
        .map_err(|error| format!("latest visit query failed: {error}"))?;

    match rows.next() {
        Some(row) => row
            .map(Some)
            .map_err(|error| format!("latest visit row decode failed: {error}")),
        None => Ok(None),
    }
}

fn clamp_limit(limit: Option<i64>) -> i64 {
    limit
        .unwrap_or(DEFAULT_RECENT_VISIT_LIMIT)
        .clamp(MIN_RECENT_VISIT_LIMIT, MAX_RECENT_VISIT_LIMIT)
}

fn normalize_library_limit(limit: i64) -> i64 {
    match limit {
        10 | 25 => limit,
        _ => 25,
    }
}

fn normalize_library_sort_key(sort_key: &str) -> &'static str {
    match sort_key {
        "visit_count" => "visit_count",
        "total_stay_duration_seconds" => "total_stay_duration_seconds",
        "world_name" => "world_name",
        _ => "world_name",
    }
}

fn normalize_library_sort_direction(sort_direction: &str) -> &'static str {
    match sort_direction {
        "desc" => "desc",
        "asc" => "asc",
        _ => "asc",
    }
}

fn trimmed(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalized_range_bound(value: Option<&str>, exclusive_end: bool) -> Option<String> {
    let value = trimmed(value)?;
    if value.len() == 10 && value.as_bytes().get(4) == Some(&b'-') {
        Some(if exclusive_end {
            NaiveDate::parse_from_str(&value, "%Y-%m-%d")
                .ok()
                .and_then(|day| day.succ_opt())
                .map(format_day_start)
                .unwrap_or_else(|| format!("{value}T23:59:59"))
        } else {
            format!("{value}T00:00:00")
        })
    } else {
        Some(value)
    }
}

fn local_day_range(offset_days: i64) -> Option<(String, String)> {
    let day = Local::now().date_naive() + Duration::days(offset_days);
    let next_day = day.succ_opt()?;

    Some((format_day_start(day), format_day_start(next_day)))
}

fn format_day_start(day: NaiveDate) -> String {
    format!("{day}T00:00:00")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn clamps_recent_visit_limit() {
        assert_eq!(clamp_limit(None), DEFAULT_RECENT_VISIT_LIMIT);
        assert_eq!(clamp_limit(Some(-10)), MIN_RECENT_VISIT_LIMIT);
        assert_eq!(clamp_limit(Some(1_000)), MAX_RECENT_VISIT_LIMIT);
    }

    #[test]
    fn normalizes_date_only_range_bounds() {
        assert_eq!(
            normalized_range_bound(Some("2026-06-10"), false),
            Some("2026-06-10T00:00:00".to_string())
        );
        assert_eq!(
            normalized_range_bound(Some("2026-06-10"), true),
            Some("2026-06-11T00:00:00".to_string())
        );
        assert_eq!(
            normalized_range_bound(Some("2026-06-30"), true),
            Some("2026-07-01T00:00:00".to_string())
        );
        assert_eq!(
            normalized_range_bound(Some("2026-12-31"), true),
            Some("2027-01-01T00:00:00".to_string())
        );
    }

    #[test]
    fn all_mode_returns_more_than_recent_default_limit() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        for index in 0..=DEFAULT_RECENT_VISIT_LIMIT {
            connection
                .execute(
                    "
                    INSERT INTO visit_histories (
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
                        'public',
                        NULL,
                        NULL,
                        600,
                        NULL,
                        NULL,
                        'output_log_test.txt',
                        CURRENT_TIMESTAMP,
                        CURRENT_TIMESTAMP
                    )
                    ",
                    params![
                        format!("2026-06-{:02}T20:00:00", (index % 28) + 1),
                        format!("World {index}"),
                        format!("wrld_{index}"),
                        format!("instance_{index}"),
                    ],
                )
                .expect("visit should insert");
        }

        let recent_visits = query_visits(
            &connection,
            VisitFilterCriteria {
                mode: Some("recent".to_string()),
                start: None,
                end: None,
                world_name: None,
                tag: None,
                instance_access_type: None,
                limit: None,
            },
        )
        .expect("recent visits should query");

        let all_visits = query_visits(
            &connection,
            VisitFilterCriteria {
                mode: Some("all".to_string()),
                start: None,
                end: None,
                world_name: None,
                tag: None,
                instance_access_type: None,
                limit: None,
            },
        )
        .expect("all visits should query");

        assert_eq!(recent_visits.len(), DEFAULT_RECENT_VISIT_LIMIT as usize);
        assert_eq!(all_visits.len(), DEFAULT_RECENT_VISIT_LIMIT as usize + 1);
    }

    #[test]
    fn visit_query_returns_null_preview_fields_without_cache() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        connection
            .execute(
                "
                INSERT INTO visit_histories (
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
                    '2026-06-23T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    600,
                    NULL,
                    NULL,
                    'output_log_test.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                )
                ",
                [],
            )
            .expect("visit should insert");

        let visits = query_visits(
            &connection,
            VisitFilterCriteria {
                mode: Some("all".to_string()),
                start: None,
                end: None,
                world_name: None,
                tag: None,
                instance_access_type: None,
                limit: None,
            },
        )
        .expect("visits should query");

        assert_eq!(visits.len(), 1);
        assert_eq!(visits[0].thumbnail_url, None);
        assert_eq!(visits[0].image_url, None);
        assert_eq!(visits[0].world_preview_fetched_at, None);
    }

    #[test]
    fn visit_query_includes_world_preview_cache_fields() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        connection
            .execute(
                "
                INSERT INTO visit_histories (
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
                    '2026-06-23T20:00:00+09:00',
                    'Example World',
                    'wrld_example',
                    '12345',
                    'public',
                    NULL,
                    NULL,
                    600,
                    NULL,
                    NULL,
                    'output_log_test.txt',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                )
                ",
                [],
            )
            .expect("visit should insert");
        repository::upsert_world_preview_cache(
            &connection,
            &WorldPreviewUpsert {
                world_id: "wrld_example".to_string(),
                world_name: "Example World".to_string(),
                image_url: Some("https://example.com/image.jpg".to_string()),
                thumbnail_image_url: Some("https://example.com/thumb.jpg".to_string()),
                author_name: Some("Example Author".to_string()),
                description: Some("Example Description".to_string()),
            },
            "2026-06-23T21:00:00+09:00",
        )
        .expect("preview cache should upsert");

        let visits = query_visits(
            &connection,
            VisitFilterCriteria {
                mode: Some("all".to_string()),
                start: None,
                end: None,
                world_name: None,
                tag: None,
                instance_access_type: None,
                limit: None,
            },
        )
        .expect("visits should query");

        assert_eq!(visits.len(), 1);
        assert_eq!(
            visits[0].thumbnail_url.as_deref(),
            Some("https://example.com/thumb.jpg")
        );
        assert_eq!(
            visits[0].image_url.as_deref(),
            Some("https://example.com/image.jpg")
        );
        assert_eq!(
            visits[0].world_preview_fetched_at.as_deref(),
            Some("2026-06-23T21:00:00+09:00")
        );
    }

    fn insert_test_visit(
        connection: &Connection,
        visited_at: &str,
        world_name: &str,
        world_id: Option<&str>,
        stay_duration_seconds: Option<i64>,
    ) {
        connection
            .execute(
                "INSERT INTO visit_histories (
                    visited_at, world_name, world_id, instance_id,
                    instance_access_type, instance_nonce, instance_raw_tags,
                    stay_duration_seconds, memo, tags, source_log_file,
                    created_at, updated_at
                ) VALUES (?1, ?2, ?3, NULL, 'public', NULL, NULL, ?4, NULL, NULL, 'test.txt',
                    CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
                params![visited_at, world_name, world_id, stay_duration_seconds],
            )
            .expect("test visit insert should succeed");
    }

    fn graph_for(connection: &Connection, start: &str, end: &str) -> VisitTransitionGraphDto {
        query_visit_transition_graph(
            connection,
            TransitionGraphCriteria {
                start: start.to_string(),
                end: end.to_string(),
            },
        )
        .expect("transition graph should build")
    }

    #[test]
    fn stats_date_range_uses_oldest_and_latest_visit_dates() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        insert_test_visit(
            &connection,
            "2026-06-10T10:00:00",
            "World A",
            Some("wrld_a"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-25T10:00:00",
            "World B",
            Some("wrld_b"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-15T10:00:00",
            "World C",
            Some("wrld_c"),
            None,
        );

        let range = query_stats_date_range(&connection).expect("range should query");

        assert_eq!(range.start.as_deref(), Some("2026-06-10"));
        assert_eq!(range.end.as_deref(), Some("2026-06-25"));
    }

    #[test]
    fn stats_date_range_returns_null_when_empty() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        let range = query_stats_date_range(&connection).expect("range should query");

        assert_eq!(range.start, None);
        assert_eq!(range.end, None);
    }

    #[test]
    fn transition_graph_top_rankings_limited_to_five() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        let worlds = ["wrld_a", "wrld_b", "wrld_c", "wrld_d", "wrld_e", "wrld_f"];
        for (i, &wid) in worlds.iter().enumerate() {
            for j in 0..=(6 - i) {
                insert_test_visit(
                    &connection,
                    &format!("2026-06-15T{i:02}:{j:02}:00"),
                    &format!("World {i}"),
                    Some(wid),
                    None,
                );
            }
        }

        let graph = graph_for(&connection, "2026-06-15", "2026-06-15");

        assert!(
            graph.top_worlds.len() <= 5,
            "top_worlds should be at most 5"
        );
        assert!(
            graph.top_transitions.len() <= 5,
            "top_transitions should be at most 5"
        );
    }

    #[test]
    fn transition_graph_only_in_range() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        insert_test_visit(
            &connection,
            "2026-06-01T10:00:00",
            "World A",
            Some("wrld_a"),
            Some(60),
        );
        insert_test_visit(
            &connection,
            "2026-06-15T10:00:00",
            "World B",
            Some("wrld_b"),
            Some(60),
        );
        insert_test_visit(
            &connection,
            "2026-06-30T10:00:00",
            "World C",
            Some("wrld_c"),
            Some(60),
        );

        let graph = graph_for(&connection, "2026-06-10", "2026-06-20");

        assert_eq!(graph.summary.visit_count, 1);
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].world_id.as_deref(), Some("wrld_b"));
    }

    #[test]
    fn transition_graph_skips_same_world_consecutive() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        insert_test_visit(
            &connection,
            "2026-06-15T10:00:00",
            "World A",
            Some("wrld_a"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-15T11:00:00",
            "World A",
            Some("wrld_a"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-15T12:00:00",
            "World B",
            Some("wrld_b"),
            None,
        );

        let graph = graph_for(&connection, "2026-06-15", "2026-06-15");

        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].from, "wrld_a");
        assert_eq!(graph.edges[0].to, "wrld_b");
    }

    #[test]
    fn transition_graph_bidirectional_edges() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        insert_test_visit(
            &connection,
            "2026-06-15T10:00:00",
            "World A",
            Some("wrld_a"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-15T11:00:00",
            "World B",
            Some("wrld_b"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-15T12:00:00",
            "World A",
            Some("wrld_a"),
            None,
        );

        let graph = graph_for(&connection, "2026-06-15", "2026-06-15");

        assert_eq!(graph.edges.len(), 2);
        let has_ab = graph
            .edges
            .iter()
            .any(|e| e.from == "wrld_a" && e.to == "wrld_b");
        let has_ba = graph
            .edges
            .iter()
            .any(|e| e.from == "wrld_b" && e.to == "wrld_a");
        assert!(has_ab, "A→B edge should exist");
        assert!(has_ba, "B→A edge should exist");
    }

    #[test]
    fn transition_graph_merges_same_direction() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        insert_test_visit(
            &connection,
            "2026-06-15T10:00:00",
            "World A",
            Some("wrld_a"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-15T11:00:00",
            "World B",
            Some("wrld_b"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-15T12:00:00",
            "World A",
            Some("wrld_a"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-15T13:00:00",
            "World B",
            Some("wrld_b"),
            None,
        );

        let graph = graph_for(&connection, "2026-06-15", "2026-06-15");

        let ab_edge = graph
            .edges
            .iter()
            .find(|e| e.from == "wrld_a" && e.to == "wrld_b")
            .expect("A→B edge should exist");
        assert_eq!(ab_edge.transition_count, 2);

        let ba_edge = graph
            .edges
            .iter()
            .find(|e| e.from == "wrld_b" && e.to == "wrld_a")
            .expect("B→A edge should exist");
        assert_eq!(ba_edge.transition_count, 1);
    }

    #[test]
    fn transition_graph_world_id_key() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        insert_test_visit(
            &connection,
            "2026-06-15T10:00:00",
            "Name One",
            Some("wrld_same"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-15T11:00:00",
            "Name Two",
            Some("wrld_same"),
            None,
        );

        let graph = graph_for(&connection, "2026-06-15", "2026-06-15");

        assert_eq!(
            graph.nodes.len(),
            1,
            "same world_id should be counted as one node"
        );
        assert_eq!(graph.nodes[0].visit_count, 2);
    }

    #[test]
    fn transition_graph_world_name_key() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        insert_test_visit(
            &connection,
            "2026-06-15T10:00:00",
            "  Test World  ",
            None,
            None,
        );
        insert_test_visit(&connection, "2026-06-15T11:00:00", "test world", None, None);

        let graph = graph_for(&connection, "2026-06-15", "2026-06-15");

        assert_eq!(
            graph.nodes.len(),
            1,
            "normalized world_name should be counted as one node"
        );
        assert_eq!(graph.nodes[0].visit_count, 2);
    }

    #[test]
    fn transition_graph_null_stay_duration() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        insert_test_visit(
            &connection,
            "2026-06-15T10:00:00",
            "World A",
            Some("wrld_a"),
            None,
        );

        let graph = graph_for(&connection, "2026-06-15", "2026-06-15");

        assert_eq!(graph.nodes[0].total_stay_seconds, 0);
    }

    #[test]
    fn transition_graph_top_transitions_use_all_edges_not_display_edges() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        for hour in 0..8 {
            let (world_name, world_id) = if hour % 2 == 0 {
                ("Hidden Source", "wrld_hidden_source")
            } else {
                ("Hidden Destination", "wrld_hidden_destination")
            };
            insert_test_visit(
                &connection,
                &format!("2026-06-01T{hour:02}:00:00"),
                world_name,
                Some(world_id),
                None,
            );
        }

        for display_index in 0..30 {
            let visited_date = if display_index < 29 {
                format!("2026-06-{:02}", display_index + 2)
            } else {
                "2026-07-01".to_string()
            };

            for hour in 0..4 {
                insert_test_visit(
                    &connection,
                    &format!("{visited_date}T{hour:02}:00:00"),
                    &format!("Display World {display_index:02}"),
                    Some(&format!("wrld_display_{display_index:02}")),
                    None,
                );
            }
        }

        let graph = graph_for(&connection, "2026-06-01", "2026-07-01");

        let top_transition = graph
            .top_transitions
            .first()
            .expect("top transition should exist");
        assert_eq!(top_transition.from, "wrld_hidden_source");
        assert_eq!(top_transition.to, "wrld_hidden_destination");
        assert_eq!(top_transition.transition_count, 4);
        assert!(
            !graph
                .edges
                .iter()
                .any(|edge| edge.key == top_transition.key),
            "display edges should not include the hidden-world transition"
        );
    }

    #[test]
    fn transition_graph_summary_top_transition_matches_first_top_transition() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        insert_test_visit(
            &connection,
            "2026-06-15T10:00:00",
            "World A",
            Some("wrld_a"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-15T11:00:00",
            "World B",
            Some("wrld_b"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-15T12:00:00",
            "World A",
            Some("wrld_a"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-15T13:00:00",
            "World B",
            Some("wrld_b"),
            None,
        );

        let graph = graph_for(&connection, "2026-06-15", "2026-06-15");

        assert_eq!(
            graph
                .summary
                .top_transition
                .as_ref()
                .map(|transition| transition.key.as_str()),
            graph
                .top_transitions
                .first()
                .map(|transition| transition.key.as_str()),
        );
    }

    #[test]
    fn transition_graph_hidden_counts() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        for i in 0..31 {
            insert_test_visit(
                &connection,
                &format!("2026-06-15T{:02}:00:00", i % 24),
                &format!("World {i}"),
                Some(&format!("wrld_{i}")),
                None,
            );
        }

        let graph = graph_for(&connection, "2026-06-15", "2026-06-15");

        assert_eq!(graph.summary.hidden_node_count, 1);
        assert_eq!(graph.nodes.len(), 30);
    }

    #[test]
    fn top_longest_visit_uses_single_visit_not_total() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        // World A: 2 visits, each 3600s (total 7200s)
        insert_test_visit(
            &connection,
            "2026-06-15T10:00:00",
            "World A",
            Some("wrld_a"),
            Some(3600),
        );
        insert_test_visit(
            &connection,
            "2026-06-15T12:00:00",
            "World A",
            Some("wrld_a"),
            Some(3600),
        );
        // World B: 1 visit, 5400s (longer per-visit than World A)
        insert_test_visit(
            &connection,
            "2026-06-15T14:00:00",
            "World B",
            Some("wrld_b"),
            Some(5400),
        );

        let graph = graph_for(&connection, "2026-06-15", "2026-06-15");
        let longest = graph
            .summary
            .top_longest_visit
            .expect("top_longest_visit should be Some");

        assert_eq!(longest.world_name, "World B");
        assert_eq!(longest.stay_duration_seconds, 5400);
    }

    #[test]
    fn top_longest_visit_excludes_null_stay_duration() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        // World A: stay_duration_seconds = NULL
        insert_test_visit(
            &connection,
            "2026-06-15T10:00:00",
            "World A",
            Some("wrld_a"),
            None,
        );
        // World B: stay_duration_seconds = 1200
        insert_test_visit(
            &connection,
            "2026-06-15T12:00:00",
            "World B",
            Some("wrld_b"),
            Some(1200),
        );

        let graph = graph_for(&connection, "2026-06-15", "2026-06-15");
        let longest = graph
            .summary
            .top_longest_visit
            .expect("top_longest_visit should be Some");

        assert_eq!(longest.world_name, "World B");
        assert_eq!(longest.stay_duration_seconds, 1200);
    }

    #[test]
    fn top_longest_visit_is_none_when_all_stay_durations_null() {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");

        insert_test_visit(
            &connection,
            "2026-06-15T10:00:00",
            "World A",
            Some("wrld_a"),
            None,
        );
        insert_test_visit(
            &connection,
            "2026-06-15T12:00:00",
            "World B",
            Some("wrld_b"),
            None,
        );

        let graph = graph_for(&connection, "2026-06-15", "2026-06-15");

        assert!(
            graph.summary.top_longest_visit.is_none(),
            "top_longest_visit should be None when all stay_duration_seconds are NULL"
        );
    }
}

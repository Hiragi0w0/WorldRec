//! AIワールド推薦機能 (Gemini Developer API)。
//!
//! ワークフロー本体は Go sidecar (`sidecar/cmd/worldrec-ai`) が実行する。
//! Rust 側の責務は次の3つ。
//! - Gemini APIキーの保存 (OSキーリング = 暗号化ストレージ) と状態返却
//! - 訪問履歴DBからの候補ワールド集計
//! - sidecar プロセスの起動と stdin/stdout JSON 受け渡し
//!
//! APIキーはコマンドライン引数・環境変数には載せず、stdin JSON でのみ渡す。

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::db::{connection::open_database, schema::initialize_database};
use crate::log_watcher::service::resolve_db_path;

const KEYRING_SERVICE: &str = "WorldRec";
const KEYRING_ENTRY: &str = "gemini_api_key";
const SIDECAR_ENV_OVERRIDE: &str = "WORLDREC_AI_SIDECAR";
const SIDECAR_BINARY_NAMES: &[&str] = &["worldrec-ai.exe", "worldrec-ai"];
const RECOMMENDATION_MODE_VISITED_ONLY: &str = "visited_only";
const RECOMMENDATION_MODE_UNVISITED_ONLY: &str = "unvisited_only";
const RECOMMENDATION_MODE_MIXED: &str = "mixed";
const AI_RETRY_WAIT_SECONDS: u64 = 3;
const DEGRADED_MODE_MAX_VISITED_WORLDS: usize = 25;
const AI_DEGRADED_WARNING: &str = "AI推薦を簡略モードで表示しています。";

const ERROR_KIND_INVALID_API_KEY: &str = "invalid_api_key";
const ERROR_KIND_SERVICE_UNAVAILABLE: &str = "service_unavailable";
const ERROR_KIND_TIMEOUT: &str = "timeout";
const ERROR_KIND_RATE_LIMITED: &str = "rate_limited";
const ERROR_KIND_INVALID_RESPONSE: &str = "invalid_response";
const ERROR_KIND_NETWORK: &str = "network";
const ERROR_KIND_UNKNOWN: &str = "unknown";

/// Dify DSL の環境変数 sql と同等の候補集計クエリ。
/// home 系ワールドを除外し、world_id (無ければ正規化した world_name) で
/// グループ化して直近訪問順に最大100件返す。
const VISITED_WORLD_CANDIDATES_SQL: &str = "\
SELECT
    COALESCE(NULLIF(TRIM(world_id), ''), LOWER(TRIM(world_name))) AS world_key,
    world_name,
    MIN(world_id) AS world_id,
    COUNT(*) AS visit_count
FROM visit_histories
WHERE LOWER(TRIM(world_name)) NOT IN ('home', 'ホーム', 'ホームワールド')
GROUP BY COALESCE(NULLIF(TRIM(world_id), ''), LOWER(TRIM(world_name)))
ORDER BY MAX(visited_at) DESC
LIMIT 100";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiFailureKind {
    MissingApiKey,
    InvalidApiKey,
    ServiceUnavailable,
    Timeout,
    RateLimited,
    InvalidResponse,
    SidecarFailed,
    Network,
    Unknown,
}

#[derive(Debug)]
struct AiFailure {
    kind: AiFailureKind,
    detail: String,
}

impl AiFailure {
    fn new(kind: AiFailureKind, detail: String) -> Self {
        Self { kind, detail }
    }
}

// ─── DTOs ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiStatusDto {
    pub has_gemini_api_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisitedWorldCandidate {
    pub world_key: String,
    pub world_name: String,
    pub world_id: Option<String>,
    pub visit_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiVisitedWorldDto {
    pub world_key: String,
    pub world_name: String,
    pub world_id: Option<String>,
    pub visit_count: i64,
    pub matched: bool,
    pub searched_world_name: Option<String>,
    pub world_overview: Option<String>,
    #[serde(rename = "recommendedNumberOfPeople")]
    pub recommended_number_of_people: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiNewWorldDto {
    pub world_name: String,
    pub overview: String,
    pub recommended_number_of_people: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiRecommendationDto {
    pub source: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub text: String,
    pub wants_unvisited: bool,
    pub recommendation_mode: String,
    pub visited_worlds: Vec<AiVisitedWorldDto>,
    pub new_worlds: Vec<AiNewWorldDto>,
}

#[derive(Debug, Serialize)]
struct SidecarRequest<'a> {
    api_key: &'a str,
    model: &'a str,
    query: &'a str,
    query_history: &'a str,
    visited_worlds: &'a [VisitedWorldCandidate],
}

#[derive(Debug, Deserialize)]
struct SidecarResponse {
    ok: bool,
    #[serde(default)]
    error: String,
    #[serde(default)]
    error_kind: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    wants_unvisited: bool,
    #[serde(default)]
    recommendation_mode: String,
    #[serde(default)]
    visited_worlds: Vec<AiVisitedWorldDto>,
    #[serde(default)]
    new_worlds: Vec<AiNewWorldDto>,
}

// ─── Keyring helpers ──────────────────────────────────────────────────────────

fn load_gemini_api_key() -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ENTRY).ok()?;
    entry
        .get_password()
        .ok()
        .filter(|value| !value.trim().is_empty())
}

pub fn has_gemini_api_key() -> bool {
    load_gemini_api_key().is_some()
}

fn store_gemini_api_key(api_key: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ENTRY)
        .map_err(|e| format!("keyring entry creation failed: {e}"))?;
    entry
        .set_password(api_key)
        .map_err(|e| format!("keyring save failed: {e}"))?;

    // 読み戻して保存できたことを確認する (VRChat auth cookie と同じ方針)。
    // エラーメッセージにはキーの値を含めない。
    match load_gemini_api_key() {
        Some(saved) if saved == api_key => Ok(()),
        _ => Err("saved Gemini API key read-back mismatch".to_string()),
    }
}

fn delete_gemini_api_key() -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ENTRY)
        .map_err(|e| format!("keyring entry creation failed: {e}"))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keyring delete failed: {e}")),
    }
}

// ─── DB helpers ─────────────────────────────────────────────────────────────────

fn query_visited_world_candidates(
    connection: &Connection,
) -> Result<Vec<VisitedWorldCandidate>, String> {
    let mut statement = connection
        .prepare(VISITED_WORLD_CANDIDATES_SQL)
        .map_err(|error| format!("visited world candidates query prepare failed: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            let world_id: Option<String> = row.get("world_id")?;
            Ok(VisitedWorldCandidate {
                world_key: row.get("world_key")?,
                world_name: row.get("world_name")?,
                world_id: world_id.filter(|value| !value.trim().is_empty()),
                visit_count: row.get("visit_count")?,
            })
        })
        .map_err(|error| format!("visited world candidates query failed: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("visited world candidates row decode failed: {error}"))
}

fn normalize_recommendation_mode(value: &str) -> &'static str {
    match value.trim() {
        RECOMMENDATION_MODE_UNVISITED_ONLY => RECOMMENDATION_MODE_UNVISITED_ONLY,
        RECOMMENDATION_MODE_MIXED => RECOMMENDATION_MODE_MIXED,
        RECOMMENDATION_MODE_VISITED_ONLY => RECOMMENDATION_MODE_VISITED_ONLY,
        _ => RECOMMENDATION_MODE_VISITED_ONLY,
    }
}

fn classify_ai_error(error_kind: &str, message: &str) -> AiFailureKind {
    match error_kind.trim() {
        ERROR_KIND_INVALID_API_KEY => return AiFailureKind::InvalidApiKey,
        ERROR_KIND_SERVICE_UNAVAILABLE => return AiFailureKind::ServiceUnavailable,
        ERROR_KIND_TIMEOUT => return AiFailureKind::Timeout,
        ERROR_KIND_RATE_LIMITED => return AiFailureKind::RateLimited,
        ERROR_KIND_INVALID_RESPONSE => return AiFailureKind::InvalidResponse,
        ERROR_KIND_NETWORK => return AiFailureKind::Network,
        ERROR_KIND_UNKNOWN => return AiFailureKind::Unknown,
        _ => {}
    }

    let message = message.to_lowercase();
    if contains_any(&message, &["deadline exceeded", "timeout", "timed out"]) {
        return AiFailureKind::Timeout;
    }
    if contains_any(
        &message,
        &[
            "api key not valid",
            "api_key_invalid",
            "invalid api key",
            "unauthorized",
            "unauthenticated",
            "permission denied",
            "authentication",
            "401",
            "403",
        ],
    ) {
        return AiFailureKind::InvalidApiKey;
    }
    if contains_any(
        &message,
        &["resource_exhausted", "rate limit", "quota", "429"],
    ) {
        return AiFailureKind::RateLimited;
    }
    if contains_any(
        &message,
        &[
            "unavailable",
            "overloaded",
            "503",
            "502",
            "500",
            "internal server error",
        ],
    ) {
        return AiFailureKind::ServiceUnavailable;
    }
    if contains_any(
        &message,
        &[
            "structured output decode failed",
            "returned empty",
            "unmarshal",
            "parse",
            "応答の解析に失敗",
        ],
    ) {
        return AiFailureKind::InvalidResponse;
    }
    if contains_any(
        &message,
        &[
            "sidecar の起動に失敗",
            "sidecar が見つかりません",
            "sidecar stdin",
            "sidecar への書き込みに失敗",
            "sidecar の実行に失敗",
            "sidecar から応答がありません",
        ],
    ) {
        return AiFailureKind::SidecarFailed;
    }
    if contains_any(
        &message,
        &[
            "connection refused",
            "no such host",
            "network",
            "dial tcp",
            "connection reset",
            "eof",
        ],
    ) {
        return AiFailureKind::Network;
    }

    AiFailureKind::Unknown
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn ai_failure_message(kind: AiFailureKind) -> String {
    match kind {
        AiFailureKind::MissingApiKey => {
            "AI推薦を使うには、APIキーの設定が必要です。\n設定画面でAPIキーを登録してから、もう一度お試しください。"
        }
        AiFailureKind::InvalidApiKey => {
            "AI推薦を実行できませんでした。\n設定されているAPIキーが正しくないか、現在利用できない状態です。\n設定画面でAPIキーを確認してください。"
        }
        AiFailureKind::ServiceUnavailable => {
            "AIサービスが一時的に混み合っています。\nしばらく時間をおいてから、もう一度おすすめを生成してください。"
        }
        AiFailureKind::Timeout => {
            "AIからの返事に時間がかかりすぎたため、処理を中断しました。\n通信状況を確認するか、少し短い内容で再度お試しください。"
        }
        AiFailureKind::RateLimited => {
            "短時間にAIへのリクエストが多く送られました。\n少し時間をおいてから、もう一度お試しください。"
        }
        AiFailureKind::InvalidResponse => {
            "AIからの返答をアプリが正しく読み取れませんでした。\nもう一度実行すると成功する場合があります。"
        }
        AiFailureKind::SidecarFailed => {
            "AI推薦機能の内部処理を開始できませんでした。\nアプリを再起動してから、もう一度お試しください。"
        }
        AiFailureKind::Network | AiFailureKind::Unknown => {
            "AI推薦の生成中に問題が発生しました。\n時間をおいて再度お試しください。"
        }
    }
    .to_string()
}

fn is_retryable_ai_error(kind: AiFailureKind) -> bool {
    matches!(
        kind,
        AiFailureKind::ServiceUnavailable
            | AiFailureKind::Timeout
            | AiFailureKind::RateLimited
            | AiFailureKind::InvalidResponse
            | AiFailureKind::Network
            | AiFailureKind::Unknown
    )
}

fn is_transient_ai_error(kind: AiFailureKind) -> bool {
    matches!(
        kind,
        AiFailureKind::ServiceUnavailable
            | AiFailureKind::Timeout
            | AiFailureKind::RateLimited
            | AiFailureKind::Network
    )
}

fn failure_dto(source: &str, kind: AiFailureKind) -> AiRecommendationDto {
    AiRecommendationDto {
        source: source.to_string(),
        retryable: is_retryable_ai_error(kind),
        warning: None,
        error_message: Some(ai_failure_message(kind)),
        text: String::new(),
        wants_unvisited: false,
        recommendation_mode: RECOMMENDATION_MODE_VISITED_ONLY.to_string(),
        visited_worlds: Vec::new(),
        new_worlds: Vec::new(),
    }
}

fn success_dto(
    source: &str,
    warning: Option<&str>,
    response: SidecarResponse,
) -> AiRecommendationDto {
    AiRecommendationDto {
        source: source.to_string(),
        retryable: false,
        warning: warning.map(str::to_string),
        error_message: None,
        text: response.text,
        wants_unvisited: response.wants_unvisited,
        recommendation_mode: normalize_recommendation_mode(&response.recommendation_mode)
            .to_string(),
        visited_worlds: response.visited_worlds,
        new_worlds: response.new_worlds,
    }
}

// ─── Sidecar helpers ──────────────────────────────────────────────────────

fn resolve_sidecar_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os(SIDECAR_ENV_OVERRIDE) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "{SIDECAR_ENV_OVERRIDE} で指定された AI sidecar が見つかりません: {}",
            path.display()
        ));
    }

    let exe = env::current_exe()
        .map_err(|error| format!("current executable path resolve failed: {error}"))?;
    let Some(dir) = exe.parent() else {
        return Err("current executable directory resolve failed".to_string());
    };

    for name in SIDECAR_BINARY_NAMES {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(
        "AI sidecar (worldrec-ai) が見つかりません。アプリと同じフォルダに worldrec-ai を配置するか、npm run build:sidecar を実行してください。"
            .to_string(),
    )
}

fn run_sidecar(sidecar_path: &PathBuf, request_json: &[u8]) -> Result<SidecarResponse, String> {
    let mut command = Command::new(sidecar_path);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("AI sidecar の起動に失敗しました: {error}"))?;

    {
        let Some(stdin) = child.stdin.as_mut() else {
            let _ = child.kill();
            return Err("AI sidecar stdin open failed".to_string());
        };
        stdin
            .write_all(request_json)
            .map_err(|error| format!("AI sidecar への書き込みに失敗しました: {error}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("AI sidecar の実行に失敗しました: {error}"))?;

    if !output.stderr.is_empty() {
        eprintln!(
            "worldrec-ai stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    parse_sidecar_output(&output.stdout, output.status.code())
}

fn parse_sidecar_output(stdout: &[u8], exit_code: Option<i32>) -> Result<SidecarResponse, String> {
    if stdout.is_empty() {
        return Err(format!(
            "AI sidecar から応答がありませんでした (exit code: {exit_code:?})"
        ));
    }

    serde_json::from_slice::<SidecarResponse>(stdout)
        .map_err(|error| format!("AI sidecar 応答の解析に失敗しました: {error}"))
}

async fn run_recommendation_attempt(
    sidecar_path: &PathBuf,
    api_key: &str,
    model: &str,
    query: &str,
    query_history: &str,
    candidates: &[VisitedWorldCandidate],
) -> Result<SidecarResponse, AiFailure> {
    let sidecar_path = sidecar_path.clone();
    let api_key = api_key.to_string();
    let model = model.to_string();
    let query = query.to_string();
    let query_history = query_history.to_string();
    let candidates = candidates.to_vec();

    let response = tauri::async_runtime::spawn_blocking(move || {
        let request = SidecarRequest {
            api_key: &api_key,
            model: &model,
            query: &query,
            query_history: &query_history,
            visited_worlds: &candidates,
        };
        let request_json = serde_json::to_vec(&request)
            .map_err(|error| format!("AI sidecar リクエストの作成に失敗しました: {error}"))?;

        run_sidecar(&sidecar_path, &request_json)
    })
    .await
    .map_err(|error| {
        let detail = format!("AI recommendation task failed: {error}");
        AiFailure::new(classify_ai_error("", &detail), detail)
    })?
    .map_err(|detail| AiFailure::new(classify_ai_error("", &detail), detail))?;

    if response.ok {
        return Ok(response);
    }

    let detail = if response.error.trim().is_empty() {
        "AIワークフローの実行に失敗しました。".to_string()
    } else {
        response.error.clone()
    };
    Err(AiFailure::new(
        classify_ai_error(&response.error_kind, &detail),
        detail,
    ))
}

async fn run_degraded_recommendation(
    sidecar_path: &PathBuf,
    api_key: &str,
    model: &str,
    query: &str,
    query_history: &str,
    candidates: &[VisitedWorldCandidate],
) -> AiRecommendationDto {
    let mut degraded_candidates = candidates.to_vec();
    degraded_candidates.truncate(DEGRADED_MODE_MAX_VISITED_WORLDS);

    match run_recommendation_attempt(
        sidecar_path,
        api_key,
        model,
        query,
        query_history,
        &degraded_candidates,
    )
    .await
    {
        Ok(response) => success_dto("ai_degraded", Some(AI_DEGRADED_WARNING), response),
        Err(failure) => {
            eprintln!(
                "AI degraded recommendation failed ({:?}): {}",
                failure.kind, failure.detail
            );
            if failure.kind == AiFailureKind::InvalidApiKey {
                failure_dto("settings_required", AiFailureKind::InvalidApiKey)
            } else {
                failure_dto("ai_failed", failure.kind)
            }
        }
    }
}

// ─── Commands ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_ai_status() -> AiStatusDto {
    AiStatusDto {
        has_gemini_api_key: has_gemini_api_key(),
    }
}

#[tauri::command]
pub fn save_gemini_api_key(api_key: String) -> Result<AiStatusDto, String> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("Gemini APIキーを入力してください。".to_string());
    }

    store_gemini_api_key(api_key)?;

    Ok(AiStatusDto {
        has_gemini_api_key: true,
    })
}

#[tauri::command]
pub fn clear_gemini_api_key() -> Result<AiStatusDto, String> {
    delete_gemini_api_key()?;

    if load_gemini_api_key().is_some() {
        return Err("Gemini APIキーの削除後も保存データが残っています。".to_string());
    }

    Ok(AiStatusDto {
        has_gemini_api_key: false,
    })
}

#[tauri::command]
pub async fn recommend_worlds(
    query: String,
    query_history: String,
) -> Result<AiRecommendationDto, String> {
    if query.trim().is_empty() {
        return Err("メッセージを入力してください。".to_string());
    }

    let settings = crate::settings::load_settings();
    if !settings.ai_enabled {
        return Err("AI機能が無効です。設定画面のAI設定から有効にしてください。".to_string());
    }

    let Some(api_key) = load_gemini_api_key() else {
        return Ok(failure_dto(
            "settings_required",
            AiFailureKind::MissingApiKey,
        ));
    };

    let sidecar_path = match resolve_sidecar_path() {
        Ok(path) => path,
        Err(detail) => {
            eprintln!("AI sidecar resolve failed: {detail}");
            return Ok(failure_dto("ai_failed", AiFailureKind::SidecarFailed));
        }
    };

    let model = settings.gemini_model.clone();
    let db_path = match resolve_db_path(&settings.db_path) {
        Ok(path) => path,
        Err(detail) => {
            eprintln!("AI recommendation database path resolve failed: {detail}");
            return Ok(failure_dto("ai_failed", AiFailureKind::Unknown));
        }
    };

    let candidates = match tauri::async_runtime::spawn_blocking(move || {
        let connection = open_database(&db_path).map_err(|error| {
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
        query_visited_world_candidates(&connection)
    })
    .await
    {
        Ok(Ok(candidates)) => candidates,
        Ok(Err(detail)) => {
            eprintln!("AI recommendation candidate query failed: {detail}");
            return Ok(failure_dto("ai_failed", AiFailureKind::Unknown));
        }
        Err(error) => {
            eprintln!("AI recommendation candidate query task failed: {error}");
            return Ok(failure_dto("ai_failed", AiFailureKind::Unknown));
        }
    };

    let first_attempt = run_recommendation_attempt(
        &sidecar_path,
        &api_key,
        &model,
        &query,
        &query_history,
        &candidates,
    )
    .await;

    match first_attempt {
        Ok(response) => Ok(success_dto("ai", None, response)),
        Err(failure) => {
            eprintln!(
                "AI recommendation attempt failed ({:?}): {}",
                failure.kind, failure.detail
            );
            match failure.kind {
                AiFailureKind::InvalidApiKey => Ok(failure_dto(
                    "settings_required",
                    AiFailureKind::InvalidApiKey,
                )),
                AiFailureKind::SidecarFailed => {
                    Ok(failure_dto("ai_failed", AiFailureKind::SidecarFailed))
                }
                kind if is_transient_ai_error(kind) => {
                    tokio::time::sleep(Duration::from_secs(AI_RETRY_WAIT_SECONDS)).await;
                    let retry_attempt = run_recommendation_attempt(
                        &sidecar_path,
                        &api_key,
                        &model,
                        &query,
                        &query_history,
                        &candidates,
                    )
                    .await;

                    match retry_attempt {
                        Ok(response) => Ok(success_dto("ai", None, response)),
                        Err(retry_failure) => {
                            eprintln!(
                                "AI recommendation retry failed ({:?}): {}",
                                retry_failure.kind, retry_failure.detail
                            );
                            if retry_failure.kind == AiFailureKind::InvalidApiKey {
                                Ok(failure_dto(
                                    "settings_required",
                                    AiFailureKind::InvalidApiKey,
                                ))
                            } else if retry_failure.kind == AiFailureKind::SidecarFailed {
                                Ok(failure_dto("ai_failed", AiFailureKind::SidecarFailed))
                            } else {
                                Ok(run_degraded_recommendation(
                                    &sidecar_path,
                                    &api_key,
                                    &model,
                                    &query,
                                    &query_history,
                                    &candidates,
                                )
                                .await)
                            }
                        }
                    }
                }
                _ => Ok(run_degraded_recommendation(
                    &sidecar_path,
                    &api_key,
                    &model,
                    &query,
                    &query_history,
                    &candidates,
                )
                .await),
            }
        }
    }
}

// ─── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn insert_visit(
        connection: &Connection,
        visited_at: &str,
        world_name: &str,
        world_id: Option<&str>,
        tags: Option<&str>,
        memo: Option<&str>,
    ) {
        connection
            .execute(
                "INSERT INTO visit_histories (
                    visited_at, world_name, world_id, instance_id,
                    instance_access_type, instance_nonce, instance_raw_tags,
                    stay_duration_seconds, memo, tags, source_log_file,
                    created_at, updated_at
                ) VALUES (?1, ?2, ?3, NULL, 'public', NULL, NULL, 600, ?4, ?5, 'test.txt',
                    CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
                params![visited_at, world_name, world_id, memo, tags],
            )
            .expect("test visit insert should succeed");
    }

    fn test_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory database should open");
        initialize_database(&connection).expect("schema should initialize");
        connection
    }

    #[test]
    fn candidates_exclude_home_worlds() {
        let connection = test_connection();
        insert_visit(&connection, "2026-06-15T10:00:00", "Home", None, None, None);
        insert_visit(
            &connection,
            "2026-06-15T11:00:00",
            "ホーム",
            None,
            None,
            None,
        );
        insert_visit(
            &connection,
            "2026-06-15T12:00:00",
            "Cozy Cafe",
            Some("wrld_cafe"),
            Some("cozy"),
            Some("落ち着く"),
        );

        let candidates =
            query_visited_world_candidates(&connection).expect("candidates should query");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].world_name, "Cozy Cafe");
        assert_eq!(candidates[0].world_key, "wrld_cafe");
        assert_eq!(candidates[0].world_id.as_deref(), Some("wrld_cafe"));
        assert_eq!(candidates[0].visit_count, 1);
    }

    #[test]
    fn candidates_group_by_world_id_and_order_by_latest_visit() {
        let connection = test_connection();
        insert_visit(
            &connection,
            "2026-06-10T10:00:00",
            "World A",
            Some("wrld_a"),
            None,
            None,
        );
        insert_visit(
            &connection,
            "2026-06-20T10:00:00",
            "World A",
            Some("wrld_a"),
            None,
            None,
        );
        insert_visit(
            &connection,
            "2026-06-15T10:00:00",
            "World B",
            Some("wrld_b"),
            None,
            None,
        );

        let candidates =
            query_visited_world_candidates(&connection).expect("candidates should query");

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].world_key, "wrld_a");
        assert_eq!(candidates[0].visit_count, 2);
        assert_eq!(candidates[1].world_key, "wrld_b");
    }

    #[test]
    fn candidates_without_world_id_group_by_normalized_name() {
        let connection = test_connection();
        insert_visit(
            &connection,
            "2026-06-15T10:00:00",
            "  Test World  ",
            None,
            None,
            None,
        );
        insert_visit(
            &connection,
            "2026-06-15T11:00:00",
            "test world",
            None,
            None,
            None,
        );

        let candidates =
            query_visited_world_candidates(&connection).expect("candidates should query");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].world_key, "test world");
        assert_eq!(candidates[0].visit_count, 2);
        assert_eq!(candidates[0].world_id, None);
    }

    #[test]
    fn sidecar_request_serializes_expected_shape() {
        let candidates = vec![VisitedWorldCandidate {
            world_key: "wrld_a".to_string(),
            world_name: "World A".to_string(),
            world_id: Some("wrld_a".to_string()),
            visit_count: 3,
        }];
        let request = SidecarRequest {
            api_key: "SECRET",
            model: "gemini-2.5-flash",
            query: "静かなワールド",
            query_history: "",
            visited_worlds: &candidates,
        };

        let json = serde_json::to_value(&request).expect("request should serialize");

        assert_eq!(json["api_key"], "SECRET");
        assert_eq!(json["model"], "gemini-2.5-flash");
        assert_eq!(json["query"], "静かなワールド");
        assert_eq!(json["visited_worlds"][0]["world_key"], "wrld_a");
        assert_eq!(json["visited_worlds"][0]["visit_count"], 3);
        let visited_world = json["visited_worlds"][0]
            .as_object()
            .expect("visited world should serialize as object");
        assert!(!visited_world.contains_key("tags"));
        assert!(!visited_world.contains_key("memo"));
    }

    #[test]
    fn sidecar_response_parses_success_payload() {
        let raw = r#"{
            "ok": true,
            "text": "回答",
            "wants_unvisited": true,
            "recommendation_mode": "mixed",
            "visited_worlds": [{
                "world_key": "wrld_a",
                "world_name": "World A",
                "world_id": "wrld_a",
                "visit_count": 2,
                "matched": true,
                "searched_world_name": "World A",
                "world_overview": "概要",
                "recommendedNumberOfPeople": 4
            }],
            "new_worlds": [{
                "world_name": "New World",
                "overview": "新しい",
                "recommended_number_of_people": "不明"
            }]
        }"#;

        let response = parse_sidecar_output(raw.as_bytes(), Some(0)).expect("should parse");

        assert!(response.ok);
        assert_eq!(response.text, "回答");
        assert!(response.wants_unvisited);
        assert_eq!(response.recommendation_mode, "mixed");
        assert_eq!(response.visited_worlds.len(), 1);
        assert_eq!(response.visited_worlds[0].recommended_number_of_people, 4);
        assert_eq!(response.new_worlds[0].recommended_number_of_people, "不明");
    }

    #[test]
    fn sidecar_response_parses_error_payload() {
        let raw = r#"{"ok": false, "error": "failed", "error_kind": "invalid_api_key", "text": "", "visited_worlds": [], "new_worlds": []}"#;

        let response = parse_sidecar_output(raw.as_bytes(), Some(2)).expect("should parse");

        assert!(!response.ok);
        assert_eq!(response.error, "failed");
        assert_eq!(response.error_kind, "invalid_api_key");
    }

    #[test]
    fn empty_sidecar_output_is_an_error() {
        let result = parse_sidecar_output(&[], Some(1));

        assert!(result.is_err());
    }

    #[test]
    fn recommendation_mode_defaults_to_visited_only() {
        assert_eq!(
            normalize_recommendation_mode(""),
            RECOMMENDATION_MODE_VISITED_ONLY
        );
        assert_eq!(
            normalize_recommendation_mode("unexpected"),
            RECOMMENDATION_MODE_VISITED_ONLY
        );
        assert_eq!(
            normalize_recommendation_mode("unvisited_only"),
            RECOMMENDATION_MODE_UNVISITED_ONLY
        );
        assert_eq!(
            normalize_recommendation_mode("mixed"),
            RECOMMENDATION_MODE_MIXED
        );
    }

    #[test]
    fn ai_status_dto_uses_camel_case() {
        let dto = AiStatusDto {
            has_gemini_api_key: true,
        };
        let json = serde_json::to_string(&dto).expect("should serialize");

        assert!(json.contains("hasGeminiApiKey"));
    }

    #[test]
    fn classify_ai_error_prefers_error_kind_contract() {
        let cases = [
            (ERROR_KIND_INVALID_API_KEY, AiFailureKind::InvalidApiKey),
            (
                ERROR_KIND_SERVICE_UNAVAILABLE,
                AiFailureKind::ServiceUnavailable,
            ),
            (ERROR_KIND_TIMEOUT, AiFailureKind::Timeout),
            (ERROR_KIND_RATE_LIMITED, AiFailureKind::RateLimited),
            (ERROR_KIND_INVALID_RESPONSE, AiFailureKind::InvalidResponse),
            (ERROR_KIND_NETWORK, AiFailureKind::Network),
            (ERROR_KIND_UNKNOWN, AiFailureKind::Unknown),
        ];

        for (error_kind, expected) in cases {
            assert_eq!(
                classify_ai_error(error_kind, "invalid api key"),
                expected,
                "error_kind={error_kind}"
            );
        }
    }

    #[test]
    fn classify_ai_error_uses_message_when_error_kind_is_empty() {
        let cases = [
            ("Error 401: invalid API key", AiFailureKind::InvalidApiKey),
            (
                "AI sidecar の起動に失敗しました",
                AiFailureKind::SidecarFailed,
            ),
            (
                "AI sidecar への書き込みに失敗しました",
                AiFailureKind::SidecarFailed,
            ),
            (
                "AI sidecar 応答の解析に失敗しました",
                AiFailureKind::InvalidResponse,
            ),
            ("deadline exceeded", AiFailureKind::Timeout),
            ("unexpected workflow failure", AiFailureKind::Unknown),
        ];

        for (message, expected) in cases {
            assert_eq!(
                classify_ai_error("", message),
                expected,
                "message={message}"
            );
        }
    }

    #[test]
    fn ai_failure_message_matches_settings_required_copy() {
        assert_eq!(
            ai_failure_message(AiFailureKind::MissingApiKey),
            "AI推薦を使うには、APIキーの設定が必要です。\n設定画面でAPIキーを登録してから、もう一度お試しください。"
        );
        assert_eq!(
            ai_failure_message(AiFailureKind::InvalidApiKey),
            "AI推薦を実行できませんでした。\n設定されているAPIキーが正しくないか、現在利用できない状態です。\n設定画面でAPIキーを確認してください。"
        );
    }

    #[test]
    fn ai_failure_retryable_and_transient_flags_match_contract() {
        for kind in [
            AiFailureKind::ServiceUnavailable,
            AiFailureKind::Timeout,
            AiFailureKind::RateLimited,
            AiFailureKind::InvalidResponse,
            AiFailureKind::Network,
            AiFailureKind::Unknown,
        ] {
            assert!(is_retryable_ai_error(kind), "{kind:?} should be retryable");
        }
        for kind in [
            AiFailureKind::MissingApiKey,
            AiFailureKind::InvalidApiKey,
            AiFailureKind::SidecarFailed,
        ] {
            assert!(
                !is_retryable_ai_error(kind),
                "{kind:?} should not be retryable"
            );
        }

        for kind in [
            AiFailureKind::ServiceUnavailable,
            AiFailureKind::Timeout,
            AiFailureKind::RateLimited,
            AiFailureKind::Network,
        ] {
            assert!(is_transient_ai_error(kind), "{kind:?} should be transient");
        }
        for kind in [
            AiFailureKind::MissingApiKey,
            AiFailureKind::InvalidApiKey,
            AiFailureKind::InvalidResponse,
            AiFailureKind::SidecarFailed,
            AiFailureKind::Unknown,
        ] {
            assert!(
                !is_transient_ai_error(kind),
                "{kind:?} should not be transient"
            );
        }
    }

    #[test]
    fn settings_required_failure_dto_serializes_empty_recommendations() {
        let dto = failure_dto("settings_required", AiFailureKind::MissingApiKey);
        let json = serde_json::to_value(&dto).expect("dto should serialize");

        assert_eq!(json["source"], "settings_required");
        assert_eq!(json["retryable"], false);
        assert_eq!(json["text"], "");
        assert_eq!(json["wants_unvisited"], false);
        assert_eq!(
            json["recommendation_mode"],
            RECOMMENDATION_MODE_VISITED_ONLY
        );
        assert_eq!(json["visited_worlds"].as_array().unwrap().len(), 0);
        assert_eq!(json["new_worlds"].as_array().unwrap().len(), 0);
        assert!(json["error_message"].as_str().unwrap().contains("APIキー"));
    }

    #[test]
    fn ai_failed_unknown_failure_dto_serializes_retryable_generic_error() {
        let dto = failure_dto("ai_failed", AiFailureKind::Unknown);
        let json = serde_json::to_value(&dto).expect("dto should serialize");

        assert_eq!(json["source"], "ai_failed");
        assert_eq!(json["retryable"], true);
        assert_eq!(
            json["error_message"],
            "AI推薦の生成中に問題が発生しました。\n時間をおいて再度お試しください。"
        );
        assert_eq!(json["text"], "");
        assert_eq!(json["visited_worlds"].as_array().unwrap().len(), 0);
        assert_eq!(json["new_worlds"].as_array().unwrap().len(), 0);
    }
}

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[cfg_attr(test, allow(dead_code))]
const VRCHAT_API_BASE_URL: &str = "https://api.vrchat.cloud/api/1/worlds";
#[cfg_attr(test, allow(dead_code))]
const USER_AGENT: &str = "WorldRec/0.1.0 (https://github.com/Hiragi0w0/WorldRec)";
#[cfg_attr(test, allow(dead_code))]
const REQUEST_TIMEOUT_SECONDS: u64 = 8;
const DEFAULT_RATE_LIMIT_COOLDOWN_SECONDS: u64 = 120;
const NEGATIVE_CACHE_TTL_SECONDS: u64 = 15 * 60;
const GLOBAL_REQUEST_INTERVAL: Duration = Duration::from_millis(1500);
const REQUEST_JITTER_MAX: Duration = Duration::from_millis(300);
const MAX_RATE_LIMIT_RETRIES: u32 = 3;
const BACKOFF_BASE_DELAY: Duration = Duration::from_secs(1);
const BACKOFF_MAX_DELAY: Duration = Duration::from_secs(30);

pub struct VrchatApiState {
    inner: Mutex<VrchatApiStateInner>,
}

#[derive(Default)]
struct VrchatApiStateInner {
    cooldown_until: Option<Instant>,
    negative_cache: HashMap<String, Instant>,
    pending: HashSet<String>,
    last_request_at: Option<Instant>,
}

impl VrchatApiState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(VrchatApiStateInner::default()),
        }
    }

    fn ensure_request_allowed(&self, world_id: &str, now: Instant) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "VRChat API state lock failed".to_string())?;

        if let Some(cooldown_until) = inner.cooldown_until {
            if now < cooldown_until {
                return Err("VRChat API request skipped: rate limit cooldown active".to_string());
            }
            inner.cooldown_until = None;
        }

        if let Some(cached_until) = inner.negative_cache.get(world_id).copied() {
            if now < cached_until {
                return Err(
                    "VRChat API request skipped: world detail is temporarily unavailable"
                        .to_string(),
                );
            }
            inner.negative_cache.remove(world_id);
        }

        Ok(())
    }

    fn activate_cooldown(&self, now: Instant, seconds: u64) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "VRChat API state lock failed".to_string())?;
        inner.cooldown_until = Some(now + Duration::from_secs(seconds));

        Ok(())
    }

    fn cache_negative_result(&self, world_id: &str, now: Instant) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "VRChat API state lock failed".to_string())?;
        inner.negative_cache.insert(
            world_id.to_string(),
            now + Duration::from_secs(NEGATIVE_CACHE_TTL_SECONDS),
        );

        Ok(())
    }

    /// Marks `world_id` as in-flight. Returns `false` without modifying state
    /// if a request for the same world is already in progress.
    fn try_begin_pending(&self, world_id: &str) -> Result<bool, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "VRChat API state lock failed".to_string())?;

        if inner.pending.contains(world_id) {
            return Ok(false);
        }

        inner.pending.insert(world_id.to_string());
        Ok(true)
    }

    fn finish_pending(&self, world_id: &str) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.pending.remove(world_id);
        }
    }

    /// Atomically reserves the next available request slot and returns the
    /// time at which the caller should send. The caller must sleep until
    /// that instant before issuing the HTTP request.
    fn reserve_request_slot(&self) -> Result<Instant, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "VRChat API state lock failed".to_string())?;
        let now = Instant::now();
        let jitter = compute_request_jitter();
        let send_at = compute_global_wait(inner.last_request_at, now)
            .map(|wait| now + wait + jitter)
            .unwrap_or(now + jitter);
        inner.last_request_at = Some(send_at);
        Ok(send_at)
    }
}

/// RAII guard that clears the pending flag for `world_id` once dropped,
/// regardless of which return path `fetch_world_detail` takes.
struct PendingRequestGuard<'a> {
    state: &'a VrchatApiState,
    world_id: String,
}

impl<'a> Drop for PendingRequestGuard<'a> {
    fn drop(&mut self) {
        self.state.finish_pending(&self.world_id);
    }
}

impl Default for VrchatApiState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VrchatApiStatusKind {
    NotFound,
    Forbidden,
    RateLimited,
    Other(u16),
}

pub fn classify_status(status: reqwest::StatusCode) -> VrchatApiStatusKind {
    match status.as_u16() {
        404 => VrchatApiStatusKind::NotFound,
        403 => VrchatApiStatusKind::Forbidden,
        429 => VrchatApiStatusKind::RateLimited,
        other => VrchatApiStatusKind::Other(other),
    }
}

pub fn parse_retry_after_seconds(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok()
}

/// Returns the duration to wait before the next request can be sent,
/// or `None` if the interval has already elapsed.
fn compute_global_wait(last_request_at: Option<Instant>, now: Instant) -> Option<Duration> {
    last_request_at.and_then(|last| {
        let next = last + GLOBAL_REQUEST_INTERVAL;
        if next > now {
            Some(next - now)
        } else {
            None
        }
    })
}

/// Returns a random duration in [0, REQUEST_JITTER_MAX] for normal request spacing.
fn compute_request_jitter() -> Duration {
    let max_millis = REQUEST_JITTER_MAX.as_millis() as u64;
    Duration::from_millis(rand::thread_rng().gen_range(0..=max_millis))
}

/// Returns the exponential backoff cap for the given retry attempt (Full Jitter).
fn compute_backoff_cap(retry_count: u32) -> Duration {
    BACKOFF_MAX_DELAY.min(BACKOFF_BASE_DELAY * 2u32.saturating_pow(retry_count))
}

/// Returns a random duration in [0, cap] for the given retry attempt.
fn compute_backoff_duration(retry_count: u32) -> Duration {
    let cap = compute_backoff_cap(retry_count);
    let cap_millis = cap.as_millis() as u64;
    let jitter_millis = rand::thread_rng().gen_range(0..=cap_millis);
    Duration::from_millis(jitter_millis)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VrchatWorldDetailDto {
    pub world_id: String,
    pub name: String,
    pub author_name: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub thumbnail_image_url: Option<String>,
    pub capacity: Option<i64>,
    pub recommended_capacity: Option<i64>,
    pub visits: Option<i64>,
    pub favorites: Option<i64>,
    pub occupants: Option<i64>,
    pub public_occupants: Option<i64>,
    pub private_occupants: Option<i64>,
    pub release_status: Option<String>,
    pub tags: Vec<String>,
    pub platforms: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct VrchatWorldApiResponse {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "authorName")]
    author_name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "imageUrl")]
    image_url: Option<String>,
    #[serde(default, rename = "thumbnailImageUrl")]
    thumbnail_image_url: Option<String>,
    #[serde(default)]
    capacity: Option<i64>,
    #[serde(default, rename = "recommendedCapacity")]
    recommended_capacity: Option<i64>,
    #[serde(default)]
    visits: Option<i64>,
    #[serde(default)]
    favorites: Option<i64>,
    #[serde(default)]
    occupants: Option<i64>,
    #[serde(default, rename = "publicOccupants")]
    public_occupants: Option<i64>,
    #[serde(default, rename = "privateOccupants")]
    private_occupants: Option<i64>,
    #[serde(default, rename = "releaseStatus")]
    release_status: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default, rename = "unityPackages")]
    unity_packages: Vec<VrchatUnityPackage>,
}

#[derive(Debug, Clone, Deserialize)]
struct VrchatUnityPackage {
    #[serde(default)]
    platform: Option<String>,
}

pub fn is_valid_vrchat_world_id(world_id: &str) -> bool {
    world_id
        .strip_prefix("wrld_")
        .is_some_and(is_valid_uuid_format)
}

fn is_valid_uuid_format(value: &str) -> bool {
    const SEGMENT_LENGTHS: [usize; 5] = [8, 4, 4, 4, 12];

    let segments: Vec<&str> = value.split('-').collect();
    if segments.len() != SEGMENT_LENGTHS.len() {
        return false;
    }

    segments
        .iter()
        .zip(SEGMENT_LENGTHS.iter())
        .all(|(segment, &expected_len)| {
            segment.len() == expected_len && segment.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

#[cfg_attr(test, allow(dead_code))]
pub async fn fetch_world_detail(
    world_id: &str,
    state: &VrchatApiState,
    auth_cookie: Option<&str>,
) -> Result<VrchatWorldDetailDto, String> {
    if !is_valid_vrchat_world_id(world_id) {
        return Err("invalid VRChat world id".to_string());
    }

    state.ensure_request_allowed(world_id, Instant::now())?;

    if !state.try_begin_pending(world_id)? {
        return Err(
            "VRChat API request skipped: a request for this world is already in progress"
                .to_string(),
        );
    }
    let _pending_guard = PendingRequestGuard {
        state,
        world_id: world_id.to_string(),
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("VRChat API client initialization failed: {error}"))?;

    let mut rate_limit_retries = 0u32;
    loop {
        let send_at = state.reserve_request_slot()?;
        let now = Instant::now();
        if send_at > now {
            tokio::time::sleep(send_at - now).await;
        }

        let mut request = client.get(format!("{VRCHAT_API_BASE_URL}/{world_id}"));
        if let Some(cookie) = auth_cookie {
            request = request.header(reqwest::header::COOKIE, format!("auth={cookie}"));
        }
        let response = request.send().await.map_err(|error| {
            format!(
                "VRChat API request failed: {}",
                classify_request_error(&error)
            )
        })?;

        let status = response.status();
        if status.is_success() {
            return response
                .json::<VrchatWorldApiResponse>()
                .await
                .map(VrchatWorldDetailDto::from)
                .map_err(|error| format!("VRChat API response parse failed: {error}"));
        }

        match classify_status(status) {
            VrchatApiStatusKind::RateLimited => {
                if rate_limit_retries >= MAX_RATE_LIMIT_RETRIES {
                    state.activate_cooldown(Instant::now(), DEFAULT_RATE_LIMIT_COOLDOWN_SECONDS)?;
                    return Err(format!(
                        "VRChat API request failed: {} (after {} retries)",
                        status.as_u16(),
                        rate_limit_retries
                    ));
                }
                let sleep_duration = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .and_then(parse_retry_after_seconds)
                    .map(Duration::from_secs)
                    .unwrap_or_else(|| compute_backoff_duration(rate_limit_retries));
                rate_limit_retries += 1;
                tokio::time::sleep(sleep_duration).await;
            }
            VrchatApiStatusKind::NotFound | VrchatApiStatusKind::Forbidden => {
                state.cache_negative_result(world_id, Instant::now())?;
                return Err(format!("VRChat API request failed: {}", status.as_u16()));
            }
            VrchatApiStatusKind::Other(_) => {
                return Err(format!("VRChat API request failed: {}", status.as_u16()));
            }
        }
    }
}

#[cfg_attr(test, allow(dead_code))]
fn classify_request_error(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connect"
    } else {
        "request"
    }
}

fn map_unity_package_platforms(unity_packages: &[VrchatUnityPackage]) -> Vec<String> {
    let mut platforms = Vec::new();

    for unity_package in unity_packages {
        let Some(platform) = unity_package.platform.as_deref() else {
            continue;
        };
        let platform = platform.trim();
        if platform.is_empty() {
            continue;
        }
        if !platforms
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(platform))
        {
            platforms.push(platform.to_string());
        }
    }

    platforms
}

fn normalize_image_url(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_string();
    if value.is_empty() {
        return None;
    }

    let lower = value.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return None;
    }

    Some(value)
}

impl From<VrchatWorldApiResponse> for VrchatWorldDetailDto {
    fn from(value: VrchatWorldApiResponse) -> Self {
        let platforms = map_unity_package_platforms(&value.unity_packages);

        Self {
            world_id: value.id,
            name: value.name,
            author_name: value.author_name,
            description: value.description,
            image_url: normalize_image_url(value.image_url),
            thumbnail_image_url: normalize_image_url(value.thumbnail_image_url),
            capacity: value.capacity,
            recommended_capacity: value.recommended_capacity,
            visits: value.visits,
            favorites: value.favorites,
            occupants: value.occupants,
            public_occupants: value.public_occupants,
            private_occupants: value.private_occupants,
            release_status: value.release_status,
            tags: value.tags,
            platforms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_vrchat_world_ids() {
        assert!(is_valid_vrchat_world_id(
            "wrld_00000000-0000-0000-0000-000000000000"
        ));
        assert!(is_valid_vrchat_world_id(
            "wrld_4cf554b4-430c-4f8f-b53e-1f294eed230b"
        ));
        assert!(!is_valid_vrchat_world_id("wrld_12345"));
        assert!(!is_valid_vrchat_world_id(
            "wrld_00000000-0000-0000-0000-000000000000/extra"
        ));
        assert!(!is_valid_vrchat_world_id("world_12345"));
        assert!(!is_valid_vrchat_world_id("wrld_1"));
        assert!(!is_valid_vrchat_world_id(""));
    }

    #[test]
    fn maps_unity_package_platforms() {
        let platforms = map_unity_package_platforms(&[
            VrchatUnityPackage {
                platform: Some("standalonewindows".to_string()),
            },
            VrchatUnityPackage {
                platform: Some("android".to_string()),
            },
            VrchatUnityPackage {
                platform: Some("StandaloneWindows".to_string()),
            },
            VrchatUnityPackage { platform: None },
        ]);

        assert_eq!(platforms, vec!["standalonewindows", "android"]);
    }

    #[test]
    fn classifies_vrchat_api_status_codes() {
        assert_eq!(
            classify_status(reqwest::StatusCode::NOT_FOUND),
            VrchatApiStatusKind::NotFound
        );
        assert_eq!(
            classify_status(reqwest::StatusCode::FORBIDDEN),
            VrchatApiStatusKind::Forbidden
        );
        assert_eq!(
            classify_status(reqwest::StatusCode::TOO_MANY_REQUESTS),
            VrchatApiStatusKind::RateLimited
        );
        assert_eq!(
            classify_status(reqwest::StatusCode::INTERNAL_SERVER_ERROR),
            VrchatApiStatusKind::Other(500)
        );
    }

    #[test]
    fn parses_retry_after_seconds() {
        assert_eq!(parse_retry_after_seconds("120"), Some(120));
        assert_eq!(parse_retry_after_seconds(" 15 "), Some(15));
        assert_eq!(
            parse_retry_after_seconds("Wed, 21 Oct 2015 07:28:00 GMT"),
            None
        );
        assert_eq!(parse_retry_after_seconds("soon"), None);
        assert_eq!(parse_retry_after_seconds(""), None);
    }

    #[test]
    fn skips_requests_during_cooldown() {
        let state = VrchatApiState::new();
        let now = Instant::now();

        state
            .activate_cooldown(now, DEFAULT_RATE_LIMIT_COOLDOWN_SECONDS)
            .expect("cooldown should be set");

        assert!(state
            .ensure_request_allowed("wrld_00000000-0000-0000-0000-000000000000", now)
            .is_err());
        assert!(state
            .ensure_request_allowed(
                "wrld_00000000-0000-0000-0000-000000000000",
                now + Duration::from_secs(DEFAULT_RATE_LIMIT_COOLDOWN_SECONDS + 1),
            )
            .is_ok());
    }

    #[test]
    fn dedupes_pending_requests_for_same_world() {
        let state = VrchatApiState::new();
        let world_id = "wrld_00000000-0000-0000-0000-000000000000";

        assert!(state.try_begin_pending(world_id).expect("lock should work"));
        assert!(!state.try_begin_pending(world_id).expect("lock should work"));

        state.finish_pending(world_id);

        assert!(state.try_begin_pending(world_id).expect("lock should work"));
    }

    #[test]
    fn skips_negative_cached_world_until_ttl_expires() {
        let state = VrchatApiState::new();
        let world_id = "wrld_00000000-0000-0000-0000-000000000000";
        let now = Instant::now();

        state
            .cache_negative_result(world_id, now)
            .expect("negative cache should be set");

        assert!(state.ensure_request_allowed(world_id, now).is_err());
        assert!(state
            .ensure_request_allowed(
                world_id,
                now + Duration::from_secs(NEGATIVE_CACHE_TTL_SECONDS + 1),
            )
            .is_ok());
    }

    #[test]
    fn global_wait_is_none_when_no_previous_request() {
        let now = Instant::now();
        assert_eq!(compute_global_wait(None, now), None);
    }

    #[test]
    fn global_wait_is_none_when_interval_has_passed() {
        let now = Instant::now();
        let past = now - GLOBAL_REQUEST_INTERVAL - Duration::from_millis(1);
        assert_eq!(compute_global_wait(Some(past), now), None);
    }

    #[test]
    fn global_wait_returns_remaining_duration() {
        let now = Instant::now();
        let last = now - Duration::from_millis(500);
        let wait = compute_global_wait(Some(last), now);
        assert_eq!(wait, Some(Duration::from_millis(1000)));
    }

    #[test]
    fn request_jitter_does_not_exceed_max() {
        for _ in 0..100 {
            let jitter = compute_request_jitter();
            assert!(
                jitter <= REQUEST_JITTER_MAX,
                "jitter {jitter:?} exceeded REQUEST_JITTER_MAX {REQUEST_JITTER_MAX:?}"
            );
        }
    }

    #[test]
    fn reserve_request_slot_schedules_at_least_interval_apart() {
        let state = VrchatApiState::new();

        let first = state.reserve_request_slot().expect("first slot");
        let second = state.reserve_request_slot().expect("second slot");

        assert!(
            second >= first + GLOBAL_REQUEST_INTERVAL,
            "second slot {second:?} should be at least GLOBAL_REQUEST_INTERVAL after first {first:?}"
        );
    }

    #[test]
    fn reserve_request_slot_includes_jitter_above_interval() {
        let state = VrchatApiState::new();

        let first = state.reserve_request_slot().expect("first slot");
        let second = state.reserve_request_slot().expect("second slot");
        let gap = second.duration_since(first + GLOBAL_REQUEST_INTERVAL);

        assert!(
            gap <= REQUEST_JITTER_MAX,
            "gap above interval {gap:?} should not exceed REQUEST_JITTER_MAX {REQUEST_JITTER_MAX:?}"
        );
    }

    #[test]
    fn backoff_cap_grows_exponentially_up_to_max() {
        assert_eq!(compute_backoff_cap(0), Duration::from_secs(1));
        assert_eq!(compute_backoff_cap(1), Duration::from_secs(2));
        assert_eq!(compute_backoff_cap(2), Duration::from_secs(4));
        assert_eq!(compute_backoff_cap(3), Duration::from_secs(8));
        assert_eq!(compute_backoff_cap(10), BACKOFF_MAX_DELAY);
    }

    #[test]
    fn backoff_duration_does_not_exceed_cap() {
        for retry in 0..5 {
            let cap = compute_backoff_cap(retry);
            for _ in 0..50 {
                let duration = compute_backoff_duration(retry);
                assert!(
                    duration <= cap,
                    "retry {retry}: duration {duration:?} exceeded cap {cap:?}"
                );
            }
        }
    }

    #[test]
    fn maps_api_response_to_detail_dto() {
        let response: VrchatWorldApiResponse = serde_json::from_str(
            r#"{
                "id": "wrld_12345",
                "name": "Test World",
                "authorName": "Test Author",
                "description": "World description",
                "imageUrl": "https://example.com/image.jpg",
                "thumbnailImageUrl": "https://example.com/thumb.jpg",
                "capacity": 40,
                "recommendedCapacity": 20,
                "visits": 1000,
                "favorites": 30,
                "occupants": 5,
                "publicOccupants": 3,
                "privateOccupants": 2,
                "releaseStatus": "public",
                "tags": ["author_tag_test"],
                "unityPackages": [
                    {"platform": "standalonewindows"},
                    {"platform": "android"}
                ]
            }"#,
        )
        .expect("response should deserialize");

        let dto = VrchatWorldDetailDto::from(response);

        assert_eq!(dto.world_id, "wrld_12345");
        assert_eq!(dto.name, "Test World");
        assert_eq!(dto.author_name.as_deref(), Some("Test Author"));
        assert_eq!(dto.description.as_deref(), Some("World description"));
        assert_eq!(
            dto.image_url.as_deref(),
            Some("https://example.com/image.jpg")
        );
        assert_eq!(
            dto.thumbnail_image_url.as_deref(),
            Some("https://example.com/thumb.jpg")
        );
        assert_eq!(dto.capacity, Some(40));
        assert_eq!(dto.recommended_capacity, Some(20));
        assert_eq!(dto.visits, Some(1000));
        assert_eq!(dto.favorites, Some(30));
        assert_eq!(dto.occupants, Some(5));
        assert_eq!(dto.public_occupants, Some(3));
        assert_eq!(dto.private_occupants, Some(2));
        assert_eq!(dto.release_status.as_deref(), Some("public"));
        assert_eq!(dto.tags, vec!["author_tag_test"]);
        assert_eq!(dto.platforms, vec!["standalonewindows", "android"]);
    }

    #[test]
    fn normalizes_api_response_image_urls() {
        let response: VrchatWorldApiResponse = serde_json::from_str(
            r#"{
                "id": "wrld_12345",
                "name": "Test World",
                "imageUrl": "  https://example.com/image.jpg  ",
                "thumbnailImageUrl": " HTTP://example.com/thumb.jpg "
            }"#,
        )
        .expect("response should deserialize");

        let dto = VrchatWorldDetailDto::from(response);

        assert_eq!(
            dto.image_url.as_deref(),
            Some("https://example.com/image.jpg")
        );
        assert_eq!(
            dto.thumbnail_image_url.as_deref(),
            Some("HTTP://example.com/thumb.jpg")
        );
    }

    #[test]
    fn drops_empty_api_response_image_urls() {
        let response: VrchatWorldApiResponse = serde_json::from_str(
            r#"{
                "id": "wrld_12345",
                "name": "Test World",
                "imageUrl": "   ",
                "thumbnailImageUrl": ""
            }"#,
        )
        .expect("response should deserialize");

        let dto = VrchatWorldDetailDto::from(response);

        assert_eq!(dto.image_url, None);
        assert_eq!(dto.thumbnail_image_url, None);
    }

    #[test]
    fn drops_unsupported_api_response_image_url_schemes() {
        let unsupported_values = [
            "data:image/png;base64,AAAA",
            "file:///C:/tmp/image.png",
            "javascript:alert(1)",
        ];

        for value in unsupported_values {
            assert_eq!(normalize_image_url(Some(value.to_string())), None);
        }
    }
}

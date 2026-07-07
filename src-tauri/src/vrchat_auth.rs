use base64::Engine;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;

const VRCHAT_API_BASE: &str = "https://api.vrchat.cloud/api/1";
const USER_AGENT: &str = "WorldRec/0.1.0 (https://github.com/Hiragi0w0/WorldRec)";
const REQUEST_TIMEOUT_SECONDS: u64 = 10;
const KEYRING_SERVICE: &str = "WorldRec";
const KEYRING_ENTRY: &str = "vrchat_auth_cookie";

// ─── State ────────────────────────────────────────────────────────────────────

pub struct VrchatAuthState {
    inner: Mutex<VrchatAuthStateInner>,
}

#[derive(Default)]
struct VrchatAuthStateInner {
    pending_auth_cookie: Option<String>,
}

impl VrchatAuthState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(VrchatAuthStateInner::default()),
        }
    }

    fn set_pending(&self, cookie: Option<String>) -> Result<(), String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "auth state lock failed".to_string())?;
        guard.pending_auth_cookie = cookie;
        Ok(())
    }

    fn take_pending(&self) -> Result<Option<String>, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "auth state lock failed".to_string())?;
        Ok(guard.pending_auth_cookie.take())
    }

    fn has_pending(&self) -> bool {
        self.inner
            .lock()
            .map(|guard| guard.pending_auth_cookie.is_some())
            .unwrap_or(false)
    }

    fn clear_pending(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.pending_auth_cookie = None;
        }
    }

    /// Returns the stored auth cookie from OS-backed keyring.
    pub fn get_saved_cookie(&self) -> Option<String> {
        load_auth_cookie()
    }
}

impl Default for VrchatAuthState {
    fn default() -> Self {
        Self::new()
    }
}

// ─── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VrchatAuthStatusDto {
    pub logged_in: bool,
    pub requires_email_2fa: bool,
    pub display_name: Option<String>,
    pub user_id: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VrchatAuthResultDto {
    pub status: String,
    pub display_name: Option<String>,
    pub user_id: Option<String>,
    pub message: Option<String>,
}

// ─── Internal API types ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AuthUserResponse {
    id: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "requiresTwoFactorAuth", default)]
    requires_two_factor_auth: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct VerifyEmailOtpResponse {
    verified: bool,
}

// ─── Keyring helpers ──────────────────────────────────────────────────────────

fn load_auth_cookie() -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ENTRY).ok()?;
    entry.get_password().ok()
}

fn validate_saved_auth_cookie(expected: &str, saved: Option<&str>) -> Result<(), String> {
    let Some(saved) = saved else {
        return Err("saved auth cookie could not be read back".to_string());
    };
    if saved.is_empty() || saved != expected {
        return Err("saved auth cookie read-back mismatch".to_string());
    }
    Ok(())
}

fn save_and_read_back_auth_cookie(cookie: &str) -> Result<String, String> {
    save_auth_cookie(cookie)?;
    let saved = load_auth_cookie();
    validate_saved_auth_cookie(cookie, saved.as_deref())?;
    saved.ok_or_else(|| "saved auth cookie could not be read back".to_string())
}

fn save_auth_cookie(cookie: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ENTRY)
        .map_err(|e| format!("keyring entry creation failed: {e}"))?;
    entry
        .set_password(cookie)
        .map_err(|e| format!("keyring save failed: {e}"))
}

fn delete_auth_cookie() -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ENTRY)
        .map_err(|e| format!("keyring entry creation failed: {e}"))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("keyring delete failed: {e}")),
    }
}

// ─── HTTP helpers ─────────────────────────────────────────────────────────────

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| format!("HTTP client build failed: {e}"))
}

/// Constructs `Authorization: Basic base64(urlencode(user):urlencode(pass))`.
pub fn build_basic_auth_header(username: &str, password: &str) -> String {
    let encoded_user = urlencoding::encode(username);
    let encoded_pass = urlencoding::encode(password);
    let credentials = format!("{encoded_user}:{encoded_pass}");
    let b64 = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
    format!("Basic {b64}")
}

/// Extracts the `auth` cookie value from response `Set-Cookie` headers.
pub fn extract_auth_cookie(headers: &reqwest::header::HeaderMap) -> Option<String> {
    for value in headers.get_all(reqwest::header::SET_COOKIE) {
        let Ok(cookie_str) = value.to_str() else {
            continue;
        };
        for part in cookie_str.split(';') {
            let part = part.trim();
            if let Some(val) = part.strip_prefix("auth=") {
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

/// Returns true when the 2FA list includes an email OTP variant.
pub fn requires_email_otp(requires: &[String]) -> bool {
    requires
        .iter()
        .any(|v| v.to_ascii_lowercase().contains("emailotp"))
}

/// Returns true when any non-email 2FA method is the sole requirement.
pub fn requires_non_email_only(requires: &[String]) -> bool {
    !requires.is_empty() && !requires_email_otp(requires)
}

// ─── Core HTTP logic ──────────────────────────────────────────────────────────

async fn call_auth_user(
    client: &reqwest::Client,
    auth_header: &str,
) -> Result<
    (
        reqwest::StatusCode,
        reqwest::header::HeaderMap,
        AuthUserResponse,
    ),
    String,
> {
    let response = client
        .get(format!("{VRCHAT_API_BASE}/auth/user"))
        .header(reqwest::header::AUTHORIZATION, auth_header)
        .send()
        .await
        .map_err(|e| format!("VRChat login request failed: {e}"))?;

    let status = response.status();
    let headers = response.headers().clone();
    let body: AuthUserResponse = response
        .json()
        .await
        .map_err(|e| format!("VRChat login response parse failed: {e}"))?;

    Ok((status, headers, body))
}

async fn call_verify_auth(
    client: &reqwest::Client,
    cookie: &str,
) -> Result<AuthUserResponse, String> {
    let response = client
        .get(format!("{VRCHAT_API_BASE}/auth/user"))
        .header(reqwest::header::COOKIE, format!("auth={cookie}"))
        .send()
        .await
        .map_err(|e| format!("VRChat auth verify request failed: {e}"))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("unauthorized".to_string());
    }

    if !response.status().is_success() {
        return Err(format!(
            "VRChat auth verify returned {}",
            response.status().as_u16()
        ));
    }

    response
        .json::<AuthUserResponse>()
        .await
        .map_err(|e| format!("VRChat auth verify parse failed: {e}"))
}

async fn persist_and_verify_login(
    client: &reqwest::Client,
    cookie: &str,
) -> Result<AuthUserResponse, String> {
    let saved_cookie = save_and_read_back_auth_cookie(cookie)?;
    call_verify_auth(client, &saved_cookie).await
}

async fn call_verify_email_otp(
    client: &reqwest::Client,
    auth_cookie: &str,
    code: &str,
) -> Result<VerifyEmailOtpResponse, String> {
    let response = client
        .post(format!(
            "{VRCHAT_API_BASE}/auth/twofactorauth/emailotp/verify"
        ))
        .header(reqwest::header::COOKIE, format!("auth={auth_cookie}"))
        .json(&serde_json::json!({ "code": code }))
        .send()
        .await
        .map_err(|e| format!("VRChat 2FA verify request failed: {e}"))?;

    response
        .json::<VerifyEmailOtpResponse>()
        .await
        .map_err(|e| format!("VRChat 2FA verify response parse failed: {e}"))
}

async fn call_logout(client: &reqwest::Client, cookie: &str) {
    let _ = client
        .put(format!("{VRCHAT_API_BASE}/logout"))
        .header(reqwest::header::COOKIE, format!("auth={cookie}"))
        .send()
        .await;
}

// ─── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_vrchat_auth_status(
    state: State<'_, VrchatAuthState>,
) -> Result<VrchatAuthStatusDto, String> {
    if state.has_pending() {
        return Ok(VrchatAuthStatusDto {
            logged_in: false,
            requires_email_2fa: true,
            display_name: None,
            user_id: None,
            message: Some("メールに届いた6桁のコードを入力してください。".to_string()),
        });
    }

    let Some(cookie) = load_auth_cookie() else {
        return Ok(VrchatAuthStatusDto {
            logged_in: false,
            requires_email_2fa: false,
            display_name: None,
            user_id: None,
            message: None,
        });
    };

    let client = build_client()?;
    match call_verify_auth(&client, &cookie).await {
        Ok(user) => Ok(VrchatAuthStatusDto {
            logged_in: true,
            requires_email_2fa: false,
            display_name: user.display_name,
            user_id: user.id,
            message: None,
        }),
        Err(msg) if msg == "unauthorized" => {
            let _ = delete_auth_cookie();
            Ok(VrchatAuthStatusDto {
                logged_in: false,
                requires_email_2fa: false,
                display_name: None,
                user_id: None,
                message: None,
            })
        }
        Err(msg) => Ok(VrchatAuthStatusDto {
            logged_in: false,
            requires_email_2fa: false,
            display_name: None,
            user_id: None,
            message: Some(format!("認証状態の確認に失敗しました: {msg}")),
        }),
    }
}

#[tauri::command]
pub async fn vrchat_login(
    username: String,
    password: String,
    state: State<'_, VrchatAuthState>,
) -> Result<VrchatAuthResultDto, String> {
    if username.trim().is_empty() {
        return Err("ユーザー名またはメールを入力してください。".to_string());
    }
    if password.is_empty() {
        return Err("パスワードを入力してください。".to_string());
    }

    let auth_header = build_basic_auth_header(&username, &password);
    let client = build_client()?;

    let (status, headers, body) = call_auth_user(&client, &auth_header).await?;

    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Ok(VrchatAuthResultDto {
            status: "logged_out".to_string(),
            display_name: None,
            user_id: None,
            message: Some("ユーザー名またはパスワードが正しくありません。".to_string()),
        });
    }

    if !status.is_success() {
        return Err(format!("VRChat API returned {}", status.as_u16()));
    }

    // 2FA required
    if !body.requires_two_factor_auth.is_empty() {
        if requires_non_email_only(&body.requires_two_factor_auth) {
            return Err(
                "このアプリではemail 2FAのみ対応しています。TOTPや回復コードは使用できません。"
                    .to_string(),
            );
        }
        let auth_cookie = extract_auth_cookie(&headers).ok_or_else(|| {
            "2FA要求レスポンスにauth cookieが含まれていませんでした。".to_string()
        })?;
        state.set_pending(Some(auth_cookie))?;
        return Ok(VrchatAuthResultDto {
            status: "requires_email_2fa".to_string(),
            display_name: None,
            user_id: None,
            message: Some("メールに届いた6桁のコードを入力してください。".to_string()),
        });
    }

    // Logged in
    let auth_cookie = extract_auth_cookie(&headers).ok_or_else(|| {
        "ログイン成功レスポンスにauth cookieが含まれていませんでした。".to_string()
    })?;
    let user = persist_and_verify_login(&client, &auth_cookie).await?;
    state.clear_pending();

    Ok(VrchatAuthResultDto {
        status: "logged_in".to_string(),
        display_name: user.display_name,
        user_id: user.id,
        message: None,
    })
}

#[tauri::command]
pub async fn vrchat_complete_email_2fa(
    code: String,
    state: State<'_, VrchatAuthState>,
) -> Result<VrchatAuthResultDto, String> {
    if code.trim().is_empty() {
        return Err("コードを入力してください。".to_string());
    }

    let pending_cookie = state
        .take_pending()?
        .ok_or_else(|| "先にログインしてください。".to_string())?;

    let client = build_client()?;
    let verify = match call_verify_email_otp(&client, &pending_cookie, code.trim()).await {
        Ok(verify) => verify,
        Err(err) => {
            state.set_pending(Some(pending_cookie))?;
            return Err(err);
        }
    };

    if !verify.verified {
        state.set_pending(Some(pending_cookie))?;
        return Err("認証コードが正しくありません。もう一度お試しください。".to_string());
    }

    match persist_and_verify_login(&client, &pending_cookie).await {
        Ok(user) => Ok(VrchatAuthResultDto {
            status: "logged_in".to_string(),
            display_name: user.display_name,
            user_id: user.id,
            message: None,
        }),
        Err(err) => {
            let _ = delete_auth_cookie();
            state.set_pending(Some(pending_cookie))?;
            Err(err)
        }
    }
}

#[tauri::command]
pub async fn clear_vrchat_login_data(
    state: State<'_, VrchatAuthState>,
) -> Result<VrchatAuthStatusDto, String> {
    let client = build_client().ok();

    if let Some(cookie) = state.take_pending()? {
        if let Some(client) = client.as_ref() {
            call_logout(client, &cookie).await;
        }
    }

    if let Some(cookie) = load_auth_cookie() {
        if let Some(client) = client.as_ref() {
            call_logout(client, &cookie).await;
        }
    }

    delete_auth_cookie()?;
    if load_auth_cookie().is_some() {
        return Err("saved auth cookie still exists after delete".to_string());
    }

    Ok(VrchatAuthStatusDto {
        logged_in: false,
        requires_email_2fa: false,
        display_name: None,
        user_id: None,
        message: None,
    })
}

// ─── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_auth_header_url_encodes_and_base64s() {
        // Plain ASCII credentials
        let header = build_basic_auth_header("user@example.com", "pass");
        assert!(header.starts_with("Basic "));
        let b64 = header.strip_prefix("Basic ").unwrap();
        let decoded = String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .expect("valid base64"),
        )
        .expect("valid utf8");
        // URL-encoded email: @ → %40
        assert_eq!(decoded, "user%40example.com:pass");
    }

    #[test]
    fn basic_auth_header_encodes_special_chars() {
        let header = build_basic_auth_header("user", "p@ss:w/rd");
        let b64 = header.strip_prefix("Basic ").unwrap();
        let decoded = String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .expect("valid base64"),
        )
        .expect("valid utf8");
        assert_eq!(decoded, "user:p%40ss%3Aw%2Frd");
    }

    #[test]
    fn extract_auth_cookie_from_set_cookie_header() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::SET_COOKIE,
            "auth=TOKEN_VALUE; Path=/; HttpOnly"
                .parse()
                .expect("valid header value"),
        );
        assert_eq!(
            extract_auth_cookie(&headers),
            Some("TOKEN_VALUE".to_string())
        );
    }

    #[test]
    fn extract_auth_cookie_from_multiple_set_cookie_headers() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.append(
            reqwest::header::SET_COOKIE,
            "session=abc; Path=/".parse().expect("valid header value"),
        );
        headers.append(
            reqwest::header::SET_COOKIE,
            "auth=TOKEN_VALUE; Path=/; HttpOnly"
                .parse()
                .expect("valid header value"),
        );
        headers.append(
            reqwest::header::SET_COOKIE,
            "other=def; Path=/".parse().expect("valid header value"),
        );

        assert_eq!(
            extract_auth_cookie(&headers),
            Some("TOKEN_VALUE".to_string())
        );
    }

    #[test]
    fn extract_auth_cookie_ignores_other_cookies() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::SET_COOKIE,
            "session=abc; Path=/".parse().expect("valid header value"),
        );
        assert_eq!(extract_auth_cookie(&headers), None);
    }

    #[test]
    fn extract_auth_cookie_empty_value_is_ignored() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::SET_COOKIE,
            "auth=; Path=/".parse().expect("valid header value"),
        );
        assert_eq!(extract_auth_cookie(&headers), None);
    }

    #[test]
    fn requires_email_otp_detects_email_otp() {
        assert!(requires_email_otp(&["emailOtp".to_string()]));
        assert!(requires_email_otp(&["emailotp".to_string()]));
        assert!(requires_email_otp(&[
            "totp".to_string(),
            "emailOtp".to_string()
        ]));
        assert!(!requires_email_otp(&["totp".to_string()]));
        assert!(!requires_email_otp(&[]));
    }

    #[test]
    fn requires_non_email_only_detects_totp_only() {
        assert!(requires_non_email_only(&["totp".to_string()]));
        assert!(!requires_non_email_only(&["emailOtp".to_string()]));
        assert!(!requires_non_email_only(&[]));
        // If both, email is present so it is NOT non-email-only
        assert!(!requires_non_email_only(&[
            "totp".to_string(),
            "emailOtp".to_string()
        ]));
    }

    #[test]
    fn auth_status_dto_does_not_expose_cookie() {
        let dto = VrchatAuthStatusDto {
            logged_in: true,
            requires_email_2fa: false,
            display_name: Some("User".to_string()),
            user_id: Some("usr_xxx".to_string()),
            message: None,
        };
        let json = serde_json::to_string(&dto).expect("should serialize");
        assert!(!json.contains("cookie"));
        assert!(!json.contains("auth="));
    }

    #[test]
    fn auth_result_dto_does_not_expose_cookie() {
        let dto = VrchatAuthResultDto {
            status: "logged_in".to_string(),
            display_name: Some("User".to_string()),
            user_id: Some("usr_xxx".to_string()),
            message: None,
        };
        let json = serde_json::to_string(&dto).expect("should serialize");
        assert!(!json.contains("cookie"));
        assert!(!json.contains("auth="));
    }

    #[test]
    fn saved_auth_cookie_validation_accepts_matching_cookie() {
        assert!(validate_saved_auth_cookie("TOKEN_VALUE", Some("TOKEN_VALUE")).is_ok());
    }

    #[test]
    fn saved_auth_cookie_validation_errors_do_not_expose_cookie() {
        let secret = "SECRET_TOKEN_VALUE";

        let missing_error = validate_saved_auth_cookie(secret, None).unwrap_err();
        assert!(!missing_error.contains(secret));
        assert!(!missing_error.contains("auth="));

        let mismatch_error = validate_saved_auth_cookie(secret, Some("OTHER_TOKEN")).unwrap_err();
        assert!(!mismatch_error.contains(secret));
        assert!(!mismatch_error.contains("OTHER_TOKEN"));
        assert!(!mismatch_error.contains("auth="));
    }

    #[test]
    fn vrchat_auth_state_pending_lifecycle() {
        let state = VrchatAuthState::new();

        assert!(!state.has_pending());
        state.set_pending(Some("cookie1".to_string())).unwrap();
        assert!(state.has_pending());

        let taken = state.take_pending().unwrap();
        assert_eq!(taken, Some("cookie1".to_string()));
        assert!(!state.has_pending());
    }

    #[test]
    fn vrchat_auth_state_clear_pending() {
        let state = VrchatAuthState::new();
        state.set_pending(Some("cookie".to_string())).unwrap();
        state.clear_pending();
        assert!(!state.has_pending());
    }

    #[test]
    fn vrchat_auth_state_pending_is_clear_after_successful_take() {
        let state = VrchatAuthState::new();
        state.set_pending(Some("cookie".to_string())).unwrap();
        let taken = state.take_pending().unwrap();
        assert!(taken.is_some());
        assert!(!state.has_pending());
    }
}

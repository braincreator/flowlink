//! Auth API endpoints - OAuth callbacks, JWT tokens, user info
//!
//! Flow:
//! 1. User clicks "Login via VK/Yandex/GitHub" → redirects to OAuth provider
//! 2. Provider redirects back with ?code=...&state=...
//! 3. Backend exchanges code for access_token → creates/finds account → issues JWT
//! 4. JWT stored in localStorage → user logged in

use axum::{
    extract::{Path, Query, State, Extension},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use flowlink_core::config::RelayConfig;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::AuthState;

// --------------------------------------------------------------------------- //
// Types
// --------------------------------------------------------------------------- //

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub token_type: String,
}

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
enum AuthError {
    #[error("Provider error: {0}")]
    Provider(String),
    #[error("No auth engine configured")]
    NoEngine,
}

// --------------------------------------------------------------------------- //
// Helpers
// --------------------------------------------------------------------------- //

/// Percent-encode a string for URL query params (without form_urlencoded crate)
fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
            out.push(c);
        } else {
            for byte in c.encode_utf8(&mut [0u8; 4]).as_bytes() {
                out.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    out
}

/// PKCE storage: state → code_verifier (in-memory, cleared on use)
static PKCE_STORE: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, String>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Generate PKCE code_verifier and code_challenge (S256)
fn generate_pkce() -> (String, String) {
    use sha2::{Digest, Sha256};
    let code_verifier: String = std::iter::repeat_with(|| {
        let b = rand::random::<u8>();
        match b % 3 {
            0 => char::from(b'A' + (b % 26)),
            1 => char::from(b'a' + (b % 26)),
            _ => char::from(b'0' + (b % 10)),
        }
    })
    .take(64)
    .collect();
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    let code_challenge = base64_encode_url_safe(&hash);
    (code_verifier, code_challenge)
}

/// Base64url encode without padding
fn base64_encode_url_safe(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// Exchange VK ID code for access_token using PKCE (no client_secret needed)
async fn exchange_vk_token(code: &str, config: &RelayConfig, state: &str) -> Result<String, AuthError> {
    let code_verifier = {
        let mut store = PKCE_STORE.lock().unwrap();
        store.remove(state).ok_or_else(|| {
            AuthError::Provider("VK: expired or invalid state (PKCE code_verifier not found)".into())
        })?
    };

    let resp = reqwest::Client::new()
        .post("https://id.vk.ru/oauth2/auth")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", &config.oauth.vk.app_id),
            (
                "redirect_uri",
                &format!("{}/api/auth/vk/callback", config.dashboard_url_or_public()),
            ),
            ("code_verifier", &code_verifier),
        ])
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| AuthError::Provider(e.to_string()))?;

    let json: serde_json::Value = resp.json().await.map_err(|e| AuthError::Provider(e.to_string()))?;

    json["access_token"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| AuthError::Provider(format!("No access_token in VK response: {:?}", json)))
}

async fn exchange_yandex_token(code: &str, config: &RelayConfig) -> Result<String, AuthError> {
    let resp = reqwest::Client::new()
        .post("https://oauth.yandex.ru/token")
        .form(&[
            ("code", code),
            ("client_id", &config.oauth.yandex.client_id),
            ("client_secret", &config.oauth.yandex.client_secret),
            ("grant_type", "authorization_code"),
        ])
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| AuthError::Provider(e.to_string()))?;

    let json: serde_json::Value = resp.json().await.map_err(|e| AuthError::Provider(e.to_string()))?;

    json["access_token"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| AuthError::Provider("No access_token in Yandex response".into()))
}

async fn exchange_github_token(code: &str, config: &RelayConfig) -> Result<String, AuthError> {
    let resp = reqwest::Client::new()
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .json(&json!({
            "client_id": config.oauth.github.client_id,
            "client_secret": config.oauth.github.client_secret,
            "code": code,
        }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| AuthError::Provider(e.to_string()))?;

    let json: serde_json::Value = resp.json().await.map_err(|e| AuthError::Provider(e.to_string()))?;

    json["access_token"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| AuthError::Provider("No access_token in GitHub response".into()))
}

/// Fetch user info from VK ID using access_token (VK ID OAuth 2.1)
async fn fetch_vk_user(access_token: &str) -> Result<(String, String, Option<String>), AuthError> {
    let resp = reqwest::Client::new()
        .get("https://id.vk.ru/oauth2/user_info")
        .header("Authorization", format!("Bearer {}", access_token))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| AuthError::Provider(e.to_string()))?;

    let json: serde_json::Value = resp.json().await.map_err(|e| AuthError::Provider(e.to_string()))?;
    let _user = json["user"]["first_name"]
        .as_str()
        .or_else(|| json["user"]["email"].as_str())
        .ok_or_else(|| AuthError::Provider(format!("No user info in VK ID response: {:?}", json)))?;

    let user_obj = json["user"].as_object().unwrap();
    let vk_id = json["user_id"]
        .as_str()
        .or_else(|| json["client_id"].as_str())
        .unwrap_or("unknown")
        .to_string();
    let first = user_obj.get("first_name").and_then(|v| v.as_str()).unwrap_or("");
    let last = user_obj.get("last_name").and_then(|v| v.as_str()).unwrap_or("");
    let name = format!("{} {}", first, last).trim().to_string();
    let avatar = user_obj
        .get("avatar")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            user_obj
                .get("photo_200")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

    Ok((vk_id, name, avatar))
}

/// Fetch user info from Yandex
async fn fetch_yandex_user(access_token: &str) -> Result<(String, String, Option<String>), AuthError> {
    let resp = reqwest::Client::new()
        .get("https://login.yandex.ru/info")
        .header("Authorization", format!("OAuth {}", access_token))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| AuthError::Provider(e.to_string()))?;

    let json: serde_json::Value = resp.json().await.map_err(|e| AuthError::Provider(e.to_string()))?;

    let yandex_id = json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let name = json
        .get("real_name")
        .or_else(|| json.get("login"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let avatar = json
        .get("default_avatar_id")
        .and_then(|v| v.as_str())
        .map(|id| format!("https://avatars.yandex.net/get-yapic/{}/islands-200", id));

    Ok((yandex_id, name, avatar))
}

/// Fetch user info from GitHub
async fn fetch_github_user(access_token: &str) -> Result<(String, String, Option<String>), AuthError> {
    let resp = reqwest::Client::new()
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "flowlink")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| AuthError::Provider(e.to_string()))?;

    let json: serde_json::Value = resp.json().await.map_err(|e| AuthError::Provider(e.to_string()))?;

    let gh_id = json
        .get("id")
        .and_then(|v| v.as_i64())
        .map(|i| i.to_string())
        .unwrap_or_default();
    let name = json.get("login").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let avatar = json.get("avatar_url").and_then(|v| v.as_str()).map(|s| s.to_string());

    Ok((gh_id, name, avatar))
}

/// Extract JWT from Authorization header
fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

/// Extract token from cookie (fl_access_token) or Authorization header
fn extract_token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(cookie_header) = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
    {
        for cookie in cookie_header.split(';') {
            let c = cookie.trim();
            if let Some(t) = c.strip_prefix("fl_access_token=") {
                let token = t.trim().to_string();
                if !token.is_empty() {
                    return Some(token);
                }
            }
        }
    }
    extract_bearer_token(headers).map(|t| t.to_string())
}

/// Build cookie-based auth response: Set-Cookie headers + redirect to dashboard
fn auth_cookie_redirect(config: &RelayConfig, access_token: &str, refresh_token: &str) -> axum::response::Response {
    use axum::http::header::SET_COOKIE;

    let base = config.dashboard_url_or_public().to_string();
    let is_https = base.starts_with("https://");
    let secure_flag = if is_https { "; Secure" } else { "" };
    let same_site = "; SameSite=Lax";
    let path = "; Path=/";
    let access_cookie = format!(
        "fl_access_token={}; HttpOnly{}{}{}; Max-Age=3600",
        access_token, secure_flag, same_site, path
    );
    let refresh_cookie = format!(
        "fl_refresh_token={}; HttpOnly{}{}{}; Max-Age=2592000",
        refresh_token, secure_flag, same_site, path
    );
    let mut response =
        axum::response::Redirect::temporary(&format!("{}/auth/callback", base)).into_response();
    let resp_headers = response.headers_mut();
    if let Ok(val) = axum::http::HeaderValue::from_str(&access_cookie) {
        resp_headers.insert(SET_COOKIE, val);
    }
    if let Ok(val) = axum::http::HeaderValue::from_str(&refresh_cookie) {
        resp_headers.insert(SET_COOKIE, val);
    }
    response
}

// --------------------------------------------------------------------------- //
// Helpers for user org creation
// --------------------------------------------------------------------------- //

/// Slugify a name for org URL slug
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else if c.is_whitespace() { '-' } else { '\0' })
        .filter(|c| *c != '\0')
        .collect::<String>()
        .split(|c: char| c == '-' && { true })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Auto-create personal org for user if they have none.
/// Returns org_id string if found/created, None on failure.
pub async fn ensure_user_org(pool: &sqlx::PgPool, account_id: &str, name: Option<&str>) -> Option<String> {
    use flowlink_db::orgs::OrgRepo;
    let existing = OrgRepo::list_by_account(pool, account_id).await.unwrap_or_default();
    if existing.is_empty() {
        let org_name = name.unwrap_or("Personal").to_string();
        let base_slug = slugify(&org_name);
        let slug = match OrgRepo::get_by_slug(pool, &base_slug).await {
            Ok(None) => base_slug,
            _ => format!("{}-{}", base_slug, &uuid::Uuid::new_v4().to_string()[..4]),
        };
        match OrgRepo::create(pool, &org_name, &slug, account_id, "trial").await {
            Ok(org) => {
                let trial_ends = chrono::Utc::now() + chrono::Duration::days(7);
                let _ = sqlx::query(
                    "UPDATE organizations SET is_trial = true, trial_ends_at = $2 WHERE org_id = $1",
                )
                .bind(org.org_id)
                .bind(trial_ends)
                .execute(pool)
                .await;
                let _ = OrgRepo::add_member(pool, org.org_id, account_id, "owner", None).await;
                log::info!(
                    "🏢 Auto-created personal org '{}' for user {}",
                    org_name,
                    account_id
                );
                Some(org.org_id.to_string())
            }
            Err(e) => {
                log::error!("Failed to auto-create org for {}: {}", account_id, e);
                None
            }
        }
    } else {
        existing.first().map(|o| o.org_id.to_string())
    }
}

// --------------------------------------------------------------------------- //
// Internal: issue tokens or return 2FA challenge
// --------------------------------------------------------------------------- //

async fn issue_tokens_or_2fa(
    state: &AuthState,
    user_id: &str,
    account_id: &str,
    email: Option<&str>,
    name: Option<&str>,
    avatar_url: Option<&str>,
) -> Option<axum::response::Response> {
    let engine = state.auth_engine.as_ref()?;

    let is_admin = if let Some(ref db) = state.db {
        let admin = flowlink_db::accounts::AccountRepo::get(db.pool(), account_id)
            .await
            .ok()
            .flatten()
            .map(|a| a.is_admin)
            .unwrap_or(false);
        // Check if 2FA is enabled
        if let Ok((totp_enabled, _)) = flowlink_db::accounts::AccountRepo::get_totp(db.pool(), account_id).await {
            if totp_enabled {
                if let Some(response) = crate::two_factor::check_2fa(account_id, email, name, admin) {
                    log::info!("🔐 2FA required for account {account_id}");
                    let config = state.config_reloader.as_ref()?.get_config().await;
                    let base = config.dashboard_url_or_public().to_string();
                    let temp_token = response["temp_token"].as_str().unwrap_or("");
                    let redirect_url = format!("{}/dashboard?requires_2fa=1&temp_token={}", base, temp_token);
                    return Some(Redirect::temporary(&redirect_url).into_response());
                }
            }
        }
        admin
    } else {
        false
    };

    // Auto-create personal org if user has none
    let org_id = if let Some(ref db) = state.db {
        ensure_user_org(db.pool(), account_id, name).await
    } else {
        None
    };

    match engine.create_tokens(user_id, account_id, email, name, avatar_url, is_admin, org_id.as_deref()) {
        Ok(tokens) => {
            let config = state.config_reloader.as_ref()?.get_config().await;
            Some(auth_cookie_redirect(&config, &tokens.access_token, &tokens.refresh_token))
        }
        Err(e) => {
            log::error!("JWT creation failed: {e}");
            Some(
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "token creation failed"})),
                )
                    .into_response(),
            )
        }
    }
}

// --------------------------------------------------------------------------- //
// Route Handlers
// --------------------------------------------------------------------------- //

/// Verify CSRF state returned by OAuth provider
fn verify_state(state_param: &Option<String>, jwt_secret: &str) -> bool {
    let state = match state_param {
        Some(s) => s,
        None => return false,
    };

    let parts: Vec<&str> = state.rsplitn(2, '_').collect();
    if parts.len() != 2 {
        return false;
    }
    let sig_part = parts[0];
    let raw = parts[1];

    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<sha2::Sha256>;
    let mut mac = HmacSha256::new_from_slice(jwt_secret.as_bytes()).expect("HMAC key");
    mac.update(raw.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());

    &signature[..16] == sig_part
}

/// GET /api/auth/oauth-url?provider=...&redirect=...
/// Generates OAuth URL with CSRF state - client never sees client_secret
pub async fn oauth_url(
    Extension(state): Extension<Arc<AuthState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let provider = params.get("provider").map(|s| s.as_str()).unwrap_or("");
    let _redirect = params.get("redirect").map(|s| s.as_str()).unwrap_or("");

    let config = state
        .config_reloader
        .as_ref()
        .expect("config_reloader")
        .get_config()
        .await;
    let callback_base = config.dashboard_url_or_public().to_string();

    // Generate CSRF state: random 32 hex chars, sign with JWT secret
    let raw_state = format!("{:x}", rand::random::<u64>()) + &format!("{:x}", rand::random::<u64>());
    let jwt_secret = if config.auth.jwt_secret.is_empty() {
        log::error!("jwt_secret not set in config - OAuth CSRF protection is insecure!");
        format!("insecure-{}", rand::random::<u64>())
    } else {
        config.auth.jwt_secret.clone()
    };
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<sha2::Sha256>;
    let mut mac = HmacSha256::new_from_slice(jwt_secret.as_bytes()).expect("HMAC key");
    mac.update(raw_state.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());
    let state_param = format!("{}_{}", raw_state, &signature[..16]);

    let url = match provider {
        "vk" => {
            let client_id = &config.oauth.vk.app_id;
            let (code_verifier, code_challenge) = generate_pkce();
            {
                let mut store = PKCE_STORE.lock().unwrap();
                store.insert(state_param.clone(), code_verifier);
                if store.len() > 1000 {
                    let keys: Vec<String> = store.keys().take(store.len() - 500).cloned().collect();
                    for k in keys {
                        store.remove(&k);
                    }
                }
            }
            let callback_url = format!("{}/api/auth/vk/callback", callback_base);
            format!(
                "https://id.vk.ru/authorize?client_id={}&redirect_uri={}&response_type=code&state={}&code_challenge={}&code_challenge_method=S256&scope=email",
                pct_encode(client_id),
                pct_encode(&callback_url),
                pct_encode(&state_param),
                pct_encode(&code_challenge),
            )
        }
        "yandex" => {
            format!(
                "https://oauth.yandex.ru/authorize?client_id={}&response_type=code&redirect_uri={}/api/auth/yandex/callback&state={}",
                pct_encode(&config.oauth.yandex.client_id),
                callback_base,
                pct_encode(&state_param)
            )
        }
        "github" => {
            format!(
                "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}/api/auth/github/callback&state={}",
                pct_encode(&config.oauth.github.client_id),
                callback_base,
                pct_encode(&state_param)
            )
        }
        _ => {
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "unknown provider"}))).into_response();
        }
    };

    info!("Generated OAuth URL for provider={}", provider);
    (StatusCode::OK, Json(json!({"url": url, "state": state_param}))).into_response()
}

pub async fn list_providers(Extension(state): Extension<Arc<AuthState>>) -> impl IntoResponse {
    let config = state
        .config_reloader
        .as_ref()
        .expect("config_reloader")
        .get_config()
        .await;
    let mut providers = vec!["email".to_string()];

    if !config.oauth.vk.app_id.is_empty() && config.oauth.vk.app_id != "mock_vk_app_id" {
        providers.push("vk".to_string());
    }
    if !config.oauth.yandex.client_id.is_empty() && config.oauth.yandex.client_id != "mock_yandex_client_id" {
        providers.push("yandex".to_string());
    }
    if !config.oauth.github.client_id.is_empty() && config.oauth.github.client_id != "mock_github_client_id" {
        providers.push("github".to_string());
    }

    (StatusCode::OK, Json(json!({ "providers": providers })))
}

pub async fn vk_callback(
    Extension(state): Extension<Arc<AuthState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
    let config = state
        .config_reloader
        .as_ref()
        .expect("config_reloader")
        .get_config()
        .await;
    let jwt_secret = &config.auth.jwt_secret;
    if !verify_state(&query.state, jwt_secret) {
        warn!("VK OAuth: invalid CSRF state");
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid state parameter"}))).into_response();
    }
    let config = state
        .config_reloader
        .as_ref()
        .expect("config_reloader")
        .get_config()
        .await;

    let access_token = match exchange_vk_token(&query.code, &config, query.state.as_deref().unwrap_or("")).await {
        Ok(t) => t,
        Err(e) => {
            error!("VK OAuth exchange failed: {}", e);
            return (StatusCode::BAD_GATEWAY, Json(json!({"error": "VK auth failed"}))).into_response();
        }
    };

    let (vk_id, name, avatar) = match fetch_vk_user(&access_token).await {
        Ok(u) => u,
        Err(e) => {
            warn!("VK user fetch failed (non-fatal): {}", e);
            ("unknown".into(), "VK User".into(), None)
        }
    };

    if let Some(ref db) = state.db {
        let pool = db.pool();
        if let Err(e) = flowlink_db::accounts::AccountRepo::get_or_create(pool, &vk_id, "free").await {
            error!("Failed to ensure account for VK user {}: {}", vk_id, e);
        }
    }

    if let Some(response) =
        issue_tokens_or_2fa(&state, &vk_id, &vk_id, None, Some(&name), avatar.as_deref()).await
    {
        info!("VK OAuth success: user={}", name);
        return response;
    }

    info!("VK OAuth success (no AuthEngine, raw token)");
    auth_cookie_redirect(&config, &access_token, "")
}

pub async fn yandex_callback(
    Extension(state): Extension<Arc<AuthState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
    let config = state
        .config_reloader
        .as_ref()
        .expect("config_reloader")
        .get_config()
        .await;
    let jwt_secret = &config.auth.jwt_secret;
    if !verify_state(&query.state, jwt_secret) {
        warn!("Yandex OAuth: invalid CSRF state");
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid state parameter"}))).into_response();
    }
    let access_token = match exchange_yandex_token(&query.code, &config).await {
        Ok(t) => t,
        Err(e) => {
            error!("Yandex OAuth exchange failed: {}", e);
            return (StatusCode::BAD_GATEWAY, Json(json!({"error": "Yandex auth failed"}))).into_response();
        }
    };

    let (yandex_id, name, avatar) = match fetch_yandex_user(&access_token).await {
        Ok(u) => u,
        Err(e) => {
            warn!("Yandex user fetch failed (non-fatal): {}", e);
            ("unknown".into(), "Yandex User".into(), None)
        }
    };

    if let Some(ref db) = state.db {
        let pool = db.pool();
        if let Err(e) = flowlink_db::accounts::AccountRepo::get_or_create(pool, &yandex_id, "free").await {
            error!("Failed to ensure account for Yandex user {}: {}", yandex_id, e);
        }
    }

    if let Some(response) =
        issue_tokens_or_2fa(&state, &yandex_id, &yandex_id, None, Some(&name), avatar.as_deref()).await
    {
        info!("Yandex OAuth success: user={}", name);
        return response;
    }

    info!("Yandex OAuth success (no AuthEngine, raw token)");
    auth_cookie_redirect(&config, &access_token, "")
}

pub async fn github_callback(
    Extension(state): Extension<Arc<AuthState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
    let config = state
        .config_reloader
        .as_ref()
        .expect("config_reloader")
        .get_config()
        .await;
    let jwt_secret = &config.auth.jwt_secret;
    if !verify_state(&query.state, jwt_secret) {
        warn!("GitHub OAuth: invalid CSRF state");
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid state parameter"}))).into_response();
    }

    let access_token = match exchange_github_token(&query.code, &config).await {
        Ok(t) => t,
        Err(e) => {
            error!("GitHub OAuth exchange failed: {}", e);
            return (StatusCode::BAD_GATEWAY, Json(json!({"error": "GitHub auth failed"}))).into_response();
        }
    };

    let (gh_id, name, avatar) = match fetch_github_user(&access_token).await {
        Ok(u) => u,
        Err(e) => {
            warn!("GitHub user fetch failed (non-fatal): {}", e);
            ("unknown".into(), "GitHub User".into(), None)
        }
    };

    if let Some(ref db) = state.db {
        let pool = db.pool();
        if let Err(e) = flowlink_db::accounts::AccountRepo::get_or_create(pool, &gh_id, "free").await {
            error!("Failed to ensure account for GitHub user {}: {}", gh_id, e);
        }
    }

    if let Some(response) =
        issue_tokens_or_2fa(&state, &gh_id, &gh_id, None, Some(&name), avatar.as_deref()).await
    {
        info!("GitHub OAuth success: user={}", name);
        return response;
    }

    info!("GitHub OAuth success (no AuthEngine, raw token)");
    auth_cookie_redirect(&config, &access_token, "")
}

pub async fn refresh_token(
    Extension(state): Extension<Arc<AuthState>>,
    headers: HeaderMap,
    Json(req): Json<RefreshTokenRequest>,
) -> impl IntoResponse {
    let engine = match &state.auth_engine {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "auth not configured"})),
            )
                .into_response()
        }
    };

    let refresh_token = if !req.refresh_token.is_empty() {
        req.refresh_token
    } else if let Some(cookie_header) = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
    {
        cookie_header
            .split(';')
            .find_map(|c| {
                c.trim()
                    .strip_prefix("fl_refresh_token=")
                    .map(|t| t.trim().to_string())
            })
            .unwrap_or_default()
    } else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "missing refresh token"})),
        )
            .into_response();
    };

    if refresh_token.is_empty() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "missing refresh token"})),
        )
            .into_response();
    }

    match engine.validate_refresh_token(&refresh_token) {
        Ok(claims) => {
            engine.blacklist_token(&refresh_token);
            match engine.create_tokens(
                &claims.sub,
                &claims.account_id,
                claims.email.as_deref(),
                claims.name.as_deref(),
                claims.avatar_url.as_deref(),
                claims.is_admin,
                claims.org_id.as_deref(),
            ) {
                Ok(tokens) => {
                    let is_https = true;
                    let secure_flag = if is_https { "; Secure" } else { "" };
                    let access_cookie = format!(
                        "fl_access_token={}; HttpOnly{}; SameSite=Lax; Path=/; Max-Age=3600",
                        tokens.access_token, secure_flag
                    );
                    let refresh_cookie = format!(
                        "fl_refresh_token={}; HttpOnly{}; SameSite=Lax; Path=/; Max-Age=2592000",
                        tokens.refresh_token, secure_flag
                    );
                    let mut response = (
                        StatusCode::OK,
                        Json(json!({
                            "ok": true,
                            "expires_in": tokens.expires_in,
                            "token_type": "cookie"
                        })),
                    )
                        .into_response();
                    let hdrs = response.headers_mut();
                    if let Ok(val) = axum::http::HeaderValue::from_str(&access_cookie) {
                        hdrs.insert(axum::http::header::SET_COOKIE, val);
                    }
                    if let Ok(val) = axum::http::HeaderValue::from_str(&refresh_cookie) {
                        hdrs.insert(axum::http::header::SET_COOKIE, val);
                    }
                    response
                }
                Err(e) => {
                    error!("Token refresh failed: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": "token creation failed"})),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            warn!("Invalid refresh token: {}", e);
            (StatusCode::UNAUTHORIZED, Json(json!({"error": "invalid refresh token"}))).into_response()
        }
    }
}

pub async fn auth_me(Extension(state): Extension<Arc<AuthState>>, headers: HeaderMap) -> impl IntoResponse {
    let token = match extract_token_from_headers(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "missing authorization header"})),
            )
                .into_response()
        }
    };

    let engine = match &state.auth_engine {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "auth not configured"})),
            )
                .into_response()
        }
    };

    match engine.validate_access_token(&token) {
        Ok(mut claims) => {
            if claims.org_id.is_none() {
                if let Some(ref db) = state.db {
                    let pool = db.pool();
                    let orgs =
                        flowlink_db::orgs::OrgRepo::list_by_account(pool, &claims.account_id)
                            .await
                            .unwrap_or_default();
                    if let Some(org) = orgs.first() {
                        claims.org_id = Some(org.org_id.to_string());
                    }
                }
            }
            (
                StatusCode::OK,
                Json(json!({
                    "user": {
                        "id": claims.account_id,
                        "account_id": claims.account_id,
                        "email": claims.email,
                        "name": claims.name,
                        "avatar_url": claims.avatar_url,
                        "sub": claims.sub,
                        "org_id": claims.org_id,
                        "is_admin": claims.is_admin,
                        "exp": claims.exp,
                        "active": true
                    }
                })),
            )
                .into_response()
        }
        Err(e) => {
            warn!("Invalid access token: {}", e);
            (StatusCode::UNAUTHORIZED, Json(json!({"error": "invalid or expired token"}))).into_response()
        }
    }
}

pub async fn logout(
    Extension(state): Extension<Arc<AuthState>>,
    headers: HeaderMap,
    Json(body): Json<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let engine = match &state.auth_engine {
        Some(e) => e,
        None => return (StatusCode::OK, Json(json!({"message": "Logged out"}))).into_response(),
    };

    if let Some(cookie_header) = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
    {
        for cookie in cookie_header.split(';') {
            let c = cookie.trim();
            if let Some(t) = c.strip_prefix("fl_access_token=") {
                engine.blacklist_token(t.trim());
            }
            if let Some(t) = c.strip_prefix("fl_refresh_token=") {
                engine.blacklist_token(t.trim());
            }
        }
    }
    if let Some(token) = extract_bearer_token(&headers) {
        engine.blacklist_token(&token);
    }
    if let Some(refresh) = body.get("refresh_token") {
        engine.blacklist_token(refresh);
    }

    info!("User logged out");
    let mut response = (StatusCode::OK, Json(json!({"message": "Logged out successfully"}))).into_response();
    let hdrs = response.headers_mut();
    hdrs.insert(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_static(
            "fl_access_token=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0",
        ),
    );
    hdrs.insert(
        axum::http::header::SET_COOKIE,
        axum::http::HeaderValue::from_static(
            "fl_refresh_token=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0",
        ),
    );
    response
}

/// DELETE /api/account - soft-delete (deactivate) account
pub async fn delete_account(
    Extension(state): Extension<Arc<AuthState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let token = match extract_token_from_headers(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing auth"})),
            )
                .into_response()
        }
    };

    let engine = match &state.auth_engine {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "Auth not configured"})),
            )
                .into_response()
        }
    };

    let claims = match engine.validate_access_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"})),
            )
                .into_response()
        }
    };

    engine.blacklist_token(&token);

    let db = match &state.db {
        Some(db) => db,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "No database"})),
            )
                .into_response()
        }
    };

    match flowlink_db::accounts::AccountRepo::set_active(db.pool(), &claims.account_id, false).await {
        Ok(()) => {
            info!("Account {} deactivated by user", claims.account_id);
            (StatusCode::OK, Json(json!({"ok": true, "message": "Account deactivated"}))).into_response()
        }
        Err(e) => {
            error!("Failed to deactivate account: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response()
        }
    }
}

/// POST /api/auth/link-email - link or update email for authenticated user
pub async fn link_email(
    Extension(state): Extension<Arc<AuthState>>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let email = match body.get("email").and_then(|v| v.as_str()) {
        Some(e) if e.contains('@') => e.to_string(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Valid email required"})),
            )
                .into_response()
        }
    };

    let engine = match &state.auth_engine {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "Auth not configured"})),
            )
                .into_response()
        }
    };

    let token = match extract_token_from_headers(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing auth"})),
            )
                .into_response()
        }
    };

    let claims = match engine.validate_access_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"})),
            )
                .into_response()
        }
    };

    let db = match &state.db {
        Some(db) => db,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "No database"})),
            )
                .into_response()
        }
    };

    match flowlink_db::accounts::AccountRepo::update_email(db.pool(), &claims.account_id, &email).await {
        Ok(true) => {
            info!("Email linked for account {}: {}", claims.account_id, email);
            (StatusCode::OK, Json(json!({"ok": true, "email": email}))).into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, Json(json!({"error": "Account not found"}))).into_response(),
        Err(e) => {
            error!("Failed to link email: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response()
        }
    }
}

/// GET /api/auth/sessions - list active sessions
pub async fn list_sessions(
    Extension(state): Extension<Arc<AuthState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let engine = match &state.auth_engine {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "Auth not configured"})),
            )
                .into_response()
        }
    };
    let token = match extract_token_from_headers(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing auth"})),
            )
                .into_response()
        }
    };
    let claims = match engine.validate_access_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"})),
            )
                .into_response()
        }
    };
    let sessions = engine.list_sessions(&claims.account_id);
    (StatusCode::OK, Json(json!({ "sessions": sessions }))).into_response()
}

/// DELETE /api/auth/sessions/:id - revoke a specific session
pub async fn revoke_session(
    Extension(state): Extension<Arc<AuthState>>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let engine = match &state.auth_engine {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "Auth not configured"})),
            )
                .into_response()
        }
    };
    let token = match extract_token_from_headers(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing auth"})),
            )
                .into_response()
        }
    };
    let claims = match engine.validate_access_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"})),
            )
                .into_response()
        }
    };
    if engine.revoke_session(&claims.account_id, &session_id) {
        (StatusCode::OK, Json(json!({"ok": true}))).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(json!({"error": "Session not found"}))).into_response()
    }
}

/// DELETE /api/auth/sessions - revoke all other sessions
pub async fn revoke_other_sessions(
    Extension(state): Extension<Arc<AuthState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let engine = match &state.auth_engine {
        Some(e) => e,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "Auth not configured"})),
            )
                .into_response()
        }
    };
    let token = match extract_token_from_headers(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing auth"})),
            )
                .into_response()
        }
    };
    let claims = match engine.validate_access_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"})),
            )
                .into_response()
        }
    };
    let current_id = &token[..token.len().min(16)];
    let revoked = engine.revoke_other_sessions(&claims.account_id, current_id);
    (StatusCode::OK, Json(json!({"ok": true, "revoked": revoked}))).into_response()
}

pub async fn account_info(
    Extension(state): Extension<Arc<AuthState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    auth_me(Extension(state), headers).await
}

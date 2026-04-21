//! Auth API endpoints — OAuth callbacks, JWT tokens, user info
//!
//! Flow:
//! 1. User clicks "Login via VK/Yandex/GitHub" → redirects to OAuth provider
//! 2. Provider redirects back with ?code=...&state=...
//! 3. Backend exchanges code for access_token → creates/finds account → issues JWT
//! 4. JWT stored in localStorage → user logged in

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use crate::server::AppState;
use flowlink_core::config::RelayConfig;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;

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

async fn exchange_vk_token(code: &str, config: &RelayConfig) -> Result<String, AuthError> {
    let resp = reqwest::Client::new()
        .post(format!("{}/oauth/token", config.oauth.vk.oauth_endpoint))
        .form(&[
            ("code", code),
            ("client_id", &config.oauth.vk.app_id),
            ("client_secret", &config.oauth.vk.app_secret),
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
        .ok_or_else(|| AuthError::Provider("No access_token in VK response".into()))
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

/// Fetch user info from VK using access_token
async fn fetch_vk_user(access_token: &str) -> Result<(String, String, Option<String>), AuthError> {
    let resp = reqwest::Client::new()
        .get("https://api.vk.com/method/users.get")
        .query(&[
            ("access_token", access_token),
            ("fields", "photo_200,email"),
            ("v", "5.131"),
        ])
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| AuthError::Provider(e.to_string()))?;

    let json: serde_json::Value = resp.json().await.map_err(|e| AuthError::Provider(e.to_string()))?;
    let user = json["response"][0].as_object().ok_or_else(|| AuthError::Provider("No user in VK response".into()))?;

    let vk_id = user.get("id").and_then(|v| v.as_i64()).map(|i| i.to_string()).unwrap_or_default();
    let name = format!("{} {}",
        user.get("first_name").and_then(|v| v.as_str()).unwrap_or(""),
        user.get("last_name").and_then(|v| v.as_str()).unwrap_or("")
    );
    let avatar = user.get("photo_200").and_then(|v| v.as_str()).map(|s| s.to_string());

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
    let name = json.get("real_name").or_else(|| json.get("login")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let avatar = json.get("default_avatar_id").and_then(|v| v.as_str()).map(|id| format!("https://avatars.yandex.net/get-yapic/{}/islands-200", id));

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

    let gh_id = json.get("id").and_then(|v| v.as_i64()).map(|i| i.to_string()).unwrap_or_default();
    let name = json.get("login").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let avatar = json.get("avatar_url").and_then(|v| v.as_str()).map(|s| s.to_string());

    Ok((gh_id, name, avatar))
}

/// Extract JWT from Authorization header
fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers.get("authorization")?
        .to_str().ok()?
        .strip_prefix("Bearer ")
}

fn dashboard_redirect(config: &RelayConfig, access_token: &str, refresh_token: &str) -> Redirect {
    let base = match config.dashboard_url.as_deref() {
        Some(url) => url.to_string(),
        None => format!("http://{}", config.http_addr),
    };
    Redirect::temporary(&format!(
        "{}/auth/callback?access_token={}&refresh_token={}",
        base, access_token, refresh_token
    ))
}

// --------------------------------------------------------------------------- //
// Route Handlers
// --------------------------------------------------------------------------- //

/// GET /api/auth/oauth-url?provider=...&redirect=...
/// Generates OAuth URL with CSRF state — client never sees client_secret
pub async fn oauth_url(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let provider = params.get("provider").map(|s| s.as_str()).unwrap_or("");
    let _redirect = params.get("redirect").map(|s| s.as_str()).unwrap_or("");

    let config = state.config_reloader.as_ref().expect("config_reloader").get_config().await;
    let callback_base = match config.dashboard_url.as_deref() {
        Some(url) => url.to_string(),
        None => format!("http://{}", config.http_addr),
    };

    // Generate CSRF state: random 32 hex chars, sign with JWT secret
    let raw_state = format!("{:x}", rand::random::<u64>()) + &format!("{:x}", rand::random::<u64>());
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        log::error!("JWT_SECRET not set — OAuth CSRF protection is insecure!");
        format!("insecure-{}", rand::random::<u64>())
    });
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<sha2::Sha256>;
    let mut mac = HmacSha256::new_from_slice(jwt_secret.as_bytes()).expect("HMAC key");
    mac.update(raw_state.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());
    let state_param = format!("{}_{}", raw_state, &signature[..16]); // 16 chars of HMAC is enough

    let url = match provider {
        "vk" => {
            let client_id = &config.oauth.vk.app_id;
            format!(
                "{}/authorize?client_id={}&redirect_uri={}/api/auth/vk/callback&response_type=code&state={}&scope=email",
                config.oauth.vk.oauth_endpoint, pct_encode(client_id),
                callback_base, pct_encode(&state_param)
            )
        }
        "yandex" => {
            format!(
                "https://oauth.yandex.ru/authorize?client_id={}&response_type=code&redirect_uri={}/api/auth/yandex/callback&state={}",
                pct_encode(&config.oauth.yandex.client_id),
                callback_base, pct_encode(&state_param)
            )
        }
        "github" => {
            format!(
                "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}/api/auth/github/callback&state={}",
                pct_encode(&config.oauth.github.client_id),
                callback_base, pct_encode(&state_param)
            )
        }
        _ => {
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "unknown provider"}))).into_response();
        }
    };

    info!("Generated OAuth URL for provider={}", provider);
    (StatusCode::OK, Json(json!({"url": url, "state": state_param}))).into_response()
}

/// Verify CSRF state returned by OAuth provider
fn verify_state(state_param: &Option<String>) -> bool {
    let state = match state_param {
        Some(s) => s,
        None => return false,
    };

    let parts: Vec<&str> = state.rsplitn(2, '_').collect();
    if parts.len() != 2 { return false; }
    let sig_part = parts[0];
    let raw = parts[1];

    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| { log::error!("JWT_SECRET not set — using random fallback. SET JWT_SECRET IN PRODUCTION!"); format!("insecure-{}", rand::random::<u64>()) });
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<sha2::Sha256>;
    let mut mac = HmacSha256::new_from_slice(jwt_secret.as_bytes()).expect("HMAC key");
    mac.update(raw.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());
    
    &signature[..16] == sig_part
}

/// Issue JWT tokens, checking 2FA if enabled.
/// Returns either full tokens or a 2FA required response.
async fn issue_tokens_or_2fa(
    state: &AppState,
    user_id: &str,
    account_id: &str,
    email: Option<&str>,
    name: Option<&str>,
) -> Option<axum::response::Response> {
    let engine = state.auth_engine.as_ref()?;

    // Fetch is_admin and check 2FA in one DB access
    let is_admin = if let Some(ref db) = state.db {
        let admin = flowlink_db::accounts::AccountRepo::get(db.pool(), account_id)
            .await.ok().flatten().map(|a| a.is_admin).unwrap_or(false);
        // Check if 2FA is enabled
        if let Ok((totp_enabled, _)) = flowlink_db::accounts::AccountRepo::get_totp(db.pool(), account_id).await {
            if totp_enabled {
                if let Some(response) = crate::auth_2fa::check_2fa(account_id, email, name, admin) {
                    log::info!("🔐 2FA required for account {account_id}");
                    let config = state.config_reloader.as_ref()?.get_config().await;
                    let base = match config.dashboard_url.as_deref() {
                        Some(url) => url.to_string(),
                        None => format!("http://{}", config.http_addr),
                    };
                    let temp_token = response["temp_token"].as_str().unwrap_or("");
                    let redirect_url = format!(
                        "{}/dashboard?requires_2fa=1&temp_token={}",
                        base, temp_token
                    );
                    return Some(Redirect::temporary(&redirect_url).into_response());
                }
            }
        }
        admin
    } else { false };

    match engine.create_tokens(user_id, account_id, email, name, is_admin, None) {
        Ok(tokens) => {
            let config = state.config_reloader.as_ref()?.get_config().await;
            Some(dashboard_redirect(&config, &tokens.access_token, &tokens.refresh_token).into_response())
        }
        Err(e) => {
            log::error!("JWT creation failed: {e}");
            Some((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "token creation failed"}))).into_response())
        }
    }
}

pub async fn list_providers(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let config = state.config_reloader.as_ref().expect("config_reloader").get_config().await;
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
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
    if !verify_state(&query.state) {
        warn!("VK OAuth: invalid CSRF state");
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid state parameter"}))).into_response();
    }
    let config = state.config_reloader.as_ref().expect("config_reloader").get_config().await;

    let access_token = match exchange_vk_token(&query.code, &config).await {
        Ok(t) => t,
        Err(e) => {
            error!("VK OAuth exchange failed: {}", e);
            return (StatusCode::BAD_GATEWAY, Json(json!({"error": "VK auth failed"}))).into_response();
        }
    };

    // Fetch user profile
    let (vk_id, name, _avatar) = match fetch_vk_user(&access_token).await {
        Ok(u) => u,
        Err(e) => {
            warn!("VK user fetch failed (non-fatal): {}", e);
            ("unknown".into(), "VK User".into(), None)
        }
    };

    // Ensure account exists in DB
    if let Some(ref db) = state.db {
        let pool = db.pool();
        if let Err(e) = flowlink_db::accounts::AccountRepo::get_or_create(pool, &vk_id, "free").await {
            error!("Failed to ensure account for VK user {}: {}", vk_id, e);
        }
    }

    // Issue JWT (with 2FA check)
    if let Some(response) = issue_tokens_or_2fa(&state, &vk_id, &vk_id, None, Some(&name)).await {
        info!("VK OAuth success: user={}", name);
        return response;
    }

    // Fallback: redirect with provider access token
    info!("VK OAuth success (no AuthEngine, raw token)");
    dashboard_redirect(&config, &access_token, "").into_response()
}

pub async fn yandex_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
    if !verify_state(&query.state) {
        warn!("Yandex OAuth: invalid CSRF state");
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid state parameter"}))).into_response();
    }
    let config = state.config_reloader.as_ref().expect("config_reloader").get_config().await;

    let access_token = match exchange_yandex_token(&query.code, &config).await {
        Ok(t) => t,
        Err(e) => {
            error!("Yandex OAuth exchange failed: {}", e);
            return (StatusCode::BAD_GATEWAY, Json(json!({"error": "Yandex auth failed"}))).into_response();
        }
    };

    let (yandex_id, name, _avatar) = match fetch_yandex_user(&access_token).await {
        Ok(u) => u,
        Err(e) => {
            warn!("Yandex user fetch failed (non-fatal): {}", e);
            ("unknown".into(), "Yandex User".into(), None)
        }
    };

    // Ensure account exists in DB
    if let Some(ref db) = state.db {
        let pool = db.pool();
        if let Err(e) = flowlink_db::accounts::AccountRepo::get_or_create(pool, &yandex_id, "free").await {
            error!("Failed to ensure account for Yandex user {}: {}", yandex_id, e);
        }
    }

    if let Some(response) = issue_tokens_or_2fa(&state, &yandex_id, &yandex_id, None, Some(&name)).await {
        info!("Yandex OAuth success: user={}", name);
        return response;
    }

    info!("Yandex OAuth success (no AuthEngine, raw token)");
    dashboard_redirect(&config, &access_token, "").into_response()
}

pub async fn github_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
    if !verify_state(&query.state) {
        warn!("GitHub OAuth: invalid CSRF state");
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid state parameter"}))).into_response();
    }
    let config = state.config_reloader.as_ref().expect("config_reloader").get_config().await;

    let access_token = match exchange_github_token(&query.code, &config).await {
        Ok(t) => t,
        Err(e) => {
            error!("GitHub OAuth exchange failed: {}", e);
            return (StatusCode::BAD_GATEWAY, Json(json!({"error": "GitHub auth failed"}))).into_response();
        }
    };

    let (gh_id, name, _avatar) = match fetch_github_user(&access_token).await {
        Ok(u) => u,
        Err(e) => {
            warn!("GitHub user fetch failed (non-fatal): {}", e);
            ("unknown".into(), "GitHub User".into(), None)
        }
    };

    // Ensure account exists in DB
    if let Some(ref db) = state.db {
        let pool = db.pool();
        if let Err(e) = flowlink_db::accounts::AccountRepo::get_or_create(pool, &gh_id, "free").await {
            error!("Failed to ensure account for GitHub user {}: {}", gh_id, e);
        }
    }

    if let Some(response) = issue_tokens_or_2fa(&state, &gh_id, &gh_id, None, Some(&name)).await {
        info!("GitHub OAuth success: user={}", name);
        return response;
    }

    info!("GitHub OAuth success (no AuthEngine, raw token)");
    dashboard_redirect(&config, &access_token, "").into_response()
}

pub async fn refresh_token(
    State(state): State<AppState>,
    Json(req): Json<RefreshTokenRequest>,
) -> impl IntoResponse {
    let engine = match &state.auth_engine {
        Some(e) => e,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "auth not configured"}))).into_response(),
    };

    match engine.validate_refresh_token(&req.refresh_token) {
        Ok(claims) => {
            match engine.create_tokens(&claims.sub, &claims.account_id, claims.email.as_deref(), claims.name.as_deref(), claims.is_admin, claims.org_id.as_deref()) {
                Ok(tokens) => (StatusCode::OK, Json(json!({
                    "access_token": tokens.access_token,
                    "refresh_token": tokens.refresh_token,
                    "expires_in": tokens.expires_in,
                    "token_type": "Bearer"
                }))).into_response(),
                Err(e) => {
                    error!("Token refresh failed: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "token creation failed"}))).into_response()
                }
            }
        }
        Err(e) => {
            warn!("Invalid refresh token: {}", e);
            (StatusCode::UNAUTHORIZED, Json(json!({"error": "invalid refresh token"}))).into_response()
        }
    }
}

pub async fn auth_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "missing authorization header"}))).into_response(),
    };

    let engine = match &state.auth_engine {
        Some(e) => e,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "auth not configured"}))).into_response(),
    };

    match engine.validate_access_token(token) {
        Ok(claims) => (StatusCode::OK, Json(json!({
            "account_id": claims.account_id,
            "email": claims.email,
            "name": claims.name,
            "sub": claims.sub,
            "exp": claims.exp,
            "active": true
        }))).into_response(),
        Err(e) => {
            warn!("Invalid access token: {}", e);
            (StatusCode::UNAUTHORIZED, Json(json!({"error": "invalid or expired token"}))).into_response()
        }
    }
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let engine = match &state.auth_engine {
        Some(e) => e,
        None => return (StatusCode::OK, Json(json!({"message": "Logged out"}))).into_response(),
    };

    // Blacklist access token
    if let Some(token) = extract_bearer_token(&headers) {
        engine.blacklist_token(token);
    }

    // Blacklist refresh token if provided
    if let Some(refresh) = body.get("refresh_token") {
        engine.blacklist_token(refresh);
    }

    info!("User logged out");
    (StatusCode::OK, Json(json!({"message": "Logged out successfully"}))).into_response()
}

/// DELETE /api/account — soft-delete (deactivate) account
pub async fn delete_account(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Missing auth"}))).into_response(),
    };

    let engine = match &state.auth_engine {
        Some(e) => e,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Auth not configured"}))).into_response(),
    };

    let claims = match engine.validate_access_token(token) {
        Ok(c) => c,
        Err(_) => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid token"}))).into_response(),
    };

    // Blacklist all tokens
    engine.blacklist_token(token);

    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "No database"}))).into_response(),
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

/// POST /api/auth/link-email — link or update email for authenticated user
pub async fn link_email(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let email = match body.get("email").and_then(|v| v.as_str()) {
        Some(e) if e.contains('@') => e.to_string(),
        _ => return (StatusCode::BAD_REQUEST, Json(json!({"error": "Valid email required"}))).into_response(),
    };

    let engine = match &state.auth_engine {
        Some(e) => e,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Auth not configured"}))).into_response(),
    };

    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Missing auth"}))).into_response(),
    };

    let claims = match engine.validate_access_token(token) {
        Ok(c) => c,
        Err(_) => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid token"}))).into_response(),
    };

    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "No database"}))).into_response(),
    };

    match flowlink_db::accounts::AccountRepo::update_email(db.pool(), &claims.account_id, &email).await {
        Ok(true) => {
            info!("Email linked for account {}: {}", claims.account_id, email);
            (StatusCode::OK, Json(json!({"ok": true, "email": email}))).into_response()
        }
        Ok(false) => {
            (StatusCode::NOT_FOUND, Json(json!({"error": "Account not found"}))).into_response()
        }
        Err(e) => {
            error!("Failed to link email: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response()
        }
    }
}

/// GET /api/auth/sessions — list active sessions
pub async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let engine = match &state.auth_engine {
        Some(e) => e,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Auth not configured"}))).into_response(),
    };
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Missing auth"}))).into_response(),
    };
    let claims = match engine.validate_access_token(token) {
        Ok(c) => c,
        Err(_) => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid token"}))).into_response(),
    };
    let sessions = engine.list_sessions(&claims.account_id);
    (StatusCode::OK, Json(json!({ "sessions": sessions }))).into_response()
}

/// DELETE /api/auth/sessions/:id — revoke a specific session
pub async fn revoke_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let engine = match &state.auth_engine {
        Some(e) => e,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Auth not configured"}))).into_response(),
    };
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Missing auth"}))).into_response(),
    };
    let claims = match engine.validate_access_token(token) {
        Ok(c) => c,
        Err(_) => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid token"}))).into_response(),
    };
    if engine.revoke_session(&claims.account_id, &session_id) {
        (StatusCode::OK, Json(json!({"ok": true}))).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(json!({"error": "Session not found"}))).into_response()
    }
}

/// DELETE /api/auth/sessions — revoke all other sessions
pub async fn revoke_other_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let engine = match &state.auth_engine {
        Some(e) => e,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Auth not configured"}))).into_response(),
    };
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Missing auth"}))).into_response(),
    };
    let claims = match engine.validate_access_token(token) {
        Ok(c) => c,
        Err(_) => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid token"}))).into_response(),
    };
    // Use session_id from current token as the one to keep — we don't have it in claims,
    // so use the token hash as identifier
    // Use first 16 chars of token as identifier
    let current_id = &token[..token.len().min(16)];
    let revoked = engine.revoke_other_sessions(&claims.account_id, current_id);
    (StatusCode::OK, Json(json!({"ok": true, "revoked": revoked}))).into_response()
}

pub async fn account_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    auth_me(State(state), headers).await
}

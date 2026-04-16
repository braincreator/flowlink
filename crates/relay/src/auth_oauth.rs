//! Auth API endpoints — OAuth callbacks, JWT tokens, user info
//!
//! Flow:
//! 1. User clicks "Login via VK/Yandex/GitHub" → redirects to OAuth provider
//! 2. Provider redirects back with ?code=...&state=...
//! 3. Backend exchanges code for access_token → creates/finds account → issues JWT
//! 4. JWT stored in localStorage → user logged in

use axum::{
    extract::{Query, State},
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
    let base = &config.http_addr;
    Redirect::temporary(&format!("http://{}/dashboard?access_token={}&refresh_token={}", base, access_token, refresh_token))
}

// --------------------------------------------------------------------------- //
// Route Handlers
// --------------------------------------------------------------------------- //

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

    // Issue JWT
    if let Some(ref engine) = state.auth_engine {
        match engine.create_tokens(&vk_id, &vk_id, None, Some(&name)) {
            Ok(tokens) => {
                info!("VK OAuth success: user={}", name);
                return dashboard_redirect(&config, &tokens.access_token, &tokens.refresh_token).into_response();
            }
            Err(e) => {
                error!("JWT creation failed: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "token creation failed"}))).into_response();
            }
        }
    }

    // Fallback: redirect with provider access token
    info!("VK OAuth success (no AuthEngine, raw token)");
    dashboard_redirect(&config, &access_token, "").into_response()
}

pub async fn yandex_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
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

    if let Some(ref engine) = state.auth_engine {
        match engine.create_tokens(&yandex_id, &yandex_id, None, Some(&name)) {
            Ok(tokens) => {
                info!("Yandex OAuth success: user={}", name);
                return dashboard_redirect(&config, &tokens.access_token, &tokens.refresh_token).into_response();
            }
            Err(e) => {
                error!("JWT creation failed: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "token creation failed"}))).into_response();
            }
        }
    }

    info!("Yandex OAuth success (no AuthEngine, raw token)");
    dashboard_redirect(&config, &access_token, "").into_response()
}

pub async fn github_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
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

    if let Some(ref engine) = state.auth_engine {
        match engine.create_tokens(&gh_id, &gh_id, None, Some(&name)) {
            Ok(tokens) => {
                info!("GitHub OAuth success: user={}", name);
                return dashboard_redirect(&config, &tokens.access_token, &tokens.refresh_token).into_response();
            }
            Err(e) => {
                error!("JWT creation failed: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "token creation failed"}))).into_response();
            }
        }
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
            match engine.create_tokens(&claims.sub, &claims.account_id, claims.email.as_deref(), claims.name.as_deref()) {
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

pub async fn logout() -> impl IntoResponse {
    // JWT is stateless — client just discards the token
    (StatusCode::OK, Json(json!({"message": "Logged out successfully"})))
}

pub async fn account_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    auth_me(State(state), headers).await
}

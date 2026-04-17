//! TOTP 2FA endpoints

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use serde_json::json;

use base64::Engine;

use crate::server::AppState;

// ═══════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════

#[derive(Deserialize)]
pub struct SetupRequest {
    #[allow(dead_code)]
    pub password: Option<String>,
}

#[derive(Deserialize)]
pub struct EnableRequest {
    pub code: String,
}

#[derive(Deserialize)]
pub struct DisableRequest {
    pub code: String,
    #[allow(dead_code)]
    pub password: Option<String>,
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    pub code: String,
}

#[derive(Deserialize)]
pub struct CompleteRequest {
    pub temp_token: String,
    pub code: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct TempClaims {
    sub: String,
    account_id: String,
    email: Option<String>,
    name: Option<String>,
    is_admin: bool,
    exp: usize,
    iat: usize,
    is_2fa_temp: bool,
}

// ═══════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers.get("authorization")?.to_str().ok()?.strip_prefix("Bearer ")
}

fn jwt_secret() -> String {
    std::env::var("JWT_SECRET").unwrap_or_else(|_| "flowlink-dev-secret".to_string())
}

fn create_temp_token(account_id: &str, email: Option<&str>, name: Option<&str>, is_admin: bool) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let claims = TempClaims {
        sub: account_id.to_string(),
        account_id: account_id.to_string(),
        email: email.map(|s| s.to_string()),
        name: name.map(|s| s.to_string()),
        is_admin,
        exp: (now + Duration::minutes(5)).timestamp() as usize,
        iat: now.timestamp() as usize,
        is_2fa_temp: true,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(jwt_secret().as_bytes()))
}

fn validate_temp_token(token: &str) -> Option<TempClaims> {
    let data = jsonwebtoken::decode::<TempClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(jwt_secret().as_bytes()),
        &jsonwebtoken::Validation::default(),
    ).ok()?;
    if data.claims.is_2fa_temp {
        Some(data.claims)
    } else {
        None
    }
}

fn verify_totp_code(secret_b32: &str, code: &str) -> bool {
    let secret_bytes = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(secret_b32) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let totp = match totp_rs::TOTP::new(
        totp_rs::Algorithm::SHA1,
        6,
        1,
        30,
        secret_bytes,
        None,
        String::new(),
    ) {
        Ok(t) => t,
        Err(_) => return false,
    };
    totp.check_current(code).unwrap_or(false)
}

/// Check if account has 2FA enabled, return (requires_2fa, temp_token) if so
pub fn check_2fa(account_id: &str, email: Option<&str>, name: Option<&str>, is_admin: bool) -> Option<serde_json::Value> {
    // This is called from auth endpoints after successful credential verification.
    // It returns None if 2FA is not set up (caller should proceed with full tokens).
    // It returns Some(json) with requires_2fa + temp_token if 2FA IS enabled.
    // Note: actual DB check is done by the caller — this just generates the temp token.
    // The caller checks DB first, then calls this if 2FA is enabled.
    let temp_token = create_temp_token(account_id, email, name, is_admin).ok()?;
    Some(json!({
        "requires_2fa": true,
        "temp_token": temp_token,
    }))
}

// ═══════════════════════════════════════════════
// Endpoints
// ═══════════════════════════════════════════════

/// POST /api/auth/2fa/setup — generate TOTP secret, return secret + otpauth URI
pub async fn setup_2fa(
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

    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "No database"}))).into_response(),
    };

    let pool = db.pool();

    // Generate new TOTP secret
    let secret = totp_rs::Secret::generate_secret();
    let secret_b32 = secret.to_encoded().to_string();

    // Store secret (not enabled yet)
    if let Err(e) = flowlink_db::accounts::AccountRepo::set_totp_secret(pool, &claims.account_id, &secret_b32).await {
        log::error!("Failed to store TOTP secret: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response();
    }

    // Generate otpauth:// URI
    let email = claims.email.as_deref().unwrap_or("flowlink-user");
    let otpauth = format!(
        "otpauth://totp/FlowLink:{}?secret={}&issuer=FlowLink&algorithm=SHA1&digits=6&period=30",
        email, secret_b32
    );

    (StatusCode::OK, Json(json!({
        "secret": secret_b32,
        "otpauth_uri": otpauth,
    }))).into_response()
}

/// POST /api/auth/2fa/enable — verify code and enable 2FA
pub async fn enable_2fa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<EnableRequest>,
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

    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "No database"}))).into_response(),
    };

    let pool = db.pool();

    // Get stored secret
    let (_currently_enabled, secret) = match flowlink_db::accounts::AccountRepo::get_totp(pool, &claims.account_id).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("Failed to get TOTP: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response();
        }
    };

    let secret_b32 = match secret {
        Some(s) => s,
        None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "2FA not set up. Call /api/auth/2fa/setup first."}))).into_response(),
    };

    // Verify the code
    if !verify_totp_code(&secret_b32, &body.code) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid TOTP code"}))).into_response();
    }

    // Enable 2FA
    if let Err(e) = flowlink_db::accounts::AccountRepo::enable_totp(pool, &claims.account_id).await {
        log::error!("Failed to enable TOTP: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response();
    }

    log::info!("2FA enabled for account {}", claims.account_id);
    (StatusCode::OK, Json(json!({"ok": true, "enabled": true}))).into_response()
}

/// POST /api/auth/2fa/disable — disable 2FA
pub async fn disable_2fa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DisableRequest>,
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

    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "No database"}))).into_response(),
    };

    let pool = db.pool();

    // Get stored secret and verify current code
    let (currently_enabled, secret) = match flowlink_db::accounts::AccountRepo::get_totp(pool, &claims.account_id).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("Failed to get TOTP: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response();
        }
    };

    if !currently_enabled {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "2FA is not enabled"}))).into_response();
    }

    let secret_b32 = match secret {
        Some(s) => s,
        None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "No TOTP secret found"}))).into_response(),
    };

    if !verify_totp_code(&secret_b32, &body.code) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid TOTP code"}))).into_response();
    }

    if let Err(e) = flowlink_db::accounts::AccountRepo::disable_totp(pool, &claims.account_id).await {
        log::error!("Failed to disable TOTP: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response();
    }

    log::info!("2FA disabled for account {}", claims.account_id);
    (StatusCode::OK, Json(json!({"ok": true, "enabled": false}))).into_response()
}

/// POST /api/auth/2fa/complete — verify temp_token + TOTP code, return full tokens
pub async fn complete_2fa(
    State(state): State<AppState>,
    Json(body): Json<CompleteRequest>,
) -> impl IntoResponse {
    let temp_claims = match validate_temp_token(&body.temp_token) {
        Some(c) => c,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid or expired temp token"}))).into_response(),
    };

    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "No database"}))).into_response(),
    };

    let pool = db.pool();

    // Get TOTP secret
    let (totp_enabled, secret) = match flowlink_db::accounts::AccountRepo::get_totp(pool, &temp_claims.account_id).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("Failed to get TOTP: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response();
        }
    };

    if !totp_enabled {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "2FA not enabled for this account"}))).into_response();
    }

    let secret_b32 = match secret {
        Some(s) => s,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "No TOTP secret"}))).into_response(),
    };

    if !verify_totp_code(&secret_b32, &body.code) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid TOTP code"}))).into_response();
    }

    // Issue real tokens
    let engine = match &state.auth_engine {
        Some(e) => e,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Auth not configured"}))).into_response(),
    };

    match engine.create_tokens(
        &temp_claims.account_id,
        &temp_claims.account_id,
        temp_claims.email.as_deref(),
        temp_claims.name.as_deref(),
        temp_claims.is_admin,
        None,
    ) {
        Ok(tokens) => {
            // Update last login
            let _ = flowlink_db::accounts::AccountRepo::update_last_login(pool, &temp_claims.account_id).await;
            log::info!("2FA complete for account {}", temp_claims.account_id);
            (StatusCode::OK, Json(json!({
                "access_token": tokens.access_token,
                "refresh_token": tokens.refresh_token,
                "expires_in": tokens.expires_in,
                "token_type": "Bearer",
            }))).into_response()
        }
        Err(e) => {
            log::error!("Token creation failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Token creation failed"}))).into_response()
        }
    }
}

/// GET /api/auth/2fa/status — check 2FA status for current user
pub async fn status_2fa(
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

    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "No database"}))).into_response(),
    };

    let pool = db.pool();

    match flowlink_db::accounts::AccountRepo::get_totp(pool, &claims.account_id).await {
        Ok((enabled, has_secret)) => (StatusCode::OK, Json(json!({
            "enabled": enabled,
            "configured": has_secret.is_some(),
        }))).into_response(),
        Err(e) => {
            log::error!("Failed to get TOTP status: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))).into_response()
        }
    }
}

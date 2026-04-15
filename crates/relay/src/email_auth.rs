//! Email-based (Magic Link / code) authentication endpoints

use axum::{
    extract::State,
    response::IntoResponse,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};

use crate::server::AppState;

// ═══════════════════════════════════════════════
// Request / Response types
// ═══════════════════════════════════════════════

#[derive(Deserialize)]
pub struct SendCodeRequest {
    pub email: String,
}

#[derive(Deserialize)]
pub struct VerifyCodeRequest {
    pub email: String,
    pub code: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Claims {
    sub: String,
    email: String,
    exp: usize,
    iat: usize,
}

// ═══════════════════════════════════════════════
// Endpoints
// ═══════════════════════════════════════════════

/// POST /api/auth/email/send-code
pub async fn send_code(
    State(state): State<AppState>,
    Json(req): Json<SendCodeRequest>,
) -> impl IntoResponse {
    let email = req.email.trim().to_lowercase();

    if email.is_empty() || !email.contains('@') {
        return (StatusCode::BAD_REQUEST, Json(json!({
            "ok": false, "error": "Invalid email address"
        }))).into_response();
    }

    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "ok": false, "error": "Database not available"
        }))).into_response(),
    };

    let pool = db.pool();

    // Rate limit check
    match flowlink_db::email_verification::EmailVerificationRepo::check_rate_limit(
        pool, &email, "auth"
    ).await {
        Ok(true) => {},
        Ok(false) => return (StatusCode::TOO_MANY_REQUESTS, Json(json!({
            "ok": false, "error": "Rate limit: wait 1 minute before requesting a new code"
        }))).into_response(),
        Err(e) => {
            log::warn!("Rate limit check failed: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "ok": false, "error": "Internal error"
            }))).into_response();
        }
    }

    // Passive registration: create account if not exists
    if let Ok(None) = flowlink_db::accounts::AccountRepo::get_by_email(pool, &email).await {
        let account_id = uuid::Uuid::new_v4().to_string();
        if let Err(e) = flowlink_db::accounts::AccountRepo::create_with_email(
            pool, &account_id, "free", &email
        ).await {
            log::warn!("Failed to create account for {email}: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "ok": false, "error": "Failed to create account"
            }))).into_response();
        }
        log::info!("📝 Passive registration: created account {account_id} for {email}");

        if let Some(queue) = state.email_queue.get() {
            if let Err(e) = queue.schedule_welcome_series(&account_id, &email).await {
                log::warn!("Failed to schedule welcome series for {email}: {e}");
            }
        }
    }

    // Generate 6-digit code
    let code = format!("{:06}", rand::random::<u32>() % 1_000_000);
    let expires_at = Utc::now() + Duration::minutes(10);

    if let Err(e) = flowlink_db::email_verification::EmailVerificationRepo::create_code(
        pool, &email, &code, "auth", expires_at
    ).await {
        log::warn!("Failed to save verification code: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "ok": false, "error": "Failed to save code"
        }))).into_response();
    }

    // TODO: send actual email when email service is connected
    log::info!("📧 Dev mode: code for {email}: {code}");
    log::info!("🔑 Verification code sent to {email}");

    (StatusCode::OK, Json(json!({
        "ok": true,
        "message": "Code sent"
    }))).into_response()
}

/// POST /api/auth/email/verify
pub async fn verify_code(
    State(state): State<AppState>,
    Json(req): Json<VerifyCodeRequest>,
) -> impl IntoResponse {
    let email = req.email.trim().to_lowercase();
    let code = req.code.trim();

    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "ok": false, "error": "Database not available"
        }))).into_response(),
    };

    let pool = db.pool();

    // Verify and consume code
    let account_id = match flowlink_db::email_verification::EmailVerificationRepo::verify_and_consume_code(
        pool, &email, code, "auth"
    ).await {
        Ok(Some(id)) => id,
        Ok(None) => return (StatusCode::UNAUTHORIZED, Json(json!({
            "ok": false, "error": "Invalid or expired code"
        }))).into_response(),
        Err(e) => {
            log::warn!("Code verification failed: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "ok": false, "error": "Internal error"
            }))).into_response();
        }
    };

    // Update last login
    if let Err(e) = flowlink_db::accounts::AccountRepo::update_last_login(pool, &account_id).await {
        log::warn!("Failed to update last_login: {e}");
    }

    // Generate JWT token
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "flowlink-dev-secret".to_string());
    let now = Utc::now();
    let claims = Claims {
        sub: account_id.clone(),
        email: email.clone(),
        exp: (now + Duration::days(30)).timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    match encode(&Header::default(), &claims, &EncodingKey::from_secret(jwt_secret.as_bytes())) {
        Ok(token) => {
            log::info!("✅ Email auth success: {email} → {account_id}");
            (StatusCode::OK, Json(json!({
                "token": token,
                "user": {
                    "account_id": account_id,
                    "email": email,
                }
            }))).into_response()
        }
        Err(e) => {
            log::warn!("JWT generation failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "ok": false, "error": "Failed to generate token"
            }))).into_response()
        }
    }
}

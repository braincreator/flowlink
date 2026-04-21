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
use crate::middleware::AccountIdExtractor;
use flowlink_db::audit;

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

fn is_valid_email(email: &str) -> bool {
    if email.len() > 254 || email.len() < 5 { return false; }
    let parts: Vec<&str> = email.rsplitn(2, '@').collect();
    if parts.len() != 2 { return false; }
    let (domain, local) = (parts[0], parts[1]);
    if local.is_empty() || local.len() > 64 || domain.is_empty() { return false; }
    if !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') { return false; }
    // Reject obvious injection chars
    if local.contains(' ') || local.contains('<') || local.contains('>') || local.contains('"') { return false; }
    true
}

/// POST /api/auth/email/send-code
pub async fn send_code(
    State(state): State<AppState>,
    Json(req): Json<SendCodeRequest>,
) -> impl IntoResponse {
    let email = req.email.trim().to_lowercase();

    if !is_valid_email(&email) {
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

    // Send verification email
    let send_result = if let Some(ref email_svc) = state.email_service {
        email_svc.send_verification_code(&email, &code).await
    } else {
        log::info!("📧 Dev mode (no SMTP): code generated for {email}");
        Ok(())
    };

    if let Err(e) = send_result {
        log::warn!("Failed to send verification email to {email}: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "ok": false, "error": "Failed to send email"
        }))).into_response();
    }

    log::info!("🔑 Verification code sent to {email}");

    (StatusCode::OK, Json(json!({
        "ok": true,
        "message": "Code sent"
    }))).into_response()
}

/// POST /api/auth/email/change-start
pub async fn change_email_start(
    State(state): State<AppState>,
    account: AccountIdExtractor,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let new_email = body
        .get("new_email")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_lowercase();

    if new_email.is_empty() || !new_email.contains('@') {
        return (StatusCode::BAD_REQUEST, Json(json!({
            "ok": false, "error": "Некорректный email"
        }))).into_response();
    }

    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "ok": false, "error": "База данных недоступна"
        }))).into_response(),
    };
    let pool = db.pool();
    let account_id = account.0;

    // Check new email isn't already taken by another account
    match flowlink_db::accounts::AccountRepo::get_by_email(pool, &new_email).await {
        Ok(Some(other)) if other.account_id != account_id => {
            return (StatusCode::CONFLICT, Json(json!({
                "ok": false, "error": "Этот email уже используется"
            }))).into_response();
        }
        _ => {}
    }

    // Rate limit
    match flowlink_db::email_verification::EmailVerificationRepo::check_rate_limit(
        pool, &new_email, "email_change"
    ).await {
        Ok(true) => {},
        Ok(false) => return (StatusCode::TOO_MANY_REQUESTS, Json(json!({
            "ok": false, "error": "Подождите минуту перед повторным запросом"
        }))).into_response(),
        Err(e) => {
            log::warn!("Rate limit check failed: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "ok": false, "error": "Внутренняя ошибка"
            }))).into_response();
        }
    }

    // Generate code
    let code = format!("{:06}", rand::random::<u32>() % 1_000_000);
    let expires_at = Utc::now() + Duration::minutes(10);

    if let Err(e) = flowlink_db::email_verification::EmailVerificationRepo::create_code(
        pool, &new_email, &code, "email_change", expires_at
    ).await {
        log::warn!("Failed to save email change code: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "ok": false, "error": "Failed to save code"
        }))).into_response();
    }

    // Store pending_email
    if let Err(e) = sqlx::query("UPDATE accounts SET pending_email = $1 WHERE account_id = $2")
        .bind(&new_email)
        .bind(&account_id)
        .execute(pool)
        .await
    {
        log::warn!("Failed to set pending_email: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "ok": false, "error": "Failed to save"
        }))).into_response();
    }

    // Send verification email
    let send_result = if let Some(ref email_svc) = state.email_service {
        email_svc.send_verification_code(&new_email, &code).await
    } else {
        log::info!("📧 Dev mode (no SMTP): email change code generated for {new_email}");
        Ok(())
    };

    if let Err(e) = send_result {
        log::warn!("Failed to send email change code to {new_email}: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "ok": false, "error": "Failed to send email"
        }))).into_response();
    }

    log::info!("📧 Email change code sent to {new_email} for account {account_id}");
    (StatusCode::OK, Json(json!({
        "ok": true,
        "message": "Код отправлен"
    }))).into_response()
}

/// POST /api/auth/email/change-confirm
pub async fn change_email_confirm(
    State(state): State<AppState>,
    account: AccountIdExtractor,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let email = body.get("email").and_then(|v| v.as_str()).unwrap_or("").trim().to_lowercase();
    let code = body.get("code").and_then(|v| v.as_str()).unwrap_or("").trim();
    let account_id = account.0;

    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "ok": false, "error": "База данных недоступна"
        }))).into_response(),
    };
    let pool = db.pool();

    // Verify and consume code
    match flowlink_db::email_verification::EmailVerificationRepo::verify_and_consume_code(
        pool, &email, code, "email_change"
    ).await {
        Ok(Some(_)) => {},
        Ok(None) => return (StatusCode::UNAUTHORIZED, Json(json!({
            "ok": false, "error": "Неверный или просроченный код"
        }))).into_response(),
        Err(e) => {
            log::warn!("Email change code verification failed: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "ok": false, "error": "Внутренняя ошибка"
            }))).into_response();
        }
    }

    // Update email: set email = pending_email, clear pending_email
    if let Err(e) = sqlx::query("UPDATE accounts SET email = pending_email, pending_email = NULL WHERE account_id = $1 AND pending_email = $2")
        .bind(&account_id)
        .bind(&email)
        .execute(pool)
        .await
    {
        log::warn!("Failed to update email for {account_id}: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "ok": false, "error": "Failed to update email"
        }))).into_response();
    }

    // Issue new tokens with updated email
    if let Some(ref engine) = state.auth_engine {
        let is_admin = flowlink_db::accounts::AccountRepo::get(pool, &account_id)
            .await.ok().flatten().map(|a| a.is_admin).unwrap_or(false);

        match engine.create_tokens(&account_id, &account_id, Some(&email), None, is_admin, None) {
            Ok(tokens) => {
                log::info!("✅ Email changed for {account_id} → {email}");
                let _ = audit::log_event(pool, None, &account_id, "auth.email_changed", Some("account"), Some(&account_id), json!({"new_email": &email}), None).await;
                return (StatusCode::OK, Json(json!({
                    "ok": true,
                    "access_token": tokens.access_token,
                    "refresh_token": tokens.refresh_token,
                    "expires_in": tokens.expires_in,
                    "token_type": "Bearer",
                    "user": {
                        "account_id": account_id,
                        "email": email,
                    }
                }))).into_response();
            }
            Err(e) => {
                log::warn!("JWT generation failed: {e}");
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                    "ok": false, "error": "Failed to generate token"
                }))).into_response();
            }
        }
    }

    log::info!("✅ Email changed for {account_id} → {email}");
    (StatusCode::OK, Json(json!({
        "ok": true,
        "message": "Email изменён"
    }))).into_response()
}

/// POST /api/auth/email/verify
pub async fn verify_code(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<VerifyCodeRequest>,
) -> impl IntoResponse {
    let email = req.email.trim().to_lowercase();
    let code = req.code.trim();

    let client_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim())
        .unwrap_or("unknown")
        .to_string();
    let _client_ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "ok": false, "error": "Database not available"
        }))).into_response(),
    };

    let pool = db.pool();

    // Verify and consume code
    // Rate limit: max 10 attempts per email
    let attempt_key = format!("verify_attempts:{email}");
    let attempts: i32 = sqlx::query_scalar(
        "SELECT COALESCE(attempt_count, 0) FROM email_verification WHERE email = $1 AND purpose = 'auth' ORDER BY created_at DESC LIMIT 1"
    ).bind(&email)
    .fetch_optional(pool).await.unwrap_or(None).unwrap_or(0);
    if attempts >= 10 {
        return (StatusCode::TOO_MANY_REQUESTS, Json(json!({
            "ok": false, "error": "Too many attempts. Request a new code."
        }))).into_response();
    }

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

    // Use AuthEngine for proper token pair (access 15min + refresh 30d)
    if let Some(ref engine) = state.auth_engine {
        // Fetch account info (is_admin) and check 2FA
        let is_admin = if let Some(ref db) = state.db {
            let admin = flowlink_db::accounts::AccountRepo::get(db.pool(), &account_id)
                .await.ok().flatten().map(|a| a.is_admin).unwrap_or(false);
            // Check if 2FA is enabled
            if let Ok((totp_enabled, _)) = flowlink_db::accounts::AccountRepo::get_totp(db.pool(), &account_id).await {
                if totp_enabled {
                    if let Some(response) = crate::auth_2fa::check_2fa(&account_id, Some(&email), None, admin) {
                        log::info!("🔐 2FA required for {email} → {account_id}");
                        return (StatusCode::OK, Json(response)).into_response();
                    }
                }
            }
            admin
        } else { false };

        match engine.create_tokens(&account_id, &account_id, Some(&email), None, is_admin, None) {
            Ok(tokens) => {
                log::info!("✅ Email auth success: {email} → {account_id}");
                let _ = audit::log_event(pool, None, &account_id, "auth.login", Some("account"), Some(&account_id), json!({"email": &email, "ip": &client_ip}), Some(&client_ip)).await;

                // Schedule login notification email (skip if sent in last 5 min)
                if let Some(ref queue) = state.email_queue.get() {
                    let recent = sqlx::query_scalar::<_, i64>(
                        r#"SELECT COUNT(*) FROM email_queue
                           WHERE account_id = $1 AND email_type = 'new_login'
                           AND created_at > NOW() - INTERVAL '5 minutes'"#,
                    )
                    .bind(&account_id)
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0);

                    if recent == 0 {
                        let mut vars = std::collections::HashMap::new();
                        vars.insert("name".into(), email.split('@').next().unwrap_or(&email).into());
                        vars.insert("ip".into(), client_ip.clone());
                        vars.insert("time".into(), chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string());
                        if let Err(e) = queue.schedule(
                            &account_id,
                            crate::email_queue::EmailType::NewLogin,
                            &email,
                            chrono::Utc::now(),
                            vars,
                        ).await {
                            log::warn!("Failed to schedule login notification: {e}");
                        }
                    }
                }

                return (StatusCode::OK, Json(json!({
                    "access_token": tokens.access_token,
                    "refresh_token": tokens.refresh_token,
                    "expires_in": tokens.expires_in,
                    "token_type": "Bearer",
                    "user": {
                        "account_id": account_id,
                        "email": email,
                    }
                }))).into_response();
            }
            Err(e) => {
                log::warn!("JWT generation failed: {e}");
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                    "ok": false, "error": "Failed to generate token"
                }))).into_response();
            }
        }
    }

    // Fallback: raw JWT without AuthEngine (dev mode)
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| { log::error!("JWT_SECRET not set — using random fallback. SET JWT_SECRET IN PRODUCTION!"); format!("insecure-{}", rand::random::<u64>()) });
    let now = Utc::now();
    let claims = Claims {
        sub: account_id.clone(),
        email: email.clone(),
        exp: (now + Duration::days(30)).timestamp() as usize,
        iat: now.timestamp() as usize,
    };
    match encode(&Header::default(), &claims, &EncodingKey::from_secret(jwt_secret.as_bytes())) {
        Ok(token) => {
            log::info!("✅ Email auth success (dev mode): {email} → {account_id}");
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

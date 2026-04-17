// Auth rate-limit middleware — wraps axum routes to enforce IP-based brute-force protection.

use axum::{
    body::Body,
    extract::Request,
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};

use crate::auth_rate_limiter;
use crate::server::AppState;

/// Apply rate limits for email auth endpoints (send-code, verify-code).
/// Limits: 5 attempts per email/5min, 10 attempts per IP/5min.
pub async fn email_auth_rate_limit(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let ip = auth_rate_limiter::extract_client_ip(&req);

    let (parts, body) = req.into_parts();
    let bytes = match axum::body::to_bytes(body, 64 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid_request"})),
            )
                .into_response();
        }
    };

    let email_key = match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(v) => v
            .get("email")
            .and_then(|e| e.as_str())
            .map(|e| format!("auth_email:{e}"))
            .unwrap_or_default(),
        Err(_) => String::new(),
    };

    let rl = &state.auth_rate_limiter;
    let mut worst_retry = 0u64;

    if let Err(secs) = rl.check(&format!("auth_ip:{ip}"), 10, 300) {
        worst_retry = worst_retry.max(secs);
    }
    if !email_key.is_empty() {
        if let Err(secs) = rl.check(&email_key, 5, 300) {
            worst_retry = worst_retry.max(secs);
        }
    }

    if worst_retry > 0 {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, worst_retry.to_string())],
            Json(serde_json::json!({
                "error": "too_many_attempts",
                "retry_after": worst_retry
            })),
        )
            .into_response();
    }

    let req = Request::from_parts(parts, Body::from(bytes));
    next.run(req).await
}

/// Apply rate limits for change-email endpoint.
/// Limits: 10 attempts per IP per hour.
pub async fn change_email_rate_limit(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let ip = auth_rate_limiter::extract_client_ip(&req);
    let rl = &state.auth_rate_limiter;

    if let Err(secs) = rl.check(&format!("change_email:ip:{ip}"), 10, 3600) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, secs.to_string())],
            Json(serde_json::json!({
                "error": "too_many_attempts",
                "retry_after": secs
            })),
        )
            .into_response();
    }

    next.run(req).await
}

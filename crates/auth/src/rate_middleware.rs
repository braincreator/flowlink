// Auth rate-limit middleware — wraps axum routes with tiered, plan-aware brute-force protection.
// Uses the TieredRateLimitProvider trait from AuthState for plan-based limits.

use axum::{
    extract::Extension,
    body::Body,
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use std::sync::Arc;

use crate::{AuthState, RateLimitCategory, RateLimitTier, FREE_TIER, STARTER_TIER};

/// Resolve rate limit tier from request parts headers.
fn resolve_tier_from_parts(parts: &axum::http::request::Parts) -> &'static RateLimitTier {
    if parts.headers.get(header::AUTHORIZATION).is_some() {
        &STARTER_TIER
    } else {
        &FREE_TIER
    }
}

/// Extract client IP from request parts.
fn extract_client_ip_from_parts(parts: &axum::http::request::Parts) -> String {
    if let Some(forwarded) = parts.headers.get("forwarded") {
        if let Ok(val) = forwarded.to_str() {
            for part in val.split(';') {
                let part = part.trim();
                if let Some(ip) = part.strip_prefix("for=") {
                    let ip = ip.trim();
                    let ip = ip.trim_matches('"').split(':').next().unwrap_or(ip);
                    return ip.to_string();
                }
            }
        }
    }
    if let Some(xff) = parts.headers.get("x-forwarded-for") {
        if let Ok(val) = xff.to_str() {
            if let Some(ip) = val.split(',').next() {
                return ip.trim().to_string();
            }
        }
    }
    if let Some(real_ip) = parts.headers.get("x-real-ip") {
        if let Ok(val) = real_ip.to_str() {
            return val.trim().to_string();
        }
    }
    "unknown".to_string()
}

/// Check if request is from internal services (bypasses rate limiting).
fn is_internal_request(req: &Request) -> bool {
    if let Some(val) = req.headers().get("x-internal") {
        if val.to_str().map(|v| v == "true").unwrap_or(false) {
            return true;
        }
    }
    if let Some(val) = req.headers().get("x-forwarded-for") {
        if let Ok(v) = val.to_str() {
            if v.split(',').any(|ip| ip.trim().starts_with("10.") || ip.trim().starts_with("172.") || ip.trim() == "127.0.0.1") {
                return true;
            }
        }
    }
    false
}

/// Extract client IP from headers (works with &Headers reference).
fn extract_client_ip_from_headers(headers: &axum::http::HeaderMap) -> String {
    if let Some(forwarded) = headers.get("forwarded") {
        if let Ok(val) = forwarded.to_str() {
            for part in val.split(';') {
                let part = part.trim();
                if let Some(ip) = part.strip_prefix("for=") {
                    let ip = ip.trim();
                    let ip = ip.trim_matches('"').split(':').next().unwrap_or(ip);
                    return ip.to_string();
                }
            }
        }
    }
    if let Some(xff) = headers.get("x-forwarded-for") {
        if let Ok(val) = xff.to_str() {
            if let Some(ip) = val.split(',').next() {
                return ip.trim().to_string();
            }
        }
    }
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(val) = real_ip.to_str() {
            return val.trim().to_string();
        }
    }
    "unknown".to_string()
}

/// Apply rate limits for email auth endpoints (send-code, verify-code).
/// Uses plan-based limits when JWT present, FREE tier for anonymous.
/// Internal requests are bypassed.
pub async fn email_auth_rate_limit(
    Extension(state): Extension<Arc<AuthState>>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    if is_internal_request(&req) {
        return next.run(req).await;
    }

    let rl = match &state.tiered_rate_limiter {
        Some(rl) => rl,
        None => return next.run(req).await,
    };

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

    let email = match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(v) => v
            .get("email")
            .and_then(|e| e.as_str())
            .unwrap_or("")
            .to_string(),
        Err(_) => String::new(),
    };

    let tier = resolve_tier_from_parts(&parts);
    let ip = extract_client_ip_from_parts(&parts);

    let mut worst_retry = 0u64;

    // IP-based check (auth category)
    if let Err(secs) = rl.check_tiered(&ip, RateLimitCategory::AuthLogin, tier) {
        worst_retry = worst_retry.max(secs);
    }

    // Email-based check
    if !email.is_empty() {
        if let Err(secs) = rl.check_tiered(&email, RateLimitCategory::AuthLogin, tier) {
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
/// Uses plan-based limits; internal requests bypassed.
pub async fn change_email_rate_limit(
    Extension(state): Extension<Arc<AuthState>>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    if is_internal_request(&req) {
        return next.run(req).await;
    }

    let rl = match &state.tiered_rate_limiter {
        Some(rl) => rl,
        None => return next.run(req).await,
    };

    let ip = extract_client_ip_from_headers(req.headers());

    let tier = if req.headers().get(header::AUTHORIZATION).is_some() {
        &STARTER_TIER
    } else {
        &FREE_TIER
    };

    if let Err(secs) = rl.check_tiered(&ip, RateLimitCategory::EmailChange, tier) {
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

// Middleware — auth, rate limiting, request ID, CORS, logging
// Port of internal/relay/middleware.go

use axum::{
    body::Body,
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use log::{info, warn};
use std::sync::Arc;
use std::time::Instant;

use crate::auth::AuthManager;
use crate::ratelimit::RateLimiter;

// ── Extensions ──

#[derive(Clone)]
pub struct RequestId(pub String);

#[derive(Clone)]
pub struct ClientId(pub String);

// ── Auth Middleware ──

pub fn auth_middleware(
    auth: Arc<AuthManager>,
    static_token: Option<String>,
    skip_paths: Vec<String>,
) -> impl Fn(Request, Next) -> Response + Clone {
    move |req: Request, next: Next| {
        let auth = auth.clone();
        let static_token = static_token.clone();
        let skip_paths = skip_paths.clone();

        let path = req.uri().path().to_string();

        // Skip configured paths
        if skip_paths.iter().any(|p| path == *p || path.starts_with(&format!("{}/", p))) {
            return next.run(req);
        }

        // Dev mode: no auth configured
        if auth.is_empty() && static_token.is_none() {
            warn!("AUTH_DISABLED: no token or auth configured (dev mode)");
            return next.run(req);
        }

        let auth_header = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());

        let token = match auth_header {
            None => {
                return json_error(StatusCode::UNAUTHORIZED, "token_missing", "Missing Authorization header");
            }
            Some(h) => h.strip_prefix("Bearer ").unwrap_or(h),
        };

        // Try AuthManager
        if let Some(client) = auth.validate_token(token) {
            if client.active {
                let mut req = req;
                req.extensions_mut().insert(ClientId(client.client_id));
                return next.run(req);
            }
        }

        // Try static token (constant-time compare via subtle would be ideal, but this is fine for now)
        if let Some(ref st) = static_token {
            if subtle_eq(token.as_bytes(), st.as_bytes()) {
                let mut req = req;
                req.extensions_mut().insert(ClientId("static-client".into()));
                return next.run(req);
            }
        }

        json_error(StatusCode::UNAUTHORIZED, "token_invalid", "Invalid token")
    }
}

fn subtle_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0, |acc, (x, y)| acc | x ^ y) == 0
}

// ── Rate Limit Middleware ──

pub fn rate_limit_middleware(
    limiter: Arc<RateLimiter>,
) -> impl Fn(Request, Next) -> Response + Clone {
    move |req: Request, next: Next| {
        // Use client_id if available, otherwise fall back to IP
        let key = req
            .extensions()
            .get::<ClientId>()
            .map(|c| format!("client:{}", c.0))
            .unwrap_or_else(|| {
                let ip = req
                    .headers()
                    .get("x-forwarded-for")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.split(',').next())
                    .unwrap_or("unknown");
                format!("ip:{}", ip)
            });

        if !limiter.allow(&key) {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, "60")],
                axum::Json(serde_json::json!({
                    "error": "rate limit exceeded",
                    "code": "rate_limit_exceeded"
                })),
            )
                .into_response();
        }

        next.run(req)
    }
}

// ── Request ID Middleware ──

pub fn request_id_middleware() -> impl Fn(Request, Next) -> Response + Clone {
    |req: Request, next: Next| {
        let request_id = uuid::Uuid::new_v4().to_string();
        req.extensions_mut().insert(RequestId(request_id.clone()));

        let mut response = next.run(req);
        response.headers_mut().insert("x-request-id", request_id.parse().unwrap());
        response
    }
}

// ── CORS Middleware ──

pub fn cors_layer(allowed_origins: Vec<String>) -> tower_http::cors::CorsLayer {
    use tower_http::cors::{Any, AllowOrigin};

    if allowed_origins.is_empty() || allowed_origins.iter().any(|o| o == "*") {
        return tower_http::cors::CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
            .allow_credentials(false);
    }

    let origins: Vec<axum::http::HeaderValue> = allowed_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();

    tower_http::cors::CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            axum::http::header::HeaderName::from_static("x-client-id"),
        ])
        .allow_credentials(false)
}

// ── Logging Middleware ──

pub fn logging_middleware() -> impl Fn(Request, Next) -> Response + Clone {
    |req: Request, next: Next| {
        let method = req.method().clone();
        let path = req.uri().path().to_string();
        let client_id = req.extensions().get::<ClientId>().map(|c| c.0.clone()).unwrap_or_default();

        let start = Instant::now();
        let response = next.run(req);
        let duration = start.elapsed();

        // Skip noisy paths
        if path != "/health" && path != "/metrics" && path != "/api/events" {
            let status = response.status().as_u16();
            if status >= 400 {
                warn!(
                    "HTTP {} {} {} {}ms client={}",
                    method, path, status, duration.as_millis(), client_id
                );
            } else {
                info!(
                    "HTTP {} {} {} {}ms client={}",
                    method, path, status, duration.as_millis(), client_id
                );
            }
        }

        response
    }
}

// ── Helper ──

fn json_error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        axum::Json(serde_json::json!({
            "error": message,
            "code": code
        })),
    )
        .into_response()
}

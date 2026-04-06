// Middleware — auth, rate limiting, request ID, CORS, logging
// Port of internal/relay/middleware.go

use axum::{
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

// ── Auth Middleware (simple, dev-mode passthrough) ──

pub async fn auth_middleware_simple(req: Request, next: Next) -> Response {
    next.run(req).await
}

/// Build a full auth middleware layer.
pub fn auth_layer(
    auth: Arc<AuthManager>,
    static_token: Option<String>,
    skip_paths: Vec<String>,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> + Clone {
    move |req: Request, next: Next| {
        let auth = auth.clone();
        let static_token = static_token.clone();
        let skip_paths = skip_paths.clone();
        Box::pin(async move {
            let path = req.uri().path().to_string();

            if skip_paths.iter().any(|p| path == *p || path.starts_with(&format!("{}/", p))) {
                return next.run(req).await;
            }

            if auth.is_empty() && static_token.is_none() {
                warn!("AUTH_DISABLED: no token or auth configured (dev mode)");
                return next.run(req).await;
            }

            let auth_header = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());

            let token = match auth_header {
                None => {
                    return json_error(StatusCode::UNAUTHORIZED, "token_missing", "Missing Authorization header");
                }
                Some(h) => h.strip_prefix("Bearer ").unwrap_or(h),
            };

            if let Some(client) = auth.validate_token(token) {
                if client.active {
                    let mut req = req;
                    req.extensions_mut().insert(ClientId(client.client_id));
                    return next.run(req).await;
                }
            }

            if let Some(ref st) = static_token {
                if subtle_eq(token.as_bytes(), st.as_bytes()) {
                    let mut req = req;
                    req.extensions_mut().insert(ClientId("static-client".into()));
                    return next.run(req).await;
                }
            }

            json_error(StatusCode::UNAUTHORIZED, "token_invalid", "Invalid token")
        })
    }
}

fn subtle_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | x ^ y) == 0
}

// ── Rate Limit Middleware ──

pub async fn rate_limit_middleware(
    req: Request,
    next: Next,
) -> Response {
    // Simple per-IP rate limit (stateless — in production use shared state)
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

    // Use a thread-local limiter for now — production should inject via state
    use std::sync::LazyLock;
    static LIMITER: LazyLock<RateLimiter> = LazyLock::new(|| RateLimiter::new(60, 60));

    if !LIMITER.allow(&key) {
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

    next.run(req).await
}

// ── Request ID Middleware ──

pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    let request_id = uuid::Uuid::new_v4().to_string();
    req.extensions_mut().insert(RequestId(request_id.clone()));

    let mut response = next.run(req).await;
    response.headers_mut().insert("x-request-id", request_id.parse().unwrap());
    response
}

// ── CORS Layer ──

pub fn cors_layer(allowed_origins: Vec<String>) -> tower_http::cors::CorsLayer {
    use tower_http::cors::Any;

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

    tower_http::cors::CorsLayer::permissive()
}

// ── Logging Middleware ──

pub async fn logging_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let client_id = req.extensions().get::<ClientId>().map(|c| c.0.clone()).unwrap_or_default();

    let start = Instant::now();
    let response = next.run(req).await;
    let duration = start.elapsed();
    let status = response.status().as_u16();

    if path != "/health" && path != "/metrics" && path != "/api/events" {
        if status >= 400 {
            warn!("HTTP {} {} {} {}ms client={}", method, path, status, duration.as_millis(), client_id);
        } else {
            info!("HTTP {} {} {} {}ms client={}", method, path, status, duration.as_millis(), client_id);
        }
    }

    response
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

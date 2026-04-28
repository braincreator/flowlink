// Middleware — auth, rate limiting, request ID, CORS, logging
// Port of internal/relay/middleware.go

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use log::{info, warn};
use std::sync::Arc;
use std::time::Instant;

use crate::auth::AuthManager;
use crate::server::AppState;

/// Extract JWT token from request: cookie (fl_access_token) or Authorization: Bearer header
fn extract_token_from_request(req: &Request) -> Option<String> {
    // 1. Try cookie first (httpOnly, most secure)
    if let Some(cookie_header) = req.headers().get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        for cookie in cookie_header.split(';') {
            let cookie = cookie.trim();
            if cookie.starts_with("fl_access_token=") {
                let token = cookie.strip_prefix("fl_access_token=")?.trim().to_string();
                if !token.is_empty() {
                    return Some(token);
                }
            }
        }
    }
    // 2. Fallback: Authorization: Bearer header (for API keys, programmatic access, mobile apps)
    if let Some(auth_header) = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            let token = token.trim().to_string();
            if !token.is_empty() {
                return Some(token);
            }
        }
    }
    None
}
use crate::ratelimit::RateLimiter;
use crate::rbac_manager::RbacManager;

// ── Extensions ──

#[derive(Clone)]
pub struct RequestId(pub String);

#[derive(Clone)]
pub struct ClientId(pub String);

#[derive(Clone)]
pub struct UserRoles(pub Vec<flowlink_core::rbac::Role>);

#[derive(Clone)]
pub struct AccountId(pub String);

/// Extract AccountId from request extensions.
/// Set by auth middleware (from JWT or ClientId fallback).
#[derive(Clone)]
pub struct AccountIdExtractor(pub String);

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for AccountIdExtractor {
    type Rejection = axum::http::StatusCode;

    fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let account = parts.extensions.get::<AccountId>().map(|a| a.0.clone())
            .or_else(|| parts.extensions.get::<ClientId>().map(|c| c.0.clone()))
            .unwrap_or_else(|| "default".to_string());
        std::future::ready(Ok(AccountIdExtractor(account)))
    }
}

/// Extractor for JWT Claims from request extensions.
/// Returns `None`-equivalent Claims (all Optional fields None) if no JWT was present.
pub struct ClaimsExtractor(pub crate::auth::Claims);

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for ClaimsExtractor {
    type Rejection = axum::http::StatusCode;

    fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        let claims = parts.extensions.get::<crate::auth::Claims>().cloned()
            .unwrap_or_else(|| crate::auth::Claims {
                sub: "".into(), account_id: "".into(),
                email: None, name: None, avatar_url: None,
                is_admin: false, org_id: None,
                iat: 0, exp: 0,
            });
        std::future::ready(Ok(ClaimsExtractor(claims)))
    }
}

// ── JWT Auth Middleware (validates Bearer token via AuthEngine) ──

/// JWT auth middleware — extracts account_id from Bearer token using AuthEngine.
/// Sets `AccountId(String)` extension on the request for handlers to use.
/// If AuthEngine is not configured (no jwt_secret), passes through (dev mode).
pub async fn jwt_auth(
    State(state): State<std::sync::Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    // Dev mode: no AuthEngine configured
    let auth_engine = match &state.auth_engine {
        Some(e) => e,
        None => {
            // Dev mode: no JWT auth, but still check API keys
            let auth_header = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
            if let Some(h) = auth_header {
                if let Some(token) = h.strip_prefix("Bearer ") {
                    if token.starts_with("flk_") {
                        if let Some(ref db_opt) = state.db {
                            let pool = db_opt.pool();
                            if let Ok(Some(identity)) = crate::api_keys::ApiKeyRepo::validate(pool, token).await {
                                let rate_key = identity.key_id.to_string();
                                let is_admin = matches!(identity.role, crate::api_keys::ApiKeyRole::Admin);
                                let claims = crate::auth::Claims {
                                    sub: identity.account_id.clone(),
                                    account_id: identity.account_id.clone(),
                                    email: None,
                                    name: Some(identity.name.clone()),
                                    avatar_url: None,
                                    is_admin,
                                    org_id: Some(identity.org_id.to_string()),
                                    iat: 0,
                                    exp: 0,
                                };
                                req.extensions_mut().insert(AccountId(identity.account_id.clone()));
                                let account_id = identity.account_id.clone();
                                req.extensions_mut().insert(claims.clone());
                                req.extensions_mut().insert(identity);
                                // Resolve plan for billing enforcement
                                let mut plan_rate_limit: u32 = 0;
                                if let Some(ref billing) = state.billing {
                                    let billing_acc = billing.get_or_create_account(&account_id);
                                    if let Some(plan) = billing.plans().get(&billing_acc.plan_id) {
                                        plan_rate_limit = plan.limits.api_rate_limit;
                                        req.extensions_mut().insert(plan);
                                    }
                                }
                                // Per-plan rate limiting
                                if !state.key_rate_limiter.check_plan(&rate_key, plan_rate_limit).await {
                                    return json_error(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "API key rate limit exceeded");
                                }
                                return next.run(req).await;
                            }
                        }
                    }
                }
            }
            // Insert default Claims for unauthenticated dev mode
            let default_claims = crate::auth::Claims {
                sub: "dev".into(), account_id: "dev".into(),
                email: None, name: Some("Dev User".into()), avatar_url: None,
                is_admin: true, org_id: None, iat: 0, exp: 0,
            };
            req.extensions_mut().insert(AccountId("dev".into()));
            req.extensions_mut().insert(default_claims);
            warn!("JWT_AUTH_DISABLED: no auth_engine configured (dev mode)");
            return next.run(req).await;
        }
    };

    let token = match extract_token_from_request(&req) {
        Some(t) => t,
        None => return json_error(StatusCode::UNAUTHORIZED, "token_missing", "Missing Authorization header or cookie"),
    };

    match auth_engine.validate_access_token(&token) {
        Ok(claims) => {
            req.extensions_mut().insert(AccountId(claims.account_id.clone()));
            req.extensions_mut().insert(claims.clone());
            // Resolve plan for billing enforcement
            if let Some(ref billing) = state.billing {
                let billing_acc = billing.get_or_create_account(&claims.account_id);
                if let Some(plan) = billing.plans().get(&billing_acc.plan_id) {
                    req.extensions_mut().insert(plan);
                }
            }
            next.run(req).await
        }
        Err(_) => {
            // Fallback: check if this is an API key (flk_...)
            if token.starts_with("flk_") {
                if let Some(ref db_opt) = state.db {
                    let pool = db_opt.pool();
                    match crate::api_keys::ApiKeyRepo::validate(pool, &token).await {
                        Ok(Some(identity)) => {
                            // Create synthetic Claims from API key identity
                            let is_admin = matches!(identity.role, crate::api_keys::ApiKeyRole::Admin);
                            let claims = crate::auth::Claims {
                                sub: identity.account_id.clone(),
                                account_id: identity.account_id.clone(),
                                email: None,
                                name: Some(identity.name.clone()),
                                avatar_url: None,
                                is_admin,
                                org_id: Some(identity.org_id.to_string()),
                                iat: 0,
                                exp: 0,
                            };
                            req.extensions_mut().insert(AccountId(identity.account_id.clone()));
                            let account_id = identity.account_id.clone();
                            req.extensions_mut().insert(claims.clone());
                            // Also store KeyIdentity for scope-aware handlers
                            req.extensions_mut().insert(identity);
                            // Resolve plan for billing enforcement
                            let mut plan_rate_limit: u32 = 0;
                            if let Some(ref billing) = state.billing {
                                let billing_acc = billing.get_or_create_account(&account_id);
                                if let Some(plan) = billing.plans().get(&billing_acc.plan_id) {
                                    plan_rate_limit = plan.limits.api_rate_limit;
                                    req.extensions_mut().insert(plan);
                                }
                            }
                            // Per-plan rate limiting
                            let rate_key = account_id.clone();
                            if !state.key_rate_limiter.check_plan(&rate_key, plan_rate_limit).await {
                                return json_error(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "API key rate limit exceeded");
                            }
                            return next.run(req).await;
                        }
                        Ok(None) => return json_error(StatusCode::UNAUTHORIZED, "api_key_invalid", "Invalid or expired API key"),
                        Err(e) => {
                            warn!("API key validation error: {}", e);
                            return json_error(StatusCode::INTERNAL_SERVER_ERROR, "auth_error", "Authentication error");
                        }
                    }
                }
            }
            json_error(StatusCode::UNAUTHORIZED, "token_invalid", "Invalid or expired token")
        }
    }
}

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

/// RBAC middleware layer — validates token via RbacManager, stores roles in extensions.
/// If RbacManager has no users configured, passes through (dev mode).
pub fn rbac_layer(
    rbac: Arc<RbacManager>,
    skip_paths: Vec<String>,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> + Clone {
    move |req: Request, next: Next| {
        let rbac = rbac.clone();
        let skip_paths = skip_paths.clone();
        Box::pin(async move {
            let path = req.uri().path().to_string();

            if skip_paths.iter().any(|p| path == *p || path.starts_with(&format!("{}/", p))) {
                return next.run(req).await;
            }

            // Dev mode: no users configured
            if rbac.list_users().is_empty() {
                return next.run(req).await;
            }

            let auth_header = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
            let token = match auth_header {
                None => return json_error(StatusCode::UNAUTHORIZED, "token_missing", "Missing Authorization header"),
                Some(h) => h.strip_prefix("Bearer ").unwrap_or(h),
            };

            match rbac.validate_token(token) {
                Some(tok) => {
                    let mut req = req;
                    req.extensions_mut().insert(UserRoles(tok.roles));
                    next.run(req).await
                }
                None => json_error(StatusCode::UNAUTHORIZED, "token_invalid", "Invalid or expired RBAC token"),
            }
        })
    }
}

/// Helper: check if the current request has a specific RBAC permission.
pub fn require_permission(
    roles: &Option<UserRoles>,
    permission: &flowlink_core::rbac::Permission,
) -> bool {
    match roles {
        None => true, // dev mode (no RBAC)
        Some(UserRoles(rs)) => rs.iter().any(|r| r.permissions().contains(permission)),
    }
}

fn subtle_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | x ^ y) == 0
}

// ── Rate Limit Middleware (state-injected, path-aware) ──

/// Build a rate-limit middleware layer with state injection and path skipping.
///
/// Uses `Arc<RateLimiter>` shared state and skips whitelisted paths
/// (e.g. `/healthz`, `/ws`) that must never be throttled.
pub fn rate_limit_layer(
    limiter: Arc<RateLimiter>,
    skip_paths: Vec<String>,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> + Clone {
    move |req: Request, next: Next| {
        let limiter = limiter.clone();
        let skip_paths = skip_paths.clone();
        Box::pin(async move {
            let path = req.uri().path().to_string();

            // Skip whitelisted paths entirely
            if skip_paths.iter().any(|p| path == *p || path.starts_with(&format!("{}/", p))) {
                return next.run(req).await;
            }

            // Extract key: prefer authenticated ClientId, then X-Forwarded-For IP, then "global"
            let key = req.extensions().get::<ClientId>()
                .map(|c| format!("client:{}", c.0))
                .unwrap_or_else(|| {
                    let ip = req.headers().get("x-forwarded-for")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.split(',').next())
                        .unwrap_or("unknown");
                    format!("ip:{}", ip)
                });

            if !limiter.allow(&key) {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    [(header::RETRY_AFTER, "10")],
                    axum::Json(serde_json::json!({
                        "error": "rate limit exceeded",
                        "code": "rate_limit_exceeded"
                    })),
                ).into_response();
            }

            next.run(req).await
        })
    }
}

/// Backward-compatible no-op kept for any call-sites that reference the old
/// stateless function signature.  Prefer `rate_limit_layer` in new code.
pub async fn rate_limit_middleware(
    req: Request,
    next: Next,
) -> Response {
    next.run(req).await
}

// ── Request ID Middleware ──

pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    let request_id = uuid::Uuid::new_v4().to_string();
    req.extensions_mut().insert(RequestId(request_id.clone()));

    let mut response = next.run(req).await;
    if let Ok(val) = request_id.parse() {
        response.headers_mut().insert("x-request-id", val);
    }
    response
}

// ── CORS Layer ──

pub fn cors_layer(allowed_origins: Vec<String>) -> tower_http::cors::CorsLayer {
    use tower_http::cors::Any;

    if allowed_origins.is_empty() || allowed_origins.iter().any(|o| o == "*") {
        // Wildcard: allow all origins but restrict methods
        return tower_http::cors::CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([axum::http::Method::GET, axum::http::Method::POST, axum::http::Method::PUT, axum::http::Method::DELETE, axum::http::Method::PATCH, axum::http::Method::OPTIONS])
            .allow_headers([axum::http::header::CONTENT_TYPE, axum::http::header::AUTHORIZATION, axum::http::header::HeaderName::from_static("x-api-key")])
            .allow_credentials(false);
    }

    // Specific origins
    let origins: Vec<axum::http::HeaderValue> = allowed_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();

    if origins.is_empty() {
        // Fallback: no CORS if no valid origins
        return tower_http::cors::CorsLayer::new()
            .allow_origin("http://localhost:3000".parse::<axum::http::HeaderValue>().unwrap())
            .allow_methods(Any)
            .allow_headers(Any);
    }

    tower_http::cors::CorsLayer::new()
        .allow_origin(origins)
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_credentials(false)
}

/// Security headers middleware
pub async fn security_headers_middleware(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    headers.insert("Referrer-Policy", "strict-origin-when-cross-origin".parse().unwrap());
    headers.insert("Permissions-Policy", "camera=(), microphone=(), geolocation=()".parse().unwrap());
    response
}

// ── Prometheus HTTP Metrics Middleware ──

/// Normalize a URL path for Prometheus labels — strip UUIDs and numeric IDs
/// to avoid cardinality explosion. E.g. /api/v1/accounts/abc-123 → /api/v1/accounts/:id
fn normalize_path(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').collect();
    let mut out = Vec::with_capacity(segments.len());
    for seg in &segments {
        if seg.is_empty() {
            out.push(*seg);
        } else if seg.len() >= 32 && seg.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
            out.push(":id");
        } else if seg.parse::<u64>().is_ok() && out.last().map_or(false, |&p| p == ":id" || p.starts_with("api")) {
            out.push(":id");
        } else {
            out.push(*seg);
        }
    }
    out.join("/")
}

/// Prometheus middleware — records HTTP request count, duration, and errors.
/// Must be layered with State (from_fn_with_state).
pub async fn prometheus_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let normalized = normalize_path(&path);

    let start = std::time::Instant::now();
    let response = next.run(req).await;
    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    // Skip noisy paths from metrics
    if !path.starts_with("/health") && !path.starts_with("/metrics") && !path.starts_with("/api/events") {
        let m = &state.metrics;
        let _ = m.http_requests_total.with_label_values(&[method.as_str(), &normalized, &status]).inc();
        let _ = m.http_request_duration_ms.with_label_values(&[method.as_str(), &normalized]).observe(duration);
    }

    response
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use tower::ServiceExt;

    fn test_app() -> axum::Router {
        axum::Router::new()
            .route("/health", axum::routing::get(|| async { "ok" }))
            .route("/healthz", axum::routing::get(|| async { "ok" }))
            .route("/ws", axum::routing::get(|| async { "ok" }))
    }

    #[tokio::test]
    async fn test_request_id_middleware() {
        let app = test_app().layer(axum::middleware::from_fn(request_id_middleware));
        let resp = app.oneshot(HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(resp.headers().get("x-request-id").is_some());
    }

    #[tokio::test]
    async fn test_request_id_is_uuid() {
        let app = test_app().layer(axum::middleware::from_fn(request_id_middleware));
        let resp = app.oneshot(HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
        let id = resp.headers().get("x-request-id").unwrap().to_str().unwrap();
        assert!(uuid::Uuid::parse_str(id).is_ok());
    }

    #[tokio::test]
    async fn test_request_id_different_per_request() {
        let app = test_app().layer(axum::middleware::from_fn(request_id_middleware));
        let req1 = HttpRequest::builder().uri("/health").body(Body::empty()).unwrap();
        let req2 = HttpRequest::builder().uri("/health").body(Body::empty()).unwrap();
        let id1 = app.clone().oneshot(req1).await.unwrap().headers().get("x-request-id").unwrap().to_str().unwrap().to_string();
        let id2 = app.oneshot(req2).await.unwrap().headers().get("x-request-id").unwrap().to_str().unwrap().to_string();
        assert_ne!(id1, id2);
    }

    #[tokio::test]
    async fn test_auth_simple_passthrough() {
        let app = test_app().layer(axum::middleware::from_fn(auth_middleware_simple));
        let resp = app.oneshot(HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_layer_no_config_passthrough() {
        let auth = Arc::new(AuthManager::new(None));
        let layer = auth_layer(auth, None, vec![]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        let resp = app.oneshot(HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_layer_rejects_missing_token() {
        let auth = Arc::new(AuthManager::new(None));
        auth.register_client(crate::auth::Client {
            client_id: "c1".into(), api_token: "tok1".into(), name: "c1".into(), active: true,
        });
        let layer = auth_layer(auth, None, vec![]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        let resp = app.oneshot(HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_layer_accepts_valid_token() {
        let auth = Arc::new(AuthManager::new(None));
        auth.register_client(crate::auth::Client {
            client_id: "c1".into(), api_token: "tok1".into(), name: "c1".into(), active: true,
        });
        let layer = auth_layer(auth, None, vec![]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        let req = HttpRequest::builder().uri("/health")
            .header("Authorization", "Bearer tok1")
            .body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_layer_invalid_token() {
        let auth = Arc::new(AuthManager::new(None));
        auth.register_client(crate::auth::Client {
            client_id: "c1".into(), api_token: "secret".into(), name: "c1".into(), active: true,
        });
        let layer = auth_layer(auth, None, vec![]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        let req = HttpRequest::builder().uri("/health")
            .header("Authorization", "Bearer wrong")
            .body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_layer_skip_paths() {
        let auth = Arc::new(AuthManager::new(None));
        auth.register_client(crate::auth::Client {
            client_id: "c1".into(), api_token: "tok1".into(), name: "c1".into(), active: true,
        });
        let layer = auth_layer(auth, None, vec!["/health".into()]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        let resp = app.oneshot(HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_layer_static_token() {
        let auth = Arc::new(AuthManager::new(None));
        let layer = auth_layer(auth, Some("static-secret".into()), vec![]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        let req = HttpRequest::builder().uri("/health")
            .header("Authorization", "Bearer static-secret")
            .body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_logging_middleware() {
        let app = test_app().layer(axum::middleware::from_fn(logging_middleware));
        let resp = app.oneshot(HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn test_subtle_eq() {
        assert!(subtle_eq(b"abc", b"abc"));
        assert!(!subtle_eq(b"abc", b"abd"));
        assert!(!subtle_eq(b"ab", b"abc"));
    }

    #[test]
    fn test_cors_layer_wildcard() {
        let _layer = cors_layer(vec!["*".to_string()]);
    }

    // ── RBAC middleware tests ──

    #[tokio::test]
    async fn test_rbac_layer_dev_mode_passthrough() {
        let rbac = Arc::new(RbacManager::new());
        let layer = rbac_layer(rbac, vec![]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        let resp = app.oneshot(HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_rbac_layer_skip_paths() {
        let rbac = Arc::new(RbacManager::new());
        let user = flowlink_core::rbac::RbacUser {
            id: "u1".into(), username: "a".into(), roles: vec![flowlink_core::rbac::Role::Admin],
            allowed_paths: None, denied_commands: None, metadata: std::collections::HashMap::new(),
        };
        rbac.add_user(user).unwrap();
        let layer = rbac_layer(rbac, vec!["/health".into()]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        let resp = app.oneshot(HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn test_require_permission_dev_mode() {
        assert!(require_permission(&None, &flowlink_core::rbac::Permission::UserManage));
    }

    #[test]
    fn test_require_permission_viewer_denied() {
        let roles = UserRoles(vec![flowlink_core::rbac::Role::Viewer]);
        assert!(require_permission(&Some(roles), &flowlink_core::rbac::Permission::MetricsView));
        let roles2 = UserRoles(vec![flowlink_core::rbac::Role::Viewer]);
        assert!(!require_permission(&Some(roles2), &flowlink_core::rbac::Permission::CommandExecute));
    }

    // ── Rate limit layer tests ──

    #[tokio::test]
    async fn test_rate_limit_layer_allows_under_limit() {
        let limiter = Arc::new(RateLimiter::new(5, 1));
        let layer = rate_limit_layer(limiter.clone(), vec![]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        for _ in 0..5 {
            let resp = app.clone().oneshot(
                HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()
            ).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }
    }

    #[tokio::test]
    async fn test_rate_limit_layer_blocks_over_limit() {
        let limiter = Arc::new(RateLimiter::new(3, 10));
        let layer = rate_limit_layer(limiter.clone(), vec![]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        // First 3 should pass
        for _ in 0..3 {
            let resp = app.clone().oneshot(
                HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()
            ).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }
        // 4th should be rate-limited
        let resp = app.oneshot(
            HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn test_rate_limit_layer_skips_healthz() {
        let limiter = Arc::new(RateLimiter::new(1, 10)); // very strict
        let layer = rate_limit_layer(limiter.clone(), vec!["/healthz".to_string()]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        // First request uses the token
        let resp = app.clone().oneshot(
            HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        // Second request is blocked (different path)
        let resp = app.clone().oneshot(
            HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        // /healthz should still pass even though we're rate-limited
        let resp = app.oneshot(
            HttpRequest::builder().uri("/healthz").body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_rate_limit_layer_skips_ws() {
        let limiter = Arc::new(RateLimiter::new(1, 10));
        let layer = rate_limit_layer(limiter.clone(), vec!["/ws".to_string()]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        // Exhaust the limit
        let resp = app.clone().oneshot(
            HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let resp = app.clone().oneshot(
            HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        // /ws should pass
        let resp = app.oneshot(
            HttpRequest::builder().uri("/ws").body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_rate_limit_layer_response_body_is_json() {
        let limiter = Arc::new(RateLimiter::new(1, 10));
        let layer = rate_limit_layer(limiter.clone(), vec![]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        // First passes
        let _ = app.clone().oneshot(
            HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()
        ).await.unwrap();
        // Second is rate-limited; check body
        let resp = app.oneshot(
            HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(resp.headers().get("retry-after").is_some());
        assert!(resp.headers().get("content-type").is_some());
    }

    #[tokio::test]
    async fn test_rate_limit_layer_different_keys() {
        let limiter = Arc::new(RateLimiter::new(1, 10));
        let layer = rate_limit_layer(limiter.clone(), vec![]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        // Exhaust limit for default IP key
        let _ = app.clone().oneshot(
            HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()
        ).await.unwrap();
        let resp = app.clone().oneshot(
            HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        // Different X-Forwarded-For should have its own bucket
        let resp = app.oneshot(
            HttpRequest::builder().uri("/health")
                .header("x-forwarded-for", "1.2.3.4")
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}

pub(crate) fn json_error(status: StatusCode, code: &str, message: &str) -> Response {
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

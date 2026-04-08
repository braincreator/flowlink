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
    if let Ok(val) = request_id.parse() {
        response.headers_mut().insert("x-request-id", val);
    }
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

    let _origins: Vec<axum::http::HeaderValue> = allowed_origins
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, header, StatusCode};
    use tower::ServiceExt;

    fn test_app() -> axum::Router {
        axum::Router::new().route("/health", axum::routing::get(|| async { "ok" }))
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
        let auth = Arc::new(AuthManager::new());
        let layer = auth_layer(auth, None, vec![]);
        let app = test_app().layer(axum::middleware::from_fn(layer));
        let resp = app.oneshot(HttpRequest::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_auth_layer_rejects_missing_token() {
        let auth = Arc::new(AuthManager::new());
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
        let auth = Arc::new(AuthManager::new());
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
        let auth = Arc::new(AuthManager::new());
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
        let auth = Arc::new(AuthManager::new());
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
        let auth = Arc::new(AuthManager::new());
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
}

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

// FlowLink Shield — HTTP API for remote management

use std::sync::Arc;
use axum::{
    Router, Json, extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::Serialize;
use crate::guard::{ShieldGuard, InterceptResult};

/// Shared state for the HTTP server
#[derive(Clone)]
pub struct ShieldState {
    pub guard: Arc<ShieldGuard>,
}

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Stats response
#[derive(Serialize)]
pub struct StatsResponse {
    pub total_analyzed: u64,
    pub allowed: u64,
    pub blocked: u64,
    pub released: u64,
    pub timeout_killed: u64,
    pub pending: u64,
}

/// Pending interception item
#[derive(Serialize)]
pub struct PendingItem {
    pub pid: u32,
    pub threat: String,
    pub command: String,
}

/// Action response
#[derive(Serialize)]
pub struct ActionResponse {
    pub success: bool,
    pub message: String,
}

/// Build the shield HTTP router
pub fn shield_router(guard: Arc<ShieldGuard>) -> Router {
    let state = ShieldState { guard };
    Router::new()
        .route("/health", get(health))
        .route("/api/pending", get(list_pending))
        .route("/api/approve/{pid}", post(approve))
        .route("/api/reject/{pid}", post(reject))
        .route("/api/stats", get(stats))
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn stats(State(state): State<ShieldState>) -> Json<StatsResponse> {
    let s = state.guard.stats().await;
    Json(StatsResponse {
        total_analyzed: s.total_analyzed,
        allowed: s.allowed,
        blocked: s.blocked,
        released: s.released,
        timeout_killed: s.timeout_killed,
        pending: s.pending,
    })
}

async fn list_pending(State(state): State<ShieldState>) -> Json<Vec<PendingItem>> {
    let items: Vec<PendingItem> = state.guard.list_pending()
        .into_iter()
        .map(|(pid, threat, command)| PendingItem { pid, threat, command })
        .collect();
    Json(items)
}

async fn approve(
    State(state): State<ShieldState>,
    Path(pid): Path<u32>,
) -> (StatusCode, Json<ActionResponse>) {
    match state.guard.resolve_approval(pid, true).await {
        Ok(true) => (StatusCode::OK, Json(ActionResponse {
            success: true,
            message: format!("PID {} approved and resumed", pid),
        })),
        Ok(false) => (StatusCode::NOT_FOUND, Json(ActionResponse {
            success: false,
            message: format!("PID {} not found in pending list", pid),
        })),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ActionResponse {
            success: false,
            message: format!("Error: {}", e),
        })),
    }
}

async fn reject(
    State(state): State<ShieldState>,
    Path(pid): Path<u32>,
) -> (StatusCode, Json<ActionResponse>) {
    match state.guard.resolve_approval(pid, false).await {
        Ok(true) => (StatusCode::OK, Json(ActionResponse {
            success: true,
            message: format!("PID {} rejected and killed", pid),
        })),
        Ok(false) => (StatusCode::NOT_FOUND, Json(ActionResponse {
            success: false,
            message: format!("PID {} not found in pending list", pid),
        })),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ActionResponse {
            success: false,
            message: format!("Error: {}", e),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    use tempfile::NamedTempFile;

    fn make_test_router() -> Router {
        let tmp = NamedTempFile::new().unwrap();
        let audit = Arc::new(tokio::sync::RwLock::new(
            crate::audit::AuditLog::open(tmp.path()).unwrap()
        ));
        let notifier = crate::notifier::Notifier::new(None);
        let engine = crate::engine::AnalysisEngine { enable_ast: false, enable_interpreter: false };
        let guard = Arc::new(crate::guard::ShieldGuard::new(
            engine,
            crate::snapshot::SnapshotBackend::None,
            audit,
            notifier,
            crate::guard::ShieldGuardConfig::default(),
        ));
        shield_router(guard)
    }

    #[tokio::test]
    async fn health_endpoint() {
        let app = make_test_router();
        let req = Request::builder().uri("/health").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert!(json["version"].is_string());
    }

    #[tokio::test]
    async fn stats_endpoint() {
        let app = make_test_router();
        let req = Request::builder().uri("/api/stats").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total_analyzed"], 0);
        assert_eq!(json["allowed"], 0);
        assert_eq!(json["pending"], 0);
    }

    #[tokio::test]
    async fn pending_endpoint_empty() {
        let app = make_test_router();
        let req = Request::builder().uri("/api/pending").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.is_array());
        assert_eq!(json.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn approve_nonexistent_returns_404() {
        let app = make_test_router();
        let req = Request::builder().method("POST").uri("/api/approve/99999").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn reject_nonexistent_returns_404() {
        let app = make_test_router();
        let req = Request::builder().method("POST").uri("/api/reject/99999").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn health_response_serialization() {
        let r = HealthResponse { status: "ok".into(), version: "0.1.0".into() };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("ok"));
    }

    #[test]
    fn stats_response_serialization() {
        let r = StatsResponse { total_analyzed: 10, allowed: 8, blocked: 1, released: 1, timeout_killed: 0, pending: 0 };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("total_analyzed"));
    }

    #[test]
    fn pending_item_serialization() {
        let r = PendingItem { pid: 1234, threat: "rm_rf".into(), command: "rm -rf /".into() };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("rm_rf"));
    }

    #[test]
    fn action_response_serialization() {
        let r = ActionResponse { success: true, message: "done".into() };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("done"));
    }
}

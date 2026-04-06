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

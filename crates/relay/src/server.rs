use axum::{
    extract::State,
    response::Json,
    routing::get,
    Router,
};
use serde::Serialize;
use std::sync::Arc;

use crate::approval::ApprovalQueue;
use crate::pool::AgentPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: Arc<AgentPool>,
    pub approvals: Arc<ApprovalQueue>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

async fn list_agents(State(state): State<AppState>) -> Json<Vec<crate::pool::AgentInfo>> {
    Json(state.pool.list())
}

async fn list_approvals(
    State(state): State<AppState>,
) -> Json<Vec<crate::approval::ApprovalRequest>> {
    Json(state.approvals.list_pending())
}

pub fn build_router(pool: Arc<AgentPool>, approvals: Arc<ApprovalQueue>) -> Router {
    let state = AppState { pool, approvals };
    Router::new()
        .route("/health", get(health))
        .route("/api/agents", get(list_agents))
        .route("/api/approvals", get(list_approvals))
        .with_state(state)
}

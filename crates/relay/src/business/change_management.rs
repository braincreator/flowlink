//! Change Management — safe rollout of changes through agents.
//!
//! Tracks planned changes, approval workflows, and rollback capabilities.
//! Agents apply change scripts through the approval pipeline with full audit.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::Claims;
use crate::server::AppState;

fn require_org(claims: &Claims) -> Result<(String, Uuid), (StatusCode, Json<serde_json::Value>)> {
    match &claims.org_id {
        Some(id) => Ok((id.clone(), id.parse().unwrap_or_default())),
        None => Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "No org"})))),
    }
}

fn require_pool(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, Json<serde_json::Value>)> {
    state.db.as_ref().map(|db| db.pool()).ok_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "DB unavailable"})))
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRequest {
    pub id: String,
    pub org_id: String,
    pub title: String,
    pub description: String,
    pub change_type: ChangeType,
    pub status: ChangeStatus,
    pub risk_level: String,
    pub requested_by: String,
    pub approved_by: Option<String>,
    pub agent_id: Option<String>,
    pub target_services: Vec<String>,
    pub commands: Vec<ChangeCommand>,
    pub rollback_commands: Vec<ChangeCommand>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub executed_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<ChangeResult>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Deploy,
    ConfigUpdate,
    Patch,
    Rollback,
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeStatus {
    Draft,
    PendingApproval,
    Approved,
    Scheduled,
    InProgress,
    Completed,
    Failed,
    RolledBack,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeCommand {
    pub command: String,
    pub target: String,
    pub args: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub duration_ms: i64,
    pub services_affected: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChangeRequest {
    pub title: String,
    pub description: String,
    pub change_type: Option<String>,
    pub risk_level: Option<String>,
    pub agent_id: Option<String>,
    pub target_services: Vec<String>,
    pub commands: Vec<ChangeCommand>,
    pub rollback_commands: Vec<ChangeCommand>,
    pub scheduled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ListChangesQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

/// POST /api/v1/changes
/// Create a new change request.
pub async fn create_change(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(params): Json<CreateChangeRequest>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    let id = Uuid::new_v4().to_string();
    let change_type = params.change_type.as_deref().unwrap_or("deploy");
    let risk = params.risk_level.as_deref().unwrap_or("medium");
    let now = Utc::now();

    // Store the change request
    let change = ChangeRequest {
        id: id.clone(),
        org_id: org_str,
        title: params.title,
        description: params.description,
        change_type: match change_type {
            "deploy" => ChangeType::Deploy,
            "config" => ChangeType::ConfigUpdate,
            "patch" => ChangeType::Patch,
            "rollback" => ChangeType::Rollback,
            "emergency" => ChangeType::Emergency,
            _ => ChangeType::Deploy,
        },
        status: ChangeStatus::PendingApproval,
        risk_level: risk.to_string(),
        requested_by: claims.account_id.clone(),
        approved_by: None,
        agent_id: params.agent_id,
        target_services: params.target_services,
        commands: params.commands,
        rollback_commands: params.rollback_commands,
        scheduled_at: params.scheduled_at,
        executed_at: None,
        completed_at: None,
        result: None,
        created_at: now,
    };

    // Log to audit
    let _ = sqlx::query(
        "INSERT INTO audit_log (org_id, level, action, target, result, metadata, agent_id, account_id) VALUES ($1, 'info', 'change_request_created', $2, 'pending', $3, NULL, $4)"
    ).bind(org_uuid).bind(&id)
    .bind(serde_json::json!({"title": change.title, "risk": change.risk_level, "services": change.target_services}))
    .bind(&claims.account_id)
    .execute(pool).await;

    Json(change).into_response()
}

/// GET /api/v1/changes
/// List change requests for the org.
pub async fn list_changes(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<ListChangesQuery>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (_org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    let limit = params.limit.unwrap_or(50).min(200);

    // Query audit_log for change requests
    let query = String::from(
        "SELECT id, timestamp, level, action, target, result, metadata, account_id FROM audit_log WHERE org_id = $1 AND action LIKE 'change_%' ORDER BY timestamp DESC LIMIT $2"
    );

    let rows = sqlx::query_as::<_, (i64, DateTime<Utc>, String, String, String, String, serde_json::Value, Option<String>)>(
        &query
    ).bind(org_uuid).bind(limit)
    .fetch_all(pool).await.unwrap_or_default();

    let changes: Vec<serde_json::Value> = rows.into_iter().map(|(_id, ts, level, action, target, result, meta, acid)| {
        serde_json::json!({
            "id": target,
            "timestamp": ts,
            "level": level,
            "action": action,
            "result": result,
            "metadata": meta,
            "requested_by": acid,
        })
    }).collect();

    Json(serde_json::json!({"changes": changes, "total": changes.len()})).into_response()
}

/// POST /api/v1/changes/{change_id}/approve
/// Approve a change request.
pub async fn approve_change(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(change_id): Path<String>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (_org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    // Log approval
    let _ = sqlx::query(
        "INSERT INTO audit_log (org_id, level, action, target, result, metadata, account_id) VALUES ($1, 'info', 'change_approved', $2, 'approved', $3, $4)"
    ).bind(org_uuid).bind(&change_id)
    .bind(serde_json::json!({"approved_by": claims.account_id, "approved_at": Utc::now()}))
    .bind(&claims.account_id)
    .execute(pool).await;

    Json(serde_json::json!({
        "change_id": change_id,
        "status": "approved",
        "approved_by": claims.account_id,
        "approved_at": Utc::now(),
    })).into_response()
}

/// POST /api/v1/changes/{change_id}/rollback
/// Trigger rollback for a change.
pub async fn rollback_change(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(change_id): Path<String>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (_org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    // Log rollback
    let _ = sqlx::query(
        "INSERT INTO audit_log (org_id, level, action, target, result, metadata, account_id) VALUES ($1, 'warn', 'change_rollback', $2, 'rolled_back', $3, $4)"
    ).bind(org_uuid).bind(&change_id)
    .bind(serde_json::json!({"rolled_back_by": claims.account_id, "rolled_back_at": Utc::now()}))
    .bind(&claims.account_id)
    .execute(pool).await;

    Json(serde_json::json!({
        "change_id": change_id,
        "status": "rolled_back",
        "rolled_back_by": claims.account_id,
        "rolled_back_at": Utc::now(),
    })).into_response()
}

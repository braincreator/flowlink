use sqlx::Row;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::AppState;
use crate::auth::Claims;

fn gp(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, String)> {
    state.db.as_ref().map(|db| db.pool()).ok_or((StatusCode::SERVICE_UNAVAILABLE, "Database not configured".to_string()))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub org_id: Option<Uuid>,
    pub agent_id: String,
    pub account_id: Option<String>,
    pub cwd: String,
    pub env: serde_json::Value,
    pub shell: String,
    pub status: String,
    pub created_at: String,
    pub last_activity: String,
    pub closed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub agent_id: String,
    pub shell: Option<String>,
    pub cwd: Option<String>,
    pub env: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct SessionQuery {
    pub agent_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
}

pub async fn create_session(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<Session>), (StatusCode, String)> {
    if !claims.is_admin && claims.org_id.is_none() {
        return Err((StatusCode::FORBIDDEN, "Organization required".into()));
    }
    let pool = gp(&state)?;
    let shell = body.shell.unwrap_or_else(|| "/bin/sh".to_string());
    let cwd = body.cwd.unwrap_or_else(|| "/".to_string());
    let env = body.env.unwrap_or(serde_json::json!({}));

    let row = sqlx::query(
        "INSERT INTO interactive_sessions (agent_id, shell, cwd, env) VALUES ($1, $2, $3, $4) RETURNING id, agent_id, cwd, env, shell, status, created_at::text, last_activity::text, closed_at"
    )
    .bind(&body.agent_id).bind(&shell).bind(&cwd).bind(&env)
    .fetch_one(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(Session {
        id: row.get("id"), org_id: row.get("org_id"), agent_id: row.get("agent_id"),
        account_id: row.get("account_id"), cwd: row.get("cwd"), env: row.get("env"),
        shell: row.get("shell"), status: row.get("status"),
        created_at: row.get("created_at"), last_activity: row.get("last_activity"),
        closed_at: row.get("closed_at"),
    })))
}

pub async fn list_sessions(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Query(q): Query<SessionQuery>,
) -> Result<(StatusCode, Json<Vec<Session>>), (StatusCode, String)> {
    let pool = gp(&state)?;
    let limit = q.limit.unwrap_or(50).min(200);

    let rows = sqlx::query(
        "SELECT id, org_id, agent_id, account_id, cwd, env, shell, status, created_at::text, last_activity::text, closed_at FROM interactive_sessions WHERE ($1::text IS NULL OR agent_id = $1) AND ($2::text IS NULL OR status = $2) ORDER BY last_activity DESC LIMIT $3"
    ).bind(&q.agent_id).bind(&q.status).bind(limit)
    .fetch_all(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let sessions: Vec<Session> = rows.iter().map(|r| Session {
        id: r.get("id"), org_id: r.get("org_id"), agent_id: r.get("agent_id"),
        account_id: r.get("account_id"), cwd: r.get("cwd"), env: r.get("env"),
        shell: r.get("shell"), status: r.get("status"),
        created_at: r.get("created_at"), last_activity: r.get("last_activity"),
        closed_at: r.get("closed_at"),
    }).collect();

    Ok((StatusCode::OK, Json(sessions)))
}

pub async fn get_session(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<Session>), (StatusCode, String)> {
    let pool = gp(&state)?;
    let row = sqlx::query(
        "SELECT id, org_id, agent_id, account_id, cwd, env, shell, status, created_at::text, last_activity::text, closed_at FROM interactive_sessions WHERE id = $1"
    ).bind(id).fetch_optional(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match row {
        Some(r) => Ok((StatusCode::OK, Json(Session {
            id: r.get("id"), org_id: r.get("org_id"), agent_id: r.get("agent_id"),
            account_id: r.get("account_id"), cwd: r.get("cwd"), env: r.get("env"),
            shell: r.get("shell"), status: r.get("status"),
            created_at: r.get("created_at"), last_activity: r.get("last_activity"),
            closed_at: r.get("closed_at"),
        }))),
        None => Err((StatusCode::NOT_FOUND, "Session not found".into())),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateSessionRequest {
    pub cwd: Option<String>,
    pub env: Option<serde_json::Value>,
}

pub async fn update_session(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateSessionRequest>,
) -> Result<(StatusCode, Json<Session>), (StatusCode, String)> {
    let pool = gp(&state)?;
    let row = sqlx::query(
        "UPDATE interactive_sessions SET cwd = COALESCE($2, cwd), env = COALESCE($3, env), last_activity = NOW() WHERE id = $1 AND status = 'active' RETURNING id, org_id, agent_id, account_id, cwd, env, shell, status, created_at::text, last_activity::text, closed_at"
    ).bind(id).bind(&body.cwd).bind(&body.env)
    .fetch_optional(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match row {
        Some(r) => Ok((StatusCode::OK, Json(Session {
            id: r.get("id"), org_id: r.get("org_id"), agent_id: r.get("agent_id"),
            account_id: r.get("account_id"), cwd: r.get("cwd"), env: r.get("env"),
            shell: r.get("shell"), status: r.get("status"),
            created_at: r.get("created_at"), last_activity: r.get("last_activity"),
            closed_at: r.get("closed_at"),
        }))),
        None => Err((StatusCode::NOT_FOUND, "Session not found or closed".into())),
    }
}

pub async fn close_session(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    if !claims.is_admin { return Err((StatusCode::FORBIDDEN, "Admin required".into())); }
    let pool = gp(&state)?;
    let result = sqlx::query(
        "UPDATE interactive_sessions SET status = 'closed', closed_at = NOW() WHERE id = $1 AND status = 'active'"
    ).bind(id).execute(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 { return Err((StatusCode::NOT_FOUND, "Session not found or already closed".into())); }
    Ok(StatusCode::NO_CONTENT)
}

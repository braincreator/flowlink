//! Context Snapshots — point-in-time state capture for audit/rollback.

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
pub struct ContextSnapshot {
    pub snapshot_id: String,
    pub org_id: String,
    pub label: String,
    pub captured_at: DateTime<Utc>,
    pub captured_by: String,
    pub agents: Vec<serde_json::Value>,
    pub infra_nodes: Vec<serde_json::Value>,
    pub policies: Vec<serde_json::Value>,
    pub secrets_config: Vec<serde_json::Value>,
    pub summary: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct SnapshotQuery {
    pub label: Option<String>,
}

/// POST /api/v1/forensics/snapshot
pub async fn create_snapshot(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(params): Json<SnapshotQuery>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    let snapshot_id = format!("snap-{}", Utc::now().format("%Y%m%d%H%M%S"));

    let agents: Vec<serde_json::Value> = sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<String>, Option<DateTime<Utc>>)>(
        "SELECT agent_id, name, status, os, version, last_heartbeat FROM agents WHERE org_id = $1"
    ).bind(org_uuid).fetch_all(pool).await.unwrap_or_default()
    .into_iter().map(|(aid, name, status, os, ver, lh)| serde_json::json!({
        "agent_id": aid, "name": name.unwrap_or_default(), "status": status,
        "os": os.unwrap_or_default(), "version": ver.unwrap_or_default(), "last_heartbeat": lh
    })).collect();

    let infra_nodes: Vec<serde_json::Value> = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, name, node_type, criticality, environment, owner FROM infra_map_nodes WHERE org_id = $1"
    ).bind(org_uuid).fetch_all(pool).await.unwrap_or_default()
    .into_iter().map(|(id, name, ntype, crit, env, owner)| serde_json::json!({
        "node_id": id, "name": name, "node_type": ntype,
        "criticality": crit.unwrap_or("medium".into()), "environment": env.unwrap_or("unknown".into()), "owner": owner
    })).collect();

    let policies: Vec<serde_json::Value> = sqlx::query_as::<_, (String, Option<String>, bool)>(
        "SELECT policy_id, name, is_active FROM policies WHERE org_id = $1"
    ).bind(org_uuid).fetch_all(pool).await.unwrap_or_default()
    .into_iter().map(|(pid, name, active)| serde_json::json!({"policy_id": pid, "name": name.unwrap_or_default(), "is_active": active})).collect();

    let secrets_config: Vec<serde_json::Value> = sqlx::query_as::<_, (String, String, bool, Option<DateTime<Utc>>)>(
        "SELECT config_id, provider, credentials IS NOT NULL, created_at FROM org_secret_configs WHERE org_id = $1"
    ).bind(org_uuid).fetch_all(pool).await.unwrap_or_default()
    .into_iter().map(|(cid, provider, has_creds, created)| serde_json::json!({
        "config_id": cid, "provider": provider, "has_credentials": has_creds, "created_at": created
    })).collect();

    let total_agents = agents.len();
    let total_nodes = infra_nodes.len();
    let total_policies = policies.len();
    let high_crit = infra_nodes.iter().filter(|n| n.get("criticality").and_then(|v| v.as_str()) == Some("high") || n.get("criticality").and_then(|v| v.as_str()) == Some("critical")).count();

    let summary = serde_json::json!({
        "total_agents": total_agents, "total_services": total_nodes,
        "total_policies": total_policies, "total_secret_configs": secrets_config.len(),
        "high_criticality_count": high_crit,
    });

    let snapshot = ContextSnapshot {
        snapshot_id: snapshot_id.clone(),
        org_id: org_str,
        label: params.label.unwrap_or_else(|| format!("Auto-snapshot {}", Utc::now().format("%Y-%m-%d %H:%M"))),
        captured_at: Utc::now(),
        captured_by: claims.account_id.clone(),
        agents, infra_nodes, policies, secrets_config, summary: summary.clone(),
    };

    let snapshot_json = serde_json::to_value(&snapshot).unwrap_or(serde_json::json!({}));
    let _ = sqlx::query(
        "INSERT INTO infra_map_snapshots (org_id, agent_id, snapshot, version) VALUES ($1, 'system', $2, 1)"
    ).bind(org_uuid).bind(&snapshot_json).execute(pool).await;

    Json(snapshot).into_response()
}

/// GET /api/v1/forensics/snapshots
pub async fn list_snapshots(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (_org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    let snapshots: Vec<serde_json::Value> = sqlx::query_as::<_, (i32, String, DateTime<Utc>, serde_json::Value)>(
        "SELECT id, agent_id, created_at, snapshot FROM infra_map_snapshots WHERE org_id = $1 ORDER BY created_at DESC LIMIT 50"
    ).bind(org_uuid).fetch_all(pool).await.unwrap_or_default()
    .into_iter().map(|(id, agent_id, created, snap)| serde_json::json!({
        "id": id, "agent_id": agent_id, "created_at": created,
        "label": snap.get("label").and_then(|v| v.as_str()).unwrap_or("unnamed"),
        "summary": snap.get("summary"),
    })).collect();

    Json(serde_json::json!({"snapshots": snapshots})).into_response()
}

/// GET /api/v1/forensics/snapshot/{snapshot_id}
pub async fn get_snapshot(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(snapshot_id): Path<String>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (_org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    match sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT snapshot FROM infra_map_snapshots WHERE org_id = $1 AND snapshot::text LIKE $2 ORDER BY created_at DESC LIMIT 1"
    ).bind(org_uuid).bind(format!("%{}%", snapshot_id)).fetch_optional(pool).await {
        Ok(Some(data)) => Json(data).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("{}", e)}))).into_response(),
    }
}

/// GET /api/v1/forensics/diff/{ida}/{idb}
pub async fn diff_snapshots(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((ida, idb)): Path<(String, String)>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (_org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    let a = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT snapshot FROM infra_map_snapshots WHERE org_id = $1 AND id = $2"
    ).bind(org_uuid).bind(ida.parse::<i32>().unwrap_or(0)).fetch_optional(pool).await;

    let b = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT snapshot FROM infra_map_snapshots WHERE org_id = $1 AND id = $2"
    ).bind(org_uuid).bind(idb.parse::<i32>().unwrap_or(0)).fetch_optional(pool).await;

    match (a, b) {
        (Ok(Some(a)), Ok(Some(b))) => {
            let nodes_a = a.get("infra_nodes").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let nodes_b = b.get("infra_nodes").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let agents_a = a.get("agents").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let agents_b = b.get("agents").and_then(|v| v.as_array()).cloned().unwrap_or_default();

            let nodes_added = nodes_b.iter().filter(|b| !nodes_a.iter().any(|a| a.get("node_id") == b.get("node_id"))).count();
            let nodes_removed = nodes_a.iter().filter(|a| !nodes_b.iter().any(|b| b.get("node_id") == a.get("node_id"))).count();
            let agents_added = agents_b.iter().filter(|b| !agents_a.iter().any(|a| a.get("agent_id") == b.get("agent_id"))).count();
            let agents_removed = agents_a.iter().filter(|a| !agents_b.iter().any(|b| b.get("agent_id") == a.get("agent_id"))).count();

            Json(serde_json::json!({
                "snapshot_a": ida, "snapshot_b": idb,
                "diff": {"agents_added": agents_added, "agents_removed": agents_removed, "nodes_added": nodes_added, "nodes_removed": nodes_removed, "summary_a": a.get("summary"), "summary_b": b.get("summary")}
            })).into_response()
        }
        _ => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Not found"}))).into_response(),
    }
}

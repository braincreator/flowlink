// Infrastructure Map API — semantic GPS for AI agents
// Agents query this to understand infrastructure topology

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::Claims;
use crate::server::AppState;

fn get_pool(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, Json<serde_json::Value>)> {
    state.db.as_ref().map(|p| &p.write_pool).ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"ok": false, "error": "Database unavailable"})),
        )
    })
}

async fn require_org_member(
    pool: &sqlx::PgPool,
    org_id: Uuid,
    account_id: &str,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let role = sqlx::query_scalar::<_, String>(
        "SELECT role FROM org_members WHERE org_id = $1 AND account_id = $2"
    )
    .bind(org_id)
    .bind(account_id)
    .fetch_optional(pool)
    .await;

    match role {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"ok": false, "error": "Not a member of this organization"})),
        )),
        Err(e) => {
            log::error!("DB error: {e}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"ok": false, "error": "Internal error"})),
            ))
        }
    }
}

/// GET /api/orgs/{org_id}/map/services?name=billing&env=prod
/// Find services by name (fuzzy search) with optional environment filter
#[derive(Debug, Deserialize)]
pub struct FindServiceParams {
    pub name: Option<String>,
    pub env: Option<String>,
    #[serde(rename = "type")]
    pub service_type: Option<String>,
    pub owner: Option<String>,
}

pub async fn find_services(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Query(params): Query<FindServiceParams>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(r) => return r.into_response(),
    };

    if let Err(r) = require_org_member(pool, org_id, &claims.account_id).await {
        return r.into_response();
    }

    let mut query_str = String::from(
        "SELECT id, node_type, data, name, environment, criticality, owner FROM infra_map_nodes WHERE org_id = $1 AND node_type = 'service'"
    );
    let mut param_idx = 2u32;
    let mut param_values: Vec<(String, String)> = vec![];

    if let Some(name) = &params.name {
        query_str.push_str(&format!(" AND name ILIKE '%{}%'", name.replace('\'', "''")));
    }
    if let Some(env) = &params.env {
        query_str.push_str(&format!(" AND environment = '${}'", env.replace('\'', "''")));
    }
    if let Some(stype) = &params.service_type {
        query_str.push_str(&format!(" AND data->>'service_type' ILIKE '%{}%'", stype.replace('\'', "''")));
    }
    if let Some(owner) = &params.owner {
        query_str.push_str(&format!(" AND owner = '${}'", owner.replace('\'', "''")));
    }

    query_str.push_str(" ORDER BY name LIMIT 50");

    let rows = sqlx::query_as::<_, (String, String, serde_json::Value, String, Option<String>, Option<String>, Option<String>)>(
        &query_str
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let services: Vec<serde_json::Value> = rows.into_iter().map(|(id, node_type, data, name, env, criticality, owner)| {
        serde_json::json!({
            "id": id,
            "type": node_type,
            "name": name,
            "service_type": data.get("service_type").and_then(|v| v.as_str()).unwrap_or("unknown"),
            "environment": env,
            "criticality": criticality,
            "owner": owner,
            "labels": data.get("labels").cloned().unwrap_or(serde_json::json!({})),
        })
    }).collect();

    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "services": services,
    }))).into_response()
}

/// GET /api/orgs/{org_id}/map/service/{service_id}/topology
/// Get full topology for a service (dependencies, host, secrets, monitoring)
pub async fn service_topology(
    State(state): State<AppState>,
    Path((org_id, service_id)): Path<(Uuid, String)>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(r) => return r.into_response(),
    };

    if let Err(r) = require_org_member(pool, org_id, &claims.account_id).await {
        return r.into_response();
    }

    // Get the service node
    let service = sqlx::query_as::<_, (String, String, serde_json::Value, String)>(
        "SELECT id, node_type, data, name FROM infra_map_nodes WHERE id = $1 AND org_id = $2"
    )
    .bind(&service_id)
    .bind(org_id)
    .fetch_optional(pool)
    .await;

    let (svc_id, _, svc_data, svc_name) = match service {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "ok": false, "error": "Service not found"
            }))).into_response();
        }
        Err(e) => {
            log::error!("DB error: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "ok": false, "error": "Internal error"
            }))).into_response();
        }
    };

    // Get all edges from/to this service
    let edges = sqlx::query_as::<_, (String, String, String, String, serde_json::Value)>(
        "SELECT id, from_id, to_id, rel_type, metadata FROM infra_map_edges WHERE org_id = $1 AND (from_id = $2 OR to_id = $2)"
    )
    .bind(org_id)
    .bind(&service_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Collect all connected node IDs
    let connected_ids: Vec<String> = edges.iter()
        .flat_map(|(_, from, to, _, _)| vec![from.clone(), to.clone()])
        .filter(|id| id != &service_id)
        .collect();

    // Get connected nodes (without revealing secret values!)
    let connected_nodes = if !connected_ids.is_empty() {
        let placeholders: Vec<String> = connected_ids.iter().enumerate()
            .map(|(i, _)| format!("${}", i + 2))
            .collect();
        let q = format!(
            "SELECT id, node_type, data, name FROM infra_map_nodes WHERE org_id = $1 AND id IN ({})",
            placeholders.join(",")
        );

        let mut query = sqlx::query_as::<_, (String, String, serde_json::Value, String)>(&q).bind(org_id);
        for id in &connected_ids {
            query = query.bind(id);
        }
        query.fetch_all(pool).await.unwrap_or_default()
    } else {
        vec![]
    };

    // Build topology (mask secret values)
    let nodes: Vec<serde_json::Value> = std::iter::once(serde_json::json!({
        "id": svc_id,
        "type": "service",
        "name": svc_name,
        "data": mask_sensitive_data(&svc_data),
    }))
    .chain(connected_nodes.into_iter().map(|(id, ntype, data, name)| {
        serde_json::json!({
            "id": id,
            "type": ntype,
            "name": name,
            "data": mask_sensitive_data(&data),
        })
    }))
    .collect();

    let edges_json: Vec<serde_json::Value> = edges.into_iter().map(|(id, from, to, rel, meta)| {
        serde_json::json!({
            "id": id,
            "from": from,
            "to": to,
            "relation": rel,
            "metadata": meta,
        })
    }).collect();

    // Generate human-readable answer for agent
    let answer = generate_topology_answer(&svc_name, &edges_json);

    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "service": svc_name,
        "nodes": nodes,
        "edges": edges_json,
        "answer": answer,
    }))).into_response()
}

/// GET /api/orgs/{org_id}/map/service/{service_id}/secrets
/// Get secret references for a service (names and types only — no values!)
pub async fn service_secrets(
    State(state): State<AppState>,
    Path((org_id, service_id)): Path<(Uuid, String)>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(r) => return r.into_response(),
    };

    if let Err(r) = require_org_member(pool, org_id, &claims.account_id).await {
        return r.into_response();
    }

    // Find secret_ref nodes connected to this service
    let secrets = sqlx::query_as::<_, (String, String, serde_json::Value, String)>(
        r#"SELECT n.id, n.node_type, n.data, n.name
           FROM infra_map_nodes n
           JOIN infra_map_edges e ON e.to_id = n.id AND e.rel_type = 'SERVICE_HAS_SECRET'
           WHERE e.org_id = $1 AND e.from_id = $2 AND n.node_type = 'secret_ref'"#
    )
    .bind(org_id)
    .bind(&service_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let secret_refs: Vec<serde_json::Value> = secrets.into_iter().map(|(id, _, data, name)| {
        serde_json::json!({
            "id": id,
            "key_name": name,
            "secret_type": data.get("secret_type").and_then(|v| v.as_str()).unwrap_or("unknown"),
            "vault_path": data.get("vault_path").and_then(|v| v.as_str()),
            // NOTE: no value! Agent only knows the name and type
        })
    }).collect();

    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "secrets": secret_refs,
    }))).into_response()
}

/// GET /api/orgs/{org_id}/map/summary
/// High-level infrastructure summary for the org
pub async fn map_summary(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(r) => return r.into_response(),
    };

    if let Err(r) = require_org_member(pool, org_id, &claims.account_id).await {
        return r.into_response();
    }

    // Count nodes by type
    let counts = sqlx::query_as::<_, (String, i64)>(
        "SELECT node_type, COUNT(*) FROM infra_map_nodes WHERE org_id = $1 GROUP BY node_type"
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let by_type: HashMap<String, i64> = counts.into_iter().collect();

    // Count edges by type
    let edge_counts = sqlx::query_as::<_, (String, i64)>(
        "SELECT rel_type, COUNT(*) FROM infra_map_edges WHERE org_id = $1 GROUP BY rel_type"
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let edges_by_type: HashMap<String, i64> = edge_counts.into_iter().collect();

    // List environments
    let envs = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT environment FROM infra_map_nodes WHERE org_id = $1 AND environment IS NOT NULL ORDER BY environment"
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "nodes_by_type": by_type,
        "edges_by_type": edges_by_type,
        "environments": envs,
        "total_nodes": by_type.values().sum::<i64>(),
        "total_edges": edges_by_type.values().sum::<i64>(),
    }))).into_response()
}

/// Mask sensitive fields in node data (never expose secret values in map)
fn mask_sensitive_data(data: &serde_json::Value) -> serde_json::Value {
    if let Some(obj) = data.as_object() {
        let mut masked = obj.clone();
        // Remove any value-like fields
        masked.remove("value");
        masked.remove("password");
        masked.remove("token");
        masked.remove("secret");
        masked.remove("dsn");
        masked.remove("connection_string");
        serde_json::Value::Object(masked)
    } else {
        data.clone()
    }
}

/// Generate human-readable topology answer for AI agent
fn generate_topology_answer(service_name: &str, edges: &[serde_json::Value]) -> String {
    if edges.is_empty() {
        return format!("Service '{service_name}' has no known dependencies.");
    }

    let mut parts = vec![format!("Service '{service_name}' topology:")];
    let mut databases = vec![];
    let mut queues = vec![];
    let mut endpoints = vec![];
    let mut secrets = vec![];
    let mut hosts = vec![];
    let mut monitors = vec![];

    for edge in edges {
        let rel = edge.get("relation").and_then(|v| v.as_str()).unwrap_or("");
        match rel {
            "SERVICE_USES_DB" => databases.push(edge.get("to").and_then(|v| v.as_str()).unwrap_or("?")),
            "SERVICE_USES_QUEUE" => queues.push(edge.get("to").and_then(|v| v.as_str()).unwrap_or("?")),
            "SERVICE_EXPOSES_API" => endpoints.push(edge.get("to").and_then(|v| v.as_str()).unwrap_or("?")),
            "SERVICE_HAS_SECRET" => secrets.push(edge.get("to").and_then(|v| v.as_str()).unwrap_or("?")),
            "HOSTS_SERVICE" => hosts.push(edge.get("from").and_then(|v| v.as_str()).unwrap_or("?")),
            "SERVICE_MONITORED_BY" => monitors.push(edge.get("to").and_then(|v| v.as_str()).unwrap_or("?")),
            _ => {}
        }
    }

    if !hosts.is_empty() { parts.push(format!("  Runs on: {}", hosts.join(", "))); }
    if !databases.is_empty() { parts.push(format!("  Uses databases: {}", databases.join(", "))); }
    if !queues.is_empty() { parts.push(format!("  Uses queues: {}", queues.join(", "))); }
    if !endpoints.is_empty() { parts.push(format!("  Exposes endpoints: {}", endpoints.join(", "))); }
    if !secrets.is_empty() { parts.push(format!("  Requires {} secret(s) (names available, values hidden)", secrets.len())); }
    if !monitors.is_empty() { parts.push(format!("  Monitored by: {}", monitors.join(", "))); }

    parts.join("\n")
}

// Health Monitor API — real-time infrastructure health
// GET /api/orgs/{org_id}/health — full health snapshot
// SSE endpoint for real-time updates (future)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use uuid::Uuid;

use crate::auth::Claims;
use crate::server::AppState;

/// GET /api/orgs/{org_id}/health
/// Get real-time infrastructure health snapshot
pub async fn health_snapshot(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let pool = match state.db.as_ref().map(|p| &p.write_pool).ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"ok": false, "error": "Database unavailable"})),
        )
    }) {
        Ok(p) => p,
        Err(r) => return r.into_response(),
    };

    // Verify org membership
    let role = sqlx::query_scalar::<_, String>(
        "SELECT role FROM org_members WHERE org_id = $1 AND account_id = $2"
    )
    .bind(org_id)
    .bind(&claims.account_id)
    .fetch_optional(pool)
    .await;

    match role {
        Ok(Some(_)) => {},
        _ => {
            return (StatusCode::FORBIDDEN, Json(serde_json::json!({
                "ok": false, "error": "Not authorized"
            }))).into_response();
        }
    }

    // Get node health from infra_map_nodes + recent audit events
    let nodes = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>)>(
        "SELECT id, node_type, name, environment, criticality FROM infra_map_nodes WHERE org_id = $1"
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Count recent audit events per service (last 1 hour)
    let recent_events = sqlx::query_as::<_, (String, i64)>(
        r#"SELECT 
            COALESCE(metadata->>'agent_id', metadata->>'scan_id', 'system') as source,
            COUNT(*) as cnt
           FROM audit_events 
           WHERE org_id = $1 
             AND created_at > NOW() - INTERVAL '1 hour'
           GROUP BY source"#
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let blocked_count = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM audit_events 
           WHERE org_id = $1 
             AND event_type = 'command_blocked'
             AND created_at > NOW() - INTERVAL '1 hour'"#
    )
    .bind(org_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let alert_events = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM audit_events 
           WHERE org_id = $1 
             AND (event_type = 'command_blocked' OR event_type LIKE '%anomal%' OR event_type LIKE '%crash%')
             AND created_at > NOW() - INTERVAL '1 hour'"#
    )
    .bind(org_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let event_counts: std::collections::HashMap<String, i64> = recent_events.into_iter().collect();

    // Build health per node
    let mut healthy = 0u64;
    let mut degraded = 0u64;
    let mut alert = 0u64;
    let mut unknown = 0u64;

    let node_health: Vec<serde_json::Value> = nodes.into_iter().map(|(id, ntype, name, env, crit)| {
        let event_count = event_counts.get(&id).copied().unwrap_or(0);
        
        // Determine health based on events
        let (status, status_color) = if alert_events > 0 && event_count > 5 {
            alert += 1;
            ("alert", "rose")
        } else if blocked_count > 0 && event_count > 2 {
            degraded += 1;
            ("degraded", "amber")
        } else if event_count > 0 {
            healthy += 1;
            ("healthy", "emerald")
        } else {
            unknown += 1;
            ("unknown", "gray")
        };

        serde_json::json!({
            "id": id,
            "type": ntype,
            "name": name,
            "environment": env,
            "criticality": crit,
            "status": status,
            "status_color": status_color,
            "events_1h": event_count,
        })
    }).collect();

    let total = healthy + degraded + alert + unknown;

    // Recent events for feed
    let recent_feed = sqlx::query_as::<_, (String, String, String, Option<String>, String)>(
        r#"SELECT event_type, 
                  COALESCE(agent_id, '') as agent_id,
                  created_at::text,
                  metadata->>'command' as command,
                  CASE 
                    WHEN event_type LIKE '%block%' OR event_type LIKE '%reject%' THEN 'warning'
                    WHEN event_type LIKE '%crash%' OR event_type LIKE '%error%' THEN 'error'
                    ELSE 'info'
                  END as severity
           FROM audit_events 
           WHERE org_id = $1 
           ORDER BY created_at DESC LIMIT 20"#
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let events_feed: Vec<serde_json::Value> = recent_feed.into_iter().map(|(etype, agent, ts, cmd, sev)| {
        serde_json::json!({
            "type": etype,
            "agent_id": agent,
            "timestamp": ts,
            "command": cmd,
            "severity": sev,
        })
    }).collect();

    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "total_nodes": total,
        "healthy": healthy,
        "degraded": degraded,
        "alert": alert,
        "unknown": unknown,
        "blocked_commands_1h": blocked_count,
        "alert_events_1h": alert_events,
        "nodes": node_health,
        "recent_events": events_feed,
    }))).into_response()
}

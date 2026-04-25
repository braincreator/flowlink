//! Service Catalog & Ownership — live service catalog for business users.

use axum::{
    extract::{Query, State},
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
        Some(id) => {
            let uuid: Uuid = id.parse().unwrap_or_default();
            Ok((id.clone(), uuid))
        }
        None => Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "No org"})))),
    }
}

fn require_pool(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, Json<serde_json::Value>)> {
    state.db.as_ref().map(|db| db.pool()).ok_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "DB unavailable"})))
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCatalogEntry {
    pub node_id: String,
    pub name: String,
    pub service_type: String,
    pub environment: String,
    pub criticality: String,
    pub owner: Option<String>,
    pub team: Option<String>,
    pub sla_tier: Option<String>,
    pub data_sensitivity: Option<String>,
    pub dependencies_count: usize,
    pub dependents_count: usize,
    pub health_status: String,
    pub last_incident: Option<DateTime<Utc>>,
    pub agent_interactions_24h: i64,
    pub risk_score: f64,
    pub labels: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSummary {
    pub total_services: i64,
    pub by_environment: serde_json::Value,
    pub by_criticality: serde_json::Value,
    pub by_health: serde_json::Value,
    pub by_owner: Vec<OwnerSummary>,
    pub at_risk_count: i64,
    pub no_owner_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerSummary {
    pub owner: String,
    pub service_count: i64,
    pub critical_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct CatalogQuery {
    pub environment: Option<String>,
    pub criticality: Option<String>,
    pub owner: Option<String>,
    pub search: Option<String>,
    pub health: Option<String>,
}

/// GET /api/v1/catalog/services
pub async fn list_catalog(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(_params): Query<CatalogQuery>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (_org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    let nodes: Vec<(String, String, String, String, String, Option<String>, serde_json::Value)> =
        sqlx::query_as(
            "SELECT id, name, node_type, COALESCE(environment, 'unknown'), COALESCE(criticality, 'medium'), owner, labels FROM infra_map_nodes WHERE org_id = $1 ORDER BY name LIMIT 200"
        )
        .bind(org_uuid)
        .fetch_all(pool).await.unwrap_or_default();

    let mut entries = Vec::new();
    for (id, name, ntype, env, crit, owner, labels) in &nodes {
        let deps: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM infra_map_edges WHERE org_id = $1 AND from_id = $2"
        ).bind(org_uuid).bind(id).fetch_one(pool).await.unwrap_or(0);

        let dependents: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM infra_map_edges WHERE org_id = $1 AND to_id = $2"
        ).bind(org_uuid).bind(id).fetch_one(pool).await.unwrap_or(0);

        let agent_hits: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM audit_log WHERE org_id = $1 AND target = $2 AND timestamp > NOW() - INTERVAL '24 hours'"
        ).bind(org_uuid).bind(name).fetch_one(pool).await.unwrap_or(0);

        let risk = compute_service_risk(deps as f64, dependents as f64, crit, agent_hits);

        let errors: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM audit_log WHERE org_id = $1 AND target = $2 AND level = 'error' AND timestamp > NOW() - INTERVAL '1 hour'"
        ).bind(org_uuid).bind(name).fetch_one(pool).await.unwrap_or(0);

        let health = if errors > 5 { "critical" } else if errors > 2 { "degraded" } else if agent_hits > 0 { "active" } else { "healthy" };

        let team = labels.get("team").and_then(|v| v.as_str()).map(String::from);
        let sla_tier = labels.get("sla").and_then(|v| v.as_str()).map(String::from);
        let data_sensitivity = labels.get("data_sensitivity").and_then(|v| v.as_str()).map(String::from);

        entries.push(ServiceCatalogEntry {
            node_id: id.clone(), name: name.clone(), service_type: ntype.clone(),
            environment: env.clone(), criticality: crit.clone(), owner: owner.clone(),
            team, sla_tier, data_sensitivity,
            dependencies_count: deps as usize, dependents_count: dependents as usize,
            health_status: health.to_string(), last_incident: None,
            agent_interactions_24h: agent_hits, risk_score: risk, labels: labels.clone(),
        });
    }

    Json(serde_json::json!({"services": entries, "total": entries.len()})).into_response()
}

/// GET /api/v1/catalog/summary
pub async fn catalog_summary(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (_org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM infra_map_nodes WHERE org_id = $1")
        .bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let by_env: Vec<(String, i64)> = sqlx::query_as(
        "SELECT COALESCE(environment, 'unknown'), count(*) FROM infra_map_nodes WHERE org_id = $1 GROUP BY environment"
    ).bind(org_uuid).fetch_all(pool).await.unwrap_or_default();
    let by_environment = serde_json::to_value(by_env.into_iter().collect::<std::collections::HashMap<String,i64>>()).unwrap_or(serde_json::json!({}));

    let by_crit: Vec<(String, i64)> = sqlx::query_as(
        "SELECT COALESCE(criticality, 'medium'), count(*) FROM infra_map_nodes WHERE org_id = $1 GROUP BY criticality"
    ).bind(org_uuid).fetch_all(pool).await.unwrap_or_default();
    let by_criticality = serde_json::to_value(by_crit.into_iter().collect::<std::collections::HashMap<String,i64>>()).unwrap_or(serde_json::json!({}));

    let error_targets: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT target) FROM audit_log WHERE org_id = $1 AND level = 'error' AND timestamp > NOW() - INTERVAL '1 hour'"
    ).bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let by_owner: Vec<OwnerSummary> = sqlx::query_as::<_, (Option<String>, i64, i64)>(
        "SELECT owner, count(*), count(*) FILTER (WHERE criticality IN ('high', 'critical')) FROM infra_map_nodes WHERE org_id = $1 GROUP BY owner"
    ).bind(org_uuid).fetch_all(pool).await.unwrap_or_default()
    .into_iter()
    .map(|(owner, cnt, crit_cnt)| OwnerSummary {
        owner: owner.unwrap_or_else(|| "unassigned".into()),
        service_count: cnt, critical_count: crit_cnt,
    }).collect();

    let at_risk: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT target) FROM audit_log WHERE org_id = $1 AND level = 'error' AND timestamp > NOW() - INTERVAL '24 hours'"
    ).bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let no_owner: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM infra_map_nodes WHERE org_id = $1 AND (owner IS NULL OR owner = '')"
    ).bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let summary = CatalogSummary {
        total_services: total,
        by_environment,
        by_criticality,
        by_health: serde_json::json!({"healthy": total - error_targets, "degraded": error_targets}),
        by_owner,
        at_risk_count: at_risk,
        no_owner_count: no_owner,
    };

    Json(summary).into_response()
}

#[derive(Debug, Deserialize)]
pub struct EfficiencyQuery {
    pub days: Option<i64>,
}

/// GET /api/v1/catalog/efficiency
/// Cost & efficiency insights — where agents save or waste time.
pub async fn efficiency_insights(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<EfficiencyQuery>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (_org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    let days = params.days.unwrap_or(7);
    let since = Utc::now() - chrono::Duration::days(days);

    let agent_stats: Vec<serde_json::Value> = sqlx::query_as::<_, (String, i64, i64, i64, f64)>(
        "SELECT agent_id, count(*) as total, count(*) FILTER (WHERE exit_code = 0) as successes, count(*) FILTER (WHERE shield_result = 'blocked') as blocked, COALESCE(avg(duration_ms), 0) as avg_duration FROM command_history WHERE org_id = $1 AND executed_at > $2 GROUP BY agent_id ORDER BY total DESC LIMIT 20"
    )
    .bind(org_uuid).bind(since)
    .fetch_all(pool).await.unwrap_or_default()
    .into_iter()
    .map(|(aid, total, ok, blocked, avg_ms)| {
        let rate = if total > 0 { ok as f64 / total as f64 * 100.0 } else { 100.0 };
        let eff = if total > 0 { ((ok - blocked).max(0) as f64 / total as f64 * 100.0) as u8 } else { 100 };
        serde_json::json!({"agent_id": aid, "total_commands": total, "successful": ok, "blocked": blocked, "success_rate": format!("{:.1}%", rate), "avg_duration_ms": avg_ms, "efficiency_score": eff})
    }).collect();

    let total_commands: i64 = agent_stats.iter().filter_map(|v| v.get("total_commands").and_then(|v| v.as_i64())).sum();
    let time_saved_hours = (total_commands as f64 * 0.25).round();

    let service_stats: Vec<serde_json::Value> = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT COALESCE(target, 'unknown'), count(*), count(*) FILTER (WHERE level IN ('warn', 'error')) FROM audit_log WHERE org_id = $1 AND timestamp > $2 AND target IS NOT NULL GROUP BY target ORDER BY count(*) DESC LIMIT 15"
    )
    .bind(org_uuid).bind(since)
    .fetch_all(pool).await.unwrap_or_default()
    .into_iter()
    .map(|(target, hits, errors)| {
        let rate = if hits > 0 { format!("{:.1}%", errors as f64 / hits as f64 * 100.0) } else { "0%".into() };
        serde_json::json!({"service": target, "interactions": hits, "errors": errors, "error_rate": rate})
    }).collect();

    Json(serde_json::json!({
        "period_days": days,
        "total_commands": total_commands,
        "estimated_time_saved_hours": time_saved_hours,
        "agent_efficiency": agent_stats,
        "service_interactions": service_stats,
        "generated_at": Utc::now(),
    })).into_response()
}

fn compute_service_risk(deps: f64, dependents: f64, criticality: &str, agent_hits: i64) -> f64 {
    let crit_mult = match criticality { "critical" => 3.0, "high" => 2.0, "medium" => 1.0, _ => 0.5 };
    (deps + dependents) * crit_mult + (agent_hits as f64 * 0.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_risk_critical_high_deps() {
        let risk = compute_service_risk(10.0, 5.0, "critical", 20);
        assert!(risk > 40.0, "critical with many deps should be high risk");
    }

    #[test]
    fn test_service_risk_low() {
        let risk = compute_service_risk(0.0, 0.0, "low", 0);
        assert!(risk < 1.0, "isolated low-criticality service should have near-zero risk");
    }

    #[test]
    fn test_service_risk_scales_with_agents() {
        let r1 = compute_service_risk(5.0, 5.0, "medium", 0);
        let r2 = compute_service_risk(5.0, 5.0, "medium", 100);
        assert!(r2 > r1, "more agent interactions should increase risk");
    }

    #[test]
    fn test_service_risk_criticality_multiplier() {
        let r_low = compute_service_risk(5.0, 5.0, "low", 0);
        let r_med = compute_service_risk(5.0, 5.0, "medium", 0);
        let r_high = compute_service_risk(5.0, 5.0, "high", 0);
        let r_crit = compute_service_risk(5.0, 5.0, "critical", 0);
        assert!(r_low < r_med);
        assert!(r_med < r_high);
        assert!(r_high < r_crit);
    }
}

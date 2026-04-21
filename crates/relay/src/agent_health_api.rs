use sqlx::Row;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::server::AppState;

fn gp(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, String)> {
    state.db.as_ref().map(|db| db.pool()).ok_or((StatusCode::SERVICE_UNAVAILABLE, "Database not configured".to_string()))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthMetric {
    pub agent_id: String,
    pub cpu_percent: Option<f32>,
    pub ram_percent: Option<f32>,
    pub disk_percent: Option<f32>,
    pub load_avg_1m: Option<f32>,
    pub uptime_seconds: Option<i64>,
    pub reported_at: String,
}

#[derive(Debug, Serialize)]
pub struct HealthTimePoint {
    pub reported_at: String,
    pub cpu_percent: Option<f32>,
    pub ram_percent: Option<f32>,
    pub disk_percent: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<i64>,
}

pub async fn get_latest(
    State(state): State<AppState>,
    axum::extract::Extension(_claims): axum::extract::Extension<crate::auth::Claims>,
    Path(agent_id): Path<String>,
) -> Result<(StatusCode, Json<HealthMetric>), (StatusCode, String)> {
    let row = sqlx::query(
        "SELECT agent_id, cpu_percent, ram_percent, disk_percent, load_avg_1m, uptime_seconds, reported_at::text FROM agent_health_metrics WHERE agent_id = $1 ORDER BY reported_at DESC LIMIT 1"
    ).bind(&agent_id).fetch_optional(gp(&state)?).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match row {
        Some(r) => Ok((StatusCode::OK, Json(HealthMetric {
            agent_id: r.get("agent_id"), cpu_percent: r.get("cpu_percent"),
            ram_percent: r.get("ram_percent"), disk_percent: r.get("disk_percent"),
            load_avg_1m: r.get("load_avg_1m"), uptime_seconds: r.get("uptime_seconds"),
            reported_at: r.get("reported_at"),
        }))),
        None => Err((StatusCode::NOT_FOUND, "No health metrics for this agent".into())),
    }
}

pub async fn get_timeseries(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(q): Query<MetricsQuery>,
) -> Result<(StatusCode, Json<Vec<HealthTimePoint>>), (StatusCode, String)> {
    let limit = q.limit.unwrap_or(100).min(1000);

    let rows = sqlx::query(
        "SELECT reported_at::text, cpu_percent, ram_percent, disk_percent FROM agent_health_metrics WHERE agent_id = $1 AND ($2::text IS NULL OR reported_at >= $2::timestamptz) AND ($3::text IS NULL OR reported_at <= $3::timestamptz) ORDER BY reported_at DESC LIMIT $4"
    ).bind(&agent_id).bind(&q.from).bind(&q.to).bind(limit)
    .fetch_all(gp(&state)?).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let pts: Vec<HealthTimePoint> = rows.iter().map(|r| HealthTimePoint {
        reported_at: r.get("reported_at"), cpu_percent: r.get("cpu_percent"),
        ram_percent: r.get("ram_percent"), disk_percent: r.get("disk_percent"),
    }).collect();

    Ok((StatusCode::OK, Json(pts)))
}

/// Store health metrics (called by agent via WS)
pub async fn store_metrics(pool: &sqlx::PgPool, metric: &HealthMetric) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO agent_health_metrics (agent_id, cpu_percent, ram_percent, disk_percent, load_avg_1m, uptime_seconds) VALUES ($1, $2, $3, $4, $5, $6)"
    ).bind(&metric.agent_id).bind(metric.cpu_percent).bind(metric.ram_percent)
    .bind(metric.disk_percent).bind(metric.load_avg_1m).bind(metric.uptime_seconds)
    .execute(pool).await.map_err(|e| e.to_string())?;

    // Keep only last 7 days
    sqlx::query("DELETE FROM agent_health_metrics WHERE agent_id = $1 AND reported_at < NOW() - INTERVAL '7 days'")
        .bind(&metric.agent_id).execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct AgentHealthOverview {
    pub agent_id: String,
    pub status: String,
    pub cpu_percent: Option<f32>,
    pub ram_percent: Option<f32>,
    pub disk_percent: Option<f32>,
    pub last_report: Option<String>,
}

pub async fn overview(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<Vec<AgentHealthOverview>>), (StatusCode, String)> {
    let rows = sqlx::query(
        "SELECT agent_id, cpu_percent, ram_percent, disk_percent, reported_at::text as last_report FROM (SELECT DISTINCT ON (agent_id) agent_id, cpu_percent, ram_percent, disk_percent, reported_at FROM agent_health_metrics ORDER BY agent_id, reported_at DESC) sub ORDER BY agent_id"
    ).fetch_all(gp(&state)?).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let overview: Vec<AgentHealthOverview> = rows.iter().map(|r| {
        let cpu: Option<f32> = r.get("cpu_percent");
        let ram: Option<f32> = r.get("ram_percent");
        let disk: Option<f32> = r.get("disk_percent");
        let status = if cpu.unwrap_or(0.0) > 90.0 || ram.unwrap_or(0.0) > 90.0 || disk.unwrap_or(0.0) > 90.0 {
            "critical"
        } else if cpu.unwrap_or(0.0) > 75.0 || ram.unwrap_or(0.0) > 75.0 || disk.unwrap_or(0.0) > 75.0 {
            "warning"
        } else {
            "healthy"
        };
        AgentHealthOverview {
            agent_id: r.get("agent_id"), status: status.to_string(),
            cpu_percent: cpu, ram_percent: ram, disk_percent: disk,
            last_report: r.get("last_report"),
        }
    }).collect();

    Ok((StatusCode::OK, Json(overview)))
}

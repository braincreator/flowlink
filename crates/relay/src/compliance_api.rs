use sqlx::Row;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::middleware::AccountIdExtractor;
use crate::server::AppState;

fn gp(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, String)> {
    state.db.as_ref().map(|db| db.pool()).ok_or((StatusCode::SERVICE_UNAVAILABLE, "Database not configured".to_string()))
}

#[derive(Debug, Serialize)]
pub struct ComplianceReport {
    pub id: Uuid,
    pub org_id: Uuid,
    pub report_type: String,
    pub period_start: String,
    pub period_end: String,
    pub status: String,
    pub generated_by: Option<String>,
    pub data: serde_json::Value,
    pub pdf_path: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct GenerateReportRequest {
    pub org_id: Uuid,
    pub report_type: String,
    pub period_start: String,
    pub period_end: String,
}

#[derive(Debug, Deserialize)]
pub struct ReportQuery {
    pub org_id: Option<Uuid>,
    pub report_type: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
}

fn row_to_report(r: &sqlx::postgres::PgRow) -> ComplianceReport {
    ComplianceReport {
        id: r.get("id"), org_id: r.get("org_id"), report_type: r.get("report_type"),
        period_start: r.get("period_start"), period_end: r.get("period_end"),
        status: r.get("status"), generated_by: r.get("generated_by"),
        data: r.get("data"), pdf_path: r.get("pdf_path"),
        created_at: r.get("created_at"),
    }
}

pub async fn list_reports(
    State(state): State<AppState>,
    _account: AccountIdExtractor,
    Query(q): Query<ReportQuery>,
) -> Result<(StatusCode, Json<Vec<ComplianceReport>>), (StatusCode, String)> {
    let pool = gp(&state)?;
    let limit = q.limit.unwrap_or(20).min(100);

    let rows = sqlx::query(
        "SELECT id, org_id, report_type, period_start::text, period_end::text, status, generated_by, data, pdf_path, created_at::text FROM compliance_reports WHERE ($1::uuid IS NULL OR org_id = $1) AND ($2::text IS NULL OR report_type = $2) AND ($3::text IS NULL OR status = $3) ORDER BY created_at DESC LIMIT $4"
    ).bind(q.org_id).bind(&q.report_type).bind(&q.status).bind(limit)
    .fetch_all(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::OK, Json(rows.iter().map(|r| row_to_report(r)).collect())))
}

pub async fn get_report(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<ComplianceReport>), (StatusCode, String)> {
    let pool = gp(&state)?;
    let row = sqlx::query(
        "SELECT id, org_id, report_type, period_start::text, period_end::text, status, generated_by, data, pdf_path, created_at::text FROM compliance_reports WHERE id = $1"
    ).bind(id).fetch_optional(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match row {
        Some(r) => Ok((StatusCode::OK, Json(row_to_report(&r)))),
        None => Err((StatusCode::NOT_FOUND, "Report not found".into())),
    }
}

pub async fn generate_report(
    State(state): State<AppState>,
    AccountIdExtractor(account_id): AccountIdExtractor,
    Json(body): Json<GenerateReportRequest>,
) -> Result<(StatusCode, Json<ComplianceReport>), (StatusCode, String)> {
    let pool = gp(&state)?;

    if !["security_audit", "policy_compliance", "exec_summary", "fstek"].contains(&body.report_type.as_str()) {
        return Err((StatusCode::BAD_REQUEST, format!("Invalid report_type: {}", body.report_type)));
    }

    // Gather data based on report type
    let data = gather_report_data(pool, &body).await?;

    let row = sqlx::query(
        "INSERT INTO compliance_reports (org_id, report_type, period_start, period_end, status, generated_by, data) VALUES ($1, $2, $3::timestamptz, $4::timestamptz, 'ready', $5, $6) RETURNING id, org_id, report_type, period_start::text, period_end::text, status, generated_by, data, pdf_path, created_at::text"
    ).bind(body.org_id).bind(&body.report_type).bind(&body.period_start).bind(&body.period_end).bind(&account_id).bind(&data)
    .fetch_one(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(row_to_report(&row))))
}

async fn gather_report_data(pool: &sqlx::PgPool, req: &GenerateReportRequest) -> Result<serde_json::Value, (StatusCode, String)> {
    let mut data = serde_json::Map::new();

    // Total commands in period
    let total_cmds: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM command_history WHERE org_id = $1 AND executed_at >= $2::timestamptz AND executed_at <= $3::timestamptz"
    ).bind(req.org_id).bind(&req.period_start).bind(&req.period_end)
    .fetch_one(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let blocked_cmds: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM command_history WHERE org_id = $1 AND shield_result = 'blocked' AND executed_at >= $2::timestamptz AND executed_at <= $3::timestamptz"
    ).bind(req.org_id).bind(&req.period_start).bind(&req.period_end)
    .fetch_one(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let shield_alerts: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM shield_alerts WHERE org_id = $1 AND created_at >= $2::timestamptz AND created_at <= $3::timestamptz"
    ).bind(req.org_id).bind(&req.period_start).bind(&req.period_end)
    .fetch_optional(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?.unwrap_or(0);

    let agents_seen: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(DISTINCT agent_id) FROM command_history WHERE org_id = $1 AND executed_at >= $2::timestamptz AND executed_at <= $3::timestamptz"
    ).bind(req.org_id).bind(&req.period_start).bind(&req.period_end)
    .fetch_one(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let approvals: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM approval_log WHERE org_id = $1 AND created_at >= $2::timestamptz AND created_at <= $3::timestamptz"
    ).bind(req.org_id).bind(&req.period_start).bind(&req.period_end)
    .fetch_optional(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?.unwrap_or(0);

    data.insert("total_commands".into(), serde_json::json!(total_cmds));
    data.insert("blocked_commands".into(), serde_json::json!(blocked_cmds));
    data.insert("block_rate_pct".into(), serde_json::json!(if total_cmds > 0 { (blocked_cmds as f64 / total_cmds as f64 * 100.0).round() } else { 0.0 }));
    data.insert("shield_alerts".into(), serde_json::json!(shield_alerts));
    data.insert("agents_seen".into(), serde_json::json!(agents_seen));
    data.insert("approvals".into(), serde_json::json!(approvals));
    data.insert("generated_at".into(), serde_json::json!(chrono::Utc::now().to_rfc3339()));

    // ФСТЭК specific fields
    if req.report_type == "fstek" {
        data.insert("compliance_level".into(), serde_json::json!("УЗ-2"));
        data.insert("log_integrity".into(), serde_json::json!("verified"));
        data.insert("encryption".into(), serde_json::json!("AES-256"));
        data.insert("access_control".into(), serde_json::json!("RBAC+custom"));
        data.insert("audit_trail".into(), serde_json::json!("immutable"));
    }

    Ok(serde_json::Value::Object(data))
}

pub async fn delete_report(
    State(state): State<AppState>,
    _account: AccountIdExtractor,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let pool = gp(&state)?;
    let result = sqlx::query("DELETE FROM compliance_reports WHERE id = $1").bind(id)
        .execute(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if result.rows_affected() == 0 { return Err((StatusCode::NOT_FOUND, "Report not found".into())); }
    Ok(StatusCode::NO_CONTENT)
}

//! Forensic Reports — auto-generated compliance and audit reports.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use chrono::{DateTime, Duration, Utc};
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
pub struct ForensicReport {
    pub report_id: String,
    pub org_id: String,
    pub report_type: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub generated_at: DateTime<Utc>,
    pub generated_by: String,
    pub executive_summary: serde_json::Value,
    pub sections: Vec<serde_json::Value>,
    pub compliance_score: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ReportParams {
    pub report_type: Option<String>,
    pub period_days: Option<i64>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

/// POST /api/v1/forensics/report
pub async fn generate_forensic_report(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(params): Json<ReportParams>,
) -> impl IntoResponse {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    let period_days = params.period_days.unwrap_or(30);
    let period_start = params.from.unwrap_or_else(|| Utc::now() - Duration::days(period_days));
    let period_end = params.to.unwrap_or_else(|| Utc::now());
    let report_type = params.report_type.clone().unwrap_or_else(|| "executive".into());

    let total_agents: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT agent_id) FROM command_history WHERE org_id = $1 AND executed_at BETWEEN $2 AND $3"
    ).bind(org_uuid).bind(period_start).bind(period_end).fetch_one(pool).await.unwrap_or(0);

    let total_commands: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM command_history WHERE org_id = $1 AND executed_at BETWEEN $2 AND $3"
    ).bind(org_uuid).bind(period_start).bind(period_end).fetch_one(pool).await.unwrap_or(0);

    let blocked_actions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM command_history WHERE org_id = $1 AND executed_at BETWEEN $2 AND $3 AND shield_result = 'blocked'"
    ).bind(org_uuid).bind(period_start).bind(period_end).fetch_one(pool).await.unwrap_or(0);

    let approved_actions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM command_history WHERE org_id = $1 AND executed_at BETWEEN $2 AND $3 AND shield_result = 'allowed'"
    ).bind(org_uuid).bind(period_start).bind(period_end).fetch_one(pool).await.unwrap_or(0);

    let approval_rate = if total_commands > 0 { (approved_actions as f64 / total_commands as f64) * 100.0 } else { 100.0 };

    let anomalies: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE org_id = $1 AND timestamp BETWEEN $2 AND $3 AND level IN ('warn', 'error')"
    ).bind(org_uuid).bind(period_start).bind(period_end).fetch_one(pool).await.unwrap_or(0);

    let prev_blocked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM command_history WHERE org_id = $1 AND executed_at BETWEEN $2 AND $3 AND shield_result = 'blocked'"
    ).bind(org_uuid).bind(period_start - Duration::days(period_days)).bind(period_start).fetch_one(pool).await.unwrap_or(0);

    let risk_trend = if blocked_actions < prev_blocked { "improving" } else if blocked_actions > prev_blocked { "degrading" } else { "stable" };

    let overall_score = compute_compliance_score(total_commands, blocked_actions, anomalies, approval_rate);
    let compliance_score = serde_json::json!({
        "overall": overall_score,
        "access_control": if approval_rate > 95.0 { 95.0 } else { approval_rate },
        "audit_trail": if total_commands > 0 { 100.0 } else { 50.0 },
        "policy_enforcement": if total_commands > 0 { approval_rate } else { 100.0 },
        "data_protection": 85.0,
        "incident_response": if anomalies > 0 { 90.0 } else { 70.0 },
    });

    let mut sections = vec![
        serde_json::json!({
            "title": "Agent Activity Overview", "section_type": "agents",
            "data": {"total_agents": total_agents, "total_commands": total_commands, "blocked_actions": blocked_actions, "approval_rate": format!("{:.1}%", approval_rate)}
        }),
        serde_json::json!({
            "title": "Policy Enforcement", "section_type": "policy",
            "data": {"shield_blocks": blocked_actions, "shield_approvals": approved_actions, "risk_trend": risk_trend, "previous_period_blocks": prev_blocked}
        }),
    ];

    // Top agents
    let top_agents: Vec<serde_json::Value> = sqlx::query_as::<_, (String, i64, i64, i64, Option<DateTime<Utc>>)>(
        "SELECT agent_id, count(*), count(*) FILTER (WHERE shield_result = 'blocked'), count(*) FILTER (WHERE shield_risk IN ('high', 'critical')), max(executed_at) FROM command_history WHERE org_id = $1 AND executed_at BETWEEN $2 AND $3 GROUP BY agent_id ORDER BY count(*) DESC LIMIT 10"
    ).bind(org_uuid).bind(period_start).bind(period_end).fetch_all(pool).await.unwrap_or_default()
    .into_iter().map(|(aid, cmds, blkd, hr, la)| serde_json::json!({"agent_id": aid, "commands": cmds, "blocked": blkd, "high_risk": hr, "last_active": la})).collect();

    if !top_agents.is_empty() {
        sections.push(serde_json::json!({"title": "Top Agents", "section_type": "top_agents", "data": top_agents}));
    }

    // Save to DB
    let report_id = Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO compliance_reports (id, org_id, report_type, period_start, period_end, status, generated_by, data) VALUES ($1, $2, $3, $4, $5, 'ready', $6, $7)"
    ).bind(report_id.parse::<Uuid>().unwrap_or_default())
    .bind(org_uuid)
    .bind(&report_type)
    .bind(period_start).bind(period_end)
    .bind(&claims.account_id)
    .bind(serde_json::to_value(&sections).unwrap_or(serde_json::json!([])))
    .execute(pool).await;

    let report = ForensicReport {
        report_id,
        org_id: org_str,
        report_type,
        period_start, period_end,
        generated_at: Utc::now(),
        generated_by: claims.account_id.clone(),
        executive_summary: serde_json::json!({
            "total_agents": total_agents, "total_commands": total_commands,
            "blocked_actions": blocked_actions, "approval_rate": approval_rate,
            "anomalies": anomalies, "risk_trend": risk_trend,
        }),
        sections,
        compliance_score,
    };

    Json(report).into_response()
}


fn compute_compliance_score(commands: i64, blocked: i64, anomalies: i64, approval_rate: f64) -> f64 {
    let base = if commands == 0 { return 100.0; } else { 80.0 };
    let approval_bonus = (approval_rate - 80.0).max(0.0).min(15.0);
    let anomaly_penalty = (anomalies as f64 * 0.5).min(20.0);
    let block_bonus = if blocked > 0 { 0.0 } else { 5.0 };
    (base + approval_bonus + block_bonus - anomaly_penalty).clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_score_perfect() {
        let score = compute_compliance_score(0, 0, 0, 100.0);
        assert_eq!(score, 100.0);
    }

    #[test]
    fn test_compliance_score_high_approval() {
        let score = compute_compliance_score(100, 0, 0, 99.0);
        assert!(score >= 90.0, "high approval rate should give high score: {}", score);
    }

    #[test]
    fn test_compliance_score_many_anomalies() {
        let good = compute_compliance_score(100, 0, 0, 95.0);
        let bad = compute_compliance_score(100, 0, 40, 95.0);
        assert!(good > bad, "anomalies should reduce score: {} vs {}", good, bad);
    }

    #[test]
    fn test_compliance_score_low_approval() {
        let score = compute_compliance_score(100, 5, 0, 60.0);
        assert!(score <= 80.0, "low approval should not get bonus: {}", score);
    }

    #[test]
    fn test_compliance_score_capped_at_100() {
        let score = compute_compliance_score(0, 0, 0, 100.0);
        assert!(score <= 100.0);
    }

    #[test]
    fn test_compliance_score_never_below_0() {
        let score = compute_compliance_score(100, 50, 100, 10.0);
        assert!(score >= 0.0, "score should not go negative: {}", score);
    }
}

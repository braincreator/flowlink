use sqlx::Row;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::AppState;

fn gp(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, String)> {
    state.db.as_ref().map(|db| db.pool()).ok_or((StatusCode::SERVICE_UNAVAILABLE, "Database not configured".to_string()))
}

#[derive(Debug, Serialize)]
pub struct CommandHistoryEntry {
    pub id: Uuid,
    pub agent_id: String,
    pub command: String,
    pub args: String,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<i32>,
    pub shield_result: String,
    pub shield_risk: String,
    pub approval_id: Option<String>,
    pub account_id: Option<String>,
    pub executed_at: String,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub agent_id: Option<String>,
    pub org_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub shield_result: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list_history(
    State(state): State<AppState>,
    _claims: axum::extract::Extension<crate::auth::Claims>,
    Query(q): Query<HistoryQuery>,
) -> Result<(StatusCode, Json<Vec<CommandHistoryEntry>>), (StatusCode, String)> {
    let limit = q.limit.unwrap_or(50).min(200);
    let offset = q.offset.unwrap_or(0);

    let rows = sqlx::query(
        "SELECT id, agent_id, command, args, exit_code, duration_ms, shield_result, shield_risk, approval_id, account_id, executed_at::text FROM command_history WHERE ($1::text IS NULL OR agent_id = $1) AND ($2::uuid IS NULL OR org_id = $2::uuid) AND ($3::text IS NULL OR executed_at >= $3::timestamptz) AND ($4::text IS NULL OR executed_at <= $4::timestamptz) AND ($5::text IS NULL OR shield_result = $5) ORDER BY executed_at DESC LIMIT $6 OFFSET $7"
    )
    .bind(&q.agent_id).bind(&q.org_id).bind(&q.from).bind(&q.to).bind(&q.shield_result).bind(limit).bind(offset)
    .fetch_all(gp(&state)?).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let entries: Vec<CommandHistoryEntry> = rows.iter().map(|r| CommandHistoryEntry {
        id: r.get("id"),
        agent_id: r.get("agent_id"),
        command: r.get("command"),
        args: r.get::<Option<String>, _>("args").unwrap_or_default(),
        exit_code: r.get("exit_code"),
        duration_ms: r.get("duration_ms"),
        shield_result: r.get::<String, _>("shield_result"),
        shield_risk: r.get::<String, _>("shield_risk"),
        approval_id: r.get("approval_id"),
        account_id: r.get("account_id"),
        executed_at: r.get("executed_at"),
    }).collect();

    Ok((StatusCode::OK, Json(entries)))
}

pub async fn get_entry(
    State(state): State<AppState>,
    _claims: axum::extract::Extension<crate::auth::Claims>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<CommandHistoryEntry>), (StatusCode, String)> {
    let row = sqlx::query(
        "SELECT id, agent_id, command, args, exit_code, duration_ms, shield_result, shield_risk, approval_id, account_id, executed_at::text FROM command_history WHERE id = $1"
    ).bind(id).fetch_optional(gp(&state)?).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match row {
        Some(r) => Ok((StatusCode::OK, Json(CommandHistoryEntry {
            id: r.get("id"), agent_id: r.get("agent_id"), command: r.get("command"),
            args: r.get::<Option<String>, _>("args").unwrap_or_default(),
            exit_code: r.get("exit_code"), duration_ms: r.get("duration_ms"),
            shield_result: r.get("shield_result"), shield_risk: r.get("shield_risk"),
            approval_id: r.get("approval_id"), account_id: r.get("account_id"),
            executed_at: r.get("executed_at"),
        }))),
        None => Err((StatusCode::NOT_FOUND, "Command not found".into())),
    }
}

#[derive(Debug, Serialize)]
pub struct CommandStats {
    pub total_commands: i64,
    pub blocked_commands: i64,
    pub avg_duration_ms: Option<f64>,
    pub top_commands: Vec<TopCommand>,
}

#[derive(Debug, Serialize)]
pub struct TopCommand { pub command: String, pub count: i64, pub last_risk: String }

pub async fn command_stats(
    State(state): State<AppState>,
    _claims: axum::extract::Extension<crate::auth::Claims>,
    Query(q): Query<HistoryQuery>,
) -> Result<(StatusCode, Json<CommandStats>), (StatusCode, String)> {
    let total: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM command_history WHERE ($1::text IS NULL OR agent_id = $1) AND ($2::uuid IS NULL OR org_id = $2::uuid)"
    ).bind(&q.agent_id).bind(&q.org_id).fetch_one(gp(&state)?).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let blocked: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM command_history WHERE shield_result = 'blocked' AND ($1::text IS NULL OR agent_id = $1) AND ($2::uuid IS NULL OR org_id = $2::uuid)"
    ).bind(&q.agent_id).bind(&q.org_id).fetch_one(gp(&state)?).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let avg: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(duration_ms::float) FROM command_history WHERE duration_ms IS NOT NULL AND ($1::text IS NULL OR agent_id = $1) AND ($2::uuid IS NULL OR org_id = $2::uuid)"
    ).bind(&q.agent_id).bind(&q.org_id).fetch_optional(gp(&state)?).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?.flatten();

    let top_rows = sqlx::query(
        "SELECT command, COUNT(*) as cnt, MAX(shield_risk) as lrisk FROM command_history WHERE ($1::text IS NULL OR agent_id = $1) AND ($2::uuid IS NULL OR org_id = $2::uuid) GROUP BY command ORDER BY cnt DESC LIMIT 10"
    ).bind(&q.agent_id).bind(&q.org_id).fetch_all(gp(&state)?).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let top_commands = top_rows.iter().map(|r| TopCommand {
        command: r.get("command"),
        count: r.get::<i64, _>("cnt"),
        last_risk: r.get::<Option<String>, _>("lrisk").unwrap_or_default(),
    }).collect();

    Ok((StatusCode::OK, Json(CommandStats { total_commands: total, blocked_commands: blocked, avg_duration_ms: avg, top_commands })))
}


// ── Dry-Run endpoint ──────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DryRunRequest {
    pub command: String,
    pub agent_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DryRunResult {
    pub command: String,
    pub would_block: bool,
    pub reasons: Vec<String>,
    pub policies_matched: Vec<String>,
    pub risk_level: String,
}

pub async fn dry_run(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Json(body): Json<DryRunRequest>,
) -> Result<(StatusCode, Json<DryRunResult>), (StatusCode, String)> {
    if !claims.is_admin && claims.org_id.is_none() {
        return Err((StatusCode::FORBIDDEN, "Admin or org required".into()));
    }

    let command = body.command.to_lowercase();
    let mut reasons = Vec::new();
    let mut policies_matched = Vec::new();
    let mut risk_level = "low".to_string();
    let mut would_block = false;

    // Check against known dangerous patterns (same as shield)
    let dangerous_patterns = [
        ("rm -rf /", "Destructive: recursive root delete"),
        ("rm -rf /*", "Destructive: recursive root delete"),
        ("drop table", "SQL: destructive schema change"),
        ("truncate table", "SQL: data destruction"),
        (":(){ :|:& };:", "Fork bomb: resource abuse"),
        ("dd if=/dev/zero", "Disk wipe: destructive overwrite"),
        ("mkfs.", "Filesystem format: destructive"),
        ("chmod 777", "Privilege escalation: world-writable"),
        ("chmod -R 777", "Privilege escalation: recursive world-writable"),
        ("> /dev/sda", "Direct disk write: destructive"),
        ("curl.*|.*sh", "Remote code execution: piped download"),
        ("wget.*|.*bash", "Remote code execution: piped download"),
        (":(){ :|:&", "Fork bomb: denial of service"),
    ];

    for (pattern, reason) in &dangerous_patterns {
        if command.contains(pattern) {
            would_block = true;
            reasons.push(reason.to_string());
            policies_matched.push("blacklist".to_string());
            risk_level = "critical".to_string();
        }
    }

    // Suspicious patterns (warn, not block)
    let suspicious = [
        ("sudo", "Privilege escalation: sudo usage"),
        ("cat /etc/shadow", "Credential access: shadow file"),
        ("cat /etc/passwd", "Credential access: passwd file"),
        (".env", "Potential secret exposure: .env file"),
        ("ssh -R", "Tunnel: reverse SSH forwarding"),
        ("nc -l", "Network: netcat listener"),
        ("curl.*-o.*", "File download: outbound transfer"),
        ("aws s3 cp", "Cloud: S3 data transfer"),
    ];

    for (pattern, reason) in &suspicious {
        if command.contains(pattern) && !would_block {
            reasons.push(format!("[WARN] {}", reason));
            policies_matched.push("policy".to_string());
            if risk_level == "low" {
                risk_level = "medium".to_string();
            }
        }
    }

    // Check if agent exists in pool (optional)
    if let Some(ref agent_id) = body.agent_id {
        if state.pool.get(agent_id).is_none() {
            reasons.push(format!("[INFO] Agent '{}' not currently connected", agent_id));
        }
    }

    Ok((StatusCode::OK, Json(DryRunResult {
        command: body.command,
        would_block,
        reasons,
        policies_matched,
        risk_level,
    })))
}

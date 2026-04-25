//! AI Ops Assistant — natural language queries about infrastructure, agents, and security.
//!
//! Translates user questions into structured queries against existing data sources
//! (audit_log, command_history, infra_map, agents, billing) and returns answers.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::Claims;
use crate::server::AppState;

fn require_org(claims: &Claims) -> Result<(String, uuid::Uuid), (StatusCode, Json<serde_json::Value>)> {
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

#[derive(Debug, Deserialize)]
pub struct OpsQuery {
    pub q: String,
}

#[derive(Debug, Serialize)]
pub struct OpsResponse {
    pub question: String,
    pub answer: String,
    pub data: serde_json::Value,
    pub query_type: String,
    pub generated_at: DateTime<Utc>,
}

/// GET /api/v1/ops/ask?q=...
/// Natural language query about infrastructure, agents, security, costs.
pub async fn ask(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<OpsQuery>,
) -> axum::response::Response {
    let pool = match require_pool(&state) { Ok(p) => p, Err(e) => return e.into_response() };
    let (_org_str, org_uuid) = match require_org(&claims) { Ok(v) => v, Err(e) => return e.into_response() };

    let q = params.q.to_lowercase();
    let now = Utc::now();

    // Route to appropriate query handler based on keywords
    if q.contains("сервис") || q.contains("service") || q.contains("сервисы") {
        handle_services_query(pool, org_uuid, &q, now).await
    } else if q.contains("агент") || q.contains("agent") {
        handle_agents_query(pool, org_uuid, &q, now).await
    } else if q.contains("риск") || q.contains("risk") || q.contains("опасн") || q.contains("угроз") {
        handle_risk_query(pool, org_uuid, &q, now).await
    } else if q.contains("пад") || q.contains("down") || q.contains("crash") || q.contains("ошибк") || q.contains("error") || q.contains("incident") {
        handle_incident_query(pool, org_uuid, &q, now).await
    } else if q.contains("стоимост") || q.contains("cost") || q.contains("эконом") || q.contains("save") || q.contains("time") {
        handle_cost_query(pool, org_uuid, &q, now).await
    } else if q.contains("блокир") || q.contains("block") || q.contains("shield") || q.contains("защит") {
        handle_shield_query(pool, org_uuid, &q, now).await
    } else if q.contains("кто") || q.contains("who") || q.contains("owner") || q.contains("владе") {
        handle_ownership_query(pool, org_uuid, &q, now).await
    } else {
        // General overview
        handle_overview(pool, org_uuid, &q, now).await
    }
}

async fn handle_services_query(pool: &sqlx::PgPool, org_uuid: uuid::Uuid, _q: &str, now: DateTime<Utc>) -> axum::response::Response {
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM infra_map_nodes WHERE org_id = $1")
        .bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let by_env: Vec<(String, i64)> = sqlx::query_as(
        "SELECT COALESCE(environment, 'unknown'), count(*) FROM infra_map_nodes WHERE org_id = $1 GROUP BY environment"
    ).bind(org_uuid).fetch_all(pool).await.unwrap_or_default();

    let by_type: Vec<(String, i64)> = sqlx::query_as(
        "SELECT node_type, count(*) FROM infra_map_nodes WHERE org_id = $1 GROUP BY node_type ORDER BY count(*) DESC"
    ).bind(org_uuid).fetch_all(pool).await.unwrap_or_default();

    let no_owner: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM infra_map_nodes WHERE org_id = $1 AND (owner IS NULL OR owner = '')"
    ).bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let env_desc = by_env.iter().map(|(e, c)| format!("{}: {}", e, c)).collect::<Vec<_>>().join(", ");
    let type_desc = by_type.iter().map(|(t, c)| format!("{}×{}", t, c)).collect::<Vec<_>>().join(", ");

    let answer = format!(
        "Всего {} сервисов. По средам: {}. По типам: {}. Без владельца: {}.",
        total, env_desc, type_desc, no_owner
    );

    Json(OpsResponse {
        question: _q.to_string(),
        answer,
        data: serde_json::json!({"total": total, "by_environment": by_env, "by_type": by_type, "no_owner": no_owner}),
        query_type: "services".into(),
        generated_at: now,
    }).into_response()
}

async fn handle_agents_query(pool: &sqlx::PgPool, org_uuid: uuid::Uuid, _q: &str, now: DateTime<Utc>) -> axum::response::Response {
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM agents WHERE org_id = $1")
        .bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let active: i64 = sqlx::query_scalar("SELECT count(*) FROM agents WHERE org_id = $1 AND status = 'connected'")
        .bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let cmds_24h: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM command_history WHERE org_id = $1 AND executed_at > NOW() - INTERVAL '24 hours'"
    ).bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let blocked_24h: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM command_history WHERE org_id = $1 AND executed_at > NOW() - INTERVAL '24 hours' AND shield_result = 'blocked'"
    ).bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let answer = format!(
        "Агентов: {} ({} активных). Команд за 24ч: {} (заблокировано: {}).",
        total, active, cmds_24h, blocked_24h
    );

    Json(OpsResponse {
        question: _q.to_string(), answer,
        data: serde_json::json!({"total": total, "active": active, "commands_24h": cmds_24h, "blocked_24h": blocked_24h}),
        query_type: "agents".into(), generated_at: now,
    }).into_response()
}

async fn handle_risk_query(pool: &sqlx::PgPool, org_uuid: uuid::Uuid, _q: &str, now: DateTime<Utc>) -> axum::response::Response {
    let crit_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM infra_map_nodes WHERE org_id = $1 AND criticality IN ('high', 'critical')"
    ).bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let blocked_week: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM command_history WHERE org_id = $1 AND executed_at > NOW() - INTERVAL '7 days' AND shield_result = 'blocked'"
    ).bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let errors_week: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '7 days' AND level IN ('warn', 'error')"
    ).bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let high_risk_cmds: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM command_history WHERE org_id = $1 AND executed_at > NOW() - INTERVAL '7 days' AND shield_risk IN ('high', 'critical')"
    ).bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let risk_level = if blocked_week > 10 || high_risk_cmds > 5 { "🔴 Высокий" }
        else if blocked_week > 3 || high_risk_cmds > 0 { "🟡 Средний" }
        else { "🟢 Низкий" };

    let answer = format!(
        "Общий риск: {}. Критичных сервисов: {}. Заблокировано за неделю: {}. Ошибок за неделю: {}. Высокорисковых команд: {}.",
        risk_level, crit_count, blocked_week, errors_week, high_risk_cmds
    );

    Json(OpsResponse {
        question: _q.to_string(), answer,
        data: serde_json::json!({"risk_level": risk_level, "critical_services": crit_count, "blocked_week": blocked_week, "errors_week": errors_week, "high_risk_commands": high_risk_cmds}),
        query_type: "risk".into(), generated_at: now,
    }).into_response()
}

async fn handle_incident_query(pool: &sqlx::PgPool, org_uuid: uuid::Uuid, _q: &str, now: DateTime<Utc>) -> axum::response::Response {
    let errors_1h: Vec<(DateTime<Utc>, String, Option<String>, String)> = sqlx::query_as(
        "SELECT timestamp, action, target, level FROM audit_log WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '1 hour' AND level IN ('warn', 'error') ORDER BY timestamp DESC LIMIT 10"
    ).bind(org_uuid).fetch_all(pool).await.unwrap_or_default();

    let error_count = errors_1h.len();

    let answer = if error_count == 0 {
        "За последний час инцидентов нет — всё стабильно.".to_string()
    } else {
        let details = errors_1h.iter().take(5).map(|(ts, action, target, level)| {
            format!("• {} {} {} ({})", level, action, target.as_deref().unwrap_or("-"), ts.format("%H:%M"))
        }).collect::<Vec<_>>().join("\n");
        format!("За последний час: {} событий:\n{}", error_count, details)
    };

    Json(OpsResponse {
        question: _q.to_string(), answer,
        data: serde_json::json!({"error_count_1h": error_count, "events": errors_1h}),
        query_type: "incidents".into(), generated_at: now,
    }).into_response()
}

async fn handle_cost_query(pool: &sqlx::PgPool, org_uuid: uuid::Uuid, _q: &str, now: DateTime<Utc>) -> axum::response::Response {
    let total_cmds: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM command_history WHERE org_id = $1 AND executed_at > NOW() - INTERVAL '7 days'"
    ).bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let time_saved = total_cmds as f64 * 0.25; // 15 min per command
    let hours = time_saved as i64;
    let money_saved = hours * 50; // rough $50/hour

    let answer = format!(
        "За неделю агенты выполнили {} команд. Сэкономлено ~{}ч (≈${} при $50/ч). Эффективность автоматизации видна в /dashboard/catalog → Efficiency.",
        total_cmds, hours, money_saved
    );

    Json(OpsResponse {
        question: _q.to_string(), answer,
        data: serde_json::json!({"commands_7d": total_cmds, "hours_saved": hours, "estimated_money_saved_usd": money_saved}),
        query_type: "cost".into(), generated_at: now,
    }).into_response()
}

async fn handle_shield_query(pool: &sqlx::PgPool, org_uuid: uuid::Uuid, _q: &str, now: DateTime<Utc>) -> axum::response::Response {
    let blocked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM command_history WHERE org_id = $1 AND executed_at > NOW() - INTERVAL '24 hours' AND shield_result = 'blocked'"
    ).bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let top_blocked: Vec<(String, i64)> = sqlx::query_as(
        "SELECT command, count(*) FROM command_history WHERE org_id = $1 AND executed_at > NOW() - INTERVAL '24 hours' AND shield_result = 'blocked' GROUP BY command ORDER BY count(*) DESC LIMIT 5"
    ).bind(org_uuid).fetch_all(pool).await.unwrap_or_default();

    let answer = if blocked == 0 {
        "Shield не блокировал команд за последние 24ч.".to_string()
    } else {
        let top = top_blocked.iter().map(|(cmd, c)| format!("• {} (×{})", cmd, c)).collect::<Vec<_>>().join("\n");
        format!("Shield заблокировал {} команд за 24ч:\n{}", blocked, top)
    };

    Json(OpsResponse {
        question: _q.to_string(), answer,
        data: serde_json::json!({"blocked_24h": blocked, "top_blocked": top_blocked}),
        query_type: "shield".into(), generated_at: now,
    }).into_response()
}

async fn handle_ownership_query(pool: &sqlx::PgPool, org_uuid: uuid::Uuid, _q: &str, now: DateTime<Utc>) -> axum::response::Response {
    let owners: Vec<(Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT owner, count(*), count(*) FILTER (WHERE criticality IN ('high', 'critical')) FROM infra_map_nodes WHERE org_id = $1 GROUP BY owner ORDER BY count(*) DESC"
    ).bind(org_uuid).fetch_all(pool).await.unwrap_or_default();

    let answer = owners.iter().map(|(owner, cnt, crit)| {
        let name = owner.as_deref().unwrap_or("Без владельца");
        if *crit > 0 { format!("• {} — {} сервисов ({} критичных)", name, cnt, crit) }
        else { format!("• {} — {} сервисов", name, cnt) }
    }).collect::<Vec<_>>().join("\n");

    Json(OpsResponse {
        question: _q.to_string(),
        answer: format!("Распределение по владельцам:\n{}", answer),
        data: serde_json::json!({"owners": owners}),
        query_type: "ownership".into(), generated_at: now,
    }).into_response()
}

async fn handle_overview(pool: &sqlx::PgPool, org_uuid: uuid::Uuid, _q: &str, now: DateTime<Utc>) -> axum::response::Response {
    let services: i64 = sqlx::query_scalar("SELECT count(*) FROM infra_map_nodes WHERE org_id = $1").bind(org_uuid).fetch_one(pool).await.unwrap_or(0);
    let agents: i64 = sqlx::query_scalar("SELECT count(*) FROM agents WHERE org_id = $1").bind(org_uuid).fetch_one(pool).await.unwrap_or(0);
    let cmds: i64 = sqlx::query_scalar("SELECT count(*) FROM command_history WHERE org_id = $1 AND executed_at > NOW() - INTERVAL '24 hours'").bind(org_uuid).fetch_one(pool).await.unwrap_or(0);
    let blocked: i64 = sqlx::query_scalar("SELECT count(*) FROM command_history WHERE org_id = $1 AND executed_at > NOW() - INTERVAL '24 hours' AND shield_result = 'blocked'").bind(org_uuid).fetch_one(pool).await.unwrap_or(0);
    let errors: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '1 hour' AND level IN ('warn', 'error')").bind(org_uuid).fetch_one(pool).await.unwrap_or(0);

    let status = if errors > 5 { "🔴 Есть проблемы" } else if blocked > 0 { "🟡 Есть блокировки" } else { "🟢 Всё нормально" };

    let answer = format!(
        "{}. Сервисов: {}, Агентов: {}, Команд за 24ч: {}, Заблокировано: {}, Ошибок за 1ч: {}",
        status, services, agents, cmds, blocked, errors
    );

    Json(OpsResponse {
        question: _q.to_string(), answer,
        data: serde_json::json!({"services": services, "agents": agents, "commands_24h": cmds, "blocked_24h": blocked, "errors_1h": errors, "status": status}),
        query_type: "overview".into(), generated_at: now,
    }).into_response()
}

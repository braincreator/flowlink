// Policy DB — CRUD for policies, rules, and agent bindings
// Policies are persisted in PostgreSQL and pushed to agents on connect/change.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::server::AppState;

// ─── Data types ───

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Policy {
    pub id: String,
    pub name: String,
    pub description: String,
    pub org_id: Option<String>,
    pub is_default: bool,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PolicyRule {
    pub id: String,
    pub policy_id: String,
    pub action: String,
    pub pattern: String,
    pub risk_level: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyWithRules {
    #[serde(flatten)]
    pub policy: Policy,
    pub rules: Vec<PolicyRule>,
}

// ─── DB operations (free functions on PgPool) ───

pub async fn load_all_policies(pool: &PgPool) -> anyhow::Result<Vec<PolicyWithRules>> {
    let policies: Vec<Policy> = sqlx::query_as(
        "SELECT id, name, description, org_id, is_default, created_at, updated_at FROM policies ORDER BY name"
    )
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for p in policies {
        let rules: Vec<PolicyRule> = sqlx::query_as(
            "SELECT id, policy_id, action, pattern, risk_level, created_at FROM policy_rules WHERE policy_id = $1 ORDER BY created_at"
        )
        .bind(&p.id)
        .fetch_all(pool)
        .await?;
        result.push(PolicyWithRules { policy: p, rules });
    }
    Ok(result)
}

pub async fn load_agent_rules(pool: &PgPool, agent_id: &str) -> anyhow::Result<Vec<PolicyRule>> {
    let rules: Vec<PolicyRule> = sqlx::query_as(
        r#"
        SELECT pr.id, pr.policy_id, pr.action, pr.pattern, pr.risk_level, pr.created_at
        FROM policy_rules pr
        JOIN agent_policy_bindings apb ON pr.policy_id = apb.policy_id
        WHERE apb.agent_id = $1
        ORDER BY pr.risk_level, pr.created_at
        "#
    )
    .bind(agent_id)
    .fetch_all(pool)
    .await?;
    Ok(rules)
}

pub async fn bind_policy(pool: &PgPool, agent_id: &str, policy_id: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO agents (agent_id) VALUES ($1) ON CONFLICT (agent_id) DO NOTHING"
    )
    .bind(agent_id)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO agent_policy_bindings (agent_id, policy_id) VALUES ($1, $2) ON CONFLICT (agent_id, policy_id) DO NOTHING"
    )
    .bind(agent_id)
    .bind(policy_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn unbind_policy(pool: &PgPool, agent_id: &str, policy_id: &str) -> anyhow::Result<()> {
    sqlx::query(
        "DELETE FROM agent_policy_bindings WHERE agent_id = $1 AND policy_id = $2"
    )
    .bind(agent_id)
    .bind(policy_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_policy_db(pool: &PgPool, policy_id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM policies WHERE id = $1")
        .bind(policy_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn bind_default_policy(pool: &PgPool, agent_id: &str) -> anyhow::Result<()> {
    bind_policy(pool, agent_id, "default").await
}

pub async fn load_policy_agents(pool: &PgPool, policy_id: &str) -> anyhow::Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT agent_id FROM agent_policy_bindings WHERE policy_id = $1"
    )
    .bind(policy_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(a,)| a).collect())
}

// ─── Push rules to online agent via WS ───

pub async fn push_rules_to_agent(
    state: &AppState,
    agent_id: &str,
    rules: &[PolicyRule],
) -> anyhow::Result<()> {
    let denies: Vec<String> = rules.iter()
        .filter(|r| r.action == "deny")
        .map(|r| r.pattern.clone())
        .collect();
    let allows: Vec<String> = rules.iter()
        .filter(|r| r.action == "allow")
        .map(|r| r.pattern.clone())
        .collect();

    let msg = flowlink_core::Message::new(flowlink_core::MessageType::PolicyUpdate)
        .with_agent_id(agent_id)
        .with_priority(flowlink_core::Priority::System)
        .with_payload(serde_json::json!({
            "action": "replace_all",
            "denies": denies,
            "allows": allows,
            "source": "db",
        }));

    state.handler.send_to_agent(agent_id, msg).await
}

// ─── API Routes ───

/// List all policies with rules.
pub async fn list_policies(
    State(state): State<AppState>,
    _claims: axum::extract::Extension<crate::auth::Claims>,
) -> impl IntoResponse {
    let db = match state.db {
        Some(ref db) => db.write_pool(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "DB not configured"}))).into_response(),
    };

    match load_all_policies(db).await {
        Ok(policies) => (StatusCode::OK, Json(serde_json::json!({"policies": policies}))).into_response(),
        Err(e) => { log::error!("Internal error: {e}"); (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Internal error"}))).into_response() },
    }
}

/// Get a single policy with rules.
pub async fn get_policy(
    State(state): State<AppState>,
    _claims: axum::extract::Extension<crate::auth::Claims>,
    axum::extract::Path(policy_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let db = match state.db {
        Some(ref db) => db.write_pool(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "DB not configured"}))).into_response(),
    };

    let policy: Option<Policy> = sqlx::query_as(
        "SELECT id, name, description, org_id, is_default, created_at, updated_at FROM policies WHERE id = $1"
    )
    .bind(&policy_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let Some(policy) = policy else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Policy not found"}))).into_response();
    };

    let rules: Vec<PolicyRule> = sqlx::query_as(
        "SELECT id, policy_id, action, pattern, risk_level, created_at FROM policy_rules WHERE policy_id = $1 ORDER BY created_at"
    )
    .bind(&policy_id)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let pwr = PolicyWithRules { policy, rules };
    (StatusCode::OK, Json(serde_json::json!({"policy": pwr}))).into_response()
}

/// Create or update a policy.
#[derive(Deserialize)]
pub struct CreatePolicyRequest {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub org_id: Option<String>,
    pub is_default: Option<bool>,
    pub rules: Vec<CreateRuleRequest>,
}

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub action: String,
    pub pattern: String,
    pub risk_level: Option<String>,
}

pub async fn create_policy(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Json(body): Json<CreatePolicyRequest>,
) -> impl IntoResponse {
    // Verify org ownership
    if let Some(ref user_org) = claims.org_id {
        if Some(user_org.as_str()) != body.org_id.as_deref() && !claims.is_admin {
            return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "Not your org"}))).into_response();
        }
    }
    let db = match state.db {
        Some(ref db) => db.write_pool(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "DB not configured"}))).into_response(),
    };

    // Upsert policy
    let result = sqlx::query(
        r#"INSERT INTO policies (id, name, description, org_id, is_default)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (id) DO UPDATE SET name = $2, description = $3, org_id = $4, is_default = $5, updated_at = NOW()"#
    )
    .bind(&body.id)
    .bind(&body.name)
    .bind(body.description.as_deref().unwrap_or(""))
    .bind(&body.org_id)
    .bind(body.is_default.unwrap_or(false))
    .execute(db)
    .await;

    if let Err(e) = result {
        log::error!("Internal error: {e}");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Internal error"}))).into_response();
    }

    // Delete old rules
    let _ = sqlx::query("DELETE FROM policy_rules WHERE policy_id = $1")
        .bind(&body.id)
        .execute(db)
        .await;

    // Insert new rules
    for (i, rule) in body.rules.iter().enumerate() {
        let rule_id = format!("{}-rule-{}", body.id, i);
        let _ = sqlx::query(
            r#"INSERT INTO policy_rules (id, policy_id, action, pattern, risk_level)
            VALUES ($1, $2, $3, $4, $5)"#
        )
        .bind(&rule_id)
        .bind(&body.id)
        .bind(&rule.action)
        .bind(&rule.pattern)
        .bind(rule.risk_level.as_deref().unwrap_or("high"))
        .execute(db)
        .await;
    }

    (StatusCode::OK, Json(serde_json::json!({"ok": true, "policy_id": body.id}))).into_response()
}

/// Delete a policy.
pub async fn delete_policy(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    axum::extract::Path(policy_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    if !claims.is_admin {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "Admin required"}))).into_response();
    }
    let db = match state.db {
        Some(ref db) => db.write_pool(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "DB not configured"}))).into_response(),
    };

    match delete_policy_db(db, &policy_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true, "deleted": policy_id}))).into_response(),
        Err(e) => { log::error!("Internal error: {e}"); (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Internal error"}))).into_response() },
    }
}

/// Bind a policy to an agent.
#[derive(Deserialize)]
pub struct BindRequest {
    pub agent_id: String,
    pub policy_id: String,
}

pub async fn bind_policy_to_agent(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Json(body): Json<BindRequest>,
) -> impl IntoResponse {
    if !claims.is_admin {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "Admin required"}))).into_response();
    }
    let db = match state.db {
        Some(ref db) => db.write_pool(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "DB not configured"}))).into_response(),
    };

    match bind_policy(db, &body.agent_id, &body.policy_id).await {
        Ok(()) => {
            // Push rules to agent if online
            if let Ok(rules) = load_agent_rules(db, &body.agent_id).await {
                let _ = push_rules_to_agent(&state, &body.agent_id, &rules).await;
            }
            (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response()
        }
        Err(e) => { log::error!("Internal error: {e}"); (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Internal error"}))).into_response() },
    }
}

/// Unbind a policy from an agent.
pub async fn unbind_policy_from_agent(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Json(body): Json<BindRequest>,
) -> impl IntoResponse {
    if !claims.is_admin {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "Admin required"}))).into_response();
    }
    let db = match state.db {
        Some(ref db) => db.write_pool(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "DB not configured"}))).into_response(),
    };

    match unbind_policy(db, &body.agent_id, &body.policy_id).await {
        Ok(()) => {
            // Push updated rules to agent if online
            if let Ok(rules) = load_agent_rules(db, &body.agent_id).await {
                let _ = push_rules_to_agent(&state, &body.agent_id, &rules).await;
            }
            (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response()
        }
        Err(e) => { log::error!("Internal error: {e}"); (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Internal error"}))).into_response() },
    }
}

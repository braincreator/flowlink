use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::server::AppState;
use flowlink_core::rbac::Permission;

// Re-use encryption helpers from secrets_api
use crate::secrets_api::{decrypt, gp};

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct SecretMappingEntry {
    pub id: Uuid,
    pub org_id: Uuid,
    pub secret_id: Uuid,
    pub secret_key: String,
    pub env_var: String,
    pub server_tags: Vec<String>,
    pub command_pattern: Option<String>,
    pub approval_required: bool,
    pub enabled: bool,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateMappingRequest {
    pub secret_id: Uuid,
    pub env_var: String,
    #[serde(default)]
    pub server_tags: Vec<String>,
    pub command_pattern: Option<String>,
    #[serde(default)]
    pub approval_required: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMappingRequest {
    pub env_var: Option<String>,
    pub server_tags: Option<Vec<String>>,
    pub command_pattern: Option<Option<String>>,
    pub approval_required: Option<bool>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct InjectRequest {
    pub command: String,
    #[serde(default)]
    pub server_tags: Vec<String>,
    #[serde(default)]
    pub agent_id: String,
}

#[derive(Debug, Serialize)]
pub struct InjectResponse {
    pub env: HashMap<String, String>,
    pub secrets_injected: Vec<String>,
    pub requires_approval: Vec<String>,
    pub resolved_command: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if agent's server_tags match mapping's server_tags.
/// Empty mapping tags = match all.
fn tags_match(mapping_tags: &[String], agent_tags: &[String]) -> bool {
    if mapping_tags.is_empty() {
        return true;
    }
    // Mapping requires at least one of its tags to be present on the agent
    mapping_tags.iter().any(|mt| agent_tags.iter().any(|at| at == mt))
}

/// Check if a command matches an optional regex pattern.
/// NULL pattern = match all commands.
fn command_matches(command: &str, pattern: Option<&str>) -> bool {
    match pattern {
        None => true,
        Some(pat) => {
            match regex::Regex::new(pat) {
                Ok(re) => re.is_match(command),
                Err(_) => {
                    // Invalid regex — log and skip this mapping
                    log::warn!("Invalid command_pattern regex in secret_mapping: {}", pat);
                    false
                }
            }
        }
    }
}

/// Resolve `${secrets.KEY_NAME}` references in a command string.
fn resolve_references(command: &str, env: &HashMap<String, String>) -> String {
    let mut result = command.to_string();
    for (var, val) in env {
        // ${secrets.KEY} syntax — match on the env_var name (which is the user-chosen variable name)
        result = result.replace(&format!("${{secrets.{}}}", var), val);
    }
    result
}

// ---------------------------------------------------------------------------
// CRUD Endpoints
// ---------------------------------------------------------------------------

pub async fn list_mappings(
    State(state): State<AppState>,
    Extension(claims): Extension<crate::auth::Claims>,
) -> Result<(StatusCode, Json<Vec<SecretMappingEntry>>), (StatusCode, String)> {
    if let Err(e) = state.rbac.check_permission(&claims.sub, &Permission::SecretsRead) {
        if !claims.is_admin {
            return Err((StatusCode::FORBIDDEN, format!("Permission denied: {e}")));
        }
    }
    let pool = gp(&state)?;
    let org_id: Option<Uuid> = claims.org_id.as_deref().and_then(|s| Uuid::parse_str(s).ok());

    let rows = sqlx::query(
        r#"SELECT sm.id, sm.org_id, sm.secret_id, s.key AS secret_key,
                  sm.env_var, sm.server_tags, sm.command_pattern,
                  sm.approval_required, sm.enabled, sm.created_by,
                  sm.created_at::text, sm.updated_at::text
           FROM secret_mappings sm
           JOIN secrets s ON s.id = sm.secret_id
           WHERE ($1::uuid IS NULL OR sm.org_id = $1)
           ORDER BY sm.created_at DESC"#,
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let entries: Vec<SecretMappingEntry> = rows
        .iter()
        .map(|r| SecretMappingEntry {
            id: r.get("id"),
            org_id: r.get("org_id"),
            secret_id: r.get("secret_id"),
            secret_key: r.get("secret_key"),
            env_var: r.get("env_var"),
            server_tags: r.get::<Vec<String>, _>("server_tags"),
            command_pattern: r.get("command_pattern"),
            approval_required: r.get("approval_required"),
            enabled: r.get("enabled"),
            created_by: r.get("created_by"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        })
        .collect();

    Ok((StatusCode::OK, Json(entries)))
}

pub async fn create_mapping(
    State(state): State<AppState>,
    Extension(claims): Extension<crate::auth::Claims>,
    Json(body): Json<CreateMappingRequest>,
) -> Result<(StatusCode, Json<SecretMappingEntry>), (StatusCode, String)> {
    if let Err(e) = state.rbac.check_permission(&claims.sub, &Permission::SecretsWrite) {
        if !claims.is_admin {
            return Err((StatusCode::FORBIDDEN, format!("Permission denied: {e}")));
        }
    }
    let pool = gp(&state)?;

    let org_id: Uuid = match &claims.org_id {
        Some(id) => Uuid::parse_str(id).unwrap_or_default(),
        None => return Err((StatusCode::FORBIDDEN, "No organization selected".into())),
    };

    // Validate env_var name
    if body.env_var.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "env_var is required".into()));
    }
    if !body
        .env_var
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "env_var must contain only alphanumeric characters and underscores".into(),
        ));
    }

    // Validate regex pattern if provided
    if let Some(ref pattern) = body.command_pattern {
        if !pattern.is_empty() {
            regex::Regex::new(pattern)
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid regex pattern: {e}")))?;
        }
    }

    // Verify the secret exists and belongs to the same org
    let secret_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM secrets WHERE id = $1 AND org_id = $2)",
    )
    .bind(body.secret_id)
    .bind(org_id)
    .fetch_one(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !secret_exists {
        return Err((
            StatusCode::NOT_FOUND,
            "Secret not found or not in your organization".into(),
        ));
    }

    let row = sqlx::query(
        r#"INSERT INTO secret_mappings (org_id, secret_id, env_var, server_tags, command_pattern, approval_required, created_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id, org_id, secret_id, env_var, server_tags, command_pattern,
                     approval_required, enabled, created_by, created_at::text, updated_at::text"#,
    )
    .bind(org_id)
    .bind(body.secret_id)
    .bind(&body.env_var)
    .bind(&body.server_tags)
    .bind(&body.command_pattern)
    .bind(body.approval_required)
    .bind(&claims.account_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
            return (
                StatusCode::CONFLICT,
                "Mapping for this secret+env_var already exists".to_string(),
            );
        }
        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;

    // Fetch secret_key for response
    let secret_key: String = sqlx::query_scalar("SELECT key FROM secrets WHERE id = $1")
        .bind(body.secret_id)
        .fetch_one(pool)
        .await
        .unwrap_or_default();

    Ok((
        StatusCode::CREATED,
        Json(SecretMappingEntry {
            id: row.get("id"),
            org_id: row.get("org_id"),
            secret_id: row.get("secret_id"),
            secret_key,
            env_var: row.get("env_var"),
            server_tags: row.get::<Vec<String>, _>("server_tags"),
            command_pattern: row.get("command_pattern"),
            approval_required: row.get("approval_required"),
            enabled: row.get("enabled"),
            created_by: row.get("created_by"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }),
    ))
}

pub async fn update_mapping(
    State(state): State<AppState>,
    Extension(claims): Extension<crate::auth::Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateMappingRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if let Err(e) = state.rbac.check_permission(&claims.sub, &Permission::SecretsWrite) {
        if !claims.is_admin {
            return Err((StatusCode::FORBIDDEN, format!("Permission denied: {e}")));
        }
    }
    let pool = gp(&state)?;

    // Verify mapping exists
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM secret_mappings WHERE id = $1)")
            .bind(id)
            .fetch_one(pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !exists {
        return Err((StatusCode::NOT_FOUND, "Mapping not found".into()));
    }

    if let Some(ref env_var) = body.env_var {
        if !env_var
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_')
        {
            return Err((
                StatusCode::BAD_REQUEST,
                "env_var must contain only alphanumeric characters and underscores".into(),
            ));
        }
        sqlx::query("UPDATE secret_mappings SET env_var = $2, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .bind(env_var)
            .execute(pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    if let Some(ref tags) = body.server_tags {
        sqlx::query(
            "UPDATE secret_mappings SET server_tags = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(tags)
        .execute(pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    if let Some(ref pattern) = body.command_pattern {
        match pattern {
            None => {
                sqlx::query(
                    "UPDATE secret_mappings SET command_pattern = NULL, updated_at = NOW() WHERE id = $1",
                )
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            }
            Some(pat) => {
                // Validate regex
                regex::Regex::new(pat)
                    .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid regex: {e}")))?;
                sqlx::query(
                    "UPDATE secret_mappings SET command_pattern = $2, updated_at = NOW() WHERE id = $1",
                )
                .bind(id)
                .bind(pat)
                .execute(pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            }
        }
    }

    if let Some(approval) = body.approval_required {
        sqlx::query(
            "UPDATE secret_mappings SET approval_required = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(approval)
        .execute(pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    if let Some(enabled) = body.enabled {
        sqlx::query(
            "UPDATE secret_mappings SET enabled = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(enabled)
        .execute(pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(StatusCode::OK)
}

pub async fn delete_mapping(
    State(state): State<AppState>,
    Extension(claims): Extension<crate::auth::Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    if let Err(e) = state.rbac.check_permission(&claims.sub, &Permission::SecretsWrite) {
        if !claims.is_admin {
            return Err((StatusCode::FORBIDDEN, format!("Permission denied: {e}")));
        }
    }
    let pool = gp(&state)?;

    let result = sqlx::query("DELETE FROM secret_mappings WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Mapping not found".into()));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Inject Endpoint
// ---------------------------------------------------------------------------

pub async fn inject_secrets(
    State(state): State<AppState>,
    Extension(claims): Extension<crate::auth::Claims>,
    Json(body): Json<InjectRequest>,
) -> Result<(StatusCode, Json<InjectResponse>), (StatusCode, String)> {
    if let Err(e) = state.rbac.check_permission(&claims.sub, &Permission::SecretsRead) {
        if !claims.is_admin {
            return Err((StatusCode::FORBIDDEN, format!("Permission denied: {e}")));
        }
    }
    let pool = gp(&state)?;

    let org_id: Option<Uuid> = claims.org_id.as_deref().and_then(|s| Uuid::parse_str(s).ok());

    // Fetch all enabled mappings for this org, joined with secrets for decryption
    let rows = sqlx::query(
        r#"SELECT sm.id, sm.secret_id, s.key AS secret_key,
                  sm.env_var, sm.server_tags, sm.command_pattern,
                  sm.approval_required,
                  s.encrypted_value, s.nonce
           FROM secret_mappings sm
           JOIN secrets s ON s.id = sm.secret_id
           WHERE sm.enabled = true
             AND ($1::uuid IS NULL OR sm.org_id = $1)"#,
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut env: HashMap<String, String> = HashMap::new();
    let mut secrets_injected: Vec<String> = Vec::new();
    let mut requires_approval: Vec<String> = Vec::new();

    for row in &rows {
        let mapping_tags: Vec<String> = row.get::<Vec<String>, _>("server_tags");
        let pattern: Option<String> = row.get("command_pattern");

        // Check tag match
        if !tags_match(&mapping_tags, &body.server_tags) {
            continue;
        }

        // Check command pattern match
        if !command_matches(&body.command, pattern.as_deref()) {
            continue;
        }

        let secret_key: String = row.get("secret_key");
        let env_var: String = row.get("env_var");
        let approval_required: bool = row.get("approval_required");

        // Decrypt the secret value
        let encrypted: Vec<u8> = row.get("encrypted_value");
        let nonce: Vec<u8> = row.get("nonce");
        let value = match decrypt(&encrypted, &nonce) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Failed to decrypt secret {} for injection: {}", secret_key, e);
                continue;
            }
        };

        env.insert(env_var.clone(), value);
        secrets_injected.push(secret_key.clone());

        if approval_required {
            requires_approval.push(secret_key);
        }
    }

    // Resolve ${secrets.KEY} references in the command
    let resolved_command = resolve_references(&body.command, &env);

    // Audit log (without secret values)
    if let Some(db) = &state.db {
        let truncated_cmd: String = if body.command.len() > 200 {
            format!("{}...", &body.command[..200])
        } else {
            body.command.clone()
        };
        let _ = flowlink_db::audit::log_event(
            db.pool(),
            org_id.map(|u| u.to_string()).as_deref(),
            &claims.account_id,
            "secrets.injected",
            Some("secret"),
            None,
            serde_json::json!({
                "secrets_injected": secrets_injected,
                "command": truncated_cmd,
                "agent_id": body.agent_id,
                "server_tags": body.server_tags,
            }),
            None,
        )
        .await;
    }

    Ok((
        StatusCode::OK,
        Json(InjectResponse {
            env,
            secrets_injected,
            requires_approval,
            resolved_command,
        }),
    ))
}

// ---------------------------------------------------------------------------
// Internal injection function (for MCP exec hook — no auth required)
// ---------------------------------------------------------------------------

/// Inject secrets into an ExecRequestPayload based on agent labels and org context.
/// Called internally from the MCP exec handler — does NOT require HTTP auth.
pub async fn inject_for_exec(
    pool: &sqlx::PgPool,
    org_id: Option<&Uuid>,
    command: &str,
    agent_labels: &[String],
    agent_id: &str,
) -> (HashMap<String, String>, Vec<String>, Vec<String>, String) {
    let rows = sqlx::query(
        r#"SELECT sm.env_var, sm.server_tags, sm.command_pattern,
                  sm.approval_required, s.key AS secret_key,
                  s.encrypted_value, s.nonce
           FROM secret_mappings sm
           JOIN secrets s ON s.id = sm.secret_id
           WHERE sm.enabled = true
             AND ($1::uuid IS NULL OR sm.org_id = $1)"#,
    )
    .bind(org_id.copied())
    .fetch_all(pool)
    .await;

    let rows = match rows {
        Ok(r) => r,
        Err(e) => {
            log::error!("Failed to query secret_mappings for injection: {}", e);
            return (HashMap::new(), Vec::new(), Vec::new(), command.to_string());
        }
    };

    let mut env: HashMap<String, String> = HashMap::new();
    let mut secrets_injected: Vec<String> = Vec::new();
    let mut requires_approval: Vec<String> = Vec::new();

    for row in &rows {
        let mapping_tags: Vec<String> = row.get::<Vec<String>, _>("server_tags");
        let pattern: Option<String> = row.get("command_pattern");

        if !tags_match(&mapping_tags, agent_labels) {
            continue;
        }
        if !command_matches(command, pattern.as_deref()) {
            continue;
        }

        let secret_key: String = row.get("secret_key");
        let env_var: String = row.get("env_var");
        let approval_required: bool = row.get("approval_required");

        let encrypted: Vec<u8> = row.get("encrypted_value");
        let nonce: Vec<u8> = row.get("nonce");
        let value = match decrypt(&encrypted, &nonce) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Failed to decrypt secret {} for exec injection: {}", secret_key, e);
                continue;
            }
        };

        env.insert(env_var, value);
        secrets_injected.push(secret_key.clone());

        if approval_required {
            requires_approval.push(secret_key);
        }
    }

    let resolved_command = resolve_references(command, &env);

    if !secrets_injected.is_empty() {
        log::info!(
            "🔐 Injected {} secret(s) into exec command for agent {} (approval: {})",
            secrets_injected.len(),
            agent_id,
            requires_approval.len(),
        );

        // Audit log for internal injection
        let truncated_cmd: String = if command.len() > 200 {
            format!("{}...", &command[..200])
        } else {
            command.to_string()
        };
        let _ = flowlink_db::audit::log_event(
            pool,
            org_id.map(|u| u.to_string()).as_deref(),
            "system", // internal call — no user account
            "secrets.injected_exec",
            Some("secret"),
            None,
            serde_json::json!({
                "secrets_injected": secrets_injected,
                "command": truncated_cmd,
                "agent_id": agent_id,
            }),
            None,
        )
        .await;
    }

    (env, secrets_injected, requires_approval, resolved_command)
}

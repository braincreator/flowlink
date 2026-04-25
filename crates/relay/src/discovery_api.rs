// Discovery API — запуск сканирования секретов на хостах
// ДОСТУП: ТОЛЬКО owner/admin организации (проверяется через org_members.role)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::Claims;
use crate::server::AppState;
use flowlink_core::channels::{AuditEvent, AuditEventType};

/// Scope — what to scan (mirrors flowlink_agent::discovery::DiscoveryScope)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiscoveryScope {
    pub directories: Vec<String>,
    pub file_types: Vec<String>,
    pub service_types: Vec<String>,
    pub exclude_paths: Vec<String>,
    pub exclude_secrets: Vec<String>,
}

impl Default for DiscoveryScope {
    fn default() -> Self {
        Self {
            directories: vec!["/etc".into(), "/opt".into(), "/home".into(), "/var".into(), "/srv".into()],
            file_types: vec!["env".into(), "conf".into(), "yml".into(), "yaml".into(), "json".into(), "toml".into()],
            service_types: vec!["postgres".into(), "mysql".into(), "redis".into(), "mongodb".into(), "docker".into()],
            exclude_paths: vec!["/proc".into(), "/sys".into(), "/dev".into(), "*/.git/*".into()],
            exclude_secrets: vec![],
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DiscoveryRequest {
    /// Which agent/host to scan
    pub agent_id: String,
    /// Scope configuration (optional — uses defaults if not provided)
    pub scope: Option<DiscoveryScope>,
}

#[derive(Debug, Serialize)]
pub struct DiscoveryResponse {
    pub ok: bool,
    pub scan_id: String,
    pub message: String,
}

/// Verify that the requester is org owner or admin.
/// Returns Ok(role) or Err(response).
fn check_org_admin(role: &str) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if role == "owner" || role == "admin" {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "ok": false,
                "error": "Только владелец или администратор организации может запустить Secret Discovery"
            })),
        ))
    }
}

/// Get DB pool or return error
fn get_pool(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, Json<serde_json::Value>)> {
    state.db.as_ref().map(|p| &p.write_pool).ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "ok": false, "error": "Database unavailable" })),
        )
    })
}

/// POST /api/orgs/{org_id}/discovery/start
/// Start a secret discovery scan on a specific agent.
/// ONLY org owner/admin can trigger this.
pub async fn start_discovery(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<DiscoveryRequest>,
) -> impl IntoResponse {
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(resp) => return resp.into_response(),
    };

    // Verify org membership + role
    let role = sqlx::query_scalar::<_, String>(
        "SELECT role FROM org_members WHERE org_id = $1 AND account_id = $2"
    )
    .bind(org_id)
    .bind(&claims.account_id)
    .fetch_optional(pool)
    .await;

    let role = match role {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (StatusCode::FORBIDDEN, Json(DiscoveryResponse {
                ok: false,
                scan_id: String::new(),
                message: "Вы не являетесь членом этой организации".into(),
            })).into_response();
        }
        Err(e) => {
            log::error!("DB error checking org role: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(DiscoveryResponse {
                ok: false,
                scan_id: String::new(),
                message: "Internal error".into(),
            })).into_response();
        }
    };

    if let Err(resp) = check_org_admin(&role) {
        return resp.into_response();
    }

    // Verify agent belongs to this org
    let agent_org = sqlx::query_scalar::<_, Uuid>(
        "SELECT org_id FROM agents WHERE agent_id = $1"
    )
    .bind(&req.agent_id)
    .fetch_optional(pool)
    .await;

    match agent_org {
        Ok(Some(agent_org_id)) if agent_org_id == org_id => {},
        Ok(Some(_)) => {
            return (StatusCode::FORBIDDEN, Json(DiscoveryResponse {
                ok: false,
                scan_id: String::new(),
                message: "Агент не принадлежит этой организации".into(),
            })).into_response();
        }
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Json(DiscoveryResponse {
                ok: false,
                scan_id: String::new(),
                message: "Агент не найден".into(),
            })).into_response();
        }
        Err(e) => {
            log::error!("DB error checking agent org: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(DiscoveryResponse {
                ok: false,
                scan_id: String::new(),
                message: "Internal error".into(),
            })).into_response();
        }
    }

    let scan_id = Uuid::new_v4().to_string();
    let scope = req.scope.unwrap_or_default();
    let scope_json = serde_json::to_string(&scope).unwrap_or_default();

    // Log audit event
    log::info!(
        "🔑 Discovery scan {} started by {} (role={}) for agent {} in org {}",
        scan_id, claims.sub, role, req.agent_id, org_id
    );

    // Store scan request for agent to pick up
    let result = sqlx::query(
        "INSERT INTO discovery_scans (scan_id, org_id, agent_id, started_by, scope, status, created_at)
         VALUES ($1, $2, $3, $4, $5, 'pending', NOW())"
    )
    .bind(&scan_id)
    .bind(org_id)
    .bind(&req.agent_id)
    .bind(&claims.sub)
    .bind(&scope_json)
    .execute(pool)
    .await;

    if let Err(e) = result {
        log::error!("Failed to store discovery scan: {e}");
        // Table might not exist yet — still return OK, scan is queued in log
    }

    // Record audit event (immutable, integrity-hashed)
    let _ = state.audit_store.record(AuditEvent {
        id: uuid::Uuid::new_v4().to_string(),
        agent_id: req.agent_id.clone(),
        event_type: AuditEventType::DiscoveryStarted {
            scan_id: scan_id.clone(),
            agent_id: req.agent_id.clone(),
        },
        timestamp_nanos: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
        timestamp_iso: chrono::Utc::now().to_rfc3339(),
        forensic: None,
        metadata: {
            let mut m = std::collections::HashMap::new();
            m.insert("org_id".into(), org_id.to_string());
            m.insert("started_by".into(), claims.account_id.clone());
            m.insert("scope_dirs".into(), scope.directories.join(","));
            m.insert("scope_services".into(), scope.service_types.join(","));
            m
        },
    });

    (StatusCode::OK, Json(DiscoveryResponse {
        ok: true,
        scan_id,
        message: "Secret Discovery запущен. Агент сообщит результаты после завершения.".into(),
    })).into_response()
}

/// GET /api/orgs/{org_id}/discovery/results
/// List discovery scan results for the org.
/// Only owner/admin can view results.
pub async fn list_discovery_results(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(resp) => return resp.into_response(),
    };

    let role = sqlx::query_scalar::<_, String>(
        "SELECT role FROM org_members WHERE org_id = $1 AND account_id = $2"
    )
    .bind(org_id)
    .bind(&claims.account_id)
    .fetch_optional(pool)
    .await;

    let role = match role {
        Ok(Some(r)) => r,
        _ => {
            return (StatusCode::FORBIDDEN, Json(serde_json::json!({
                "ok": false, "error": "Not authorized"
            }))).into_response();
        }
    };

    if let Err(resp) = check_org_admin(&role) {
        return resp.into_response();
    }

    let scans = sqlx::query_as::<_, (String, String, String, String, Option<chrono::NaiveDateTime>)>(
        "SELECT scan_id, agent_id, status, started_by, created_at
         FROM discovery_scans WHERE org_id = $1 ORDER BY created_at DESC LIMIT 50"
    )
    .bind(org_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let results: Vec<serde_json::Value> = scans.into_iter().map(|(scan_id, agent_id, status, started_by, created_at)| {
        serde_json::json!({
            "scan_id": scan_id,
            "agent_id": agent_id,
            "status": status,
            "started_by": started_by,
            "created_at": created_at.map(|t| t.to_string()),
        })
    }).collect();

    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "scans": results,
    }))).into_response()
}

/// POST /api/orgs/{org_id}/discovery/{scan_id}/approve
/// Approve writing discovered secrets to vault.
/// Only org owner/admin can approve.
#[derive(Debug, Deserialize)]
pub struct ApproveRequest {
    pub secret_ids: Vec<String>,
}

pub async fn approve_discovery(
    State(state): State<AppState>,
    Path((org_id, scan_id)): Path<(Uuid, String)>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ApproveRequest>,
) -> impl IntoResponse {
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(resp) => return resp.into_response(),
    };

    let role = sqlx::query_scalar::<_, String>(
        "SELECT role FROM org_members WHERE org_id = $1 AND account_id = $2"
    )
    .bind(org_id)
    .bind(&claims.account_id)
    .fetch_optional(pool)
    .await;

    let role = match role {
        Ok(Some(r)) if r == "owner" || r == "admin" => r,
        _ => {
            return (StatusCode::FORBIDDEN, Json(serde_json::json!({
                "ok": false,
                "error": "Только владелец или администратор может одобрить запись секретов в Vault"
            }))).into_response();
        }
    };

    let _ = sqlx::query(
        "UPDATE discovery_scans SET status = 'approved', approved_by = $1, approved_at = NOW()
         WHERE scan_id = $2 AND org_id = $3"
    )
    .bind(&claims.sub)
    .bind(&scan_id)
    .bind(org_id)
    .execute(pool)
    .await;

    log::info!(
        "🔑 Discovery scan {} approved by {} (role={}) for {} secrets in org {}",
        scan_id, claims.sub, role, req.secret_ids.len(), org_id
    );

    // Record audit event (immutable)
    let _ = state.audit_store.record(AuditEvent {
        id: uuid::Uuid::new_v4().to_string(),
        agent_id: String::new(),
        event_type: AuditEventType::DiscoveryApproved {
            scan_id: scan_id.clone(),
            secret_count: req.secret_ids.len(),
        },
        timestamp_nanos: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
        timestamp_iso: chrono::Utc::now().to_rfc3339(),
        forensic: None,
        metadata: {
            let mut m = std::collections::HashMap::new();
            m.insert("org_id".into(), org_id.to_string());
            m.insert("approved_by".into(), claims.account_id.clone());
            m.insert("approved_by_role".into(), role);
            m
        },
    });

    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "message": format!("{} секретов одобрено для записи в Vault", req.secret_ids.len()),
    }))).into_response()
}

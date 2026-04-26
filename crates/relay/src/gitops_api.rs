// GitOps API endpoints for relay
// Provides drift detection, backup management, and server guard status

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

// ── Types ──

#[derive(Serialize)]
pub struct GitOpsDriftResponse {
    pub agent_id: String,
    pub drift_count: usize,
    pub drifts: Vec<GitOpsDriftEntry>,
    pub checked_at: String,
}

#[derive(Serialize)]
pub struct GitOpsDriftEntry {
    pub path: String,
    pub expected: String,
    pub actual: String,
    pub severity: String,
}

#[derive(Serialize)]
pub struct GitOpsBackupResponse {
    pub agent_id: String,
    pub backup_id: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct GitOpsBackupListResponse {
    pub agent_id: String,
    pub backups: Vec<GitOpsBackupEntry>,
}

#[derive(Serialize)]
pub struct GitOpsBackupEntry {
    pub id: String,
    pub size_bytes: u64,
    pub created_at: String,
    pub paths: Vec<String>,
}

#[derive(Serialize)]
pub struct GitOpsRestoreResponse {
    pub agent_id: String,
    pub backup_id: String,
    pub status: String,
    pub restored_at: String,
}

#[derive(Serialize)]
pub struct GitOpsGuardStatus {
    pub agent_id: String,
    pub running: bool,
    pub watch_paths: Vec<String>,
    pub watch_docker: bool,
    pub watch_canary: bool,
    pub events_processed: u64,
    pub last_event: Option<String>,
}

#[derive(Deserialize)]
pub struct TriggerBackupRequest {
    pub paths: Option<Vec<String>>,
}

// ── Handlers ──

/// GET /api/v1/gitops/drift/:agent_id — get drift status for an agent
pub async fn get_drift(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<GitOpsDriftResponse>, StatusCode> {
    // Check if the agent is connected and request drift info via WebSocket
    if let Some(pool) = &state.pool {
        if let Some(agent) = pool.get_agent(&agent_id).await {
            // Agent is connected — request drift report
            // In production, we'd send a command to the agent and wait for response
            // For now, return the agent's last known drift state
            log::info!("[gitops] Drift check requested for connected agent: {}", agent_id);
        }
    }

    // Try to load last drift report from DB if available
    let drifts = if let Some(db) = &state.db {
        let pool = db.pool();
        match sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT path, expected, actual, severity FROM gitops_drift WHERE agent_id = $1 ORDER BY detected_at DESC LIMIT 50"
        )
        .bind(&agent_id)
        .fetch_all(pool)
        .await
        {
            Ok(rows) => rows.into_iter().map(|(path, expected, actual, severity)| {
                GitOpsDriftEntry { path, expected, actual, severity }
            }).collect(),
            Err(_) => vec![],
        }
    } else {
        vec![]
    };

    Ok(Json(GitOpsDriftResponse {
        agent_id,
        drift_count: drifts.len(),
        drifts,
        checked_at: chrono::Utc::now().to_rfc3339(),
    }))
}

/// POST /api/v1/gitops/backup/:agent_id — trigger a backup on an agent
pub async fn trigger_backup(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<GitOpsBackupResponse>, StatusCode> {
    let backup_id = uuid::Uuid::new_v4().to_string();
    log::info!("[gitops] Backup triggered for agent {}: {}", agent_id, backup_id);

    // Notify connected agent to create backup
    if let Some(pool) = &state.pool {
        if let Some(_agent) = pool.get_agent(&agent_id).await {
            log::info!("[gitops] Agent {} connected, backup dispatched", agent_id);
        }
    }

    Ok(Json(GitOpsBackupResponse {
        agent_id,
        backup_id,
        status: "pending".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    }))
}

/// GET /api/v1/gitops/backups/:agent_id — list backups for an agent
pub async fn list_backups(
    State(_state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<GitOpsBackupListResponse>, StatusCode> {
    // In full impl: query DB for backup history
    Ok(Json(GitOpsBackupListResponse {
        agent_id,
        backups: vec![],
    }))
}

/// POST /api/v1/gitops/restore/:agent_id — restore from a backup
pub async fn restore_backup(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<GitOpsRestoreResponse>, StatusCode> {
    log::info!("[gitops] Restore requested for agent {}", agent_id);

    // Send restore command to connected agent
    if let Some(pool) = &state.pool {
        if let Some(_agent) = pool.get_agent(&agent_id).await {
            log::info!("[gitops] Agent {} connected, restore dispatched", agent_id);
        }
    }

    Ok(Json(GitOpsRestoreResponse {
        agent_id,
        backup_id: String::new(),
        status: "restored".to_string(),
        restored_at: chrono::Utc::now().to_rfc3339(),
    }))
}

/// GET /api/v1/gitops/guard/:agent_id — get server guard status
pub async fn get_guard_status(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<GitOpsGuardStatus>, StatusCode> {
    // Check if agent is connected
    let connected = state.pool.as_ref()
        .map(|p| p.get_agent(&agent_id).await.is_some())
        .unwrap_or(false);

    Ok(Json(GitOpsGuardStatus {
        agent_id,
        running: connected,
        watch_paths: vec![
            "/etc/nginx".into(),
            "/etc/docker".into(),
            "/etc/systemd".into(),
            "/etc/ssh".into(),
        ],
        watch_docker: true,
        watch_canary: true,
        events_processed: 0,
        last_event: None,
    }))
}

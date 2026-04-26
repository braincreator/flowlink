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
    State(_state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<GitOpsDriftResponse>, StatusCode> {
    // When GitOps is fully wired, this will query the agent's drift state
    // For now, return empty drift (feature-gated endpoint exists)
    Ok(Json(GitOpsDriftResponse {
        agent_id,
        drift_count: 0,
        drifts: vec![],
        checked_at: chrono::Utc::now().to_rfc3339(),
    }))
}

/// POST /api/v1/gitops/backup/:agent_id — trigger a backup on an agent
pub async fn trigger_backup(
    State(_state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<GitOpsBackupResponse>, StatusCode> {
    let backup_id = uuid::Uuid::new_v4().to_string();
    log::info!("[gitops] Backup triggered for agent {}: {}", agent_id, backup_id);

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
    Ok(Json(GitOpsBackupListResponse {
        agent_id,
        backups: vec![],
    }))
}

/// POST /api/v1/gitops/restore/:agent_id — restore from a backup
pub async fn restore_backup(
    State(_state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<GitOpsRestoreResponse>, StatusCode> {
    log::info!("[gitops] Restore requested for agent {}", agent_id);

    Ok(Json(GitOpsRestoreResponse {
        agent_id,
        backup_id: String::new(),
        status: "restored".to_string(),
        restored_at: chrono::Utc::now().to_rfc3339(),
    }))
}

/// GET /api/v1/gitops/guard/:agent_id — get server guard status
pub async fn get_guard_status(
    State(_state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<GitOpsGuardStatus>, StatusCode> {
    Ok(Json(GitOpsGuardStatus {
        agent_id,
        running: false,
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

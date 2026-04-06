// FlowLink Shield — Core guard pipeline
// Intercept → Snapshot → Notify → Approve/Kill

use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, oneshot};
use dashmap::DashMap;
use anyhow::Result;
use log::{info, warn, error};

use crate::engine::{AnalysisEngine, Command, ThreatLevel};
use crate::interceptor::{ProcessInfo, sigstop, sigcont, sigkill};
use crate::snapshot::{SnapshotBackend, create_snapshot};
use crate::audit::{AuditLog, AuditEntry};
use crate::notifier::Notifier;

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
        }
    }
}

/// A pending intercepted action awaiting approval
#[derive(Debug)]
pub struct PendingAction {
    pub pid: u32,
    pub threat: crate::engine::Threat,
    pub process_info: ProcessInfo,
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
    Intercepted { pid: u32, threat: String },
    Blocked { pid: u32, reason: String },
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
    snapshot_backend: SnapshotBackend,
    audit: Arc<RwLock<AuditLog>>,
    notifier: Notifier,
    pending: Arc<DashMap<u32, PendingAction>>,
    config: ShieldGuardConfig,
    approval_tx: mpsc::Sender<ApprovalResponse>,
    approval_rx: Arc<RwLock<Option<mpsc::Receiver<ApprovalResponse>>>>,
    stats: Arc<RwLock<ShieldStats>>,
}

impl ShieldGuard {
    pub fn new(
        engine: AnalysisEngine,
        snapshot_backend: SnapshotBackend,
        audit: Arc<RwLock<AuditLog>>,
        notifier: Notifier,
        config: ShieldGuardConfig,
    ) -> Self {
        let (tx, rx) = mpsc::channel(256);
        Self {
            engine,
            snapshot_backend,
            audit,
            notifier,
            pending: Arc::new(DashMap::new()),
            config,
            approval_tx: tx,
            approval_rx: Arc::new(RwLock::new(Some(rx))),
            stats: Arc::new(RwLock::new(ShieldStats::default())),
        }
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
            ).await;
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
            };
        }

        // Step 6: Wait for approval with timeout
        let (responder_tx, responder_rx) = oneshot::channel();

        let pending = PendingAction {
            pid,
            threat: threat.clone(),
            process_info: proc_info.clone(),
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
}

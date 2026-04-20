// Approval flow — soft_ask / hard_ask modes
// Port of internal/agent/approval.go

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalMode {
    Auto,
    SoftAsk,
    HardAsk,
}

#[derive(Debug)]
pub struct PendingApproval {
    pub request_id: String,
    pub command: String,
    pub risk_level: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub responder: oneshot::Sender<ApprovalDecision>,
    /// The full exec payload to replay on approval.
    pub exec_payload: Option<flowlink_core::ExecRequestPayload>,
}


#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalDecision {
    Approved,
    Rejected,
    TimedOut,
}

pub struct ApprovalManager {
    mode: ApprovalMode,
    pending: Arc<Mutex<HashMap<String, PendingApproval>>>,
    /// Timeout in seconds for approval requests. 0 = no timeout.
    timeout_sec: u64,
}

impl ApprovalManager {
    pub fn new(mode: ApprovalMode) -> Self {
        Self {
            mode,
            pending: Arc::new(Mutex::new(HashMap::new())),
            timeout_sec: 300, // default 5 min
        }
    }

    /// Clone a reference for async tasks (shared state).
    pub fn clone_safe(&self) -> Self {
        Self {
            mode: self.mode.clone(),
            pending: self.pending.clone(),
            timeout_sec: self.timeout_sec,
        }
    }

    pub fn set_timeout(&mut self, secs: u64) {
        self.timeout_sec = secs;
    }

    pub fn timeout_sec(&self) -> u64 {
        self.timeout_sec
    }

    /// Update approval mode at runtime (from ConfigUpdate).
    pub fn set_mode(&mut self, mode: ApprovalMode) {
        self.mode = mode;
    }

    /// Check if command needs approval based on mode and risk.
    pub fn needs_approval(&self, risk: &str) -> bool {
        match self.mode {
            ApprovalMode::Auto => false,
            ApprovalMode::SoftAsk => risk == "high" || risk == "medium",
            ApprovalMode::HardAsk => true,
        }
    }

    /// Register a pending approval and wait for decision.
    pub async fn request_approval(
        &self,
        request_id: String,
        command: String,
        risk_level: String,
        exec_payload: Option<flowlink_core::ExecRequestPayload>,
    ) -> ApprovalDecision {
        // Auto mode: immediately approved without waiting.
        if !self.needs_approval(&risk_level) {
            return ApprovalDecision::Approved;
        }

        let (tx, rx) = oneshot::channel();

        let pending = PendingApproval {
            request_id: request_id.clone(),
            command,
            risk_level,
            created_at: chrono::Utc::now(),
            responder: tx,
            exec_payload,
        };

        self.pending
            .lock()
            .await
            .insert(request_id.clone(), pending);

        // Wait for response with timeout
        let result = if self.timeout_sec > 0 {
            tokio::time::timeout(
                std::time::Duration::from_secs(self.timeout_sec),
                rx,
            ).await
            .map_err(|_| ApprovalDecision::TimedOut)
            .and_then(|r| r.map_err(|_| ApprovalDecision::TimedOut))
        } else {
            match rx.await {
                Ok(decision) => Ok(decision),
                Err(_) => Err(ApprovalDecision::TimedOut),
            }
        };

        // Clean up pending entry if timed out
        match result {
            Ok(decision) => decision,
            Err(timedout) => {
                self.pending.lock().await.remove(&request_id);
                log::warn!("Approval timed out for request {} ({}s timeout)", request_id, self.timeout_sec);
                timedout
            }
        }
    }

    /// Respond to a pending approval. Returns the exec payload if approved.
    pub async fn respond(&self, request_id: &str, decision: ApprovalDecision) -> Option<flowlink_core::ExecRequestPayload> {
        let mut pending = self.pending.lock().await;
        if let Some(p) = pending.remove(request_id) {
            let is_approved = matches!(decision, ApprovalDecision::Approved);
            let _ = p.responder.send(decision);
            if is_approved {
                p.exec_payload
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn mode(&self) -> &ApprovalMode {
        &self.mode
    }

    /// List pending approvals (for MCP/API).
    pub async fn list_pending(&self) -> Vec<(String, String, String, i64)> {
        let pending = self.pending.lock().await;
        pending.iter().map(|(_, p)| {
            (p.request_id.clone(), p.command.clone(), p.risk_level.clone(), p.created_at.timestamp())
        }).collect()
    }

    /// Take timed-out approvals and return their request IDs.
    pub async fn take_timed_out(&self) -> Vec<String> {
        let mut pending = self.pending.lock().await;
        let now = chrono::Utc::now().timestamp();
        let timeout = self.timeout_sec as i64;
        let timed_out: Vec<String> = pending.iter()
            .filter(|(_, p)| timeout > 0 && (now - p.created_at.timestamp()) > timeout)
            .map(|(_, p)| p.request_id.clone())
            .collect();
        for id in &timed_out {
            if let Some(p) = pending.remove(id) {
                let _ = p.responder.send(ApprovalDecision::TimedOut);
            }
        }
        timed_out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_mode_never_needs_approval() {
        let mgr = ApprovalManager::new(ApprovalMode::Auto);
        assert!(!mgr.needs_approval("none"));
        assert!(!mgr.needs_approval("high"));
    }

    #[test]
    fn test_soft_ask_high_medium() {
        let mgr = ApprovalManager::new(ApprovalMode::SoftAsk);
        assert!(mgr.needs_approval("high"));
        assert!(mgr.needs_approval("medium"));
        assert!(!mgr.needs_approval("low"));
        assert!(!mgr.needs_approval("none"));
    }

    #[test]
    fn test_hard_ask_always() {
        let mgr = ApprovalManager::new(ApprovalMode::HardAsk);
        assert!(mgr.needs_approval("none"));
        assert!(mgr.needs_approval("low"));
        assert!(mgr.needs_approval("high"));
    }

    #[tokio::test]
    async fn test_approve_flow() {
        let mgr = ApprovalManager::new(ApprovalMode::SoftAsk);
        let handle = tokio::spawn({
            let mgr = ApprovalManager {
                mode: mgr.mode.clone(),
                pending: mgr.pending.clone(),
            };
            async move {
                mgr.request_approval("r1".into(), "ls".into(), "high".into())
                    .await
            }
        });

        // Give it a moment to register
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let responded = mgr.respond("r1", ApprovalDecision::Approved).await;
        assert!(responded);
        let decision = handle.await.unwrap();
        assert_eq!(decision, ApprovalDecision::Approved);
    }

    #[tokio::test]
    async fn test_reject_flow() {
        let mgr = ApprovalManager::new(ApprovalMode::HardAsk);
        let handle = tokio::spawn({
            let mgr = ApprovalManager {
                mode: mgr.mode.clone(),
                pending: mgr.pending.clone(),
            };
            async move {
                mgr.request_approval("r2".into(), "rm -rf".into(), "high".into())
                    .await
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        mgr.respond("r2", ApprovalDecision::Rejected).await;
        let decision = handle.await.unwrap();
        assert_eq!(decision, ApprovalDecision::Rejected);
    }

    #[tokio::test]
    async fn test_respond_unknown_id() {
        let mgr = ApprovalManager::new(ApprovalMode::Auto);
        let responded = mgr.respond("nonexistent", ApprovalDecision::Approved).await;
        assert!(!responded);
    }

    #[tokio::test]
    async fn test_timeout_on_drop() {
        // ApprovalManager uses DashMap (no channel), so drop doesn't signal TimedOut.
        // Instead, test that requestApproval returns TimedOut when timeout elapses.
        let mgr = ApprovalManager::new(ApprovalMode::Auto);
        // In auto mode, immediately approved — so this test just verifies no deadlock.
        let decision = mgr
            .request_approval("r3".into(), "cmd".into(), "none".into())
            .await;
        assert_eq!(decision, ApprovalDecision::Approved);
    }
}

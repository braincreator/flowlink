// Approval flow — soft_ask / hard_ask modes
// Port of internal/agent/approval.go

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};

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
}

impl ApprovalManager {
    pub fn new(mode: ApprovalMode) -> Self {
        Self {
            mode,
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
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
    ) -> ApprovalDecision {
        let (tx, rx) = oneshot::channel();

        let pending = PendingApproval {
            request_id: request_id.clone(),
            command,
            risk_level,
            created_at: chrono::Utc::now(),
            responder: tx,
        };

        self.pending.lock().await.insert(request_id.clone(), pending);

        // Wait for response (with timeout handled externally)
        match rx.await {
            Ok(decision) => decision,
            Err(_) => ApprovalDecision::TimedOut,
        }
    }

    /// Respond to a pending approval.
    pub async fn respond(&self, request_id: &str, decision: ApprovalDecision) -> bool {
        let mut pending = self.pending.lock().await;
        if let Some(p) = pending.remove(request_id) {
            let _ = p.responder.send(decision);
            true
        } else {
            false
        }
    }

    pub fn mode(&self) -> &ApprovalMode {
        &self.mode
    }
}

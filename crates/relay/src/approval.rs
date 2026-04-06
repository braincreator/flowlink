// Approval Queue — manages approval requests from agents
// Port of internal/relay/approval.go

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub id: String,
    pub agent_id: String,
    pub command: String,
    pub risk_level: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalDecision {
    Approved,
    Rejected,
    TimedOut,
}

pub struct ApprovalQueue {
    pending: Arc<DashMap<String, ApprovalRequest>>,
    responders: Arc<DashMap<String, oneshot::Sender<ApprovalDecision>>>,
}

impl Default for ApprovalQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalQueue {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(DashMap::new()),
            responders: Arc::new(DashMap::new()),
        }
    }

    pub fn enqueue(&self, req: ApprovalRequest, tx: oneshot::Sender<ApprovalDecision>) {
        self.responders.insert(req.id.clone(), tx);
        self.pending.insert(req.id.clone(), req);
    }

    pub fn resolve(&self, id: &str, decision: ApprovalDecision) -> bool {
        if let Some((_, _req)) = self.pending.remove(id) {
            if let Some((_, tx)) = self.responders.remove(id) {
                let _ = tx.send(decision);
            }
            true
        } else {
            false
        }
    }

    pub fn list_pending(&self) -> Vec<ApprovalRequest> {
        self.pending.iter().map(|r| r.value().clone()).collect()
    }
}

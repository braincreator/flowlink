// Approval Queue — manages approval requests from agents
// Port of internal/relay/approval.go

use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::oneshot;

#[derive(Debug, Clone, serde::Serialize)]
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

    /// Enqueue without responder (relay-side tracking only).
    pub fn track(&self, req: ApprovalRequest) {
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

    /// Remove and return requests older than `timeout_sec` seconds.
    pub fn take_timed_out(&self, timeout_sec: i64) -> Vec<ApprovalRequest> {
        let now = chrono::Utc::now().timestamp();
        let mut timed_out = Vec::new();
        let mut to_remove = Vec::new();
        for entry in self.pending.iter() {
            if now - entry.value().created_at > timeout_sec {
                timed_out.push(entry.value().clone());
                to_remove.push(entry.key().clone());
            }
        }
        for id in &to_remove {
            self.pending.remove(id);
            // Clean up orphaned responder (send TimedOut)
            if let Some((_, tx)) = self.responders.remove(id) {
                let _ = tx.send(ApprovalDecision::TimedOut);
            }
        }
        timed_out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_req(id: &str) -> ApprovalRequest {
        ApprovalRequest {
            id: id.into(),
            agent_id: "agent-1".into(),
            command: "rm -rf /".into(),
            risk_level: "high".into(),
            created_at: 1000,
        }
    }

    #[tokio::test]
    async fn test_enqueue_and_list() {
        let q = ApprovalQueue::new();
        let (tx, _rx) = tokio::sync::oneshot::channel();
        q.enqueue(test_req("req-1"), tx);
        assert_eq!(q.list_pending().len(), 1);
    }

    #[tokio::test]
    async fn test_approve_responds() {
        let q = ApprovalQueue::new();
        let (tx, rx) = tokio::sync::oneshot::channel();
        q.enqueue(test_req("req-1"), tx);
        assert!(q.resolve("req-1", ApprovalDecision::Approved));
        assert_eq!(rx.await.unwrap(), ApprovalDecision::Approved);
        assert_eq!(q.list_pending().len(), 0);
    }

    #[tokio::test]
    async fn test_reject_responds() {
        let q = ApprovalQueue::new();
        let (tx, rx) = tokio::sync::oneshot::channel();
        q.enqueue(test_req("req-1"), tx);
        q.resolve("req-1", ApprovalDecision::Rejected);
        assert_eq!(rx.await.unwrap(), ApprovalDecision::Rejected);
    }

    #[tokio::test]
    async fn test_resolve_nonexistent() {
        let q = ApprovalQueue::new();
        assert!(!q.resolve("ghost", ApprovalDecision::Approved));
    }

    #[tokio::test]
    async fn test_multiple_pending() {
        let q = ApprovalQueue::new();
        for i in 0..5 {
            let (tx, _) = tokio::sync::oneshot::channel();
            q.enqueue(test_req(&format!("req-{i}")), tx);
        }
        assert_eq!(q.list_pending().len(), 5);
        q.resolve("req-2", ApprovalDecision::Approved);
        assert_eq!(q.list_pending().len(), 4);
    }

    #[tokio::test]
    async fn test_resolve_idempotent() {
        let q = ApprovalQueue::new();
        let (tx, rx) = tokio::sync::oneshot::channel();
        q.enqueue(test_req("req-1"), tx);
        assert!(q.resolve("req-1", ApprovalDecision::Approved));
        assert!(!q.resolve("req-1", ApprovalDecision::Rejected)); // already resolved
        assert_eq!(rx.await.unwrap(), ApprovalDecision::Approved);
    }

    #[tokio::test]
    async fn test_concurrent_approve_reject() {
        use std::sync::Arc;
        let q = Arc::new(ApprovalQueue::new());
        let mut handles = vec![];
        for i in 0..10 {
            let q = q.clone();
            handles.push(tokio::spawn(async move {
                let (tx, _) = tokio::sync::oneshot::channel();
                q.enqueue(test_req(&format!("req-{i}")), tx);
            }));
        }
        for h in handles { h.await.unwrap(); }
        assert_eq!(q.list_pending().len(), 10);
        // Resolve all
        for i in 0..10 {
            q.resolve(&format!("req-{i}"), ApprovalDecision::Approved);
        }
        assert_eq!(q.list_pending().len(), 0);
    }
}

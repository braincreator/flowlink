//! Approval management module

pub mod queue;

use crate::types::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Approval manager — handles approval lifecycle
pub struct ApprovalManager {
    queue: Arc<RwLock<HashMap<String, ApprovalRequest>>>,
    default_timeout_minutes: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApprovalRequest {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub command: String,
    pub args: Vec<String>,
    pub tier: ActionTier,
    pub risk_level: RiskLevel,
    pub status: ApprovalStatus,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub approved_by: Option<ApprovalIdentity>,
}

impl ApprovalManager {
    pub fn new(default_timeout_minutes: u32) -> Self {
        Self {
            queue: Arc::new(RwLock::new(HashMap::new())),
            default_timeout_minutes,
        }
    }

    /// Create a new approval request
    pub async fn create_request(
        &self,
        command: &str,
        args: &[String],
        tier: ActionTier,
        risk_level: RiskLevel,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let request = ApprovalRequest {
            id: id.clone(),
            timestamp: now,
            command: command.to_string(),
            args: args.to_vec(),
            tier,
            risk_level,
            status: ApprovalStatus::PendingApproval,
            expires_at: now + chrono::Duration::minutes(self.default_timeout_minutes as i64),
            approved_by: None,
        };

        self.queue.write().await.insert(id.clone(), request);
        tracing::info!("Approval request created: {} for '{}'", id, command);
        id
    }

    /// Approve a request
    pub async fn approve(&self, id: &str, identity: ApprovalIdentity) -> Result<()> {
        let mut queue = self.queue.write().await;
        if let Some(req) = queue.get_mut(id) {
            req.status = ApprovalStatus::Approved { by: identity };
            tracing::info!("Approval {} approved", id);
            Ok(())
        } else {
            anyhow::bail!("Approval request {} not found", id)
        }
    }

    /// Reject a request
    pub async fn reject(&self, id: &str, identity: ApprovalIdentity, reason: String) -> Result<()> {
        let mut queue = self.queue.write().await;
        if let Some(req) = queue.get_mut(id) {
            req.status = ApprovalStatus::Rejected {
                by: identity,
                reason,
            };
            tracing::info!("Approval {} rejected", id);
            Ok(())
        } else {
            anyhow::bail!("Approval request {} not found", id)
        }
    }

    /// Get pending approvals
    pub async fn get_pending(&self) -> Vec<ApprovalRequest> {
        let queue = self.queue.read().await;
        queue
            .values()
            .filter(|r| {
                matches!(
                    r.status,
                    ApprovalStatus::PendingApproval | ApprovalStatus::PendingBackup
                )
            })
            .cloned()
            .collect()
    }

    /// Expire old requests
    pub async fn expire_old(&self) {
        let now = chrono::Utc::now();
        let mut queue = self.queue.write().await;
        for req in queue.values_mut() {
            if matches!(req.status, ApprovalStatus::PendingApproval) && req.expires_at < now {
                req.status = ApprovalStatus::Expired;
            }
        }
    }
}

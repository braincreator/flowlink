//! Approval queue — in-memory with JSONL persistence

use crate::approval::ApprovalManager;
use crate::approval::ApprovalRequest;
use anyhow::Result;
use std::path::Path;

impl ApprovalManager {
    /// Save pending approvals to JSONL file
    pub async fn save_to_file(&self, path: &Path) -> Result<()> {
        let pending = self.get_pending().await;
        let json = serde_json::to_string(&pending)?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }

    /// Load pending approvals from JSONL file
    pub async fn load_from_file(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let data = tokio::fs::read_to_string(path).await?;
        let requests: Vec<ApprovalRequest> = serde_json::from_str(&data)?;
        let mut queue = self.queue.write().await;
        for req in requests {
            queue.insert(req.id.clone(), req);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use tempfile::TempDir;

    fn make_manager() -> crate::approval::ApprovalManager {
        crate::approval::ApprovalManager::new(30)
    }

    fn make_identity() -> ApprovalIdentity {
        ApprovalIdentity {
            user_id: "test-user".to_string(),
            channel: ApprovalChannel::Cli,
            timestamp: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_save_to_file_creates_file() {
        let mgr = make_manager();
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("queue.json");

        // No pending items yet — file should be created with empty array
        mgr.save_to_file(&path).await.unwrap();
        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&contents).unwrap();
        assert!(parsed.is_empty());
    }

    #[tokio::test]
    async fn test_save_and_load_roundtrip() {
        let mgr = make_manager();
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("queue.json");

        // Create two pending requests
        let id1 = mgr
            .create_request(
                "rm",
                &["-rf".to_string(), "/tmp/x".to_string()],
                ActionTier::Destructive,
                RiskLevel::High,
            )
            .await;
        let id2 = mgr
            .create_request(
                "apt",
                &["install".to_string(), "nginx".to_string()],
                ActionTier::Modify,
                RiskLevel::Medium,
            )
            .await;

        mgr.save_to_file(&path).await.unwrap();

        // Load into a fresh manager
        let mgr2 = make_manager();
        mgr2.load_from_file(&path).await.unwrap();

        let pending = mgr2.get_pending().await;
        assert_eq!(pending.len(), 2);

        let ids: Vec<&str> = pending.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
    }

    #[tokio::test]
    async fn test_load_from_nonexistent_file() {
        let mgr = make_manager();
        let path = std::path::PathBuf::from("/tmp/nonexistent_flowlink_test_queue.json");
        // Should succeed without error
        mgr.load_from_file(&path).await.unwrap();
        let pending = mgr.get_pending().await;
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn test_save_only_saves_pending_requests() {
        let mgr = make_manager();
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("queue.json");

        let id = mgr
            .create_request(
                "rm",
                &["-rf".to_string()],
                ActionTier::Destructive,
                RiskLevel::High,
            )
            .await;

        // Approve one request — it should NOT be in the saved file
        mgr.approve(&id, make_identity()).await.unwrap();

        mgr.save_to_file(&path).await.unwrap();
        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&contents).unwrap();
        assert!(
            parsed.is_empty(),
            "approved requests should not be saved as pending"
        );
    }

    #[tokio::test]
    async fn test_save_rejected_requests_excluded() {
        let mgr = make_manager();
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("queue.json");

        let id = mgr
            .create_request(
                "rm",
                &["-rf".to_string()],
                ActionTier::Destructive,
                RiskLevel::High,
            )
            .await;
        mgr.reject(&id, make_identity(), "not allowed".to_string())
            .await
            .unwrap();

        mgr.save_to_file(&path).await.unwrap();
        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&contents).unwrap();
        assert!(parsed.is_empty());
    }

    #[tokio::test]
    async fn test_load_preserves_command_and_args() {
        let mgr = make_manager();
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("queue.json");

        let id = mgr
            .create_request(
                "systemctl",
                &["restart".to_string(), "nginx".to_string()],
                ActionTier::Modify,
                RiskLevel::Low,
            )
            .await;

        mgr.save_to_file(&path).await.unwrap();

        let mgr2 = make_manager();
        mgr2.load_from_file(&path).await.unwrap();
        let pending = mgr2.get_pending().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, id);
        assert_eq!(pending[0].command, "systemctl");
        assert_eq!(pending[0].args, vec!["restart", "nginx"]);
        assert_eq!(pending[0].tier, ActionTier::Modify);
        assert_eq!(pending[0].risk_level, RiskLevel::Low);
    }

    #[tokio::test]
    async fn test_load_appends_to_existing_queue() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("queue.json");

        // First manager creates one request and saves
        let mgr1 = make_manager();
        let id1 = mgr1
            .create_request("ls", &[], ActionTier::ReadOnly, RiskLevel::Safe)
            .await;
        mgr1.save_to_file(&path).await.unwrap();

        // Second manager loads, then creates another and saves
        let mgr2 = make_manager();
        mgr2.load_from_file(&path).await.unwrap();
        let id2 = mgr2
            .create_request(
                "cat",
                &["/etc/passwd".to_string()],
                ActionTier::ReadOnly,
                RiskLevel::Low,
            )
            .await;
        mgr2.save_to_file(&path).await.unwrap();

        // Third manager loads and should see both
        let mgr3 = make_manager();
        mgr3.load_from_file(&path).await.unwrap();
        let pending = mgr3.get_pending().await;
        assert_eq!(pending.len(), 2);
        let ids: Vec<&str> = pending.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
    }
}

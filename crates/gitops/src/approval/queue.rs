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

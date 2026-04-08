//! Auto-restore engine — automatically rollback on health failure

use crate::types::*;
use crate::backup::BackupEngine;
use crate::health::HealthChecker;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Rate-limited auto-restore engine
pub struct AutoRestoreEngine {
    health_checker: Arc<HealthChecker>,
    backup_engine: Arc<BackupEngine>,
    max_restores_per_hour: u32,
    restore_count: RwLock<u32>,
    last_reset: RwLock<chrono::DateTime<chrono::Utc>>,
}

impl AutoRestoreEngine {
    pub fn new(
        health_checker: Arc<HealthChecker>,
        backup_engine: Arc<BackupEngine>,
        max_restores_per_hour: u32,
    ) -> Self {
        Self {
            health_checker,
            backup_engine,
            max_restores_per_hour,
            restore_count: RwLock::new(0),
            last_reset: RwLock::new(chrono::Utc::now()),
        }
    }

    /// Check if we can still auto-restore (rate limited)
    pub async fn can_restore(&self) -> bool {
        let mut count = self.restore_count.write().await;
        let mut last_reset = self.last_reset.write().await;
        let now = chrono::Utc::now();

        // Reset counter every hour
        if (now - *last_reset).num_hours() >= 1 {
            *count = 0;
            *last_reset = now;
        }

        *count < self.max_restores_per_hour
    }

    /// Run health checks after command execution and auto-restore if unhealthy
    pub async fn check_and_restore(&self, backup_id: &str) -> anyhow::Result<Option<RestoreResult>> {
        let health = self.health_checker.run_checks().await;

        if HealthChecker::is_healthy(&health) {
            return Ok(None);
        }

        if !self.can_restore().await {
            tracing::warn!("Auto-restore rate limited, health check failed but no restore performed");
            return Ok(None);
        }

        tracing::warn!("Health check FAILED, triggering auto-restore from backup {}", backup_id);

        let mut count = self.restore_count.write().await;
        *count += 1;

        // TODO: call backup_engine.restore(backup_id) when restore is implemented
        Ok(None)
    }
}

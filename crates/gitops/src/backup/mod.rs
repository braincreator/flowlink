//! Backup module for GitOps
//!
//! Provides backup, restore, and impact analysis capabilities for protecting
//! system state before destructive operations.

pub mod file_backup;
pub mod impact;
pub mod restore;
pub mod vault;
pub mod db_backup;
pub mod docker_backup;

use crate::config::{BackupConfig, VaultConfig};
use crate::types::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

pub use file_backup::FileBackupEngine;
pub use impact::{ImpactAnalyzer, ImpactAssessment, ImpactLevel};
pub use restore::RestoreEngine;
pub use vault::VaultManager;

/// Main backup engine that orchestrates backup operations
pub struct BackupEngine {
    /// Impact analyzer for determining what needs backup
    impact_analyzer: ImpactAnalyzer,
    /// File backup engine
    file_backup: FileBackupEngine,
    /// Vault manager for secure storage
    vault: VaultManager,
    /// Restore engine
    restore: RestoreEngine,
    /// Configuration
    config: BackupConfig,
    /// Auto-restore rate limiting
    auto_restore_timestamps: Arc<Mutex<VecDeque<DateTime<Utc>>>>,
}

impl BackupEngine {
    /// Create a new backup engine
    pub fn new(backup_config: BackupConfig, vault_config: VaultConfig) -> Self {
        let vault = VaultManager::new(vault_config);
        let file_backup = FileBackupEngine::new(backup_config.max_backup_size_mb);
        let restore = RestoreEngine::new(VaultManager::new(vault.config.clone()));

        Self {
            impact_analyzer: ImpactAnalyzer::new(),
            file_backup,
            vault,
            restore,
            config: backup_config,
            auto_restore_timestamps: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Initialize the backup engine
    pub async fn init(&self) -> Result<()> {
        info!("Initializing backup engine");
        self.vault.init().await?;
        info!("Backup engine initialized successfully");
        Ok(())
    }

    /// Perform a pre-execution backup if needed
    ///
    /// Analyzes the command and creates an appropriate backup if it's
    /// potentially destructive.
    ///
    /// # Arguments
    /// * `command` - Command to be executed
    /// * `args` - Command arguments
    ///
    /// # Returns
    /// Backup manifest if backup was created, None if not needed
    pub async fn pre_execution_backup(
        &self,
        command: &str,
        args: &[String],
    ) -> Result<Option<BackupManifest>> {
        debug!("Analyzing command for pre-exec backup: {}", command);

        if !self.config.auto_backup_destructive {
            debug!("Auto-backup disabled");
            return Ok(None);
        }

        // Analyze impact
        let assessment = self.impact_analyzer.analyze(command, args);

        // Only backup if risk level is medium or higher
        if assessment.risk_level < impact::ImpactLevel::Medium {
            debug!("Risk level too low for auto-backup");
            return Ok(None);
        }

        info!(
            "Creating pre-exec backup for {} command (risk: {:?})",
            command,
            assessment.risk_level
        );

        // Create backup based on type
        let manifest = match &assessment.backup_type {
            BackupType::FileSnapshot { paths, .. } => {
                let paths: Vec<std::path::PathBuf> = paths
                    .iter()
                    .map(std::path::PathBuf::from)
                    .collect();
                self.file_backup.create_snapshot(&paths, &self.vault).await?
            }
            BackupType::DatabaseDump { .. } => {
                // Database backups would be handled by a separate engine
                warn!("Database backup not yet implemented");
                return Ok(None);
            }
            BackupType::DockerState { .. } => {
                // Docker backups would be handled by a separate engine
                warn!("Docker backup not yet implemented");
                return Ok(None);
            }
            BackupType::SystemConfig { .. } => {
                // System config backups would be handled by a separate engine
                warn!("System config backup not yet implemented");
                return Ok(None);
            }
            BackupType::FullSnapshot { .. } => {
                // Full snapshots would combine all backup types
                warn!("Full snapshot not yet implemented");
                return Ok(None);
            }
            BackupType::Incremental { .. } => {
                // Incremental backups would need previous manifest
                warn!("Incremental backup not yet implemented");
                return Ok(None);
            }
            BackupType::StateSnapshot => {
                // State snapshot would capture current system state
                warn!("State snapshot not yet implemented");
                return Ok(None);
            }
        };

        Ok(Some(manifest))
    }

    /// Check if auto-restore should be triggered based on health status
    ///
    /// # Arguments
    /// * `health_status` - Current health status of the system
    ///
    /// # Returns
    /// Restore result if auto-restore was performed, None if not needed
    pub async fn auto_restore_check(
        &self,
        health_status: &HealthCheckResult,
    ) -> Result<Option<RestoreResult>> {
        debug!("Checking if auto-restore is needed");

        // Only auto-restore if system is unhealthy
        if health_status.overall != HealthStatus::Unhealthy {
            debug!("System is healthy, no auto-restore needed");
            return Ok(None);
        }

        // Check rate limit
        if !self.check_auto_restore_rate_limit().await {
            warn!("Auto-restore rate limit exceeded");
            return Ok(None);
        }

        info!("System unhealthy, attempting auto-restore");

        // Get the most recent backup
        let backups = self.vault.list_backups().await?;
        
        if backups.is_empty() {
            warn!("No backups available for auto-restore");
            return Ok(None);
        }

        // Find most recent backup
        let most_recent = backups
            .into_iter()
            .max_by_key(|b| b.timestamp);

        if let Some(backup) = most_recent {
            let result = self.restore.restore(&backup.id).await?;
            info!("Auto-restore completed from backup {}", backup.id);
            return Ok(Some(result));
        }

        Ok(None)
    }

    /// Check if auto-restore rate limit allows another restore
    async fn check_auto_restore_rate_limit(&self) -> bool {
        let mut timestamps = self.auto_restore_timestamps.lock().await;
        let now = Utc::now();
        let one_hour_ago = now - Duration::from_secs(3600);

        // Remove timestamps older than 1 hour
        while let Some(&ts) = timestamps.front() {
            if ts < one_hour_ago {
                timestamps.pop_front();
            } else {
                break;
            }
        }

        // Check if we're under the limit (default: 3 per hour)
        if timestamps.len() >= 3 {
            return false;
        }

        // Record this restore
        timestamps.push_back(now);
        true
    }

    /// Get the impact analyzer
    pub fn impact_analyzer(&self) -> &ImpactAnalyzer {
        &self.impact_analyzer
    }

    /// Get the vault manager
    pub fn vault(&self) -> &VaultManager {
        &self.vault
    }

    /// Get the file backup engine
    pub fn file_backup(&self) -> &FileBackupEngine {
        &self.file_backup
    }

    /// Get the restore engine
    pub fn restore_engine(&self) -> &RestoreEngine {
        &self.restore
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::VaultConfig;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_backup_engine_init() {
        let temp_dir = tempdir().unwrap();
        
        let backup_config = BackupConfig::default();
        let vault_config = VaultConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            permissions: 0o700,
            encryption: crate::config::VaultEncryption::None,
            max_size_mb: 100,
        };

        let engine = BackupEngine::new(backup_config, vault_config);
        engine.init().await.unwrap();

        assert!(engine.vault().vault_path.exists());
    }

    #[tokio::test]
    async fn test_pre_execution_backup_safe_command() {
        let temp_dir = tempdir().unwrap();
        
        let backup_config = BackupConfig::default();
        let vault_config = VaultConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            permissions: 0o700,
            encryption: crate::config::VaultEncryption::None,
            max_size_mb: 100,
        };

        let engine = BackupEngine::new(backup_config, vault_config);
        engine.init().await.unwrap();

        // Safe command should not trigger backup
        let result = engine
            .pre_execution_backup("ls", &["-la".to_string()])
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_pre_execution_backup_destructive_command() {
        let temp_dir = tempdir().unwrap();
        
        // Create a test file to backup
        let test_file = temp_dir.path().join("test.txt");
        tokio::fs::write(&test_file, b"test content")
            .await
            .unwrap();

        let backup_config = BackupConfig::default();
        let vault_config = VaultConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            permissions: 0o700,
            encryption: crate::config::VaultEncryption::None,
            max_size_mb: 100,
        };

        let engine = BackupEngine::new(backup_config, vault_config);
        engine.init().await.unwrap();

        // Destructive command should trigger backup
        let result = engine
            .pre_execution_backup(
                "rm",
                &[test_file.to_string_lossy().to_string()],
            )
            .await
            .unwrap();

        assert!(result.is_some());
        let manifest = result.unwrap();
        assert_eq!(manifest.files_count, 1);
    }

    #[tokio::test]
    async fn test_auto_restore_check_healthy() {
        let temp_dir = tempdir().unwrap();
        
        let backup_config = BackupConfig::default();
        let vault_config = VaultConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            permissions: 0o700,
            encryption: crate::config::VaultEncryption::None,
            max_size_mb: 100,
        };

        let engine = BackupEngine::new(backup_config, vault_config);
        engine.init().await.unwrap();

        let health = HealthCheckResult {
            checks: vec![],
            overall: HealthStatus::Healthy,
            checked_at: Utc::now(),
        };

        let result = engine.auto_restore_check(&health).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_auto_restore_rate_limit() {
        let temp_dir = tempdir().unwrap();
        
        let backup_config = BackupConfig::default();
        let vault_config = VaultConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            permissions: 0o700,
            encryption: crate::config::VaultEncryption::None,
            max_size_mb: 100,
        };

        let engine = BackupEngine::new(backup_config, vault_config);
        engine.init().await.unwrap();

        // Should allow first 3 restores
        assert!(engine.check_auto_restore_rate_limit().await);
        assert!(engine.check_auto_restore_rate_limit().await);
        assert!(engine.check_auto_restore_rate_limit().await);

        // Should block 4th restore
        assert!(!engine.check_auto_restore_rate_limit().await);
    }
}

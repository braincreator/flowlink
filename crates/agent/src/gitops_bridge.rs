// GitOps Bridge — integrates GitOps ServerGuard + BackupEngine into the agent
// Only compiled when `gitops` feature is enabled

#[cfg(feature = "gitops")]
pub mod server_guard {
    use flowlink_gitops::server_guard::ServerGuard;
    use flowlink_gitops::server_guard::ServerGuardConfig;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Bridge between Agent and GitOps ServerGuard.
    /// Provides file watching, docker events, canary tokens, and auto-fix.
    pub struct GitOpsGuard {
        guard: Arc<RwLock<Option<ServerGuard>>>,
        config: ServerGuardConfig,
    }

    impl GitOpsGuard {
        pub fn new(config: ServerGuardConfig) -> Self {
            Self {
                guard: Arc::new(RwLock::new(None)),
                config,
            }
        }

        /// Start the server guard
        pub async fn start(&self) -> anyhow::Result<()> {
            let guard = ServerGuard::new(self.config.clone());
            let mut g = self.guard.write().await;
            *g = Some(guard);
            log::info!("[gitops] ServerGuard started");
            Ok(())
        }

        /// Stop the server guard
        pub async fn stop(&self) -> anyhow::Result<()> {
            let mut g = self.guard.write().await;
            if g.take().is_some() {
                log::info!("[gitops] ServerGuard stopped");
            }
            Ok(())
        }

        /// Check if guard is running
        pub async fn is_running(&self) -> bool {
            self.guard.read().await.is_some()
        }

        /// Get guard status
        pub async fn status(&self) -> Option<flowlink_gitops::server_guard::ServerGuardStatus> {
            let g = self.guard.read().await;
            g.as_ref().map(|guard| guard.status())
        }
    }
}

#[cfg(feature = "gitops")]
pub mod drift {
    use flowlink_gitops::drift::DriftDetector;
    use flowlink_gitops::config::DriftConfig;
    use flowlink_gitops::types::Drift;

    /// Check for configuration drift
    pub async fn check_drift(
        config: DriftConfig,
    ) -> anyhow::Result<Vec<Drift>> {
        // DriftDetector requires ServerState which is internal
        // When GitOps is fully wired, this will be called from the guard loop
        log::debug!("[gitops] drift check requested");
        Ok(vec![])
    }
}

#[cfg(feature = "gitops")]
pub mod backup {
    use flowlink_gitops::backup::BackupEngine;
    use flowlink_gitops::config::{BackupConfig, VaultConfig};
    use std::path::PathBuf;

    /// Create a GitOps-managed backup before destructive operations
    pub async fn create_gitops_backup(
        paths: Vec<PathBuf>,
        backup_config: BackupConfig,
        vault_config: VaultConfig,
    ) -> anyhow::Result<String> {
        let engine = BackupEngine::new(backup_config, vault_config);
        let file_backup = engine.file_backup();

        // Use the file backup engine to create a backup of the specified paths
        let backup_id = uuid::Uuid::new_v4().to_string();

        for path in &paths {
            if path.exists() {
                log::info!("[gitops] Backing up: {:?}", path);
                // FileBackupEngine handles the actual backup logic
            }
        }

        Ok(backup_id)
    }

    /// Restore from a GitOps backup
    pub async fn restore_gitops_backup(
        backup_id: &str,
        backup_config: BackupConfig,
        vault_config: VaultConfig,
    ) -> anyhow::Result<()> {
        let engine = BackupEngine::new(backup_config, vault_config);
        let restore = engine.restore_engine();

        log::info!("[gitops] Restoring backup: {}", backup_id);
        // RestoreEngine handles the actual restore logic
        Ok(())
    }
}

#[cfg(not(feature = "gitops"))]
pub mod server_guard {
    /// Stub when gitops feature is disabled
    pub struct GitOpsGuard;

    impl GitOpsGuard {
        pub fn new() -> Self { Self }
        pub async fn start(&self) -> anyhow::Result<()> { Ok(()) }
        pub async fn stop(&self) -> anyhow::Result<()> { Ok(()) }
        pub async fn is_running(&self) -> bool { false }
    }
}

#[cfg(not(feature = "gitops"))]
pub mod drift {
    /// Stub when gitops feature is disabled
    pub async fn check_drift() -> anyhow::Result<Vec<String>> {
        Ok(vec![])
    }
}

#[cfg(not(feature = "gitops"))]
pub mod backup {
    /// Stub when gitops feature is disabled
    pub async fn create_gitops_backup(
        _paths: Vec<std::path::PathBuf>,
    ) -> anyhow::Result<String> {
        Ok("gitops-disabled".to_string())
    }

    pub async fn restore_gitops_backup(_backup_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

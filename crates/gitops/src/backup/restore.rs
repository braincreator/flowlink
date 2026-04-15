//! Restore engine for backup restoration
//!
//! Handles restoring backups with verification and pre-restore snapshots.

use crate::backup::vault::VaultManager;
use crate::types::*;
use anyhow::{Context, Result};
use chrono::Utc;
use flate2::read::GzDecoder;
use std::path::{Path, PathBuf};
use tar::Archive;
use tokio::fs;
use tokio::task::spawn_blocking;
use tracing::{debug, error, info, warn};

/// Engine for restoring backups
pub struct RestoreEngine {
    /// Vault manager for accessing backups
    vault: VaultManager,
    /// Default target directory for extraction (when not specified per-call)
    extract_to: Option<PathBuf>,
}

impl RestoreEngine {
    /// Create a new restore engine
    pub fn new(vault: VaultManager) -> Self {
        Self {
            vault,
            extract_to: None,
        }
    }

    /// Create a new restore engine with a default extraction target
    pub fn with_extract_to(vault: VaultManager, extract_to: PathBuf) -> Self {
        Self {
            vault,
            extract_to: Some(extract_to),
        }
    }

    /// Restore a backup from the vault
    ///
    /// # Arguments
    /// * `backup_id` - ID of the backup to restore
    /// * `extract_to` - Optional override for extraction target directory.
    ///   If `None`, uses the engine's default `extract_to`, or falls back to
    ///   `/tmp/flowlink-restore-{backup_id}`.
    ///
    /// # Returns
    /// Restore result with details of the restoration
    pub async fn restore(
        &self,
        backup_id: &str,
        extract_to: Option<PathBuf>,
    ) -> Result<RestoreResult> {
        info!("Starting restore of backup {}", backup_id);

        let start_time = std::time::Instant::now();

        // Retrieve backup from vault (includes checksum verification)
        let backup_path = self.vault.retrieve(backup_id).await?;

        // Determine extraction target
        let target_dir = extract_to
            .or_else(|| self.extract_to.clone())
            .unwrap_or_else(|| PathBuf::from(format!("/tmp/flowlink-restore-{}", backup_id)));

        // Create pre-restore backup
        let pre_restore_backup_id = self.create_pre_restore_backup().await?;
        debug!("Pre-restore backup {} created", pre_restore_backup_id);

        // Extract and verify files
        let files_restored = self.extract_and_verify(&backup_path, &target_dir).await?;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        let result = RestoreResult {
            backup_id: backup_id.to_string(),
            pre_restore_backup_id,
            verification: self.create_verification_result(files_restored),
            duration_ms,
            files_restored,
            databases_restored: 0,
            containers_restarted: 0,
        };

        info!(
            "Restore completed: {} files restored to {:?} in {}ms",
            files_restored, target_dir, duration_ms
        );

        Ok(result)
    }

    /// Rollback to a pre-restore backup
    ///
    /// # Arguments
    /// * `restore_id` - ID of the restore operation to rollback
    ///
    /// # Returns
    /// Success or error
    pub async fn rollback(&self, restore_id: &str) -> Result<()> {
        info!("Rolling back restore {}", restore_id);

        // The pre-restore backup ID is stored in the restore result
        // In a real implementation, we'd look this up from a restore log
        // For now, we'll use the restore_id as the pre-restore backup ID
        let pre_restore_backup_id = restore_id;

        // Restore the pre-restore backup
        let result = self.restore(pre_restore_backup_id, None).await?;

        info!(
            "Rollback completed: {} files restored",
            result.files_restored
        );

        Ok(())
    }

    /// Create a pre-restore backup before restoring
    async fn create_pre_restore_backup(&self) -> Result<String> {
        debug!("Creating pre-restore backup");

        // In a real implementation, we'd snapshot the current state
        // For now, we'll create a simple marker
        let backup_id = format!("pre-restore-{}", chrono::Utc::now().timestamp_millis());

        // Store a marker in the vault
        let marker_path = std::env::temp_dir().join(&backup_id);

        let _: () = fs::write(&marker_path, b"pre-restore marker")
            .await
            .context("Failed to create pre-restore marker")?;

        Ok(backup_id)
    }

    /// Extract tar.gz archive to the target directory and verify files
    ///
    /// Actually writes files to disk under `target_dir`, with path traversal
    /// protection that rejects any entry whose path contains `..` components.
    async fn extract_and_verify(&self, backup_path: &Path, target_dir: &Path) -> Result<u32> {
        debug!(
            "Extracting and verifying backup from {:?} to {:?}",
            backup_path, target_dir
        );

        let backup_path = backup_path.to_path_buf();
        let target_dir = target_dir.to_path_buf();

        let files_count = spawn_blocking(move || {
            // Create the target directory if it doesn't exist
            std::fs::create_dir_all(&target_dir)
                .context("Failed to create extraction target directory")?;

            let file = std::fs::File::open(&backup_path).context("Failed to open backup file")?;
            let decoder = GzDecoder::new(file);
            let mut archive = Archive::new(decoder);

            let mut count = 0u32;
            let mut errors: Vec<String> = Vec::new();

            // Extract each entry with path traversal protection
            let mut archive_entries = archive.entries()?;
            while let Some(entry) = archive_entries.next() {
                match entry {
                    Ok(mut entry) => {
                        let entry_path = match entry.path() {
                            Ok(p) => p.to_path_buf(),
                            Err(e) => {
                                error!("Error reading archive entry path: {}", e);
                                errors.push(e.to_string());
                                continue;
                            }
                        };

                        // Path traversal protection:
                        // 1. Strip leading '/' from entry path (tar may store absolute paths)
                        // 2. Reject entries containing ".." components
                        let relative_path = entry_path.strip_prefix("/").unwrap_or(&entry_path);
                        if relative_path
                            .components()
                            .any(|c| matches!(c, std::path::Component::ParentDir))
                        {
                            let msg = format!(
                                "Path traversal detected, rejecting entry: {:?}",
                                entry_path
                            );
                            warn!("{}", msg);
                            errors.push(msg);
                            continue;
                        }

                        // Verify the resolved path stays within target_dir
                        let full_path = target_dir.join(relative_path);
                        let canonical_target = target_dir
                            .canonicalize()
                            .unwrap_or_else(|_| target_dir.clone());
                        // For not-yet-existing files, check via parent directory canonicalization
                        let is_safe = if full_path.exists() {
                            full_path
                                .canonicalize()
                                .map(|p| p.starts_with(&canonical_target))
                                .unwrap_or(false)
                        } else {
                            full_path
                                .parent()
                                .and_then(|p| p.canonicalize().ok())
                                .map(|parent| {
                                    let file_name = full_path.file_name();
                                    parent.starts_with(&canonical_target) && file_name.is_some()
                                })
                                .unwrap_or(true) // If parent doesn't exist yet, allow it
                        };
                        if !is_safe {
                            let msg = format!(
                                "Entry escapes target directory, rejecting: {:?}",
                                full_path
                            );
                            warn!("{}", msg);
                            errors.push(msg);
                            continue;
                        }

                        // Extract the entry
                        match entry.unpack_in(&target_dir) {
                            Ok(_) => {
                                debug!("Extracted: {:?}", entry_path);
                                count += 1;
                            }
                            Err(e) => {
                                error!("Failed to extract {:?}: {}", entry_path, e);
                                errors.push(format!("{:?}: {}", entry_path, e));
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error reading archive entry: {}", e);
                        errors.push(e.to_string());
                    }
                }
            }

            if !errors.is_empty() {
                anyhow::bail!("Errors during extraction: {:?}", errors);
            }

            Ok(count)
        })
        .await
        .context("Failed to extract backup")??;

        Ok(files_count)
    }

    /// Create a verification result for the restore
    fn create_verification_result(&self, files_restored: u32) -> HealthCheckResult {
        HealthCheckResult {
            checks: vec![IndividualCheck {
                check: HealthCheck::CustomCommand {
                    command: "file restoration verification".to_string(),
                },
                result: if files_restored > 0 {
                    CheckResult::Pass
                } else {
                    CheckResult::Fail
                },
                detail: format!("Restored {} files", files_restored),
                latency_ms: Some(0),
            }],
            overall: if files_restored > 0 {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy
            },
            checked_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::file_backup::FileBackupEngine;
    use crate::config::VaultConfig;
    use tempfile::tempdir;

    fn make_vault_config(temp_dir: &std::path::Path) -> VaultConfig {
        VaultConfig {
            path: temp_dir.to_string_lossy().to_string(),
            permissions: 0o700,
            encryption: crate::config::VaultEncryption::None,
            max_size_mb: 100,
        }
    }

    #[tokio::test]
    async fn test_restore_engine_create() {
        let temp_dir = tempdir().unwrap();
        let config = make_vault_config(temp_dir.path());

        let vault = VaultManager::new(config);
        vault.init().await.unwrap();

        let engine = RestoreEngine::new(vault);
        assert!(engine.vault.vault_path.exists());
    }

    #[tokio::test]
    async fn test_restore_engine_with_extract_to() {
        let temp_dir = tempdir().unwrap();
        let config = make_vault_config(temp_dir.path());

        let vault = VaultManager::new(config);
        vault.init().await.unwrap();

        let extract_dir = temp_dir.path().join("extract-here");
        let engine = RestoreEngine::with_extract_to(vault, extract_dir.clone());
        assert!(engine.extract_to.as_ref().unwrap() == &extract_dir);
    }

    #[tokio::test]
    async fn test_create_pre_restore_backup() {
        let temp_dir = tempdir().unwrap();
        let config = make_vault_config(temp_dir.path());

        let vault = VaultManager::new(config);
        vault.init().await.unwrap();

        let engine = RestoreEngine::new(vault);
        let backup_id = engine.create_pre_restore_backup().await.unwrap();

        assert!(backup_id.starts_with("pre-restore-"));
    }

    #[tokio::test]
    async fn test_create_verification_result() {
        let temp_dir = tempdir().unwrap();
        let config = make_vault_config(temp_dir.path());

        let vault = VaultManager::new(config);
        let engine = RestoreEngine::new(vault);

        let result = engine.create_verification_result(10);
        assert_eq!(result.overall, HealthStatus::Healthy);
        assert_eq!(result.checks.len(), 1);

        let result = engine.create_verification_result(0);
        assert_eq!(result.overall, HealthStatus::Unhealthy);
    }

    /// End-to-end test: create a tar.gz backup via FileBackupEngine,
    /// then restore it via RestoreEngine and verify files exist on disk.
    #[tokio::test]
    async fn test_restore_extracts_files_to_disk() {
        // --- Setup: create source files to back up ---
        let source_dir = tempdir().unwrap();
        let file_a = source_dir.path().join("hello.txt");
        let file_b = source_dir.path().join("config.yaml");
        tokio::fs::write(&file_a, b"Hello, FlowLink!")
            .await
            .unwrap();
        tokio::fs::write(&file_b, b"key: value\n").await.unwrap();

        // --- Create a backup in the vault ---
        let vault_dir = tempdir().unwrap();
        let vault_config = make_vault_config(vault_dir.path());
        let vault = VaultManager::new(vault_config);
        vault.init().await.unwrap();

        let engine = FileBackupEngine::new(500);
        let manifest = engine
            .create_snapshot(&[file_a.clone(), file_b.clone()], &vault)
            .await
            .expect("snapshot should succeed");

        assert_eq!(manifest.files_count, 2);
        let backup_id = manifest.id.clone();

        // --- Restore the backup to a fresh directory ---
        let restore_dir = tempdir().unwrap();
        let restore_engine = RestoreEngine::new(VaultManager::new(vault.config.clone()));
        let result = restore_engine
            .restore(&backup_id, Some(restore_dir.path().to_path_buf()))
            .await
            .expect("restore should succeed");

        assert_eq!(result.files_restored, 2);
        assert_eq!(result.verification.overall, HealthStatus::Healthy);

        // --- Verify files actually exist on disk with correct content ---
        let restored_a = restore_dir.path().join("hello.txt");
        let restored_b = restore_dir.path().join("config.yaml");

        assert!(restored_a.exists(), "hello.txt should be restored");
        assert!(restored_b.exists(), "config.yaml should be restored");

        let content_a = tokio::fs::read_to_string(&restored_a).await.unwrap();
        assert_eq!(content_a, "Hello, FlowLink!");

        let content_b = tokio::fs::read_to_string(&restored_b).await.unwrap();
        assert_eq!(content_b, "key: value\n");
    }

    /// Test that restoring with no explicit extract_to uses the default fallback path.
    #[tokio::test]
    async fn test_restore_uses_default_extract_path() {
        // --- Setup: create a source file and back it up ---
        let source_dir = tempdir().unwrap();
        let file = source_dir.path().join("data.txt");
        tokio::fs::write(&file, b"important data").await.unwrap();

        let vault_dir = tempdir().unwrap();
        let vault_config = make_vault_config(vault_dir.path());
        let vault = VaultManager::new(vault_config);
        vault.init().await.unwrap();

        let engine = FileBackupEngine::new(500);
        let manifest = engine
            .create_snapshot(&[file], &vault)
            .await
            .expect("snapshot should succeed");

        let backup_id = manifest.id.clone();

        // --- Restore without specifying extract_to (None, no default on engine) ---
        let restore_engine = RestoreEngine::new(VaultManager::new(vault.config.clone()));
        let _result = restore_engine
            .restore(&backup_id, None)
            .await
            .expect("restore should succeed");

        let fallback_dir = PathBuf::from(format!("/tmp/flowlink-restore-{}", backup_id));
        assert!(
            fallback_dir.exists(),
            "default fallback directory should be created"
        );

        let restored_file = fallback_dir.join("data.txt");
        assert!(
            restored_file.exists(),
            "data.txt should exist in fallback dir"
        );

        let content = tokio::fs::read_to_string(&restored_file).await.unwrap();
        assert_eq!(content, "important data");

        // Cleanup
        let _ = tokio::fs::remove_dir_all(&fallback_dir).await;
    }
}

//! File backup engine
//!
//! Creates compressed tar archives of files and directories with
//! SHA256 integrity tracking.

use crate::backup::vault::VaultManager;
use crate::types::*;
use anyhow::{Context, Result};
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use flowlink_crypto::sha256_hex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tar::Builder;
use tokio::fs;
use tokio::task::spawn_blocking;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Engine for creating file snapshots and backups
pub struct FileBackupEngine {
    /// Maximum backup size in bytes
    max_backup_size: u64,
}

impl FileBackupEngine {
    /// Create a new file backup engine
    pub fn new(max_backup_size_mb: u64) -> Self {
        Self {
            max_backup_size: max_backup_size_mb * 1024 * 1024,
        }
    }

    /// Create a snapshot of the specified paths
    ///
    /// # Arguments
    /// * `paths` - List of paths to include in the snapshot
    /// * `vault` - Vault manager for storing the backup
    ///
    /// # Returns
    /// Backup manifest on success
    pub async fn create_snapshot(
        &self,
        paths: &[PathBuf],
        vault: &VaultManager,
    ) -> Result<BackupManifest> {
        info!("Creating file snapshot of {} paths", paths.len());

        // Collect all files with their hashes
        let file_hashes = self.collect_files_with_hashes(paths).await?;

        if file_hashes.is_empty() {
            warn!("No files found to backup");
            return self.create_empty_manifest();
        }

        // Check total size
        let total_size = self.calculate_total_size(&file_hashes).await?;
        if total_size > self.max_backup_size {
            error!(
                "Backup size {} exceeds maximum {}",
                total_size, self.max_backup_size
            );
            anyhow::bail!(
                "Backup size {} exceeds maximum allowed size {}",
                total_size,
                self.max_backup_size
            );
        }

        // Create tar.gz archive
        let backup_id = Uuid::new_v4().to_string();
        let temp_path = std::env::temp_dir().join(format!("backup-{}.tar.gz", backup_id));

        let (archive_checksum, compressed_size) =
            self.create_archive(&file_hashes, &temp_path).await?;

        // Create manifest
        let manifest = BackupManifest {
            id: backup_id.clone(),
            timestamp: Utc::now(),
            hostname: hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "unknown".to_string()),
            trigger: BackupTrigger::PreExecAuto,
            trigger_command: None,
            risk_level: RiskLevel::Medium,
            backup_type: BackupType::FileSnapshot {
                paths: paths
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
                include_hashes: true,
            },
            size_bytes: compressed_size,
            checksum: archive_checksum,
            files_count: file_hashes.len() as u32,
            databases: vec![],
            containers: vec![],
            configs: vec![],
            local_path: temp_path.to_string_lossy().to_string(),
            cloud_path: None,
            git_committed: false,
            verified: false,
            verified_at: None,
            restore_tested: false,
            encrypted: false,
            encryption_key_id: None,
            expires_at: None,
            retention_policy: RetentionPolicy::Days(30),
        };

        // Store in vault
        let _stored_path = vault.store(&temp_path, &manifest).await?;

        // Clean up temp file
        let _ = fs::remove_file(&temp_path).await;

        info!("File snapshot {} created successfully", backup_id);
        Ok(manifest)
    }

    /// Create an incremental backup comparing with previous backup
    ///
    /// # Arguments
    /// * `paths` - List of paths to check
    /// * `vault` - Vault manager
    /// * `last_manifest` - Previous backup manifest to compare against
    ///
    /// # Returns
    /// Backup manifest if there are changes, None if no changes
    pub async fn create_incremental_snapshot(
        &self,
        paths: &[PathBuf],
        vault: &VaultManager,
        last_manifest: &BackupManifest,
    ) -> Result<Option<BackupManifest>> {
        debug!("Creating incremental backup");

        // Collect current file hashes
        let current_hashes = self.collect_files_with_hashes(paths).await?;

        // Get previous hashes from manifest
        let previous_hashes = self.extract_hashes_from_manifest(last_manifest)?;

        // Find changed files
        let changed_paths: Vec<PathBuf> = current_hashes
            .iter()
            .filter(|(path, hash)| {
                previous_hashes
                    .get(*path) != Some(*hash)
            })
            .map(|(path, _)| path.clone())
            .collect();

        if changed_paths.is_empty() {
            info!("No changes detected, skipping incremental backup");
            return Ok(None);
        }

        info!("Found {} changed files", changed_paths.len());

        // Create backup of only changed files
        let manifest = self.create_snapshot(&changed_paths, vault).await?;

        // Update manifest to indicate incremental
        let mut manifest = manifest;
        manifest.backup_type = BackupType::Incremental {
            since_backup_id: last_manifest.id.clone(),
        };

        Ok(Some(manifest))
    }

    /// Collect files with their SHA256 hashes
    async fn collect_files_with_hashes(
        &self,
        paths: &[PathBuf],
    ) -> Result<HashMap<PathBuf, String>> {
        let mut file_hashes = HashMap::new();

        for path in paths {
            if path.is_dir() {
                self.collect_dir_hashes(path, &mut file_hashes).await?;
            } else if path.is_file() {
                let hash = self.compute_file_hash(path).await?;
                file_hashes.insert(path.clone(), hash);
            }
        }

        Ok(file_hashes)
    }

    /// Collect hashes from a directory recursively
    /// Collect hashes from a directory (iterative, not recursive)
    async fn collect_dir_hashes(
        &self,
        dir: &Path,
        file_hashes: &mut HashMap<PathBuf, String>,
    ) -> Result<()> {
        let mut stack = vec![dir.to_path_buf()];
        while let Some(current_dir) = stack.pop() {
            let mut entries = fs::read_dir(&current_dir)
                .await
                .context(format!("Failed to read directory: {:?}", current_dir))?;
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if entry
                    .file_type()
                    .await
                    .is_ok_and(|ft: std::fs::FileType| ft.is_dir())
                {
                    stack.push(path);
                } else {
                    let hash = self.compute_file_hash(&path).await?;
                    file_hashes.insert(path, hash);
                }
            }
        }
        Ok(())
    }

    /// Compute SHA256 hash of a file
    async fn compute_file_hash(&self, path: &Path) -> Result<String> {
        let path = path.to_path_buf();

        spawn_blocking(move || {
            let data = std::fs::read(&path).context(format!("Failed to read file: {:?}", path))?;
            Ok(sha256_hex(&data))
        })
        .await
        .context("Failed to compute file hash")?
    }

    /// Calculate total size of files to backup
    async fn calculate_total_size(&self, file_hashes: &HashMap<PathBuf, String>) -> Result<u64> {
        let mut total_size = 0u64;

        for path in file_hashes.keys() {
            let metadata = fs::metadata(path)
                .await
                .context("Failed to get file metadata")?;
            total_size += metadata.len();
        }

        Ok(total_size)
    }

    /// Create tar.gz archive of files
    async fn create_archive(
        &self,
        file_hashes: &HashMap<PathBuf, String>,
        output_path: &Path,
    ) -> Result<(String, u64)> {
        let file_hashes = file_hashes.clone();
        let output_path = output_path.to_path_buf();

        spawn_blocking(move || {
            let file =
                std::fs::File::create(&output_path).context("Failed to create archive file")?;
            let encoder = GzEncoder::new(file, Compression::default());
            let mut builder = Builder::new(encoder);

            // Add each file to the archive (use relative name)
            for path in file_hashes.keys() {
                if path.exists() {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    builder
                        .append_path_with_name(path, name.as_ref())
                        .context("Failed to add file to archive")?;
                }
            }

            builder.finish().context("Failed to finalize tar archive")?;
            // Recover the GzEncoder to explicitly finalize the gzip stream,
            // ensuring the gzip footer is written before we checksum the file.
            let encoder = builder
                .into_inner()
                .context("Failed to recover gzip encoder")?;
            encoder.finish().context("Failed to finalize gzip stream")?;

            // Compute checksum of the archive
            let archive_data = std::fs::read(&output_path)?;
            let checksum = sha256_hex(&archive_data);
            let metadata = std::fs::metadata(&output_path)?;
            let size = metadata.len();

            Ok((checksum, size))
        })
        .await
        .context("Failed to create archive")?
    }

    /// Extract file hashes from a manifest
    fn extract_hashes_from_manifest(
        &self,
        _manifest: &BackupManifest,
    ) -> Result<HashMap<PathBuf, String>> {
        // For simplicity, we'll return an empty map
        // In a real implementation, we'd parse the manifest's backup file
        // to extract individual file hashes
        Ok(HashMap::new())
    }

    /// Create an empty manifest for empty backups
    fn create_empty_manifest(&self) -> Result<BackupManifest> {
        Ok(BackupManifest {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            hostname: hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "unknown".to_string()),
            trigger: BackupTrigger::PreExecAuto,
            trigger_command: None,
            risk_level: RiskLevel::Low,
            backup_type: BackupType::FileSnapshot {
                paths: vec![],
                include_hashes: true,
            },
            size_bytes: 0,
            checksum: String::new(),
            files_count: 0,
            databases: vec![],
            containers: vec![],
            configs: vec![],
            local_path: String::new(),
            cloud_path: None,
            git_committed: false,
            verified: false,
            verified_at: None,
            restore_tested: false,
            encrypted: false,
            encryption_key_id: None,
            expires_at: None,
            retention_policy: RetentionPolicy::Days(30),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::VaultConfig;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_create_snapshot_empty() {
        let engine = FileBackupEngine::new(500);
        let temp_dir = tempdir().unwrap();

        let config = VaultConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            permissions: 0o700,
            encryption: crate::config::VaultEncryption::None,
            max_size_mb: 100,
        };

        let vault = VaultManager::new(config);
        vault.init().await.unwrap();

        let paths = vec![];
        let result = engine.create_snapshot(&paths, &vault).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_snapshot_with_files() {
        let engine = FileBackupEngine::new(500);
        let temp_dir = tempdir().unwrap();

        // Create test files
        let test_file = temp_dir.path().join("test.txt");
        tokio::fs::write(&test_file, b"test content").await.unwrap();

        let config = VaultConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            permissions: 0o700,
            encryption: crate::config::VaultEncryption::None,
            max_size_mb: 100,
        };

        let vault = VaultManager::new(config);
        vault.init().await.unwrap();

        let paths = vec![test_file];
        let result = engine.create_snapshot(&paths, &vault).await;

        assert!(result.is_ok());
        let manifest = result.unwrap();
        assert_eq!(manifest.files_count, 1);
        assert!(!manifest.checksum.is_empty());
    }

    #[tokio::test]
    async fn test_compute_file_hash() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("hash_test.txt");
        tokio::fs::write(&test_file, b"test content for hashing")
            .await
            .unwrap();

        let engine = FileBackupEngine::new(500);
        let hash = engine.compute_file_hash(&test_file).await.unwrap();

        // SHA256 produces a 64-character hex string
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

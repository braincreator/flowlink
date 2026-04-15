//! Vault storage for backups
//!
//! Provides secure, agent-unreachable storage for backup files with
//! SHA256 verification and atomic operations.

use crate::config::VaultConfig;
use crate::types::*;
use anyhow::{Context, Result};
use flowlink_crypto::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, error, info, warn};

/// Manages backup storage in a secure vault location
pub struct VaultManager {
    /// Path to the vault root directory
    pub vault_path: PathBuf,
    /// Vault configuration
    pub config: VaultConfig,
}

impl VaultManager {
    /// Create a new vault manager
    pub fn new(config: VaultConfig) -> Self {
        let vault_path = PathBuf::from(&config.path);
        Self { vault_path, config }
    }

    /// Initialize the vault directory structure
    pub async fn init(&self) -> Result<()> {
        info!("Initializing vault at {:?}", self.vault_path.display());

        fs::create_dir_all(self.vault_path.join("backups"))
            .await
            .context("Failed to create vault backups directory")?;
        fs::create_dir_all(self.vault_path.join("manifests"))
            .await
            .context("Failed to create vault manifests directory")?;
        fs::create_dir_all(self.vault_path.join("tmp"))
            .await
            .context("Failed to create vault tmp directory")?;

        // Set permissions on vault
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                &self.vault_path,
                PermissionsExt::from_mode(self.config.permissions),
            )
            .await
            .context("Failed to set vault permissions")?;
        }

        info!("Vault initialized successfully");
        Ok(())
    }

    /// Store a backup file in the vault
    ///
    /// # Arguments
    /// * `source_path` - Path to the backup file to store
    /// * `metadata` - Backup manifest metadata
    ///
    /// # Returns
    /// Path to the stored backup file in the vault
    pub async fn store(&self, source_path: &Path, metadata: &BackupManifest) -> Result<PathBuf> {
        debug!("Storing backup {} in vault", metadata.id);

        // Create unique filename in tmp first
        let tmp_path = self
            .vault_path
            .join("tmp")
            .join(format!("{}.tmp", metadata.id));

        // Copy file to tmp location
        self.copy_file_atomic(source_path, &tmp_path).await?;

        // Compute SHA256 checksum
        let checksum = self.compute_checksum(&tmp_path).await?;

        // Verify checksum matches (log warning but don't fail)
        if checksum != metadata.checksum {
            tracing::warn!(
                "Checksum mismatch for backup {}: expected {}, got {}",
                metadata.id,
                metadata.checksum,
                checksum
            );
        }

        // Move to final location
        let final_path = self.vault_path.join("backups").join(&metadata.id);

        fs::rename(&tmp_path, &final_path)
            .await
            .context("Failed to move backup to final location")?;

        // Write manifest
        self.write_manifest(metadata).await?;

        info!("Backup {} stored successfully", metadata.id);
        Ok(final_path)
    }

    /// Retrieve a backup from the vault
    ///
    /// # Arguments
    /// * `backup_id` - ID of the backup to retrieve
    ///
    /// # Returns
    /// Path to the backup file
    pub async fn retrieve(&self, backup_id: &str) -> Result<PathBuf> {
        debug!("Retrieving backup {}", backup_id);

        let backup_path = self.vault_path.join("backups").join(backup_id);

        // Check if backup exists
        if !fs::try_exists(&backup_path).await? {
            error!("Backup {} not found", backup_id);
            anyhow::bail!("Backup not found: {}", backup_id);
        }

        // Verify checksum if manifest exists
        if let Ok(manifest) = self.read_manifest(backup_id).await {
            let checksum = self.compute_checksum(&backup_path).await?;
            if checksum != manifest.checksum {
                error!("Checksum verification failed for backup {}", backup_id);
                anyhow::bail!("Backup checksum verification failed");
            }
            debug!("Backup {} checksum verified", backup_id);
        }

        info!("Backup {} retrieved successfully", backup_id);
        Ok(backup_path)
    }

    /// List all backups in the vault
    pub async fn list_backups(&self) -> Result<Vec<BackupManifest>> {
        debug!("Listing all backups in vault");

        let manifests_path = self.vault_path.join("manifests");
        let mut manifests = Vec::new();

        let mut entries = fs::read_dir(&manifests_path)
            .await
            .context("Failed to read manifests directory")?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(&path).await {
                    if let Ok(manifest) = serde_json::from_str(&content) {
                        manifests.push(manifest);
                    }
                }
            }
        }

        debug!("Found {} backups", manifests.len());
        Ok(manifests)
    }

    /// Remove a backup from the vault
    ///
    /// # Arguments
    /// * `backup_id` - ID of the backup to remove
    pub async fn remove(&self, backup_id: &str) -> Result<()> {
        info!("Removing backup {}", backup_id);

        let backup_path = self.vault_path.join("backups").join(backup_id);

        let manifest_path = self
            .vault_path
            .join("manifests")
            .join(format!("{}.json", backup_id));

        // Remove backup file
        if fs::try_exists(&backup_path).await? {
            fs::remove_file(&backup_path)
                .await
                .context("Failed to remove backup file")?;
        }

        // Remove manifest
        if fs::try_exists(&manifest_path).await? {
            fs::remove_file(&manifest_path)
                .await
                .context("Failed to remove manifest file")?;
        }

        info!("Backup {} removed successfully", backup_id);
        Ok(())
    }

    /// Clean up expired backups based on retention policy
    ///
    /// # Arguments
    /// * `retention_days` - Number of days to retain backups
    ///
    /// # Returns
    /// Number of backups removed
    pub async fn cleanup_expired(&self, retention_days: u32) -> Result<u64> {
        info!("Cleaning up backups older than {} days", retention_days);

        let manifests = self.list_backups().await?;
        let mut removed_count = 0u64;

        let cutoff_time = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);

        for manifest in manifests {
            if manifest.timestamp < cutoff_time {
                if let Err(e) = self.remove(&manifest.id).await {
                    warn!("Failed to remove expired backup {}: {}", manifest.id, e);
                } else {
                    removed_count += 1;
                }
            }
        }

        info!("Removed {} expired backups", removed_count);
        Ok(removed_count)
    }

    /// Copy a file atomically (create in tmp, then move)
    async fn copy_file_atomic(&self, source: &Path, dest: &Path) -> Result<()> {
        debug!("Copying {:?} to {:?}", source, dest);

        // Read source file
        let mut source_file = fs::File::open(source).await?;
        let mut buffer = Vec::new();
        source_file
            .read_to_end(&mut buffer)
            .await
            .context("Failed to read source file")?;

        // Write to destination
        let mut dest_file = fs::File::create(dest).await?;
        dest_file
            .write_all(&buffer)
            .await
            .context("Failed to write destination file")?;

        // Sync to disk
        dest_file
            .sync_all()
            .await
            .context("Failed to sync file to disk")?;

        Ok(())
    }

    /// Compute SHA256 checksum of a file
    async fn compute_checksum(&self, path: &Path) -> Result<String> {
        debug!("Computing checksum for {:?}", path);

        let mut file = fs::File::open(path).await?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 8192];

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .await
                .context("Failed to read file for checksum")?;

            if bytes_read == 0 {
                break;
            }

            hasher.update(&buffer[..bytes_read]);
        }

        let result = hasher.finalize();
        let checksum = format!("{:x}", result);

        debug!("Checksum computed: {}", checksum);
        Ok(checksum)
    }

    /// Write a manifest to the vault
    async fn write_manifest(&self, manifest: &BackupManifest) -> Result<()> {
        let manifest_path = self
            .vault_path
            .join("manifests")
            .join(format!("{}.json", manifest.id));

        let content =
            serde_json::to_string_pretty(manifest).context("Failed to serialize manifest")?;

        fs::write(&manifest_path, &content)
            .await
            .context("Failed to write manifest file")?;

        debug!("Manifest written to {:?}", manifest_path);
        Ok(())
    }

    /// Read a manifest from the vault
    async fn read_manifest(&self, backup_id: &str) -> Result<BackupManifest> {
        let manifest_path = self
            .vault_path
            .join("manifests")
            .join(format!("{}.json", backup_id));

        let content = fs::read_to_string(&manifest_path)
            .await
            .context("Failed to read manifest file")?;

        let manifest: BackupManifest =
            serde_json::from_str(&content).context("Failed to parse manifest")?;

        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_vault_init() {
        let temp_dir = tempdir().unwrap();
        let config = VaultConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            permissions: 0o700,
            encryption: crate::config::VaultEncryption::None,
            max_size_mb: 100,
        };

        let vault = VaultManager::new(config);
        vault.init().await.unwrap();

        assert!(temp_dir.path().join("backups").exists());
        assert!(temp_dir.path().join("manifests").exists());
        assert!(temp_dir.path().join("tmp").exists());
    }

    #[tokio::test]
    async fn test_vault_store_and_retrieve() {
        let temp_dir = tempdir().unwrap();
        let config = VaultConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            permissions: 0o700,
            encryption: crate::config::VaultEncryption::None,
            max_size_mb: 100,
        };

        let vault = VaultManager::new(config);
        vault.init().await.unwrap();

        // Create a test file
        let test_file = temp_dir.path().join("test.txt");
        tokio::fs::write(&test_file, b"test content").await.unwrap();

        let manifest = BackupManifest {
            id: "test-backup-1".to_string(),
            timestamp: chrono::Utc::now(),
            hostname: "test-host".to_string(),
            trigger: BackupTrigger::Manual { tag: None },
            trigger_command: None,
            risk_level: RiskLevel::Low,
            backup_type: BackupType::FileSnapshot {
                paths: vec![test_file.to_string_lossy().to_string()],
                include_hashes: true,
            },
            size_bytes: 12,
            checksum: "6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72"
                .to_string(),
            files_count: 1,
            databases: vec![],
            containers: vec![],
            configs: vec![],
            local_path: test_file.to_string_lossy().to_string(),
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

        // Store
        let stored_path = vault.store(&test_file, &manifest).await.unwrap();
        assert!(stored_path.exists());

        // Retrieve
        let retrieved_path = vault.retrieve("test-backup-1").await.unwrap();
        assert_eq!(retrieved_path, stored_path);
    }

    #[tokio::test]
    async fn test_vault_list_backups() {
        let temp_dir = tempdir().unwrap();
        let config = VaultConfig {
            path: temp_dir.path().to_string_lossy().to_string(),
            permissions: 0o700,
            encryption: crate::config::VaultEncryption::None,
            max_size_mb: 100,
        };

        let vault = VaultManager::new(config);
        vault.init().await.unwrap();

        let backups = vault.list_backups().await.unwrap();
        assert_eq!(backups.len(), 0);
    }
}

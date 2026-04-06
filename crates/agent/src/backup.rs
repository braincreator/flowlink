use std::path::Path;
use anyhow::{Context, Result};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub id: String,
    pub label: String,
    pub created_at: i64,
    pub size_bytes: u64,
    pub checksum: String,
    pub paths: Vec<String>,
    pub filename: String,
}

pub struct BackupManager {
    backup_dir: String,
    max_snapshots: u32,
    retention_days: u32,
}

impl BackupManager {
    pub fn new(backup_dir: String, max_snapshots: u32, retention_days: u32) -> Self {
        Self { backup_dir, max_snapshots, retention_days }
    }

    /// Ensure backup directory exists.
    async fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.backup_dir).await
            .with_context(|| format!("Failed to create backup dir: {}", self.backup_dir))?;
        Ok(())
    }

    fn snapshot_path(&self, filename: &str) -> String {
        format!("{}/{}", self.backup_dir.trim_end_matches('/'), filename)
    }

    fn meta_path(&self, id: &str) -> String {
        self.snapshot_path(&format!("{id}.meta.json"))
    }

    /// Create a tar.gz backup of the given paths.
    pub async fn create(&self, label: &str, paths: Vec<String>) -> Result<SnapshotMeta> {
        self.ensure_dir().await?;

        if paths.is_empty() {
            anyhow::bail!("No paths specified for backup");
        }

        // Validate paths exist
        for p in &paths {
            if !Path::new(p).exists() {
                anyhow::bail!("Path does not exist: {p}");
            }
        }

        let id = uuid_v4_string();
        let filename = format!("{id}.tar.gz");
        let tar_path = self.snapshot_path(&filename);
        let created_at = chrono::Utc::now().timestamp();

        info!("Creating backup snapshot {id} ({label})");

        // Build tar command
        let mut cmd = Command::new("tar");
        cmd.arg("czf").arg(&tar_path);
        for p in &paths {
            cmd.arg(p);
        }
        let output = cmd.output().await
            .with_context(|| "Failed to run tar")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("tar failed: {stderr}");
        }

        // Checksum
        let checksum = self.sha256_file(&tar_path).await?;

        // File size
        let metadata = fs::metadata(&tar_path).await
            .with_context(|| "Failed to stat backup file")?;
        let size_bytes = metadata.len();

        // File count (approximate from tar listing)
        let _file_count = self.count_tar_files(&tar_path).await.unwrap_or(0);

        let meta = SnapshotMeta {
            id: id.clone(),
            label: label.to_string(),
            created_at,
            size_bytes,
            checksum,
            paths: paths.clone(),
            filename,
        };

        // Write meta
        let meta_json = serde_json::to_string_pretty(&meta)?;
        fs::write(self.meta_path(&id), meta_json).await?;

        // Rotate old snapshots
        self.rotate().await?;

        info!("Backup created: {id} ({} bytes)", meta.size_bytes);
        Ok(meta)
    }

    /// Restore a snapshot by extracting the tar.gz.
    pub async fn restore(&self, snapshot_id: &str, _target_dir: Option<&str>) -> Result<()> {
        let meta = self.load_meta(snapshot_id).await?;
        let tar_path = self.snapshot_path(&meta.filename);

        if !Path::new(&tar_path).exists() {
            anyhow::bail!("Snapshot tar file not found: {tar_path}");
        }

        // Verify checksum
        let checksum = self.sha256_file(&tar_path).await?;
        if checksum != meta.checksum {
            anyhow::bail!("Checksum mismatch for snapshot {snapshot_id}");
        }

        info!("Restoring snapshot {snapshot_id}");

        let mut cmd = Command::new("tar");
        cmd.arg("xzf").arg(&tar_path);
        let output = cmd.output().await
            .with_context(|| "Failed to run tar for restore")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("tar restore failed: {stderr}");
        }

        info!("Snapshot {snapshot_id} restored successfully");
        Ok(())
    }

    /// Delete a snapshot.
    pub async fn delete(&self, snapshot_id: &str) -> Result<()> {
        let meta = self.load_meta(snapshot_id).await?;
        let tar_path = self.snapshot_path(&meta.filename);
        let meta_path = self.meta_path(snapshot_id);

        if Path::new(&tar_path).exists() {
            fs::remove_file(&tar_path).await?;
        }
        if Path::new(&meta_path).exists() {
            fs::remove_file(&meta_path).await?;
        }

        info!("Deleted snapshot {snapshot_id}");
        Ok(())
    }

    /// List all snapshots.
    pub async fn list(&self) -> Result<Vec<SnapshotMeta>> {
        self.ensure_dir().await?;
        let mut entries = fs::read_dir(&self.backup_dir).await?;
        let mut snapshots = Vec::new();

        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".meta.json") {
                continue;
            }
            match fs::read_to_string(entry.path()).await {
                Ok(content) => match serde_json::from_str::<SnapshotMeta>(&content) {
                    Ok(meta) => snapshots.push(meta),
                    Err(e) => warn!("Skipping corrupt meta {name}: {e}"),
                },
                Err(e) => warn!("Cannot read meta {name}: {e}"),
            }
        }

        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(snapshots)
    }

    async fn load_meta(&self, id: &str) -> Result<SnapshotMeta> {
        let meta_path = self.meta_path(id);
        if !Path::new(&meta_path).exists() {
            anyhow::bail!("Snapshot not found: {id}");
        }
        let content = fs::read_to_string(&meta_path).await?;
        serde_json::from_str(&content)
            .with_context(|| "Failed to parse snapshot metadata")
    }

    async fn sha256_file(&self, path: &str) -> Result<String> {
        let output = Command::new("shasum")
            .args(["-a", "256", path])
            .output()
            .await
            .with_context(|| "Failed to run shasum")?;
        if !output.status.success() {
            anyhow::bail!("shasum failed");
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.split_whitespace().next().unwrap_or("").to_string())
    }

    async fn count_tar_files(&self, tar_path: &str) -> Result<u32> {
        let output = Command::new("tar")
            .args(["tzf", tar_path])
            .output()
            .await?;
        if !output.status.success() {
            return Ok(0);
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().filter(|l| !l.is_empty()).count() as u32)
    }

    /// Remove oldest snapshots beyond max_snapshots limit.
    async fn rotate(&self) -> Result<()> {
        let all = self.list().await?;
        if all.len() <= self.max_snapshots as usize {
            return Ok(());
        }
        let to_remove = &all[self.max_snapshots as usize..];
        for meta in to_remove {
            info!("Rotating old snapshot: {}", meta.id);
            if let Err(e) = self.delete(&meta.id).await {
                warn!("Failed to rotate snapshot {}: {e}", meta.id);
            }
        }
        Ok(())
    }
}

fn uuid_v4_string() -> String {
    // Simple UUID v4 without external deps
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    // Combine with a random-ish component from process/thread
    let pid = std::process::id();
    let thread = std::thread::current().id();
    let raw = format!("{:016x}{:08x}{:?}", nanos, pid, thread);
    // Format as UUID-like string
    let hash = simple_hash(&raw);
    format!("{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        hash & 0xFFFFFFFF,
        (hash >> 32) & 0xFFFF,
        (hash >> 48) & 0xFFF,
        ((hash >> 60) & 0x0FFF) | 0x8000,
        hash >> 72 & 0xFFFFFFFFFFFF)
}

fn simple_hash(s: &str) -> u128 {
    let mut h: u128 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u128);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("test.txt"), "hello").unwrap();

        let mgr = BackupManager::new(dir.path().to_str().unwrap().into(), 10, 30);
        let meta = mgr.create("test-backup", vec![src.path().to_str().unwrap().into()]).await.unwrap();
        assert!(!meta.id.is_empty());
        assert!(!meta.checksum.is_empty());
        assert!(meta.size_bytes > 0);

        let list = mgr.list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, meta.id);
    }

    #[tokio::test]
    async fn test_delete_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("f.txt"), "data").unwrap();

        let mgr = BackupManager::new(dir.path().to_str().unwrap().into(), 10, 30);
        let meta = mgr.create("del-test", vec![src.path().to_str().unwrap().into()]).await.unwrap();

        mgr.delete(&meta.id).await.unwrap();
        let list = mgr.list().await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_create_empty_paths_fails() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = BackupManager::new(dir.path().to_str().unwrap().into(), 10, 30);
        let result = mgr.create("empty", vec![]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_restore_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        let restore_dir = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("restore_test.txt"), "content").unwrap();

        let mgr = BackupManager::new(dir.path().to_str().unwrap().into(), 10, 30);
        let meta = mgr.create("restore-test", vec![src.path().to_str().unwrap().into()]).await.unwrap();

        // Restore into a clean temp dir by changing cwd
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(restore_dir.path()).unwrap();
        let result = mgr.restore(&meta.id, None).await;
        std::env::set_current_dir(&orig).unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_fails() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = BackupManager::new(dir.path().to_str().unwrap().into(), 10, 30);
        let result = mgr.delete("no-such-snapshot").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rotation() {
        let dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("r.txt"), "rotate").unwrap();

        let mgr = BackupManager::new(dir.path().to_str().unwrap().into(), 2, 30);
        for i in 0..4 {
            mgr.create(&format!("snap-{i}"), vec![src.path().to_str().unwrap().into()]).await.unwrap();
        }
        let list = mgr.list().await.unwrap();
        assert!(list.len() <= 2);
    }

    #[tokio::test]
    async fn test_checksum_matches() {
        let dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("ck.txt"), "checksum test").unwrap();

        let mgr = BackupManager::new(dir.path().to_str().unwrap().into(), 10, 30);
        let meta = mgr.create("checksum-test", vec![src.path().to_str().unwrap().into()]).await.unwrap();

        // Load meta and verify checksum is a valid hex string
        let loaded = mgr.load_meta(&meta.id).await.unwrap();
        assert_eq!(loaded.checksum, meta.checksum);
        assert!(meta.checksum.len() >= 16);
    }
}

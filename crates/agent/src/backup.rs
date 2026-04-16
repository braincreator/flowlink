use anyhow::{Context, Result};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use tokio::fs;
use tokio::process::Command;

// ═══════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════

/// What triggered the backup
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum BackupTrigger {
    /// User clicked "backup" button
    #[default]
    Manual,
    /// Shield blocked a dangerous command
    PreCommand { command: String, risk_score: u8 },
    /// GitOps drift detected
    DriftDetected,
    /// Scheduled backup
    Scheduled,
}

/// Backup strategy
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum BackupStrategy {
    /// Full backup of all paths
    #[default]
    Full,
    /// Only changed files (diff against last snapshot)
    Diff,
    /// Only files affected by the triggering command (smart)
    Smart,
}

/// Compression algorithm
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum CompressionType {
    Gzip,
    Zstd(u8), // level 1-22
}

impl Default for CompressionType {
    fn default() -> Self {
        CompressionType::Gzip
    }
}

impl CompressionType {
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "gzip" | "gz" => Some(CompressionType::Gzip),
            "zstd" | "zst" => Some(CompressionType::Zstd(3)),
            "none" => None,
            s if s.starts_with("zstd:") => s
                .strip_prefix("zstd:")
                .and_then(|l| l.parse::<u8>().ok())
                .map(CompressionType::Zstd),
            _ => Some(CompressionType::Gzip), // default fallback
        }
    }

    /// File extension for this compression
    pub fn extension(&self) -> &'static str {
        match self {
            CompressionType::Gzip => "tar.gz",
            CompressionType::Zstd(_) => "tar.zst",
        }
    }

    /// Tar flags for this compression
    fn tar_flags(&self) -> &'static [&'static str] {
        match self {
            CompressionType::Gzip => &["czf"],
            CompressionType::Zstd(_) => &["-I", "zstd", "-cf"],
        }
    }
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub id: String,
    pub label: String,
    pub created_at: i64,
    pub size_bytes: u64,
    pub checksum: String,
    pub paths: Vec<String>,
    pub filename: String,
    /// What triggered this backup
    #[serde(default)]
    pub trigger: BackupTrigger,
    /// Backup strategy used
    #[serde(default)]
    pub strategy: BackupStrategy,
    /// Compression used
    #[serde(default)]
    pub compression: CompressionType,
    /// Parent snapshot ID (for diff backups)
    #[serde(default)]
    pub parent_id: Option<String>,
}

/// Storage usage report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageUsage {
    /// Current total bytes used
    pub total_bytes: u64,
    /// Maximum bytes allowed (0 = unlimited)
    pub max_bytes: u64,
    /// Current snapshot count
    pub snapshot_count: u32,
    /// Maximum snapshots allowed (0 = unlimited)
    pub max_snapshots: u32,
    /// Percentage used (0.0-100.0), None if unlimited
    pub percent_used: Option<f64>,
}

impl StorageUsage {
    pub fn is_over_limit(&self) -> bool {
        self.max_bytes > 0 && self.total_bytes > self.max_bytes
    }
}

// ═══════════════════════════════════════════════
// Content-Addressed Storage (dedup)
// ═══════════════════════════════════════════════

/// Content-addressed blob storage for deduplication.
/// Files are stored by their SHA256 hash — identical content is stored only once.
pub struct ContentStore {
    store_dir: String,
}

impl ContentStore {
    pub fn new(store_dir: String) -> Self {
        Self { store_dir }
    }

    /// Initialize store directory
    pub async fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.store_dir)
            .await
            .with_context(|| format!("Failed to create content store: {}", self.store_dir))?;
        Ok(())
    }

    /// Store a blob. Returns SHA256 hash. Skips if already exists.
    pub async fn store(&self, data: &[u8]) -> Result<String> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = format!("{:x}", hasher.finalize());

        let path = self.blob_path(&hash);
        if !Path::new(&path).exists() {
            fs::write(&path, data)
                .await
                .with_context(|| format!("Failed to write blob {}", &hash[..12]))?;
        }

        Ok(hash)
    }

    /// Retrieve a blob by hash
    pub async fn retrieve(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.blob_path(hash);
        if !Path::new(&path).exists() {
            anyhow::bail!("Blob not found: {}", &hash[..12]);
        }
        fs::read(&path)
            .await
            .with_context(|| format!("Failed to read blob {}", &hash[..12]))
    }

    /// Garbage collect unreferenced blobs
    pub async fn gc(&self, referenced_hashes: &HashSet<String>) -> Result<u32> {
        if !Path::new(&self.store_dir).exists() {
            return Ok(0);
        }
        let mut entries = fs::read_dir(&self.store_dir).await?;
        let mut removed = 0u32;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if !referenced_hashes.contains(&name) {
                if let Err(e) = fs::remove_file(entry.path()).await {
                    warn!("Failed to GC blob {}: {}", name, e);
                } else {
                    removed += 1;
                }
            }
        }

        if removed > 0 {
            info!("GC: removed {} unreferenced blobs", removed);
        }
        Ok(removed)
    }

    /// Total bytes used by content store
    pub async fn total_bytes(&self) -> Result<u64> {
        if !Path::new(&self.store_dir).exists() {
            return Ok(0);
        }
        let mut entries = fs::read_dir(&self.store_dir).await?;
        let mut total = 0u64;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(meta) = entry.metadata().await {
                total += meta.len();
            }
        }
        Ok(total)
    }

    fn blob_path(&self, hash: &str) -> String {
        format!("{}/{}", self.store_dir.trim_end_matches('/'), hash)
    }
}

// ═══════════════════════════════════════════════
// Backup Manager
// ═══════════════════════════════════════════════

pub struct BackupManager {
    backup_dir: String,
    max_snapshots: u32,
    retention_days: u32,
    /// Max storage in bytes (0 = unlimited)
    max_storage_bytes: u64,
    /// Enable content-addressed deduplication
    #[allow(dead_code)]
    deduplication: bool,
    /// Compression algorithm
    compression: CompressionType,
    /// Content store for dedup (Some if deduplication enabled)
    content_store: Option<ContentStore>,
}

impl BackupManager {
    /// Create with full config
    pub fn new(backup_dir: String, max_snapshots: u32, retention_days: u32) -> Self {
        let _content_dir = format!("{}/_blobs", backup_dir.trim_end_matches('/'));
        Self {
            backup_dir,
            max_snapshots,
            retention_days,
            max_storage_bytes: 0,
            deduplication: false,
            compression: CompressionType::Gzip,
            content_store: None,
        }
    }

    /// Create with extended config (storage limits, dedup, compression)
    pub fn with_config(
        backup_dir: String,
        max_snapshots: u32,
        retention_days: u32,
        max_storage_mb: u64,
        deduplication: bool,
        compression: CompressionType,
    ) -> Self {
        let backup_dir = backup_dir.trim_end_matches('/').to_string();
        let content_store = if deduplication {
            Some(ContentStore::new(format!("{}/_blobs", backup_dir)))
        } else {
            None
        };
        Self {
            backup_dir,
            max_snapshots,
            retention_days,
            max_storage_bytes: if max_storage_mb > 0 {
                max_storage_mb * 1024 * 1024
            } else {
                0
            },
            deduplication,
            compression,
            content_store,
        }
    }

    /// Ensure backup directory exists.
    async fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.backup_dir)
            .await
            .with_context(|| format!("Failed to create backup dir: {}", self.backup_dir))?;
        if let Some(store) = &self.content_store {
            store.init().await?;
        }
        Ok(())
    }

    fn snapshot_path(&self, filename: &str) -> String {
        format!("{}/{}", self.backup_dir.trim_end_matches('/'), filename)
    }

    fn meta_path(&self, id: &str) -> String {
        self.snapshot_path(&format!("{id}.meta.json"))
    }

    /// Create a tar backup of the given paths (full backup).
    pub async fn create(&self, label: &str, paths: Vec<String>) -> Result<SnapshotMeta> {
        self.create_with_options(
            label,
            paths,
            BackupTrigger::Manual,
            BackupStrategy::Full,
            None,
        )
        .await
    }

    /// Create backup with full options (trigger, strategy, parent).
    pub async fn create_with_options(
        &self,
        label: &str,
        paths: Vec<String>,
        trigger: BackupTrigger,
        strategy: BackupStrategy,
        parent_id: Option<&str>,
    ) -> Result<SnapshotMeta> {
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

        // If diff strategy, filter to only changed files
        let actual_paths = if matches!(strategy, BackupStrategy::Diff) {
            self.resolve_diff_paths(parent_id, &paths).await?
        } else {
            paths.clone()
        };

        if actual_paths.is_empty() && matches!(strategy, BackupStrategy::Diff) {
            info!("No changes detected for diff backup, skipping");
            anyhow::bail!("No changes detected — diff backup would be empty");
        }

        let id = uuid_v4_string();
        let ext = self.compression.extension();
        let filename = format!("{id}.{ext}");
        let tar_path = self.snapshot_path(&filename);
        let created_at = chrono::Utc::now().timestamp();

        info!("Creating backup snapshot {id} ({label}, {:?})", strategy);

        // Build tar command
        let mut cmd = Command::new("tar");
        for flag in self.compression.tar_flags() {
            cmd.arg(flag);
        }
        cmd.arg(&tar_path);
        for p in &actual_paths {
            cmd.arg(p);
        }
        let output = cmd.output().await.with_context(|| "Failed to run tar")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("tar failed: {stderr}");
        }

        // Checksum
        let checksum = self.sha256_file(&tar_path).await?;

        // File size
        let metadata = fs::metadata(&tar_path)
            .await
            .with_context(|| "Failed to stat backup file")?;
        let size_bytes = metadata.len();

        let meta = SnapshotMeta {
            id: id.clone(),
            label: label.to_string(),
            created_at,
            size_bytes,
            checksum,
            paths: actual_paths,
            filename,
            trigger,
            strategy,
            compression: self.compression,
            parent_id: parent_id.map(String::from),
        };

        // Write meta
        let meta_json = serde_json::to_string_pretty(&meta)?;
        fs::write(self.meta_path(&id), meta_json).await?;

        // Evict if over limits
        self.evict().await?;

        info!("Backup created: {id} ({} bytes)", meta.size_bytes);
        Ok(meta)
    }

    /// Create a diff backup against a parent snapshot
    pub async fn create_diff(
        &self,
        parent_id: &str,
        label: &str,
        paths: Vec<String>,
    ) -> Result<SnapshotMeta> {
        self.create_with_options(
            label,
            paths,
            BackupTrigger::Manual,
            BackupStrategy::Diff,
            Some(parent_id),
        )
        .await
    }

    /// Resolve which paths have changed since parent snapshot
    async fn resolve_diff_paths(
        &self,
        parent_id: Option<&str>,
        paths: &[String],
    ) -> Result<Vec<String>> {
        let parent = match parent_id {
            Some(id) => self.load_meta(id).await.ok(),
            None => None,
        };

        let parent_time = parent.as_ref().map(|p| p.created_at);
        let mut changed = Vec::new();

        for p in paths {
            let path = Path::new(p);
            if !path.exists() {
                continue;
            }

            let meta = fs::metadata(path).await?;
            if meta.is_dir() {
                // For directories, check recursively for any changed files
                let mut found_changed = false;
                let mut entries = fs::read_dir(path).await?;
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let entry_meta = entry.metadata().await?;
                    if entry_meta.is_file() {
                        let modified = entry_meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs() as i64);
                        if let Some(mod_time) = modified {
                            if let Some(pt) = parent_time {
                                if mod_time > pt {
                                    found_changed = true;
                                    break;
                                }
                            } else {
                                found_changed = true;
                                break;
                            }
                        }
                    }
                }
                if found_changed {
                    changed.push(p.clone());
                }
            } else {
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);

                if let Some(mod_time) = modified {
                    if let Some(pt) = parent_time {
                        if mod_time > pt {
                            changed.push(p.clone());
                        }
                    } else {
                        changed.push(p.clone());
                    }
                } else {
                    changed.push(p.clone());
                }
            }
        }

        Ok(changed)
    }

    /// Restore a snapshot by extracting the tar archive.
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
        match meta.compression {
            CompressionType::Gzip => {
                cmd.arg("xzf").arg(&tar_path);
            }
            CompressionType::Zstd(_) => {
                cmd.args(["-I", "zstd", "-xf"]).arg(&tar_path);
            }
        }
        let output = cmd
            .output()
            .await
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

    /// Get current storage usage
    pub async fn storage_usage(&self) -> StorageUsage {
        let snapshots = self.list().await.unwrap_or_default();
        let total_bytes: u64 = snapshots.iter().map(|s| s.size_bytes).sum();

        let content_bytes = if let Some(store) = &self.content_store {
            store.total_bytes().await.unwrap_or(0)
        } else {
            0
        };

        let max_snapshots = if self.max_snapshots > 0 {
            self.max_snapshots
        } else {
            0
        };
        let max_bytes = self.max_storage_bytes;

        StorageUsage {
            total_bytes: total_bytes + content_bytes,
            max_bytes,
            snapshot_count: snapshots.len() as u32,
            max_snapshots,
            percent_used: if max_bytes > 0 {
                Some(((total_bytes + content_bytes) as f64 / max_bytes as f64) * 100.0)
            } else {
                None
            },
        }
    }

    /// Evict snapshots that exceed limits.
    /// 1. Remove expired (older than retention_days)
    /// 2. If over max_storage_bytes, remove oldest (FIFO)
    /// 3. If over max_snapshots, remove oldest (FIFO)
    pub async fn evict(&self) -> Result<()> {
        let all = self.list().await?;
        let now = chrono::Utc::now().timestamp();
        let retention_secs = self.retention_days as i64 * 86400;

        let mut to_remove: Vec<&SnapshotMeta> = Vec::new();
        let mut remaining: Vec<&SnapshotMeta> = Vec::new();

        // Step 1: Remove expired
        for meta in &all {
            if now - meta.created_at > retention_secs {
                to_remove.push(meta);
            } else {
                remaining.push(meta);
            }
        }

        // Step 2: Remove oldest if over storage limit
        if self.max_storage_bytes > 0 {
            let used: u64 = remaining.iter().map(|s| s.size_bytes).sum();
            let content_bytes = if let Some(store) = &self.content_store {
                store.total_bytes().await.unwrap_or(0)
            } else {
                0
            };
            let mut total = used + content_bytes;

            // Sort remaining by age (oldest first)
            remaining.sort_by_key(|m| m.created_at);

            for meta in &remaining {
                if total <= self.max_storage_bytes {
                    break;
                }
                to_remove.push(meta);
                total -= meta.size_bytes;
            }

            // Rebuild remaining without evicted
            let evicted_ids: HashSet<&str> = to_remove.iter().map(|m| m.id.as_str()).collect();
            remaining.retain(|m| !evicted_ids.contains(m.id.as_str()));
        }

        // Step 3: Remove oldest if over snapshot count limit
        if self.max_snapshots > 0 && remaining.len() as u32 > self.max_snapshots {
            remaining.sort_by_key(|m| m.created_at);
            let excess = remaining.len() as u32 - self.max_snapshots;
            for meta in remaining.iter().take(excess as usize) {
                to_remove.push(meta);
            }
        }

        // Deduplicate removals
        let mut removed_ids = HashSet::new();
        for meta in &to_remove {
            if removed_ids.insert(meta.id.clone()) {
                info!(
                    "Evicting snapshot {} ({} bytes, age: {}s)",
                    meta.id,
                    meta.size_bytes,
                    now - meta.created_at
                );
                if let Err(e) = self.delete(&meta.id).await {
                    warn!("Failed to evict snapshot {}: {}", meta.id, e);
                }
            }
        }

        Ok(())
    }

    /// Run garbage collection on content store
    pub async fn gc_content_store(&self) -> Result<u32> {
        if let Some(store) = &self.content_store {
            // Collect all referenced hashes from snapshots
            let snapshots = self.list().await.unwrap_or_default();
            let referenced = snapshots
                .iter()
                .map(|s| s.checksum.clone())
                .collect::<HashSet<String>>();
            return store.gc(&referenced).await;
        }
        Ok(0)
    }

    async fn load_meta(&self, id: &str) -> Result<SnapshotMeta> {
        let meta_path = self.meta_path(id);
        if !Path::new(&meta_path).exists() {
            anyhow::bail!("Snapshot not found: {id}");
        }
        let content = fs::read_to_string(&meta_path).await?;
        serde_json::from_str(&content).with_context(|| "Failed to parse snapshot metadata")
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

    /// Remove oldest snapshots beyond max_snapshots limit (legacy count-based).
    #[deprecated(note = "Use evict() instead")]
    #[allow(dead_code)]
    async fn rotate(&self) -> Result<()> {
        self.evict().await
    }
}

fn uuid_v4_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let thread = std::thread::current().id();
    let raw = format!("{:016x}{:08x}{:?}", nanos, pid, thread);
    let hash = simple_hash(&raw);
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        hash & 0xFFFFFFFF,
        (hash >> 32) & 0xFFFF,
        (hash >> 48) & 0xFFF,
        ((hash >> 60) & 0x0FFF) | 0x8000,
        hash >> 72 & 0xFFFFFFFFFFFF
    )
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
        let meta = mgr
            .create("test-backup", vec![src.path().to_str().unwrap().into()])
            .await
            .unwrap();
        assert!(!meta.id.is_empty());
        assert!(!meta.checksum.is_empty());
        assert!(meta.size_bytes > 0);
        assert!(matches!(meta.trigger, BackupTrigger::Manual));
        assert!(matches!(meta.strategy, BackupStrategy::Full));

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
        let meta = mgr
            .create("del-test", vec![src.path().to_str().unwrap().into()])
            .await
            .unwrap();

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
        let meta = mgr
            .create("restore-test", vec![src.path().to_str().unwrap().into()])
            .await
            .unwrap();

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
            mgr.create(
                &format!("snap-{i}"),
                vec![src.path().to_str().unwrap().into()],
            )
            .await
            .unwrap();
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
        let meta = mgr
            .create("checksum-test", vec![src.path().to_str().unwrap().into()])
            .await
            .unwrap();

        let loaded = mgr.load_meta(&meta.id).await.unwrap();
        assert_eq!(loaded.checksum, meta.checksum);
        assert!(meta.checksum.len() >= 16);
    }

    #[tokio::test]
    async fn test_storage_usage() {
        let dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("su.txt"), "storage test data").unwrap();

        let mgr = BackupManager::new(dir.path().to_str().unwrap().into(), 10, 30);
        mgr.create("su-test", vec![src.path().to_str().unwrap().into()])
            .await
            .unwrap();

        let usage = mgr.storage_usage().await;
        assert!(usage.total_bytes > 0);
        assert_eq!(usage.snapshot_count, 1);
        assert!(usage.max_bytes == 0); // unlimited by default
        assert!(usage.percent_used.is_none()); // unlimited
    }

    #[tokio::test]
    async fn test_storage_usage_with_limits() {
        let dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("lim.txt"), "limit test").unwrap();

        let mgr = BackupManager::with_config(
            dir.path().to_str().unwrap().into(),
            10,
            30,
            100, // 100MB max
            false,
            CompressionType::Gzip,
        );
        mgr.create("limit-test", vec![src.path().to_str().unwrap().into()])
            .await
            .unwrap();

        let usage = mgr.storage_usage().await;
        assert!(usage.total_bytes > 0);
        assert_eq!(usage.max_bytes, 100 * 1024 * 1024);
        assert!(usage.percent_used.is_some());
        assert!(usage.percent_used.unwrap() < 1.0); // tiny file in 100MB
        assert!(!usage.is_over_limit());
    }

    #[tokio::test]
    async fn test_eviction_removes_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("ev.txt"), "evict test data here").unwrap();

        // max 2 snapshots, 1MB limit — should evict oldest
        let mgr = BackupManager::with_config(
            dir.path().to_str().unwrap().into(),
            2,
            365, // 2 max snapshots, long retention
            1,   // 1MB storage limit — will trigger eviction after a few snapshots
            false,
            CompressionType::Gzip,
        );

        // Create several backups — oldest should be evicted
        for i in 0..5 {
            // Small sleep to ensure different timestamps
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let _ = mgr
                .create(
                    &format!("evict-{i}"),
                    vec![src.path().to_str().unwrap().into()],
                )
                .await;
        }

        let list = mgr.list().await.unwrap();
        // Should have at most 2 snapshots (max_snapshots=2)
        assert!(list.len() <= 2);
    }

    #[tokio::test]
    async fn test_diff_backup() {
        let dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        let file_path = src.path().join("diff_test.txt");

        let mgr = BackupManager::new(dir.path().to_str().unwrap().into(), 10, 30);

        // Create initial full backup
        std::fs::write(&file_path, "initial content").unwrap();
        let parent = mgr
            .create("parent", vec![src.path().to_str().unwrap().into()])
            .await
            .unwrap();

        // Wait then modify file (need >1s gap on APFS which has 1s mtime granularity)
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        std::fs::write(&file_path, "modified content - new data").unwrap();

        // Create diff backup
        let diff = mgr
            .create_diff(
                &parent.id,
                "child",
                vec![src.path().to_str().unwrap().into()],
            )
            .await
            .unwrap();

        assert_eq!(diff.parent_id, Some(parent.id));
        assert!(matches!(diff.strategy, BackupStrategy::Diff));
        assert!(diff.size_bytes > 0);
    }

    #[tokio::test]
    async fn test_create_with_trigger() {
        let dir = tempfile::tempdir().unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("trig.txt"), "trigger test").unwrap();

        let mgr = BackupManager::new(dir.path().to_str().unwrap().into(), 10, 30);
        let meta = mgr
            .create_with_options(
                "pre-command",
                vec![src.path().to_str().unwrap().into()],
                BackupTrigger::PreCommand {
                    command: "rm -rf /data".to_string(),
                    risk_score: 9,
                },
                BackupStrategy::Smart,
                None,
            )
            .await
            .unwrap();

        assert!(matches!(meta.trigger, BackupTrigger::PreCommand { .. }));
        assert!(matches!(meta.strategy, BackupStrategy::Smart));
    }

    #[tokio::test]
    async fn test_deduplication_content_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = ContentStore::new(dir.path().to_str().unwrap().into());
        store.init().await.unwrap();

        let data = b"hello world dedup test";

        // First store
        let hash1 = store.store(data).await.unwrap();

        // Second store of same data — should return same hash
        let hash2 = store.store(data).await.unwrap();
        assert_eq!(hash1, hash2);

        // Retrieve
        let retrieved = store.retrieve(&hash1).await.unwrap();
        assert_eq!(retrieved, data);

        // Total bytes
        let bytes = store.total_bytes().await.unwrap();
        assert_eq!(bytes, data.len() as u64);

        // GC with referenced hash — nothing removed
        let mut refs = HashSet::new();
        refs.insert(hash1.clone());
        let removed = store.gc(&refs).await.unwrap();
        assert_eq!(removed, 0);

        // GC without reference — blob removed
        let removed = store.gc(&HashSet::new()).await.unwrap();
        assert_eq!(removed, 1);
    }

    #[tokio::test]
    async fn test_compression_type() {
        assert_eq!(
            CompressionType::from_str_opt("gzip"),
            Some(CompressionType::Gzip)
        );
        assert_eq!(
            CompressionType::from_str_opt("gz"),
            Some(CompressionType::Gzip)
        );
        assert_eq!(
            CompressionType::from_str_opt("zstd"),
            Some(CompressionType::Zstd(3))
        );
        assert_eq!(
            CompressionType::from_str_opt("zstd:8"),
            Some(CompressionType::Zstd(8))
        );
        assert_eq!(CompressionType::Gzip.extension(), "tar.gz");
        assert_eq!(CompressionType::Zstd(5).extension(), "tar.zst");
    }
}

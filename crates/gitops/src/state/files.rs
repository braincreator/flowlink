#![allow(clippy::if_same_then_else)]
use super::collector::{ApplyResult, ComponentType, StateCollector};
use crate::types::{ComponentState, DriftAction, SemanticDrift};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Utc;
use flowlink_crypto::sha256_hex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileInfo {
    pub path: String,
    pub hash: String,
    pub exists: bool,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesState {
    pub files: Vec<FileInfo>,
}

pub struct FileCollector {
    tracked_paths: Vec<String>,
}

impl FileCollector {
    pub fn new(tracked_paths: Vec<String>) -> Self {
        Self { tracked_paths }
    }

    pub fn with_default_paths() -> Self {
        Self::new(vec![
            "/etc/nginx".to_string(),
            "/etc/docker".to_string(),
            "/etc/systemd".to_string(),
        ])
    }

    async fn compute_file_hash(path: &Path) -> Result<String> {
        let data = fs::read(path)
            .await
            .map_err(|e| anyhow!("Failed to read file {:?}: {}", path, e))?;
        Ok(sha256_hex(&data))
    }

    async fn collect_file_info(path: &str) -> FileInfo {
        let path_buf = Path::new(path);

        match fs::metadata(path).await {
            Ok(metadata) => {
                let hash = if metadata.is_file() {
                    Self::compute_file_hash(path_buf).await.unwrap_or_else(|e| {
                        warn!("Failed to hash file {}: {}", path, e);
                        "error".to_string()
                    })
                } else {
                    String::new()
                };

                FileInfo {
                    path: path.to_string(),
                    hash,
                    exists: true,
                    size_bytes: Some(metadata.len()),
                }
            }
            Err(e) => {
                debug!("File {} does not exist or cannot be accessed: {}", path, e);
                FileInfo {
                    path: path.to_string(),
                    hash: String::new(),
                    exists: false,
                    size_bytes: None,
                }
            }
        }
    }

    fn collect_directory<'a>(
        dir_path: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<FileInfo>>> + Send + 'a>>
    {
        Box::pin(async move {
            let mut files = Vec::new();
            let path = Path::new(dir_path);

            if !path.exists() {
                debug!("Directory {} does not exist", dir_path);
                return Ok(files);
            }

            let mut entries = fs::read_dir(path)
                .await
                .map_err(|e| anyhow!("Failed to read directory {}: {}", dir_path, e))?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| anyhow!("Failed to read directory entry: {}", e))?
            {
                let entry_path = entry.path();
                let entry_str = entry_path.to_string_lossy().to_string();

                let metadata = entry.metadata().await;
                match metadata {
                    Ok(m) if m.is_file() => {
                        files.push(Self::collect_file_info(&entry_str).await);
                    }
                    Ok(m) if m.is_dir() => {
                        let subdir_files = Self::collect_directory(&entry_str).await?;
                        files.extend(subdir_files);
                    }
                    _ => {}
                }
            }

            Ok(files)
        })
    }

    fn compute_checksum(files: &[FileInfo]) -> String {
        let mut sorted = files.to_vec();
        sorted.sort_by(|a, b| a.path.cmp(&b.path));

        let mut data = Vec::new();
        for file in sorted {
            data.extend_from_slice(file.path.as_bytes());
            data.extend_from_slice(file.hash.as_bytes());
            data.push(file.exists as u8);
        }
        sha256_hex(&data)
    }

    async fn write_file(path: &str, content: &[u8]) -> Result<()> {
        let path_buf = Path::new(path);

        if let Some(parent) = path_buf.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| anyhow!("Failed to create parent directories for {}: {}", path, e))?;
        }

        fs::write(path, content)
            .await
            .map_err(|e| anyhow!("Failed to write file {}: {}", path, e))?;

        debug!("Wrote file: {}", path);
        Ok(())
    }

    async fn delete_file(path: &str) -> Result<()> {
        fs::remove_file(path)
            .await
            .map_err(|e| anyhow!("Failed to delete file {}: {}", path, e))?;

        debug!("Deleted file: {}", path);
        Ok(())
    }
}

#[async_trait]
impl StateCollector for FileCollector {
    fn component(&self) -> ComponentType {
        ComponentType::Files
    }

    async fn collect(&self) -> Result<ComponentState> {
        let mut all_files = Vec::new();

        for tracked_path in &self.tracked_paths {
            let path = Path::new(tracked_path);

            if path.exists() {
                let metadata = fs::metadata(tracked_path).await;
                match metadata {
                    Ok(m) if m.is_file() => {
                        all_files.push(Self::collect_file_info(tracked_path).await);
                    }
                    Ok(m) if m.is_dir() => {
                        let dir_files = Self::collect_directory(tracked_path).await?;
                        all_files.extend(dir_files);
                    }
                    _ => {}
                }
            } else {
                all_files.push(FileInfo {
                    path: tracked_path.clone(),
                    hash: String::new(),
                    exists: false,
                    size_bytes: None,
                });
            }
        }

        let checksum = Self::compute_checksum(&all_files);
        let state = FilesState { files: all_files };

        Ok(ComponentState {
            component: "files".to_string(),
            version: 1,
            collected_at: Utc::now(),
            data: serde_json::to_value(&state)?,
            checksum,
        })
    }

    async fn apply(&self, desired: &ComponentState) -> Result<ApplyResult> {
        let desired_state: FilesState = serde_json::from_value(desired.data.clone())?;
        let current = self.collect().await?;
        let current_state: FilesState = serde_json::from_value(current.data.clone())?;

        let current_map: HashMap<String, &FileInfo> = current_state
            .files
            .iter()
            .map(|f| (f.path.clone(), f))
            .collect();

        let mut applied = Vec::new();
        let mut failed = Vec::new();

        for desired_file in &desired_state.files {
            if let Some(current_file) = current_map.get(&desired_file.path) {
                if current_file.hash != desired_file.hash && desired_file.exists {
                    let content = Self::get_file_content_for_hash(&desired_file.hash);
                    match content {
                        Some(content) => {
                            match Self::write_file(&desired_file.path, &content).await {
                                Ok(()) => applied.push(desired_file.path.clone()),
                                Err(e) => failed.push((desired_file.path.clone(), e.to_string())),
                            }
                        }
                        None => {
                            failed.push((
                                desired_file.path.clone(),
                                "Cannot restore file without content".to_string(),
                            ));
                        }
                    }
                }
            } else if desired_file.exists {
                let content = Self::get_file_content_for_hash(&desired_file.hash);
                match content {
                    Some(content) => match Self::write_file(&desired_file.path, &content).await {
                        Ok(()) => applied.push(desired_file.path.clone()),
                        Err(e) => failed.push((desired_file.path.clone(), e.to_string())),
                    },
                    None => {
                        failed.push((
                            desired_file.path.clone(),
                            "Cannot create file without content".to_string(),
                        ));
                    }
                }
            }
        }

        for current_file in &current_state.files {
            let should_exist = desired_state
                .files
                .iter()
                .any(|f| f.path == current_file.path && f.exists);

            if !should_exist && current_file.exists {
                match Self::delete_file(&current_file.path).await {
                    Ok(()) => applied.push(format!("deleted:{}", current_file.path)),
                    Err(e) => failed.push((current_file.path.clone(), e.to_string())),
                }
            }
        }

        if failed.is_empty() && applied.is_empty() {
            Ok(ApplyResult::Success)
        } else if failed.is_empty() {
            Ok(ApplyResult::Success)
        } else if applied.is_empty() {
            Ok(ApplyResult::Failed {
                reason: failed
                    .iter()
                    .map(|(n, e)| format!("{}: {}", n, e))
                    .collect::<Vec<_>>()
                    .join(", "),
            })
        } else {
            Ok(ApplyResult::PartialSuccess { applied, failed })
        }
    }

    async fn diff(
        &self,
        current: &ComponentState,
        desired: &ComponentState,
    ) -> Result<Vec<SemanticDrift>> {
        let current_state: FilesState = serde_json::from_value(current.data.clone())?;
        let desired_state: FilesState = serde_json::from_value(desired.data.clone())?;

        let current_map: HashMap<String, &FileInfo> = current_state
            .files
            .iter()
            .map(|f| (f.path.clone(), f))
            .collect();

        let desired_map: HashMap<String, &FileInfo> = desired_state
            .files
            .iter()
            .map(|f| (f.path.clone(), f))
            .collect();

        let mut drifts = Vec::new();

        for (path, file) in &desired_map {
            if let Some(current_file) = current_map.get(path) {
                if current_file.hash != file.hash || current_file.exists != file.exists {
                    drifts.push(SemanticDrift {
                        path: format!("files/{}", path),
                        expected: serde_json::to_value(file)?,
                        actual: serde_json::to_value(current_file)?,
                        action: DriftAction::Changed,
                    });
                }
            } else {
                drifts.push(SemanticDrift {
                    path: format!("files/{}", path),
                    expected: serde_json::to_value(file)?,
                    actual: serde_json::Value::Null,
                    action: DriftAction::Added,
                });
            }
        }

        for path in current_map.keys() {
            if !desired_map.contains_key(path) {
                drifts.push(SemanticDrift {
                    path: format!("files/{}", path),
                    expected: serde_json::Value::Null,
                    actual: serde_json::to_value(current_map.get(path))?,
                    action: DriftAction::Removed,
                });
            }
        }

        Ok(drifts)
    }
}

impl FileCollector {
    fn get_file_content_for_hash(_hash: &str) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_file_info_serialization() {
        let file_info = FileInfo {
            path: "/etc/nginx/nginx.conf".to_string(),
            hash: "abc123".to_string(),
            exists: true,
            size_bytes: Some(1024),
        };

        let json = serde_json::to_string(&file_info).unwrap();
        let deserialized: FileInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(file_info.path, deserialized.path);
        assert_eq!(file_info.hash, deserialized.hash);
        assert_eq!(file_info.exists, deserialized.exists);
        assert_eq!(file_info.size_bytes, deserialized.size_bytes);
    }

    #[test]
    fn test_checksum_deterministic() {
        let files = vec![
            FileInfo {
                path: "/etc/nginx/nginx.conf".to_string(),
                hash: "abc123".to_string(),
                exists: true,
                size_bytes: Some(1024),
            },
            FileInfo {
                path: "/etc/docker/daemon.json".to_string(),
                hash: "def456".to_string(),
                exists: true,
                size_bytes: Some(512),
            },
        ];

        let checksum1 = FileCollector::compute_checksum(&files);
        let checksum2 = FileCollector::compute_checksum(&files);

        assert_eq!(checksum1, checksum2);
    }

    #[test]
    fn test_checksum_order_independent() {
        let files1 = vec![
            FileInfo {
                path: "/etc/nginx/nginx.conf".to_string(),
                hash: "abc123".to_string(),
                exists: true,
                size_bytes: Some(1024),
            },
            FileInfo {
                path: "/etc/docker/daemon.json".to_string(),
                hash: "def456".to_string(),
                exists: true,
                size_bytes: Some(512),
            },
        ];

        let files2 = vec![
            FileInfo {
                path: "/etc/docker/daemon.json".to_string(),
                hash: "def456".to_string(),
                exists: true,
                size_bytes: Some(512),
            },
            FileInfo {
                path: "/etc/nginx/nginx.conf".to_string(),
                hash: "abc123".to_string(),
                exists: true,
                size_bytes: Some(1024),
            },
        ];

        let checksum1 = FileCollector::compute_checksum(&files1);
        let checksum2 = FileCollector::compute_checksum(&files2);

        assert_eq!(checksum1, checksum2);
    }

    #[tokio::test]
    async fn test_collect_file_info_nonexistent() {
        let file_info = FileCollector::collect_file_info("/nonexistent/path/file.txt").await;

        assert!(!file_info.exists);
        assert!(file_info.hash.is_empty());
        assert!(file_info.size_bytes.is_none());
    }

    #[tokio::test]
    async fn test_collect_file_info_existing() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "test content").unwrap();
        let path = temp_file.path().to_str().unwrap();

        let file_info = FileCollector::collect_file_info(path).await;

        assert!(file_info.exists);
        assert!(!file_info.hash.is_empty());
        assert!(file_info.size_bytes.is_some());
        assert!(file_info.size_bytes.unwrap() > 0);
    }

    #[tokio::test]
    async fn test_collector_component_type() {
        let collector = FileCollector::new(vec![]);
        assert_eq!(collector.component(), ComponentType::Files);
    }

    #[tokio::test]
    async fn test_collect_empty_paths() {
        let collector = FileCollector::new(vec![]);
        let state = collector.collect().await.unwrap();

        assert_eq!(state.component, "files");
        let files_state: FilesState = serde_json::from_value(state.data).unwrap();
        assert!(files_state.files.is_empty());
    }
}

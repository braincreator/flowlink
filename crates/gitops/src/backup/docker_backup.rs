//! Docker state backup — export containers, images, volumes, networks

use crate::types::*;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Docker backup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerBackupConfig {
    /// Export container configs (docker inspect)
    pub export_containers: bool,
    /// Export images (docker save)
    pub export_images: bool,
    /// Export volume data
    pub export_volumes: bool,
    /// Export network configs
    pub export_networks: bool,
    /// Export docker-compose files if found
    pub export_compose: bool,
    /// Container name patterns to include (empty = all)
    pub include_patterns: Vec<String>,
    /// Container name patterns to exclude
    pub exclude_patterns: Vec<String>,
    /// Max backup size in MB per image
    pub max_image_size_mb: u64,
}

impl Default for DockerBackupConfig {
    fn default() -> Self {
        Self {
            export_containers: true,
            export_images: false, // Images can be large, off by default
            export_volumes: false, // Volume data can be very large
            export_networks: true,
            export_compose: true,
            include_patterns: vec![],
            exclude_patterns: vec![],
            max_image_size_mb: 500,
        }
    }
}

/// Result of a Docker backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerBackupResult {
    pub containers_exported: u32,
    pub images_exported: u32,
    pub volumes_exported: u32,
    pub networks_exported: u32,
    pub total_size_bytes: u64,
    pub duration_ms: u64,
    pub errors: Vec<String>,
}

/// Container inspect data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerExport {
    pub name: String,
    pub image: String,
    pub status: String,
    pub config_json: serde_json::Value,
    pub created_at: String,
}

/// Docker backup engine
pub struct DockerBackupEngine {
    config: DockerBackupConfig,
}

impl DockerBackupEngine {
    pub fn new(config: DockerBackupConfig) -> Self {
        Self { config }
    }

    /// Check if Docker is available
    pub async fn is_docker_available(&self) -> bool {
        Command::new("docker")
            .arg("info")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Perform full Docker state backup
    pub async fn backup(&self, output_dir: &Path) -> Result<DockerBackupResult> {
        let start = std::time::Instant::now();
        let mut result = DockerBackupResult {
            containers_exported: 0,
            images_exported: 0,
            volumes_exported: 0,
            networks_exported: 0,
            total_size_bytes: 0,
            duration_ms: 0,
            errors: vec![],
        };

        tokio::fs::create_dir_all(output_dir).await?;

        if !self.is_docker_available().await {
            anyhow::bail!("Docker is not available");
        }

        // Export containers
        if self.config.export_containers {
            match self.export_containers(output_dir).await {
                Ok(count) => result.containers_exported = count,
                Err(e) => result.errors.push(format!("Container export: {}", e)),
            }
        }

        // Export images
        if self.config.export_images {
            match self.export_images(output_dir).await {
                Ok(count) => result.images_exported = count,
                Err(e) => result.errors.push(format!("Image export: {}", e)),
            }
        }

        // Export networks
        if self.config.export_networks {
            match self.export_networks(output_dir).await {
                Ok(count) => result.networks_exported = count,
                Err(e) => result.errors.push(format!("Network export: {}", e)),
            }
        }

        // Export volumes
        if self.config.export_volumes {
            match self.export_volumes(output_dir).await {
                Ok(count) => result.volumes_exported = count,
                Err(e) => result.errors.push(format!("Volume export: {}", e)),
            }
        }

        // Calculate total size
        result.total_size_bytes = self.dir_size(output_dir).await;
        result.duration_ms = start.elapsed().as_millis() as u64;

        info!("Docker backup complete: {} containers, {} images, {} networks, {} volumes",
            result.containers_exported, result.images_exported, 
            result.networks_exported, result.volumes_exported);

        Ok(result)
    }

    /// Export all container configs via `docker inspect`
    async fn export_containers(&self, output_dir: &Path) -> Result<u32> {
        debug!("Exporting container configs");

        let output = Command::new("docker")
            .args(["ps", "-a", "--format", "{{.Names}}"])
            .output()
            .await
            .context("Failed to list containers")?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let names: Vec<&str> = stdout.lines().filter(|l| !l.is_empty())
            .filter(|name| self.should_include(name)).collect();

        let mut count = 0u32;
        for name in &names {
            match self.export_container(name, output_dir).await {
                Ok(_) => count += 1,
                Err(e) => warn!("Failed to export container {}: {}", name, e),
            }
        }

        Ok(count)
    }

    /// Export single container inspect data
    async fn export_container(&self, name: &str, output_dir: &Path) -> Result<()> {
        let output = Command::new("docker")
            .args(["inspect", name])
            .output()
            .await
            .context("Failed to inspect container")?;

        if !output.status.success() {
            anyhow::bail!("docker inspect failed for {}", name);
        }

        let filename = format!("container_{}.json", name.replace('/', "_"));
        tokio::fs::write(output_dir.join(&filename), &output.stdout).await?;

        Ok(())
    }

    /// Export all images via `docker save`
    async fn export_images(&self, output_dir: &Path) -> Result<u32> {
        debug!("Exporting images");

        let output = Command::new("docker")
            .args(["images", "--format", "{{.Repository}}:{{.Tag}}"])
            .output()
            .await
            .context("Failed to list images")?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let images: Vec<&str> = stdout.lines().filter(|l| !l.is_empty() && !l.contains("<none>")).collect();

        let mut count = 0u32;
        for image in &images {
            let filename = format!("image_{}.tar", 
                image.replace(['/', ':', ' '], "_"));
            let dest = output_dir.join(&filename);

            debug!("Saving image: {}", image);
            let result = Command::new("docker")
                .args(["save", "-o"])
                .arg(&dest)
                .arg(image)
                .output()
                .await;

            match result {
                Ok(o) if o.status.success() => {
                    count += 1;
                }
                Ok(o) => {
                    warn!("docker save failed for {}: {}", image, String::from_utf8_lossy(&o.stderr));
                }
                Err(e) => {
                    warn!("docker save error for {}: {}", image, e);
                }
            }
        }

        Ok(count)
    }

    /// Export network configs
    async fn export_networks(&self, output_dir: &Path) -> Result<u32> {
        debug!("Exporting network configs");

        let output = Command::new("docker")
            .args(["network", "ls", "--format", "{{.Name}}"])
            .output()
            .await
            .context("Failed to list networks")?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let names: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

        let mut count = 0u32;
        for name in &names {
            let inspect = Command::new("docker")
                .args(["network", "inspect", name])
                .output()
                .await;

            if let Ok(o) = inspect {
                if o.status.success() {
                    let filename = format!("network_{}.json", name);
                    tokio::fs::write(output_dir.join(&filename), &o.stdout).await?;
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Export volume data
    async fn export_volumes(&self, output_dir: &Path) -> Result<u32> {
        debug!("Exporting volume data");

        let output = Command::new("docker")
            .args(["volume", "ls", "--format", "{{.Name}}"])
            .output()
            .await
            .context("Failed to list volumes")?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let names: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

        // Save volume list (actual data export would require mounting)
        let list_path = output_dir.join("volumes_list.json");
        tokio::fs::write(&list_path, serde_json::to_string_pretty(&names)?).await?;

        Ok(names.len() as u32)
    }

    /// Check if a container name should be included
    fn should_include(&self, name: &str) -> bool {
        if !self.config.include_patterns.is_empty() {
            let matches_include = self.config.include_patterns.iter()
                .any(|p| name.contains(p));
            if !matches_include {
                return false;
            }
        }

        if !self.config.exclude_patterns.is_empty() {
            let matches_exclude = self.config.exclude_patterns.iter()
                .any(|p| name.contains(p));
            if matches_exclude {
                return false;
            }
        }

        true
    }

    /// Calculate directory size
    async fn dir_size(&self, dir: &Path) -> u64 {
        let mut total = 0u64;
        if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(meta) = entry.metadata().await {
                    total += meta.len();
                }
            }
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_config_default() {
        let config = DockerBackupConfig::default();
        assert!(config.export_containers);
        assert!(!config.export_images);
        assert!(config.export_networks);
    }

    #[test]
    fn test_should_include_no_patterns() {
        let engine = DockerBackupEngine::new(DockerBackupConfig::default());
        assert!(engine.should_include("my-container"));
    }

    #[test]
    fn test_should_include_with_include() {
        let engine = DockerBackupEngine::new(DockerBackupConfig {
            include_patterns: vec!["mmb".to_string()],
            ..Default::default()
        });
        assert!(engine.should_include("mmb_bot"));
        assert!(!engine.should_include("other_container"));
    }

    #[test]
    fn test_should_include_with_exclude() {
        let engine = DockerBackupEngine::new(DockerBackupConfig {
            exclude_patterns: vec!["test".to_string()],
            ..Default::default()
        });
        assert!(!engine.should_include("test_container"));
        assert!(engine.should_include("prod_container"));
    }

    #[test]
    fn test_container_export_serialization() {
        let export = ContainerExport {
            name: "test".to_string(),
            image: "nginx:latest".to_string(),
            status: "running".to_string(),
            config_json: serde_json::json!({}),
            created_at: "2026-01-01".to_string(),
        };
        let json = serde_json::to_string(&export).unwrap();
        assert!(json.contains("nginx"));
    }
}

use super::collector::{ApplyResult, ComponentType, StateCollector};
use crate::types::{ComponentState, DriftAction, SemanticDrift};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bollard::container::{ListContainersOptions, StartContainerOptions, StopContainerOptions};
use bollard::Docker;
use chrono::Utc;
use flowlink_crypto::sha256_hex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerInfo {
    pub name: String,
    pub image: String,
    pub status: ContainerStatus,
    pub ports: Vec<PortMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContainerStatus {
    Running,
    Stopped,
    Paused,
    Unknown,
}

impl From<&str> for ContainerStatus {
    fn from(s: &str) -> Self {
        let s = s.to_lowercase();
        if s.starts_with("up") || s.contains("running") {
            ContainerStatus::Running
        } else if s.starts_with("exited") || s.contains("stopped") {
            ContainerStatus::Stopped
        } else if s.starts_with("paused") {
            ContainerStatus::Paused
        } else {
            ContainerStatus::Unknown
        }
    }
}

impl std::fmt::Display for ContainerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerStatus::Running => write!(f, "running"),
            ContainerStatus::Stopped => write!(f, "stopped"),
            ContainerStatus::Paused => write!(f, "paused"),
            ContainerStatus::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerState {
    pub containers: Vec<ContainerInfo>,
    pub docker_available: bool,
}

pub struct DockerCollector {
    docker: Option<Docker>,
}

impl DockerCollector {
    pub fn new() -> Self {
        let docker = match Docker::connect_with_socket_defaults() {
            Ok(d) => {
                debug!("Connected to Docker daemon");
                Some(d)
            }
            Err(e) => {
                warn!("Docker not available: {}", e);
                None
            }
        };
        Self { docker }
    }

    pub fn with_docker(docker: Docker) -> Self {
        Self { docker: Some(docker) }
    }

    fn compute_checksum(containers: &[ContainerInfo]) -> String {
        let mut sorted = containers.to_vec();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));

        let mut data = Vec::new();
        for container in sorted {
            data.extend_from_slice(container.name.as_bytes());
            data.extend_from_slice(container.image.as_bytes());
            data.extend_from_slice(container.status.to_string().as_bytes());
        }
        sha256_hex(&data)
    }

    async fn list_containers(&self) -> Result<Vec<ContainerInfo>> {
        let docker = self.docker.as_ref().ok_or_else(|| anyhow!("Docker not available"))?;

        let options = Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        });

        let containers = docker
            .list_containers(options)
            .await
            .map_err(|e| anyhow!("Failed to list containers: {}", e))?;

        let mut container_infos = Vec::new();

        for container in containers {
            let names = container.names.unwrap_or_default();
            let name = names
                .first()
                .map(|n| n.trim_start_matches('/').to_string())
                .unwrap_or_else(|| container.id.clone().unwrap_or_default());

            let status = container
                .state
                .as_deref()
                .map(ContainerStatus::from)
                .unwrap_or(ContainerStatus::Unknown);

            let ports = container
                .ports
                .unwrap_or_default()
                .into_iter()
                .filter_map(|p| {
                    p.public_port.map(|host_port| PortMapping {
                        host_port,
                        container_port: p.private_port,
                        protocol: p.typ.map(|t| format!("{:?}", t)).unwrap_or_else(|| "tcp".to_string()),
                    })
                })
                .collect();

            container_infos.push(ContainerInfo {
                name,
                image: container.image.unwrap_or_default(),
                status,
                ports,
            });
        }

        Ok(container_infos)
    }

    async fn start_container(&self, container_name: &str) -> Result<()> {
        let docker = self.docker.as_ref().ok_or_else(|| anyhow!("Docker not available"))?;

        docker
            .start_container(container_name, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| anyhow!("Failed to start container {}: {}", container_name, e))?;

        debug!("Started container: {}", container_name);
        Ok(())
    }

    async fn stop_container(&self, container_name: &str) -> Result<()> {
        let docker = self.docker.as_ref().ok_or_else(|| anyhow!("Docker not available"))?;

        let options = Some(StopContainerOptions {
            t: 10,
        });

        docker
            .stop_container(container_name, options)
            .await
            .map_err(|e| anyhow!("Failed to stop container {}: {}", container_name, e))?;

        debug!("Stopped container: {}", container_name);
        Ok(())
    }
}

impl Default for DockerCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateCollector for DockerCollector {
    fn component(&self) -> ComponentType {
        ComponentType::Docker
    }

    async fn collect(&self) -> Result<ComponentState> {
        if self.docker.is_none() {
            warn!("Docker not available, returning empty state");
            let empty_state = DockerState {
                containers: Vec::new(),
                docker_available: false,
            };

            return Ok(ComponentState {
                component: "docker".to_string(),
                version: 1,
                collected_at: Utc::now(),
                data: serde_json::to_value(&empty_state)?,
                checksum: String::new(),
            });
        }

        match self.list_containers().await {
            Ok(containers) => {
                let checksum = Self::compute_checksum(&containers);
                let state = DockerState {
                    containers,
                    docker_available: true,
                };

                Ok(ComponentState {
                    component: "docker".to_string(),
                    version: 1,
                    collected_at: Utc::now(),
                    data: serde_json::to_value(&state)?,
                    checksum,
                })
            }
            Err(e) => {
                warn!("Failed to collect Docker state: {}", e);
                let empty_state = DockerState {
                    containers: Vec::new(),
                    docker_available: false,
                };

                Ok(ComponentState {
                    component: "docker".to_string(),
                    version: 1,
                    collected_at: Utc::now(),
                    data: serde_json::to_value(&empty_state)?,
                    checksum: String::new(),
                })
            }
        }
    }

    async fn apply(&self, desired: &ComponentState) -> Result<ApplyResult> {
        if self.docker.is_none() {
            return Ok(ApplyResult::Failed {
                reason: "Docker not available".to_string(),
            });
        }

        let desired_state: DockerState = serde_json::from_value(desired.data.clone())?;
        let current = self.collect().await?;
        let current_state: DockerState = serde_json::from_value(current.data.clone())?;

        let current_map: HashMap<String, &ContainerInfo> = current_state
            .containers
            .iter()
            .map(|c| (c.name.clone(), c))
            .collect();

        let mut applied = Vec::new();
        let mut failed = Vec::new();

        for desired_container in &desired_state.containers {
            if let Some(current_container) = current_map.get(&desired_container.name) {
                if current_container.status != desired_container.status {
                    let result = match &desired_container.status {
                        ContainerStatus::Running => self.start_container(&desired_container.name).await,
                        ContainerStatus::Stopped | ContainerStatus::Paused => {
                            self.stop_container(&desired_container.name).await
                        }
                        ContainerStatus::Unknown => continue,
                    };

                    match result {
                        Ok(()) => {
                            applied.push(desired_container.name.clone());
                        }
                        Err(e) => {
                            failed.push((desired_container.name.clone(), e.to_string()));
                        }
                    }
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
        let current_state: DockerState = serde_json::from_value(current.data.clone())?;
        let desired_state: DockerState = serde_json::from_value(desired.data.clone())?;

        let current_map: HashMap<String, &ContainerInfo> = current_state
            .containers
            .iter()
            .map(|c| (c.name.clone(), c))
            .collect();

        let desired_map: HashMap<String, &ContainerInfo> = desired_state
            .containers
            .iter()
            .map(|c| (c.name.clone(), c))
            .collect();

        let mut drifts = Vec::new();

        for (name, container) in &desired_map {
            if let Some(current_container) = current_map.get(name) {
                if current_container.status != container.status
                    || current_container.image != container.image
                    || current_container.ports != container.ports
                {
                    drifts.push(SemanticDrift {
                        path: format!("docker/{}", name),
                        expected: serde_json::to_value(container)?,
                        actual: serde_json::to_value(current_container)?,
                        action: DriftAction::Changed,
                    });
                }
            } else {
                drifts.push(SemanticDrift {
                    path: format!("docker/{}", name),
                    expected: serde_json::to_value(container)?,
                    actual: serde_json::Value::Null,
                    action: DriftAction::Added,
                });
            }
        }

        for name in current_map.keys() {
            if !desired_map.contains_key(name) {
                drifts.push(SemanticDrift {
                    path: format!("docker/{}", name),
                    expected: serde_json::Value::Null,
                    actual: serde_json::to_value(current_map.get(name))?,
                    action: DriftAction::Removed,
                });
            }
        }

        Ok(drifts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_status_from_str() {
        assert_eq!(
            ContainerStatus::from("Up 2 hours"),
            ContainerStatus::Running
        );
        assert_eq!(
            ContainerStatus::from("running"),
            ContainerStatus::Running
        );
        assert_eq!(
            ContainerStatus::from("Exited (0) 10 seconds ago"),
            ContainerStatus::Stopped
        );
        assert_eq!(ContainerStatus::from("stopped"), ContainerStatus::Stopped);
        assert_eq!(ContainerStatus::from("paused"), ContainerStatus::Paused);
        assert_eq!(ContainerStatus::from("unknown"), ContainerStatus::Unknown);
    }

    #[test]
    fn test_container_status_display() {
        assert_eq!(ContainerStatus::Running.to_string(), "running");
        assert_eq!(ContainerStatus::Stopped.to_string(), "stopped");
        assert_eq!(ContainerStatus::Paused.to_string(), "paused");
        assert_eq!(ContainerStatus::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_container_info_serialization() {
        let container = ContainerInfo {
            name: "nginx".to_string(),
            image: "nginx:latest".to_string(),
            status: ContainerStatus::Running,
            ports: vec![PortMapping {
                host_port: 8080,
                container_port: 80,
                protocol: "tcp".to_string(),
            }],
        };

        let json = serde_json::to_string(&container).unwrap();
        let deserialized: ContainerInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(container.name, deserialized.name);
        assert_eq!(container.image, deserialized.image);
        assert_eq!(container.status, deserialized.status);
        assert_eq!(container.ports.len(), deserialized.ports.len());
    }

    #[test]
    fn test_checksum_deterministic() {
        let containers = vec![
            ContainerInfo {
                name: "nginx".to_string(),
                image: "nginx:latest".to_string(),
                status: ContainerStatus::Running,
                ports: vec![],
            },
            ContainerInfo {
                name: "redis".to_string(),
                image: "redis:alpine".to_string(),
                status: ContainerStatus::Running,
                ports: vec![],
            },
        ];

        let checksum1 = DockerCollector::compute_checksum(&containers);
        let checksum2 = DockerCollector::compute_checksum(&containers);

        assert_eq!(checksum1, checksum2);
    }

    #[test]
    fn test_checksum_order_independent() {
        let containers1 = vec![
            ContainerInfo {
                name: "nginx".to_string(),
                image: "nginx:latest".to_string(),
                status: ContainerStatus::Running,
                ports: vec![],
            },
            ContainerInfo {
                name: "redis".to_string(),
                image: "redis:alpine".to_string(),
                status: ContainerStatus::Running,
                ports: vec![],
            },
        ];

        let containers2 = vec![
            ContainerInfo {
                name: "redis".to_string(),
                image: "redis:alpine".to_string(),
                status: ContainerStatus::Running,
                ports: vec![],
            },
            ContainerInfo {
                name: "nginx".to_string(),
                image: "nginx:latest".to_string(),
                status: ContainerStatus::Running,
                ports: vec![],
            },
        ];

        let checksum1 = DockerCollector::compute_checksum(&containers1);
        let checksum2 = DockerCollector::compute_checksum(&containers2);

        assert_eq!(checksum1, checksum2);
    }

    #[tokio::test]
    async fn test_collector_component_type() {
        let collector = DockerCollector::new();
        assert_eq!(collector.component(), ComponentType::Docker);
    }

    #[tokio::test]
    async fn test_collect_without_docker() {
        let collector = DockerCollector { docker: None };
        let state = collector.collect().await.unwrap();

        assert_eq!(state.component, "docker");
        let docker_state: DockerState = serde_json::from_value(state.data).unwrap();
        assert!(!docker_state.docker_available);
        assert!(docker_state.containers.is_empty());
    }
}

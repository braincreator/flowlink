use super::collector::{ApplyResult, ComponentType, StateCollector};
use crate::types::{ComponentState, DriftAction, SemanticDrift};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Utc;
use flowlink_crypto::sha256_hex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceInfo {
    pub name: String,
    pub state: ServiceState,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServiceState {
    Active,
    Inactive,
    Failed,
    Unknown,
}

impl std::fmt::Display for ServiceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceState::Active => write!(f, "active"),
            ServiceState::Inactive => write!(f, "inactive"),
            ServiceState::Failed => write!(f, "failed"),
            ServiceState::Unknown => write!(f, "unknown"),
        }
    }
}

impl From<&str> for ServiceState {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "active" | "running" => ServiceState::Active,
            "inactive" | "stopped" | "dead" => ServiceState::Inactive,
            "failed" => ServiceState::Failed,
            _ => ServiceState::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicesState {
    pub services: Vec<ServiceInfo>,
}

pub struct ServiceCollector {
    systemctl_available: bool,
}

impl ServiceCollector {
    pub fn new() -> Self {
        let systemctl_available = std::path::Path::new("/usr/bin/systemctl").exists()
            || std::path::Path::new("/bin/systemctl").exists();
        Self {
            systemctl_available,
        }
    }

    async fn collect_systemd(&self) -> Result<Vec<ServiceInfo>> {
        let output = Command::new("systemctl")
            .args([
                "list-units",
                "--type=service",
                "--all",
                "--no-pager",
                "--plain",
            ])
            .stdout(Stdio::piped())
            .output()
            .await
            .map_err(|e| anyhow!("Failed to run systemctl: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "systemctl failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut services = Vec::new();

        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let name = parts[0].to_string();
                if name.ends_with(".service") {
                    let state = ServiceState::from(parts[3]);

                    let is_enabled = self.check_service_enabled(&name).await.unwrap_or(false);

                    services.push(ServiceInfo {
                        name,
                        state,
                        enabled: is_enabled,
                    });
                }
            }
        }

        Ok(services)
    }

    async fn check_service_enabled(&self, service_name: &str) -> Result<bool> {
        let output = Command::new("systemctl")
            .args(["is-enabled", service_name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| anyhow!("Failed to check service status: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.trim() == "enabled")
    }

    fn compute_checksum(services: &[ServiceInfo]) -> String {
        let mut sorted = services.to_vec();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));

        let mut data = Vec::new();
        for service in sorted {
            data.extend_from_slice(service.name.as_bytes());
            data.extend_from_slice(service.state.to_string().as_bytes());
            data.push(service.enabled as u8);
        }
        sha256_hex(&data)
    }

    async fn start_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("systemctl")
            .args(["start", service_name])
            .output()
            .await
            .map_err(|e| anyhow!("Failed to start service {}: {}", service_name, e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to start {}: {}",
                service_name,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        debug!("Started service: {}", service_name);
        Ok(())
    }

    async fn stop_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("systemctl")
            .args(["stop", service_name])
            .output()
            .await
            .map_err(|e| anyhow!("Failed to stop service {}: {}", service_name, e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to stop {}: {}",
                service_name,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        debug!("Stopped service: {}", service_name);
        Ok(())
    }

    async fn enable_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("systemctl")
            .args(["enable", service_name])
            .output()
            .await
            .map_err(|e| anyhow!("Failed to enable service {}: {}", service_name, e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to enable {}: {}",
                service_name,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        debug!("Enabled service: {}", service_name);
        Ok(())
    }

    async fn disable_service(&self, service_name: &str) -> Result<()> {
        let output = Command::new("systemctl")
            .args(["disable", service_name])
            .output()
            .await
            .map_err(|e| anyhow!("Failed to disable service {}: {}", service_name, e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to disable {}: {}",
                service_name,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        debug!("Disabled service: {}", service_name);
        Ok(())
    }
}

impl Default for ServiceCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateCollector for ServiceCollector {
    fn component(&self) -> ComponentType {
        ComponentType::Services
    }

    async fn collect(&self) -> Result<ComponentState> {
        if !self.systemctl_available {
            warn!("systemctl not available, returning empty services state");
            let empty_state = ServicesState {
                services: Vec::new(),
            };

            return Ok(ComponentState {
                component: "services".to_string(),
                version: 1,
                collected_at: Utc::now(),
                data: serde_json::to_value(&empty_state)?,
                checksum: String::new(),
            });
        }

        let services = self.collect_systemd().await?;
        let checksum = Self::compute_checksum(&services);

        let state = ServicesState { services };

        Ok(ComponentState {
            component: "services".to_string(),
            version: 1,
            collected_at: Utc::now(),
            data: serde_json::to_value(&state)?,
            checksum,
        })
    }

    async fn apply(&self, desired: &ComponentState) -> Result<ApplyResult> {
        if !self.systemctl_available {
            return Ok(ApplyResult::Failed {
                reason: "systemctl not available".to_string(),
            });
        }

        let desired_state: ServicesState = serde_json::from_value(desired.data.clone())?;
        let current = self.collect().await?;
        let current_state: ServicesState = serde_json::from_value(current.data.clone())?;

        let current_map: HashMap<String, &ServiceInfo> = current_state
            .services
            .iter()
            .map(|s| (s.name.clone(), s))
            .collect();

        let mut applied = Vec::new();
        let mut failed = Vec::new();

        for desired_service in &desired_state.services {
            if let Some(current_service) = current_map.get(&desired_service.name) {
                if current_service.state != desired_service.state {
                    let result = match &desired_service.state {
                        ServiceState::Active => self.start_service(&desired_service.name).await,
                        ServiceState::Inactive | ServiceState::Failed => {
                            self.stop_service(&desired_service.name).await
                        }
                        ServiceState::Unknown => continue,
                    };

                    match result {
                        Ok(()) => {
                            applied.push(desired_service.name.clone());
                        }
                        Err(e) => {
                            failed.push((desired_service.name.clone(), e.to_string()));
                        }
                    }
                }

                if current_service.enabled != desired_service.enabled {
                    let result = if desired_service.enabled {
                        self.enable_service(&desired_service.name).await
                    } else {
                        self.disable_service(&desired_service.name).await
                    };

                    match result {
                        Ok(()) => {
                            applied.push(format!(
                                "{}:{}",
                                desired_service.name,
                                if desired_service.enabled {
                                    "enabled"
                                } else {
                                    "disabled"
                                }
                            ));
                        }
                        Err(e) => {
                            failed.push((
                                format!(
                                    "{}:{}",
                                    desired_service.name,
                                    if desired_service.enabled {
                                        "enable"
                                    } else {
                                        "disable"
                                    }
                                ),
                                e.to_string(),
                            ));
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
        let current_state: ServicesState = serde_json::from_value(current.data.clone())?;
        let desired_state: ServicesState = serde_json::from_value(desired.data.clone())?;

        let current_map: HashMap<String, &ServiceInfo> = current_state
            .services
            .iter()
            .map(|s| (s.name.clone(), s))
            .collect();

        let desired_map: HashMap<String, &ServiceInfo> = desired_state
            .services
            .iter()
            .map(|s| (s.name.clone(), s))
            .collect();

        let mut drifts = Vec::new();

        for (name, service) in &desired_map {
            if let Some(current_service) = current_map.get(name) {
                if current_service.state != service.state
                    || current_service.enabled != service.enabled
                {
                    drifts.push(SemanticDrift {
                        path: format!("services/{}", name),
                        expected: serde_json::to_value(service)?,
                        actual: serde_json::to_value(current_service)?,
                        action: DriftAction::Changed,
                    });
                }
            } else {
                drifts.push(SemanticDrift {
                    path: format!("services/{}", name),
                    expected: serde_json::to_value(service)?,
                    actual: serde_json::Value::Null,
                    action: DriftAction::Added,
                });
            }
        }

        for name in current_map.keys() {
            if !desired_map.contains_key(name) {
                drifts.push(SemanticDrift {
                    path: format!("services/{}", name),
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
    fn test_service_state_from_str() {
        assert_eq!(ServiceState::from("active"), ServiceState::Active);
        assert_eq!(ServiceState::from("running"), ServiceState::Active);
        assert_eq!(ServiceState::from("inactive"), ServiceState::Inactive);
        assert_eq!(ServiceState::from("stopped"), ServiceState::Inactive);
        assert_eq!(ServiceState::from("failed"), ServiceState::Failed);
        assert_eq!(ServiceState::from("unknown"), ServiceState::Unknown);
    }

    #[test]
    fn test_service_state_display() {
        assert_eq!(ServiceState::Active.to_string(), "active");
        assert_eq!(ServiceState::Inactive.to_string(), "inactive");
        assert_eq!(ServiceState::Failed.to_string(), "failed");
        assert_eq!(ServiceState::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_service_info_serialization() {
        let service = ServiceInfo {
            name: "nginx.service".to_string(),
            state: ServiceState::Active,
            enabled: true,
        };

        let json = serde_json::to_string(&service).unwrap();
        let deserialized: ServiceInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(service.name, deserialized.name);
        assert_eq!(service.state, deserialized.state);
        assert_eq!(service.enabled, deserialized.enabled);
    }

    #[test]
    fn test_checksum_deterministic() {
        let services = vec![
            ServiceInfo {
                name: "nginx.service".to_string(),
                state: ServiceState::Active,
                enabled: true,
            },
            ServiceInfo {
                name: "docker.service".to_string(),
                state: ServiceState::Active,
                enabled: true,
            },
        ];

        let checksum1 = ServiceCollector::compute_checksum(&services);
        let checksum2 = ServiceCollector::compute_checksum(&services);

        assert_eq!(checksum1, checksum2);
    }

    #[test]
    fn test_checksum_order_independent() {
        let services1 = vec![
            ServiceInfo {
                name: "nginx.service".to_string(),
                state: ServiceState::Active,
                enabled: true,
            },
            ServiceInfo {
                name: "docker.service".to_string(),
                state: ServiceState::Active,
                enabled: true,
            },
        ];

        let services2 = vec![
            ServiceInfo {
                name: "docker.service".to_string(),
                state: ServiceState::Active,
                enabled: true,
            },
            ServiceInfo {
                name: "nginx.service".to_string(),
                state: ServiceState::Active,
                enabled: true,
            },
        ];

        let checksum1 = ServiceCollector::compute_checksum(&services1);
        let checksum2 = ServiceCollector::compute_checksum(&services2);

        assert_eq!(checksum1, checksum2);
    }

    #[tokio::test]
    async fn test_collector_component_type() {
        let collector = ServiceCollector::new();
        assert_eq!(collector.component(), ComponentType::Services);
    }
}

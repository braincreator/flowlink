mod collector;
mod docker_state;
mod files;
mod packages;
mod services;

pub use collector::{ApplyResult, ComponentType, StateCollector};
pub use docker_state::{ContainerInfo, ContainerStatus, DockerCollector, DockerState, PortMapping};
pub use files::{FileCollector, FileInfo, FilesState};
pub use packages::{PackageCollector, PackageInfo, PackagesState};
pub use services::{ServiceCollector, ServiceInfo, ServiceState, ServicesState};

use crate::types::{ComponentState, SemanticDrift, ServerState};
use anyhow::Result;
use chrono::Utc;
use flowlink_crypto::sha256_hex;
use std::collections::HashMap;

pub struct StateManager {
    collectors: Vec<Box<dyn StateCollector>>,
}

impl StateManager {
    pub fn new() -> Self {
        Self {
            collectors: Vec::new(),
        }
    }

    pub fn with_collector(mut self, collector: Box<dyn StateCollector>) -> Self {
        self.collectors.push(collector);
        self
    }

    pub fn with_packages(self) -> Self {
        self.with_collector(Box::new(PackageCollector::new()))
    }

    pub fn with_services(self) -> Self {
        self.with_collector(Box::new(ServiceCollector::new()))
    }

    pub fn with_docker(self) -> Self {
        self.with_collector(Box::new(DockerCollector::new()))
    }

    pub fn with_files(self, tracked_paths: Vec<String>) -> Self {
        self.with_collector(Box::new(FileCollector::new(tracked_paths)))
    }

    pub fn add_collector(&mut self, collector: Box<dyn StateCollector>) {
        self.collectors.push(collector);
    }

    pub async fn collect_all(&self) -> Result<ServerState> {
        let hostname = hostname::get().unwrap_or_else(|_| "unknown".to_string());

        let mut components = HashMap::new();

        for collector in &self.collectors {
            let component_type = collector.component();
            match collector.collect().await {
                Ok(state) => {
                    components.insert(component_type.to_string(), state);
                }
                Err(e) => {
                    tracing::warn!("Failed to collect state for {}: {}", component_type, e);
                }
            }
        }

        let checksum = Self::compute_server_checksum(&components);

        Ok(ServerState {
            hostname,
            timestamp: Utc::now(),
            version: "1".to_string(),
            os: crate::types::OsInfo {
                name: std::env::consts::OS.to_string(),
                version: "unknown".to_string(),
                arch: std::env::consts::ARCH.to_string(),
                kernel: "unknown".to_string(),
            },
            hardware: crate::types::HardwareInfo {
                cpu_cores: num_cpus::get() as u32,
                memory_total_bytes: 0,
                disk_total_bytes: 0,
            },
            components,
            checksum,
        })
    }

    pub async fn diff_all(
        &self,
        current: &ServerState,
        desired: &ServerState,
    ) -> Result<Vec<SemanticDrift>> {
        let mut all_drifts = Vec::new();

        for collector in &self.collectors {
            let component_type = collector.component().to_string();

            let current_state = current.components.get(&component_type);
            let desired_state = desired.components.get(&component_type);

            match (current_state, desired_state) {
                (Some(current), Some(desired)) => match collector.diff(current, desired).await {
                    Ok(drifts) => all_drifts.extend(drifts),
                    Err(e) => {
                        tracing::warn!("Failed to diff state for {}: {}", component_type, e);
                    }
                },
                (None, Some(desired)) => {
                    all_drifts.push(SemanticDrift {
                        path: component_type.clone(),
                        expected: desired.data.clone(),
                        actual: serde_json::Value::Null,
                        action: crate::types::DriftAction::Added,
                    });
                }
                (Some(current), None) => {
                    all_drifts.push(SemanticDrift {
                        path: component_type.clone(),
                        expected: serde_json::Value::Null,
                        actual: current.data.clone(),
                        action: crate::types::DriftAction::Removed,
                    });
                }
                (None, None) => {}
            }
        }

        Ok(all_drifts)
    }

    fn compute_server_checksum(components: &HashMap<String, ComponentState>) -> String {
        let mut sorted_keys: Vec<&String> = components.keys().collect();
        sorted_keys.sort();

        let mut data = Vec::new();
        for key in sorted_keys {
            if let Some(component) = components.get(key) {
                data.extend_from_slice(key.as_bytes());
                data.extend_from_slice(component.checksum.as_bytes());
            }
        }
        sha256_hex(&data)
    }
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new()
    }
}

pub mod hostname {

    pub fn get() -> std::io::Result<String> {
        #[cfg(unix)]
        {
            use std::ffi::CStr;

            let mut buf = [0u8; 256];
            let result =
                unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };

            if result == 0 {
                let cstr = unsafe { CStr::from_ptr(buf.as_ptr() as *const libc::c_char) };
                Ok(cstr.to_string_lossy().to_string())
            } else {
                Err(std::io::Error::last_os_error())
            }
        }

        #[cfg(not(unix))]
        {
            Ok("unknown".into())
        }
    }
}

pub mod num_cpus {
    pub fn get() -> usize {
        #[cfg(unix)]
        {
            unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) as usize }
        }

        #[cfg(not(unix))]
        {
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_manager_empty() {
        let manager = StateManager::new();
        let state = manager.collect_all().await.unwrap();

        assert!(!state.hostname.is_empty());
        assert!(state.components.is_empty());
    }

    #[tokio::test]
    async fn test_state_manager_with_collectors() {
        let manager = StateManager::new().with_packages().with_services();

        let state = manager.collect_all().await.unwrap();

        assert!(!state.hostname.is_empty());
        assert!(!state.components.is_empty() || cfg!(not(unix)));
    }

    #[tokio::test]
    async fn test_state_manager_diff_empty() {
        let manager = StateManager::new();

        let current = manager.collect_all().await.unwrap();
        let desired = manager.collect_all().await.unwrap();

        let drifts = manager.diff_all(&current, &desired).await.unwrap();
        assert!(drifts.is_empty());
    }

    #[test]
    fn test_compute_server_checksum_deterministic() {
        let mut components = HashMap::new();
        components.insert(
            "packages".to_string(),
            ComponentState {
                component: "packages".to_string(),
                version: 1,
                collected_at: Utc::now(),
                data: serde_json::json!({}),
                checksum: "abc123".to_string(),
            },
        );

        let checksum1 = StateManager::compute_server_checksum(&components);
        let checksum2 = StateManager::compute_server_checksum(&components);

        assert_eq!(checksum1, checksum2);
    }

    #[test]
    fn test_compute_server_checksum_order_independent() {
        let mut components1 = HashMap::new();
        components1.insert(
            "packages".to_string(),
            ComponentState {
                component: "packages".to_string(),
                version: 1,
                collected_at: Utc::now(),
                data: serde_json::json!({}),
                checksum: "abc123".to_string(),
            },
        );
        components1.insert(
            "services".to_string(),
            ComponentState {
                component: "services".to_string(),
                version: 1,
                collected_at: Utc::now(),
                data: serde_json::json!({}),
                checksum: "def456".to_string(),
            },
        );

        let mut components2 = HashMap::new();
        components2.insert(
            "services".to_string(),
            ComponentState {
                component: "services".to_string(),
                version: 1,
                collected_at: Utc::now(),
                data: serde_json::json!({}),
                checksum: "def456".to_string(),
            },
        );
        components2.insert(
            "packages".to_string(),
            ComponentState {
                component: "packages".to_string(),
                version: 1,
                collected_at: Utc::now(),
                data: serde_json::json!({}),
                checksum: "abc123".to_string(),
            },
        );

        let checksum1 = StateManager::compute_server_checksum(&components1);
        let checksum2 = StateManager::compute_server_checksum(&components2);

        assert_eq!(checksum1, checksum2);
    }
}

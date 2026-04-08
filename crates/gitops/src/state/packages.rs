use super::collector::{ApplyResult, ComponentType, StateCollector};
use crate::types::{ComponentState, DriftAction, SemanticDrift};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub architecture: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagesState {
    pub packages: Vec<PackageInfo>,
    pub package_manager: String,
}

pub struct PackageCollector {
    package_manager: PackageManager,
}

#[derive(Debug, Clone, Copy)]
enum PackageManager {
    Dpkg,
    Rpm,
    Apk,
    Unknown,
}

impl PackageCollector {
    pub fn new() -> Self {
        let package_manager = Self::detect_package_manager();
        Self { package_manager }
    }

    fn detect_package_manager() -> PackageManager {
        if std::path::Path::new("/usr/bin/dpkg").exists() || std::path::Path::new("/usr/bin/apt").exists() {
            PackageManager::Dpkg
        } else if std::path::Path::new("/usr/bin/rpm").exists() {
            PackageManager::Rpm
        } else if std::path::Path::new("/sbin/apk").exists() || std::path::Path::new("/usr/sbin/apk").exists() {
            PackageManager::Apk
        } else {
            PackageManager::Unknown
        }
    }

    async fn collect_dpkg(&self) -> Result<Vec<PackageInfo>> {
        let output = Command::new("dpkg-query")
            .args(["-W", "-f=${Package}\t${Version}\t${Architecture}\n"])
            .stdout(Stdio::piped())
            .output()
            .await
            .map_err(|e| anyhow!("Failed to run dpkg-query: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "dpkg-query failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                packages.push(PackageInfo {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    architecture: parts[2].to_string(),
                });
            } else if parts.len() >= 2 {
                packages.push(PackageInfo {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    architecture: "unknown".to_string(),
                });
            }
        }

        Ok(packages)
    }

    async fn collect_rpm(&self) -> Result<Vec<PackageInfo>> {
        let output = Command::new("rpm")
            .args(["-qa", "--queryformat", "%{NAME}\t%{VERSION}-%{RELEASE}\t%{ARCH}\n"])
            .stdout(Stdio::piped())
            .output()
            .await
            .map_err(|e| anyhow!("Failed to run rpm: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "rpm failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                packages.push(PackageInfo {
                    name: parts[0].to_string(),
                    version: parts[1].to_string(),
                    architecture: parts[2].to_string(),
                });
            }
        }

        Ok(packages)
    }

    async fn collect_apk(&self) -> Result<Vec<PackageInfo>> {
        let output = Command::new("apk")
            .args(["info", "-v"])
            .stdout(Stdio::piped())
            .output()
            .await
            .map_err(|e| anyhow!("Failed to run apk: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!(
                "apk failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines() {
            if let Some((name, version)) = line.split_once('-') {
                packages.push(PackageInfo {
                    name: name.to_string(),
                    version: version.to_string(),
                    architecture: "all".to_string(),
                });
            }
        }

        Ok(packages)
    }

    fn compute_checksum(packages: &[PackageInfo]) -> String {
        let mut hasher = Sha256::new();
        let mut sorted: Vec<_> = packages.iter().collect();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));
        
        for pkg in sorted {
            hasher.update(pkg.name.as_bytes());
            hasher.update(pkg.version.as_bytes());
            hasher.update(pkg.architecture.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    async fn install_packages_dpkg(&self, packages: &[PackageInfo]) -> Result<ApplyResult> {
        let mut applied = Vec::new();
        let mut failed = Vec::new();

        for pkg in packages {
            let result = Command::new("apt-get")
                .args(["install", "-y", &format!("{}={}", pkg.name, pkg.version)])
                .output()
                .await;

            match result {
                Ok(output) if output.status.success() => {
                    applied.push(pkg.name.clone());
                    debug!("Installed package: {}", pkg.name);
                }
                Ok(output) => {
                    let error = String::from_utf8_lossy(&output.stderr).to_string();
                    warn!("Failed to install {}: {}", pkg.name, error);
                    failed.push((pkg.name.clone(), error));
                }
                Err(e) => {
                    failed.push((pkg.name.clone(), e.to_string()));
                }
            }
        }

        if failed.is_empty() {
            Ok(ApplyResult::Success)
        } else if applied.is_empty() {
            Ok(ApplyResult::Failed {
                reason: "All package installations failed".to_string(),
            })
        } else {
            Ok(ApplyResult::PartialSuccess { applied, failed })
        }
    }

    async fn install_packages_rpm(&self, packages: &[PackageInfo]) -> Result<ApplyResult> {
        let mut applied = Vec::new();
        let mut failed = Vec::new();

        for pkg in packages {
            let result = Command::new("yum")
                .args(["install", "-y", &format!("{}-{}", pkg.name, pkg.version)])
                .output()
                .await;

            match result {
                Ok(output) if output.status.success() => {
                    applied.push(pkg.name.clone());
                    debug!("Installed package: {}", pkg.name);
                }
                Ok(output) => {
                    let error = String::from_utf8_lossy(&output.stderr).to_string();
                    warn!("Failed to install {}: {}", pkg.name, error);
                    failed.push((pkg.name.clone(), error));
                }
                Err(e) => {
                    failed.push((pkg.name.clone(), e.to_string()));
                }
            }
        }

        if failed.is_empty() {
            Ok(ApplyResult::Success)
        } else if applied.is_empty() {
            Ok(ApplyResult::Failed {
                reason: "All package installations failed".to_string(),
            })
        } else {
            Ok(ApplyResult::PartialSuccess { applied, failed })
        }
    }

    async fn install_packages_apk(&self, packages: &[PackageInfo]) -> Result<ApplyResult> {
        let mut applied = Vec::new();
        let mut failed = Vec::new();

        for pkg in packages {
            let result = Command::new("apk")
                .args(["add", &format!("{}={}", pkg.name, pkg.version)])
                .output()
                .await;

            match result {
                Ok(output) if output.status.success() => {
                    applied.push(pkg.name.clone());
                    debug!("Installed package: {}", pkg.name);
                }
                Ok(output) => {
                    let error = String::from_utf8_lossy(&output.stderr).to_string();
                    warn!("Failed to install {}: {}", pkg.name, error);
                    failed.push((pkg.name.clone(), error));
                }
                Err(e) => {
                    failed.push((pkg.name.clone(), e.to_string()));
                }
            }
        }

        if failed.is_empty() {
            Ok(ApplyResult::Success)
        } else if applied.is_empty() {
            Ok(ApplyResult::Failed {
                reason: "All package installations failed".to_string(),
            })
        } else {
            Ok(ApplyResult::PartialSuccess { applied, failed })
        }
    }

    async fn remove_packages_dpkg(&self, package_names: &[String]) -> Result<ApplyResult> {
        let mut applied = Vec::new();
        let mut failed = Vec::new();

        for name in package_names {
            let result = Command::new("apt-get")
                .args(["remove", "-y", name])
                .output()
                .await;

            match result {
                Ok(output) if output.status.success() => {
                    applied.push(name.clone());
                    debug!("Removed package: {}", name);
                }
                Ok(output) => {
                    let error = String::from_utf8_lossy(&output.stderr).to_string();
                    warn!("Failed to remove {}: {}", name, error);
                    failed.push((name.clone(), error));
                }
                Err(e) => {
                    failed.push((name.clone(), e.to_string()));
                }
            }
        }

        if failed.is_empty() {
            Ok(ApplyResult::Success)
        } else if applied.is_empty() {
            Ok(ApplyResult::Failed {
                reason: "All package removals failed".to_string(),
            })
        } else {
            Ok(ApplyResult::PartialSuccess { applied, failed })
        }
    }

    async fn remove_packages_rpm(&self, package_names: &[String]) -> Result<ApplyResult> {
        let mut applied = Vec::new();
        let mut failed = Vec::new();

        for name in package_names {
            let result = Command::new("yum")
                .args(["remove", "-y", name])
                .output()
                .await;

            match result {
                Ok(output) if output.status.success() => {
                    applied.push(name.clone());
                    debug!("Removed package: {}", name);
                }
                Ok(output) => {
                    let error = String::from_utf8_lossy(&output.stderr).to_string();
                    warn!("Failed to remove {}: {}", name, error);
                    failed.push((name.clone(), error));
                }
                Err(e) => {
                    failed.push((name.clone(), e.to_string()));
                }
            }
        }

        if failed.is_empty() {
            Ok(ApplyResult::Success)
        } else if applied.is_empty() {
            Ok(ApplyResult::Failed {
                reason: "All package removals failed".to_string(),
            })
        } else {
            Ok(ApplyResult::PartialSuccess { applied, failed })
        }
    }

    async fn remove_packages_apk(&self, package_names: &[String]) -> Result<ApplyResult> {
        let mut applied = Vec::new();
        let mut failed = Vec::new();

        for name in package_names {
            let result = Command::new("apk")
                .args(["del", name])
                .output()
                .await;

            match result {
                Ok(output) if output.status.success() => {
                    applied.push(name.clone());
                    debug!("Removed package: {}", name);
                }
                Ok(output) => {
                    let error = String::from_utf8_lossy(&output.stderr).to_string();
                    warn!("Failed to remove {}: {}", name, error);
                    failed.push((name.clone(), error));
                }
                Err(e) => {
                    failed.push((name.clone(), e.to_string()));
                }
            }
        }

        if failed.is_empty() {
            Ok(ApplyResult::Success)
        } else if applied.is_empty() {
            Ok(ApplyResult::Failed {
                reason: "All package removals failed".to_string(),
            })
        } else {
            Ok(ApplyResult::PartialSuccess { applied, failed })
        }
    }
}

impl Default for PackageCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StateCollector for PackageCollector {
    fn component(&self) -> ComponentType {
        ComponentType::Packages
    }

    async fn collect(&self) -> Result<ComponentState> {
        let packages = match self.package_manager {
            PackageManager::Dpkg => self.collect_dpkg().await?,
            PackageManager::Rpm => self.collect_rpm().await?,
            PackageManager::Apk => self.collect_apk().await?,
            PackageManager::Unknown => {
                warn!("No supported package manager detected");
                Vec::new()
            }
        };

        let package_manager_name = match self.package_manager {
            PackageManager::Dpkg => "dpkg",
            PackageManager::Rpm => "rpm",
            PackageManager::Apk => "apk",
            PackageManager::Unknown => "unknown",
        };

        let state = PackagesState {
            packages,
            package_manager: package_manager_name.to_string(),
        };

        let checksum = Self::compute_checksum(&state.packages);

        Ok(ComponentState {
            component: "packages".to_string(),
            version: 1,
            collected_at: Utc::now(),
            data: serde_json::to_value(state)?,
            checksum,
        })
    }

    async fn apply(&self, desired: &ComponentState) -> Result<ApplyResult> {
        let desired_state: PackagesState = serde_json::from_value(desired.data.clone())?;
        let current_state = self.collect().await?;
        let current_packages: PackagesState = serde_json::from_value(current_state.data)?;

        let current_map: HashMap<String, &PackageInfo> = current_packages
            .packages
            .iter()
            .map(|p| (p.name.clone(), p))
            .collect();
        
        let desired_map: HashMap<String, &PackageInfo> = desired_state
            .packages
            .iter()
            .map(|p| (p.name.clone(), p))
            .collect();

        let mut to_install = Vec::new();
        let mut to_remove = Vec::new();

        for (name, pkg) in &desired_map {
            if let Some(current) = current_map.get(name) {
                if current.version != pkg.version {
                    to_install.push((*pkg).clone());
                }
            } else {
                to_install.push((*pkg).clone());
            }
        }

        for name in current_map.keys() {
            if !desired_map.contains_key(name) {
                to_remove.push(name.clone());
            }
        }

        let mut results = Vec::new();

        if !to_install.is_empty() {
            let install_result = match self.package_manager {
                PackageManager::Dpkg => self.install_packages_dpkg(&to_install).await?,
                PackageManager::Rpm => self.install_packages_rpm(&to_install).await?,
                PackageManager::Apk => self.install_packages_apk(&to_install).await?,
                PackageManager::Unknown => {
                    ApplyResult::Failed {
                        reason: "No supported package manager".to_string(),
                    }
                }
            };
            results.push(install_result);
        }

        if !to_remove.is_empty() {
            let remove_result = match self.package_manager {
                PackageManager::Dpkg => self.remove_packages_dpkg(&to_remove).await?,
                PackageManager::Rpm => self.remove_packages_rpm(&to_remove).await?,
                PackageManager::Apk => self.remove_packages_apk(&to_remove).await?,
                PackageManager::Unknown => {
                    ApplyResult::Failed {
                        reason: "No supported package manager".to_string(),
                    }
                }
            };
            results.push(remove_result);
        }

        if results.is_empty() {
            Ok(ApplyResult::Success)
        } else if results.iter().all(|r| matches!(r, ApplyResult::Success)) {
            Ok(ApplyResult::Success)
        } else {
            let all_failed = results.iter().all(|r| matches!(r, ApplyResult::Failed { .. }));
            if all_failed {
                Ok(ApplyResult::Failed {
                    reason: "All operations failed".to_string(),
                })
            } else {
                let applied = results
                    .iter()
                    .filter_map(|r| match r {
                        ApplyResult::Success => Some(vec!["all".to_string()]),
                        ApplyResult::PartialSuccess { applied, .. } => Some(applied.clone()),
                        _ => None,
                    })
                    .flatten()
                    .collect();
                
                let failed = results
                    .iter()
                    .filter_map(|r| match r {
                        ApplyResult::PartialSuccess { failed, .. } => Some(failed.clone()),
                        ApplyResult::Failed { reason } => Some(vec![("all".to_string(), reason.clone())]),
                        _ => None,
                    })
                    .flatten()
                    .collect();

                Ok(ApplyResult::PartialSuccess { applied, failed })
            }
        }
    }

    async fn diff(
        &self,
        current: &ComponentState,
        desired: &ComponentState,
    ) -> Result<Vec<SemanticDrift>> {
        let current_state: PackagesState = serde_json::from_value(current.data.clone())?;
        let desired_state: PackagesState = serde_json::from_value(desired.data.clone())?;

        let current_map: HashMap<String, &PackageInfo> = current_state
            .packages
            .iter()
            .map(|p| (p.name.clone(), p))
            .collect();
        
        let desired_map: HashMap<String, &PackageInfo> = desired_state
            .packages
            .iter()
            .map(|p| (p.name.clone(), p))
            .collect();

        let mut drifts = Vec::new();

        for (name, pkg) in &desired_map {
            if let Some(current_pkg) = current_map.get(name) {
                if current_pkg.version != pkg.version || current_pkg.architecture != pkg.architecture {
                    drifts.push(SemanticDrift {
                        path: format!("packages/{}", name),
                        expected: serde_json::to_value(pkg)?,
                        actual: serde_json::to_value(current_pkg)?,
                        action: DriftAction::Changed,
                    });
                }
            } else {
                drifts.push(SemanticDrift {
                    path: format!("packages/{}", name),
                    expected: serde_json::to_value(pkg)?,
                    actual: serde_json::Value::Null,
                    action: DriftAction::Added,
                });
            }
        }

        for name in current_map.keys() {
            if !desired_map.contains_key(name) {
                drifts.push(SemanticDrift {
                    path: format!("packages/{}", name),
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
    fn test_package_info_serialization() {
        let pkg = PackageInfo {
            name: "nginx".to_string(),
            version: "1.18.0".to_string(),
            architecture: "amd64".to_string(),
        };
        
        let json = serde_json::to_string(&pkg).unwrap();
        let deserialized: PackageInfo = serde_json::from_str(&json).unwrap();
        
        assert_eq!(pkg.name, deserialized.name);
        assert_eq!(pkg.version, deserialized.version);
    }

    #[test]
    fn test_checksum_deterministic() {
        let packages = vec![
            PackageInfo {
                name: "nginx".to_string(),
                version: "1.18.0".to_string(),
                architecture: "amd64".to_string(),
            },
            PackageInfo {
                name: "curl".to_string(),
                version: "7.68.0".to_string(),
                architecture: "amd64".to_string(),
            },
        ];

        let checksum1 = PackageCollector::compute_checksum(&packages);
        let checksum2 = PackageCollector::compute_checksum(&packages);
        
        assert_eq!(checksum1, checksum2);
    }

    #[test]
    fn test_checksum_order_independent() {
        let packages1 = vec![
            PackageInfo {
                name: "nginx".to_string(),
                version: "1.18.0".to_string(),
                architecture: "amd64".to_string(),
            },
            PackageInfo {
                name: "curl".to_string(),
                version: "7.68.0".to_string(),
                architecture: "amd64".to_string(),
            },
        ];

        let packages2 = vec![
            PackageInfo {
                name: "curl".to_string(),
                version: "7.68.0".to_string(),
                architecture: "amd64".to_string(),
            },
            PackageInfo {
                name: "nginx".to_string(),
                version: "1.18.0".to_string(),
                architecture: "amd64".to_string(),
            },
        ];

        let checksum1 = PackageCollector::compute_checksum(&packages1);
        let checksum2 = PackageCollector::compute_checksum(&packages2);
        
        assert_eq!(checksum1, checksum2);
    }

    #[tokio::test]
    async fn test_collector_component_type() {
        let collector = PackageCollector::new();
        assert_eq!(collector.component(), ComponentType::Packages);
    }
}

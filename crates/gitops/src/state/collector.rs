//! State collection trait and common types

use crate::types::{ComponentState, SemanticDrift};
use async_trait::async_trait;

/// Type of component being collected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentType {
    /// System packages (dpkg, rpm, apk)
    Packages,
    /// Systemd services
    Services,
    /// Docker containers and images
    Docker,
    /// Tracked files with hash monitoring
    Files,
}

impl std::fmt::Display for ComponentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComponentType::Packages => write!(f, "packages"),
            ComponentType::Services => write!(f, "services"),
            ComponentType::Docker => write!(f, "docker"),
            ComponentType::Files => write!(f, "files"),
        }
    }
}

/// Result of applying a desired state
#[derive(Debug, Clone)]
pub enum ApplyResult {
    /// All changes applied successfully
    Success,
    /// Some changes applied, some failed
    PartialSuccess {
        applied: Vec<String>,
        failed: Vec<(String, String)>,
    },
    /// Apply completely failed
    Failed { reason: String },
}

/// Trait for collecting and managing component state
#[async_trait]
pub trait StateCollector: Send + Sync {
    /// Returns the type of component this collector handles
    fn component(&self) -> ComponentType;

    /// Collect current state of the component
    async fn collect(&self) -> anyhow::Result<ComponentState>;

    /// Apply desired state to bring system in sync
    async fn apply(&self, desired: &ComponentState) -> anyhow::Result<ApplyResult>;

    /// Compare current and desired states, return semantic drifts
    async fn diff(
        &self,
        current: &ComponentState,
        desired: &ComponentState,
    ) -> anyhow::Result<Vec<SemanticDrift>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_type_display() {
        assert_eq!(ComponentType::Packages.to_string(), "packages");
        assert_eq!(ComponentType::Services.to_string(), "services");
        assert_eq!(ComponentType::Docker.to_string(), "docker");
        assert_eq!(ComponentType::Files.to_string(), "files");
    }

    #[test]
    fn test_apply_result_debug() {
        let success = ApplyResult::Success;
        assert!(matches!(success, ApplyResult::Success));

        let partial = ApplyResult::PartialSuccess {
            applied: vec!["pkg1".to_string()],
            failed: vec![("pkg2".to_string(), "error".to_string())],
        };
        assert!(matches!(partial, ApplyResult::PartialSuccess { .. }));

        let failed = ApplyResult::Failed {
            reason: "test".to_string(),
        };
        assert!(matches!(failed, ApplyResult::Failed { .. }));
    }
}

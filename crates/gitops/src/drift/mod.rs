//! Drift detection module

pub mod event_driven;
pub mod semantic_diff;
pub mod auto_fix;

use crate::types::*;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Drift detector — orchestrates event-driven and periodic drift detection
pub struct DriftDetector {
    config: crate::config::DriftConfig,
    #[allow(dead_code)]
    state: Arc<RwLock<ServerState>>,
}

impl DriftDetector {
    pub fn new(config: crate::config::DriftConfig, state: Arc<RwLock<ServerState>>) -> Self {
        Self { config, state }
    }

    /// Start background drift detection tasks
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            tracing::info!("Drift detection disabled");
            return Ok(());
        }
        tracing::info!("Drift detection started");
        Ok(())
    }

    /// Detect drift between current and desired state
    pub async fn detect(&self, current: &ServerState, desired: &ServerState) -> Vec<ClassifiedDrift> {
        semantic_diff::diff_states(current, desired)
    }
}

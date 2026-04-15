pub mod models;
pub mod controllers;
pub mod operator;
pub mod reconciler;
pub mod storage;
pub mod error;

pub use models::*;
pub use controllers::*;
pub use operator::*;
pub use reconciler::*;
pub use storage::*;
pub use error::*;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

// Main K8s operator
pub struct FlowLinkOperator {
    pub config: OperatorConfig,
    pub controllers: Arc<RwLock<Vec<Box<dyn KubernetesController + Send + Sync>>>>,
    pub storage: Arc<OperatorStorage>,
}

impl FlowLinkOperator {
    pub fn new(config: OperatorConfig) -> Self {
        Self {
            config,
            controllers: Arc::new(RwLock::new(Vec::new())),
            storage: Arc::new(OperatorStorage::new()),
        }
    }

    pub async fn start(&self) -> Result<()> {
        log::info!("Starting FlowLink Kubernetes Operator");

        // Register controllers
        self.register_controllers().await?;

        // Start watching namespaces
        self.watch_namespaces().await?;

        // Start watching custom resources
        self.watch_custom_resources().await?;

        log::info!("FlowLink Kubernetes Operator started successfully");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        log::info!("Stopping FlowLink Kubernetes Operator");
        // TODO: Implement graceful shutdown
        Ok(())
    }

    pub async fn register_controllers(&self) -> Result<()> {
        let mut controllers = self.controllers.write().await;

        // Register FlowLink deployment controller
        controllers.push(Box::new(DeploymentController::new(
            self.config.clone(),
            self.storage.clone(),
        )));

        // Register FlowLink service controller
        controllers.push(Box::new(ServiceController::new(
            self.config.clone(),
            self.storage.clone(),
        )));

        // Register FlowLink ingress controller
        controllers.push(Box::new(IngressController::new(
            self.config.clone(),
            self.storage.clone(),
        )));

        // Register FlowLink configmap controller
        controllers.push(Box::new(ConfigMapController::new(
            self.config.clone(),
            self.storage.clone(),
        )));

        // Register FlowLink secret controller
        controllers.push(Box::new(SecretController::new(
            self.config.clone(),
            self.storage.clone(),
        )));

        log::info!("Registered {} Kubernetes controllers", controllers.len());
        Ok(())
    }

    pub async fn watch_namespaces(&self) -> Result<()> {
        let kube_client = kube::client::Client::try_default()
            .await?;

        log::info!("Watching Kubernetes namespaces");

        // TODO: Implement namespace watching
        // This would use kube::runtime::watcher to watch namespace events

        Ok(())
    }

    pub async fn watch_custom_resources(&self) -> Result<()> {
        let kube_client = kube::client::Client::try_default()
            .await?;

        log::info!("Watching FlowLink custom resources");

        // TODO: Implement custom resource watching
        // Watch FlowLink resources and trigger reconciliation

        Ok(())
    }

    pub async fn get_stats(&self) -> OperatorStats {
        let controllers = self.controllers.read().await;

        OperatorStats {
            total_controllers: controllers.len(),
            active_reconciliations: 0, // TODO: Track active reconciliations
            total_resources: self.storage.get_total_resources().await,
            recent_events: self.storage.get_recent_events(10).await,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OperatorConfig {
    pub namespace: String,
    pub kubeconfig_path: Option<String>,
    pub leader_election: LeaderElectionConfig,
    pub reconciliation_interval: chrono::Duration,
    pub health_check_interval: chrono::Duration,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LeaderElectionConfig {
    pub enabled: bool,
    pub lease_duration: chrono::Duration,
    pub renew_deadline: chrono::Duration,
    pub retry_period: chrono::Duration,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OperatorStats {
    pub total_controllers: usize,
    pub active_reconciliations: usize,
    pub total_resources: i64,
    pub recent_events: Vec<OperatorEvent>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OperatorEvent {
    pub resource_kind: String,
    pub resource_name: String,
    pub operation: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
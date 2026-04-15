use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;

pub struct CIWebhookRouter {
    pub config: CIConfig,
    pub handlers: Arc<RwLock<HashMap<String, Arc<dyn CIHandler + Send + Sync>>>>,
    pub storage: Arc<CIStorage>,
}

impl CIWebhookRouter {
    pub fn new(config: CIConfig) -> Self {
        Self {
            config,
            handlers: Arc::new(RwLock::new(HashMap::new())),
            storage: Arc::new(CIStorage::new()),
        }
    }

    pub async fn register_default_handlers(&self) -> Result<()> {
        // Register GitHub handler
        let github_handler = GithubCIHandler::new(
            self.config.github_secret.clone().unwrap_or_default(),
            self.config.auto_approve,
        );
        self.register_handler(Arc::new(github_handler)).await?;

        // Register GitLab handler
        let gitlab_handler = GitlabCIHandler::new(
            self.config.gitlab_secret.clone().unwrap_or_default(),
            self.config.auto_approve,
        );
        self.register_handler(Arc::new(gitlab_handler)).await?;

        log::info!("Registered default CI/CD handlers");
        Ok(())
    }

    pub async fn register_handler(&self, handler: Arc<dyn CIHandler + Send + Sync>) -> Result<()> {
        let mut handlers = self.handlers.write().await;
        handlers.insert(handler.name().to_string(), handler);
        log::info!("Registered CI/CD handler: {}", handler.name());
        Ok(())
    }

    pub async fn route_webhook(&self, provider: &str, payload: &str) -> Result<CIResponse> {
        let provider_lower = provider.to_lowercase();

        let handlers = self.handlers.read().await;
        
        match handlers.get(&provider_lower) {
            Some(handler) => {
                log::info!("Routing {} webhook to handler", provider);
                let response = handler.handle(payload).await?;
                
                // Save webhook event
                if let Ok(event) = self.parse_webhook_event(provider, payload, &response) {
                    self.storage.save_event(&event).await?;
                }
                
                Ok(response)
            }
            None => {
                log::warn!("No handler found for provider: {}", provider);
                Err(anyhow::anyhow!("No handler registered for provider: {}", provider))
            }
        }
    }

    pub async fn get_stats(&self) -> Result<CIStats> {
        let events = self.storage.get_recent_events(100).await?;
        let total_events = events.len() as i64;
        let successful_events = events.iter().filter(|e| e.status == "success").count() as i64;
        let failed_events = total_events - successful_events;

        Ok(CIStats {
            total_events,
            successful_events,
            failed_events,
            recent_events: events,
        })
    }

    fn parse_webhook_event(&self, provider: &str, payload: &str, response: &CIResponse) -> Result<CIEvent> {
        let metadata = HashMap::new();
        
        Ok(CIEvent {
            id: response.event_id.clone(),
            provider: provider.to_string(),
            event_type: "webhook".to_string(),
            repository: metadata.get("repository").unwrap_or(&"unknown".to_string()).clone(),
            timestamp: chrono::Utc::now(),
            status: response.success.to_string(),
            metadata,
        })
    }
}

// Webhook endpoint handler
pub struct WebhookEndpoint {
    pub router: Arc<CIWebhookRouter>,
}

impl WebhookEndpoint {
    pub fn new(router: Arc<CIWebhookRouter>) -> Self {
        Self { router }
    }

    pub async fn handle_github_webhook(&self, payload: &str, headers: &std::collections::HashMap<String, String>) -> Result<CIResponse> {
        log::info!("Received GitHub webhook");
        
        // Verify signature
        let signature = headers.get("x-hub-signature-256")
            .and_then(|s| s.get(7..)) // Remove 'sha256=' prefix
            .ok_or_else(|| anyhow::anyhow!("Missing signature"))?;
        
        let verification = WebhookVerification::new(
            "github".to_string(),
            signature.to_string(),
            payload.to_string(),
            self.router.config.github_secret.clone().unwrap_or_default(),
        );
        
        verification.verify()?;
        
        // Route webhook
        self.router.route_webhook("github", payload).await
    }

    pub async fn handle_gitlab_webhook(&self, payload: &str, headers: &std::collections::HashMap<String, String>) -> Result<CIResponse> {
        log::info!("Received GitLab webhook");
        
        // Verify signature
        let signature = headers.get("x-gitlab-token")
            .ok_or_else(|| anyhow::anyhow!("Missing signature"))?;
        
        let verification = WebhookVerification::new(
            "gitlab".to_string(),
            signature.to_string(),
            payload.to_string(),
            self.router.config.gitlab_secret.clone().unwrap_or_default(),
        );
        
        verification.verify()?;
        
        // Route webhook
        self.router.route_webhook("gitlab", payload).await
    }

    pub async fn handle_generic_webhook(&self, provider: &str, payload: &str) -> Result<CIResponse> {
        log::info!("Received generic webhook from {}", provider);
        self.router.route_webhook(provider, payload).await
    }
}

// CI/CD pipeline orchestrator
pub struct PipelineOrchestrator {
    pub router: Arc<CIWebhookRouter>,
    pub deployments: Arc<RwLock<HashMap<String, DeploymentEnvironment>>>,
}

impl PipelineOrchestrator {
    pub fn new(router: Arc<CIWebhookRouter>) -> Self {
        Self {
            router,
            deployments: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn trigger_deployment(&self, trigger: DeploymentTrigger) -> Result<String> {
        let deployment_id = uuid::Uuid::new_v4().to_string();
        
        log::info!("Triggering deployment: {} for repository {} at commit {}", deployment_id, trigger.repository, trigger.commit_sha);
        
        let environment = DeploymentEnvironment {
            name: trigger.provider.clone(),
            url: format!("https://{}.flowlink.dev", trigger.repository),
            status: CIStatus::Running,
            deployment_id: deployment_id.clone(),
            timestamp: chrono::Utc::now(),
            artifacts: Vec::new(),
        };
        
        let mut deployments = self.deployments.write().await;
        deployments.insert(deployment_id.clone(), environment);
        
        // TODO: Actually trigger the deployment process
        // This would integrate with the actual deployment system
        
        Ok(deployment_id)
    }

    pub async fn update_deployment_status(&self, deployment_id: &str, status: CIStatus) -> Result<()> {
        let mut deployments = self.deployments.write().await;
        
        if let Some(deployment) = deployments.get_mut(deployment_id) {
            deployment.status = status;
            log::info!("Deployment {} status updated to {:?}", deployment_id, status);
        }
        
        Ok(())
    }

    pub async fn get_deployment(&self, deployment_id: &str) -> Result<Option<DeploymentEnvironment>> {
        let deployments = self.deployments.read().await;
        Ok(deployments.get(deployment_id).cloned())
    }

    pub async fn list_deployments(&self) -> Result<Vec<DeploymentEnvironment>> {
        let deployments = self.deployments.read().await;
        Ok(deployments.values().cloned().collect())
    }
}
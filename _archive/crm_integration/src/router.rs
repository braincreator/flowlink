use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;

pub struct CRMRouter {
    pub config: CRMConfig,
    pub handlers: Arc<RwLock<HashMap<String, Arc<dyn CRMHandler + Send + Sync>>>>,
    pub storage: Arc<CRMStorage>,
}

impl CRMRouter {
    pub fn new(config: CRMConfig) -> Self {
        Self {
            config,
            handlers: Arc::new(RwLock::new(HashMap::new())),
            storage: Arc::new(CRMStorage::new()),
        }
    }

    pub async fn register_handlers(&self) -> Result<()> {
        let mut handlers = self.handlers.write().await;

        if self.config.amocrm_enabled {
            let amocrm_handler = AmoCRMHandler::new(
                self.config.amocrm_client_id.clone(),
                self.config.amocrm_client_secret.clone(),
            );
            handlers.insert("amocrm".to_string(), Arc::new(amocrm_handler));
            log::info!("Registered AmoCRM handler");
        }

        if self.config.bitrix24_enabled {
            let bitrix_handler = Bitrix24Handler::new(
                self.config.bitrix24_client_id.clone(),
                self.config.bitrix24_client_secret.clone(),
            );
            handlers.insert("bitrix24".to_string(), Arc::new(bitrix_handler));
            log::info!("Registered Bitrix24 handler");
        }

        Ok(())
    }

    pub async fn route_webhook(&self, provider: &str, payload: &str) -> Result<CRMResponse> {
        let provider_lower = provider.to_lowercase();

        let handlers = self.handlers.read().await;
        
        match handlers.get(&provider_lower) {
            Some(handler) => {
                log::info!("Routing {} webhook to handler", provider);
                let response = handler.handle_webhook(payload).await?;
                
                // Save webhook event
                if let Ok(event) = self.parse_webhook_event(provider, payload, &response) {
                    self.storage.save_webhook_event(&event).await?;
                }
                
                Ok(response)
            }
            None => {
                log::warn!("No handler found for provider: {}", provider);
                Err(anyhow::anyhow!("No handler registered for provider: {}", provider))
            }
        }
    }

    pub async fn trigger_sync(&self, provider: &str) -> Result<CRMSyncResult> {
        let handlers = self.handlers.read().await;
        
        match handlers.get(&provider.to_lowercase()) {
            Some(handler) => {
                log::info!("Triggering sync for provider: {}", provider);
                handler.sync_data().await
            }
            None => {
                log::warn!("No handler found for provider: {}", provider);
                Err(anyhow::anyhow!("No handler registered for provider: {}", provider))
            }
        }
    }

    pub async fn get_all_sync_results(&self) -> Result<Vec<CRMSyncResult>> {
        let mut results = Vec::new();
        
        if self.config.amocrm_enabled {
            if let Ok(result) = self.trigger_sync("amocrm").await {
                results.push(result);
            }
        }
        
        if self.config.bitrix24_enabled {
            if let Ok(result) = self.trigger_sync("bitrix24").await {
                results.push(result);
            }
        }
        
        Ok(results)
    }

    pub async fn get_provider_stats(&self, provider: &str) -> Result<CRMAalytics> {
        // TODO: Implement actual analytics calculation
        Ok(CRMAalytics {
            leads_count: 0,
            leads_created_today: 0,
            leads_completed_today: 0,
            customers_count: 0,
            revenue_monthly: 0.0,
            conversion_rate: 0.0,
            average_deal_size: 0.0,
        })
    }

    pub async fn get_all_stats(&self) -> Result<HashMap<String, CRMAalytics>> {
        let mut stats = HashMap::new();
        
        if self.config.amocrm_enabled {
            stats.insert("amocrm".to_string(), self.get_provider_stats("amocrm").await?);
        }
        
        if self.config.bitrix24_enabled {
            stats.insert("bitrix24".to_string(), self.get_provider_stats("bitrix24").await?);
        }
        
        Ok(stats)
    }

    fn parse_webhook_event(&self, provider: &str, payload: &str, response: &CRMResponse) -> Result<CRMWebhookEvent> {
        let metadata = HashMap::new();
        
        Ok(CRMWebhookEvent {
            id: uuid::Uuid::new_v4().to_string(),
            provider: provider.to_string(),
            event_type: "webhook".to_string(),
            data: serde_json::from_str(payload)?,
            timestamp: chrono::Utc::now(),
            metadata,
        })
    }
}

// CRM webhook endpoint
pub struct CRMWebhookEndpoint {
    pub router: Arc<CRMRouter>,
}

impl CRMWebhookEndpoint {
    pub fn new(router: Arc<CRMRouter>) -> Self {
        Self { router }
    }

    pub async fn handle_amocrm_webhook(&self, payload: &str, headers: &std::collections::HashMap<String, String>) -> Result<CRMResponse> {
        log::info!("Received AmoCRM webhook");
        
        // Verify signature
        let signature = headers.get("x-webhook-signature").unwrap_or(&"".to_string());
        
        // TODO: Implement signature verification
        
        // Route webhook
        self.router.route_webhook("amocrm", payload).await
    }

    pub async fn handle_bitrix24_webhook(&self, payload: &str, headers: &std::collections::HashMap<String, String>) -> Result<CRMResponse> {
        log::info!("Received Bitrix24 webhook");
        
        // Verify signature
        let auth = headers.get("auth").unwrap_or(&"".to_string());
        
        // TODO: Implement signature verification
        
        // Route webhook
        self.router.route_webhook("bitrix24", payload).await
    }

    pub async fn handle_generic_webhook(&self, provider: &str, payload: &str) -> Result<CRMResponse> {
        log::info!("Received generic webhook from {}", provider);
        self.router.route_webhook(provider, payload).await
    }
}

// CRM flow orchestrator
pub struct CRMFlowOrchestrator {
    pub router: Arc<CRMRouter>,
    pub flows: Arc<RwLock<HashMap<String, CRMFlow>>>,
}

impl CRMFlowOrchestrator {
    pub fn new(router: Arc<CRMRouter>) -> Self {
        Self {
            router,
            flows: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_flow(&self, flow: CRMFlow) -> Result<()> {
        let mut flows = self.flows.write().await;
        flows.insert(flow.name.clone(), flow);
        log::info!("Registered CRM flow: {}", flow.name);
        Ok(())
    }

    pub async def trigger_flow(&self, flow_name: &str, trigger_data: &serde_json::Value) -> Result<CRMResponse> {
        let flows = self.flows.read().await;
        
        match flows.get(flow_name) {
            Some(flow) => {
                log::info!("Triggering flow: {}", flow_name);
                
                // Execute flow steps
                for step in &flow.steps {
                    self.execute_flow_step(step, trigger_data).await?;
                }
                
                Ok(CRMResponse {
                    success: true,
                    message: format!("Flow {} executed successfully", flow_name),
                    provider: "crm".to_string(),
                    data: None,
                })
            }
            None => {
                Err(anyhow::anyhow!("Flow not found: {}", flow_name))
            }
        }
    }

    async fn execute_flow_step(&self, step: &CRMFlowStep, data: &serde_json::Value) -> Result<()> {
        log::info!("Executing flow step: {}", step.name);
        
        match step.action_type.as_str() {
            "create_task" => {
                self.create_task(&step.config, data).await?;
            }
            "send_notification" => {
                self.send_notification(&step.config, data).await?;
            }
            "update_status" => {
                self.update_status(&step.config, data).await?;
            }
            "create_note" => {
                self.create_note(&step.config, data).await?;
            }
            "wait" => {
                self.wait(step.config.get("duration").and_then(|d| d.as_u64()).unwrap_or(1000)).await?;
            }
            _ => {
                log::warn!("Unknown flow step action: {}", step.action_type);
            }
        }
        
        Ok(())
    }

    async fn create_task(&self, config: &serde_json::Value, data: &serde_json::Value) -> Result<()> {
        log::info!("Creating task with config: {:?}", config);
        
        // TODO: Implement actual task creation
        Ok(())
    }

    async fn send_notification(&self, config: &serde_json::Value, data: &serde_json::Value) -> Result<()> {
        log::info!("Sending notification with config: {:?}", config);
        
        // TODO: Implement actual notification sending
        Ok(())
    }

    async fn update_status(&self, config: &serde_json::Value, data: &serde_json::Value) -> Result<()> {
        log::info!("Updating status with config: {:?}", config);
        
        // TODO: Implement actual status update
        Ok(())
    }

    async fn create_note(&self, config: &serde_json::Value, data: &serde_json::Value) -> Result<()> {
        log::info!("Creating note with config: {:?}", config);
        
        // TODO: Implement actual note creation
        Ok(())
    }

    async fn wait(&self, duration_ms: u64) -> Result<()> {
        tokio::time::sleep(tokio::time::Duration::from_millis(duration_ms)).await;
        Ok(())
    }
}

// CRM flow definition
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMFlow {
    pub name: String,
    pub description: String,
    pub trigger_events: Vec<String>,
    pub steps: Vec<CRMFlowStep>,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMFlowStep {
    pub name: String,
    pub action_type: String,
    pub config: serde_json::Value,
    pub next_step: Option<String>,
}

// CRM workflow templates
pub struct CRMFlowTemplates;

impl CRMFlowTemplates {
    pub fn new_lead_flow() -> CRMFlow {
        CRMFlow {
            name: "new_lead".to_string(),
            description: "New lead processing workflow".to_string(),
            trigger_events: vec!["lead.added".to_string()],
            steps: vec![
                CRMFlowStep {
                    name: "create_task".to_string(),
                    action_type: "create_task".to_string(),
                    config: serde_json::json!({
                        "text": "Call new lead",
                        "responsible_user_id": 1,
                        "complete_before": "+3 days"
                    }),
                    next_step: Some("send_notification".to_string()),
                },
                CRMFlowStep {
                    name: "send_notification".to_string(),
                    action_type: "send_notification".to_string(),
                    config: serde_json::json!({
                        "channel": "slack",
                        "message": "New lead assigned"
                    }),
                    next_step: None,
                },
            ],
            active: true,
            created_at: chrono::Utc::now(),
        }
    }

    pub fn deal_closed_flow() -> CRMFlow {
        CRMFlow {
            name: "deal_closed".to_string(),
            description: "Deal closed workflow".to_string(),
            trigger_events: vec!["deal.completed".to_string()],
            steps: vec![
                CRMFlowStep {
                    name: "create_note".to_string(),
                    action_type: "create_note".to_string(),
                    config: serde_json::json!({
                        "text": "Deal closed successfully",
                        "element_id": "{{deal_id}}",
                        "element_type": "deal"
                    }),
                    next_step: Some("send_notification".to_string()),
                },
                CRMFlowStep {
                    name: "send_notification".to_string(),
                    action_type: "send_notification".to_string(),
                    config: serde_json::json!({
                        "channel": "email",
                        "recipients": ["{{deal_responsible_user_email}}"],
                        "template": "deal_closed"
                    }),
                    next_step: None,
                },
            ],
            active: true,
            created_at: chrono::Utc::now(),
        }
    }

    pub fn qualification_flow() -> CRMFlow {
        CRMFlow {
            name: "qualification".to_string(),
            description: "Lead qualification workflow".to_string(),
            trigger_events: vec!["lead.qualification_started".to_string()],
            steps: vec![
                CRMFlowStep {
                    name: "create_task".to_string(),
                    action_type: "create_task".to_string(),
                    config: serde_json::json!({
                        "text": "Qualify lead",
                        "responsible_user_id": 2,
                        "complete_before": "+1 day"
                    }),
                    next_step: Some("wait".to_string()),
                },
                CRMFlowStep {
                    name: "wait".to_string(),
                    action_type: "wait".to_string(),
                    config: serde_json::json!({
                        "duration": 86400000 // 1 day
                    }),
                    next_step: None,
                },
            ],
            active: true,
            created_at: chrono::Utc::now(),
        }
    }
}
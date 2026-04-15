use anyhow::Result;
use std::sync::Arc;

use super::*;

// Main CRM handler dispatcher
pub struct CRMHandlerDispatcher {
    pub router: Arc<CRMRouter>,
}

impl CRMHandlerDispatcher {
    pub fn new(router: Arc<CRMRouter>) -> Self {
        Self { router }
    }

    pub async fn handle_webhook(&self, provider: &str, payload: &str, headers: &std::collections::HashMap<String, String>) -> Result<CRMResponse> {
        match provider.to_lowercase().as_str() {
            "amocrm" => {
                let endpoint = CRMWebhookEndpoint::new(self.router.clone());
                endpoint.handle_amocrm_webhook(payload, headers).await
            }
            "bitrix24" => {
                let endpoint = CRMWebhookEndpoint::new(self.router.clone());
                endpoint.handle_bitrix24_webhook(payload, headers).await
            }
            _ => {
                log::warn!("Unsupported provider: {}", provider);
                self.router.route_webhook(provider, payload).await
            }
        }
    }
}

// Generic webhook handler for any CRM provider
pub struct GenericCRMWebhookHandler {
    pub config: CRMConfig,
}

impl GenericCRMWebhookHandler {
    pub fn new(config: CRMConfig) -> Self {
        Self { config }
    }

    pub async fn handle(&self, provider: &str, payload: &str) -> Result<CRMResponse> {
        log::info!("Processing generic webhook from CRM provider: {}", provider);

        // Basic validation
        self.validate_payload(payload)?;

        // Parse provider-specific event
        let event_info = self.parse_event_info(provider, payload)?;

        // Process based on event type
        match event_info.event_type.as_str() {
            "lead.added" | "ONCRMLEADADD" => self.handle_lead_added(&event_info),
            "lead.updated" | "ONCRMLEADUPDATE" => self.handle_lead_updated(&event_info),
            "deal.added" | "ONCRMDEALADD" => self.handle_deal_added(&event_info),
            "deal.updated" | "ONCRMDEALUPDATE" => self.handle_deal_updated(&event_info),
            "contact.added" | "ONCRMCONTACTADD" => self.handle_contact_added(&event_info),
            "contact.updated" | "ONCRMCONTACTUPDATE" => self.handle_contact_updated(&event_info),
            "task.added" | "ONCRMTASKADD" => self.handle_task_added(&event_info),
            "task.updated" | "ONCRMTASKUPDATE" => self.handle_task_updated(&event_info),
            _ => Ok(CRMResponse {
                success: true,
                message: "CRM event received".to_string(),
                provider: provider.to_string(),
                data: None,
            }),
        }
    }

    fn validate_payload(&self, payload: &str) -> Result<()> {
        if payload.trim().is_empty() {
            return Err(anyhow::anyhow!("Empty webhook payload"));
        }

        // Basic JSON validation
        let _value: serde_json::Value = serde_json::from_str(payload)
            .map_err(|e| anyhow::anyhow!("Invalid JSON payload: {}", e))?;

        Ok(())
    }

    fn parse_event_info(&self, provider: &str, payload: &str) -> Result<CRMEventInfo> {
        let event_info = CRMEventInfo {
            id: uuid::Uuid::new_v4().to_string(),
            provider: provider.to_lowercase(),
            event_type: "unknown".to_string(),
            data: serde_json::from_str(payload)?,
            timestamp: chrono::Utc::now(),
        };

        match provider.to_lowercase().as_str() {
            "amocrm" => self.parse_amocrm_event(payload),
            "bitrix24" => self.parse_bitrix24_event(payload),
            _ => Ok(event_info),
        }
    }

    fn parse_amocrm_event(&self, payload: &str) -> Result<CRMEventInfo> {
        let payload_data: serde_json::Value = serde_json::from_str(payload)?;
        let event_type = payload_data.get("action").and_then(|a| a.as_str()).unwrap_or("unknown");

        Ok(CRMEventInfo {
            id: uuid::Uuid::new_v4().to_string(),
            provider: "amocrm".to_string(),
            event_type: event_type.to_string(),
            data: payload_data,
            timestamp: chrono::Utc::now(),
        })
    }

    fn parse_bitrix24_event(&self, payload: &str) -> Result<CRMEventInfo> {
        let payload_data: serde_json::Value = serde_json::from_str(payload)?;
        let event_type = payload_data.get("event").and_then(|e| e.as_str()).unwrap_or("unknown");

        Ok(CRMEventInfo {
            id: uuid::Uuid::new_v4().to_string(),
            provider: "bitrix24".to_string(),
            event_type: event_type.to_string(),
            data: payload_data,
            timestamp: chrono::Utc::now(),
        })
    }

    fn handle_lead_added(&self, event: &CRMEventInfo) -> Result<CRMResponse> {
        log::info!("Processing CRM lead added event");

        // Create lead flow trigger
        self.trigger_crm_flow("new_lead", &event.data).await?;

        Ok(CRMResponse {
            success: true,
            message: "Lead processed successfully".to_string(),
            provider: event.provider.clone(),
            data: Some(event.data.clone()),
        })
    }

    fn handle_lead_updated(&self, event: &CRMEventInfo) -> Result<CRMResponse> {
        log::info!("Processing CRM lead updated event");

        Ok(CRMResponse {
            success: true,
            message: "Lead processed successfully".to_string(),
            provider: event.provider.clone(),
            data: Some(event.data.clone()),
        })
    }

    fn handle_deal_added(&self, event: &CRMEventInfo) -> Result<CRMResponse> {
        log::info!("Processing CRM deal added event");

        // Create deal flow trigger
        self.trigger_crm_flow("new_deal", &event.data).await?;

        Ok(CRMResponse {
            success: true,
            message: "Deal processed successfully".to_string(),
            provider: event.provider.clone(),
            data: Some(event.data.clone()),
        })
    }

    fn handle_deal_updated(&self, event: &CRMEventInfo) -> Result<CRMResponse> {
        log::info!("Processing CRM deal updated event");

        // Trigger flow based on deal stage
        if let Some(stage_id) = event.data.get("STAGE_ID").or(event.data.get("pipeline")) {
            match stage_id.as_str() {
                Some("C1:SUCCESS") | Some("closed_won") => {
                    self.trigger_crm_flow("deal_closed", &event.data).await?;
                }
                Some("C1:APPROVAL") | Some("approval") => {
                    self.trigger_crm_flow("deal_approval", &event.data).await?;
                }
                _ => {}
            }
        }

        Ok(CRMResponse {
            success: true,
            message: "Deal processed successfully".to_string(),
            provider: event.provider.clone(),
            data: Some(event.data.clone()),
        })
    }

    fn handle_contact_added(&self, event: &CRMEventInfo) -> Result<CRMResponse> {
        log::info!("Processing CRM contact added event");

        Ok(CRMResponse {
            success: true,
            message: "Contact processed successfully".to_string(),
            provider: event.provider.clone(),
            data: Some(event.data.clone()),
        })
    }

    fn handle_contact_updated(&self, event: &CRMEventInfo) -> Result<CRMResponse> {
        log::info!("Processing CRM contact updated event");

        Ok(CRMResponse {
            success: true,
            message: "Contact processed successfully".to_string(),
            provider: event.provider.clone(),
            data: Some(event.data.clone()),
        })
    }

    fn handle_task_added(&self, event: &CRMEventInfo) -> Result<CRMResponse> {
        log::info!("Processing CRM task added event");

        Ok(CRMResponse {
            success: true,
            message: "Task processed successfully".to_string(),
            provider: event.provider.clone(),
            data: Some(event.data.clone()),
        })
    }

    fn handle_task_updated(&self, event: &CRMEventInfo) -> Result<CRMResponse> {
        log::info!("Processing CRM task updated event");

        Ok(CRMResponse {
            success: true,
            message: "Task processed successfully".to_string(),
            provider: event.provider.clone(),
            data: Some(event.data.clone()),
        })
    }

    async fn trigger_crm_flow(&self, flow_name: &str, data: &serde_json::Value) -> Result<()> {
        log::info!("Triggering CRM flow: {} with data: {}", flow_name, data);
        
        // TODO: Integrate with FlowLink flow system
        // This would trigger automated workflows based on CRM events
        
        Ok(())
    }
}

// CRM event info structure
#[derive(Debug, Clone)]
pub struct CRMEventInfo {
    pub id: String,
    pub provider: String,
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

// CRM integration health checker
pub struct CRMHealthChecker {
    pub router: Arc<CRMRouter>,
    pub check_interval: std::time::Duration,
}

impl CRMHealthChecker {
    pub fn new(router: Arc<CRMRouter>) -> Self {
        Self {
            router,
            check_interval: std::time::Duration::from_secs(300), // 5 minutes
        }
    }

    pub async fn start_health_checks(&self) -> Result<()> {
        log::info!("Starting CRM health checks");

        loop {
            match self.check_health().await {
                Ok(status) => {
                    log::info!("CRM health check passed: {}", status);
                }
                Err(e) => {
                    log::error!("CRM health check failed: {}", e);
                }
            }

            tokio::time::sleep(self.check_interval).await;
        }
    }

    async fn check_health(&self) -> Result<String> {
        let mut healthy_providers = Vec::new();

        if self.config.amocrm_enabled {
            if let Ok(_) = self.check_amocrm_health().await {
                healthy_providers.push("amocrm");
            }
        }

        if self.config.bitrix24_enabled {
            if let Ok(_) = self.check_bitrix24_health().await {
                healthy_providers.push("bitrix24");
            }
        }

        if healthy_providers.is_empty() {
            return Err(anyhow::anyhow!("No healthy CRM providers"));
        }

        Ok(format!("Healthy providers: {}", healthy_providers.join(", ")))
    }

    async fn check_amocrm_health(&self) -> Result<()> {
        // TODO: Implement actual AmoCRM health check
        Ok(())
    }

    async fn check_bitrix24_health(&self) -> Result<()> {
        // TODO: Implement actual Bitrix24 health check
        Ok(())
    }
}

// CRM analytics collector
pub struct CRMAalyticsCollector {
    pub router: Arc<CRMRouter>,
    pub storage: Arc<CRMStorage>,
}

impl CRMAalyticsCollector {
    pub fn new(router: Arc<CRMRouter>, storage: Arc<CRMStorage>) -> Self {
        Self { router, storage }
    }

    pub async fn collect_analytics(&self) -> Result<()> {
        let stats = self.router.get_all_stats().await?;

        for (provider, analytics) in stats {
            self.save_analytics_metrics(provider, &analytics).await?;
        }

        Ok(())
    }

    async fn save_analytics_metrics(&self, provider: String, analytics: &CRMAalytics) -> Result<()> {
        let date = chrono::Utc::now().date();

        // Save leads count metric
        self.save_analytics_metric(
            &provider,
            "leads_count",
            analytics.leads_count as f64,
            date,
            serde_json::json!({}),
        ).await?;

        // Save leads created today
        self.save_analytics_metric(
            &provider,
            "leads_created_today",
            analytics.leads_created_today as f64,
            date,
            serde_json::json!({}),
        ).await?;

        // Save leads completed today
        self.save_analytics_metric(
            &provider,
            "leads_completed_today",
            analytics.leads_completed_today as f64,
            date,
            serde_json::json!({}),
        ).await?;

        // Save customers count
        self.save_analytics_metric(
            &provider,
            "customers_count",
            analytics.customers_count as f64,
            date,
            serde_json::json!({}),
        ).await?;

        // Save revenue
        self.save_analytics_metric(
            &provider,
            "revenue_monthly",
            analytics.revenue_monthly,
            date,
            serde_json::json!({}),
        ).await?;

        log::info!("Saved analytics metrics for provider: {}", provider);
        Ok(())
    }

    async fn save_analytics_metric(&self, provider: &str, metric: &str, value: f64, date: chrono::Date<chrono::Utc>, metadata: serde_json::Value) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO crm_analytics (provider, metric, value, date, metadata)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (provider, metric, date) DO UPDATE SET
                value = EXCLUDED.value,
                metadata = EXCLUDED.metadata,
                updated_at = NOW()
        "#;

        sqlx::query(insert_sql)
            .bind(provider)
            .bind(metric)
            .bind(value)
            .bind(date)
            .bind(metadata)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }
}
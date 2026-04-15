use anyhow::Result;
use std::sync::Arc;
use std::collections::HashMap;

use super::*;
use super::models::*;

pub struct AmoCRMHandler {
    pub config: AmoCRMConfig,
    pub storage: Arc<CRMStorage>,
}

#[derive(Debug, Clone)]
pub struct AmoCRMConfig {
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub webhook_secret: Option<String>,
    pub api_url: String,
    pub auth_url: String,
}

impl AmoCRMHandler {
    pub fn new(client_id: Option<String>, client_secret: Option<String>) -> Self {
        Self {
            config: AmoCRMConfig {
                client_id,
                client_secret,
                webhook_secret: Some("your-webhook-secret".to_string()),
                api_url: "https://amocrm.ru/api/v4".to_string(),
                auth_url: "https://amocrm.ru/oauth2/authorize".to_string(),
            },
            storage: Arc::new(CRMStorage::new()), // TODO: Pass actual storage
        }
    }
}

#[async_trait::async_trait]
impl CRMHandler for AmoCRMHandler {
    fn name(&self) -> &str {
        "amocrm"
    }

    async fn handle_webhook(&self, payload: &str) -> Result<CRMResponse> {
        let event = self.parse_webhook(payload)?;
        let headers = self.get_headers(payload);

        log::info!("Handling AmoCRM webhook: {}", event.event_type);

        match event.event_type.as_str() {
            "lead.added" => self.handle_lead_added(&event).await,
            "lead.updated" => self.handle_lead_updated(&event).await,
            "lead.status.updated" => self.handle_lead_status_updated(&event).await,
            "contact.added" => self.handle_contact_added(&event).await,
            "contact.updated" => self.handle_contact_updated(&event).await,
            "task.added" => self.handle_task_added(&event).await,
            "task.completed" => self.handle_task_completed(&event).await,
            "note.added" => self.handle_note_added(&event).await,
            _ => {
                log::warn!("Unhandled AmoCRM event: {}", event.event_type);
                Ok(CRMResponse {
                    success: true,
                    message: format!("Event {} received", event.event_type),
                    provider: "amocrm".to_string(),
                    data: None,
                })
            }
        }
    }

    async fn sync_data(&self) -> Result<CRMSyncResult> {
        log::info!("Starting AmoCRM data sync");

        let mut entities_synced = 0;
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Sync leads
        match self.sync_leads().await {
            Ok(count) => {
                entities_synced += count;
                log::info!("Synced {} leads", count);
            }
            Err(e) => {
                errors.push(format!("Failed to sync leads: {}", e));
            }
        }

        // Sync contacts
        match self.sync_contacts().await {
            Ok(count) => {
                entities_synced += count;
                log::info!("Synced {} contacts", count);
            }
            Err(e) => {
                errors.push(format!("Failed to sync contacts: {}", e));
            }
        }

        // Sync accounts
        match self.sync_accounts().await {
            Ok(count) => {
                entities_synced += count;
                log::info!("Synced {} accounts", count);
            }
            Err(e) => {
                errors.push(format!("Failed to sync accounts: {}", e));
            }
        }

        // Save sync result
        let sync_result = CRMSyncResult {
            entities_synced,
            errors,
            warnings,
        };

        Ok(sync_result)
    }
}

impl AmoCRMHandler {
    fn parse_webhook(&self, payload: &str) -> Result<AmoCRMWebhookEvent> {
        let event: AmoCRMWebhookEvent = serde_json::from_str(payload)?;
        Ok(event)
    }

    fn get_headers(&self, payload: &str) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("x-webhook-signature".to_string(), "".to_string());
        headers
    }

    async fn handle_lead_added(&self, event: &AmoCRMWebhookEvent) -> Result<CRMResponse> {
        let lead_data = event.data.get("lead").and_then(|l| l.get("data")).unwrap_or(&serde_json::Value::Null);
        
        log::info!("Lead added: {}", lead_data.get("name").unwrap_or(&serde_json::Value::Null));

        // Trigger flow for new lead
        if let Some(lead_name) = lead_data.get("name").and_then(|n| n.as_str()) {
            self.trigger_lead_flow(lead_name, "new_lead").await?;
        }

        Ok(CRMResponse {
            success: true,
            message: "Lead processed successfully".to_string(),
            provider: "amocrm".to_string(),
            data: Some(lead_data.clone()),
        })
    }

    async fn handle_lead_updated(&self, event: &AmoCRMWebhookEvent) -> Result<CRMResponse> {
        let lead_data = event.data.get("lead").and_then(|l| l.get("data")).unwrap_or(&serde_json::Value::Null);
        
        log::info!("Lead updated: {}", lead_data.get("name").unwrap_or(&serde_json::Value::Null));

        Ok(CRMResponse {
            success: true,
            message: "Lead processed successfully".to_string(),
            provider: "amocrm".to_string(),
            data: Some(lead_data.clone()),
        })
    }

    async fn handle_lead_status_updated(&self, event: &AmoCRMWebhookEvent) -> Result<CRMResponse> {
        let lead_data = event.data.get("lead").and_then(|l| l.get("data")).unwrap_or(&serde_json::Value::Null);
        let old_status = event.data.get("old_value").unwrap_or(&serde_json::Value::Null);
        let new_status = event.data.get("new_value").unwrap_or(&serde_json::Value::Null);

        log::info!("Lead status changed: {} -> {}", old_status, new_status);

        // Trigger status-specific actions
        if let Some(status) = new_status.get("pipeline").and_then(|p| p.get("name")).and_then(|n| n.as_str()) {
            match status {
                "Согласование" => {
                    self.trigger_lead_flow("lead@status:approval", "approval_required").await?;
                }
                "Закрытый" => {
                    self.trigger_lead_flow("lead@status:closed", "deal_closed").await?;
                }
                "Договор" => {
                    self.trigger_lead_flow("lead@status:contract", "contract_sent").await?;
                }
                _ => {}
            }
        }

        Ok(CRMResponse {
            success: true,
            message: "Lead status processed successfully".to_string(),
            provider: "amocrm".to_string(),
            data: Some(lead_data.clone()),
        })
    }

    async fn handle_contact_added(&self, event: &AmoCRMWebhookEvent) -> Result<CRMResponse> {
        let contact_data = event.data.get("contact").and_then(|c| c.get("data")).unwrap_or(&serde_json::Value::Null);
        
        log::info!("Contact added: {}", contact_data.get("name").unwrap_or(&serde_json::Value::Null));

        Ok(CRMResponse {
            success: true,
            message: "Contact processed successfully".to_string(),
            provider: "amocrm".to_string(),
            data: Some(contact_data.clone()),
        })
    }

    async fn handle_contact_updated(&self, event: &AmoCRMWebhookEvent) -> Result<CRMResponse> {
        let contact_data = event.data.get("contact").and_then(|c| c.get("data")).unwrap_or(&serde_json::Value::Null);
        
        log::info!("Contact updated: {}", contact_data.get("name").unwrap_or(&serde_json::Value::Null));

        Ok(CRMResponse {
            success: true,
            message: "Contact processed successfully".to_string(),
            provider: "amocrm".to_string(),
            data: Some(contact_data.clone()),
        })
    }

    async fn handle_task_added(&self, event: &AmoCRMWebhookEvent) -> Result<CRMResponse> {
        let task_data = event.data.get("task").and_then(|t| t.get("data")).unwrap_or(&serde_json::Value::Null);
        
        log::info!("Task added: {}", task_data.get("text").unwrap_or(&serde_json::Value::Null));

        Ok(CRMResponse {
            success: true,
            message: "Task processed successfully".to_string(),
            provider: "amocrm".to_string(),
            data: Some(task_data.clone()),
        })
    }

    async fn handle_task_completed(&self, event: &AmoCRMWebhookEvent) -> Result<CRMResponse> {
        let task_data = event.data.get("task").and_then(|t| t.get("data")).unwrap_or(&serde_json::Value::Null);
        
        log::info!("Task completed: {}", task_data.get("text").unwrap_or(&serde_json::Value::Null));

        Ok(CRMResponse {
            success: true,
            message: "Task completed successfully".to_string(),
            provider: "amocrm".to_string(),
            data: Some(task_data.clone()),
        })
    }

    async fn handle_note_added(&self, event: &AmoCRMWebhookEvent) -> Result<CRMResponse> {
        let note_data = event.data.get("note").and_then(|n| n.get("data")).unwrap_or(&serde_json::Value::Null);
        
        log::info!("Note added to {}", note_data.get("element_type").unwrap_or(&serde_json::Value::Null));

        Ok(CRMResponse {
            success: true,
            message: "Note processed successfully".to_string(),
            provider: "amocrm".to_string(),
            data: Some(note_data.clone()),
        })
    }

    async fn trigger_lead_flow(&self, lead_name: &str, trigger: &str) -> Result<()> {
        log::info!("Triggering flow for lead: {} ({})", lead_name, trigger);
        
        // TODO: Integrate with FlowLink flow system
        // This would trigger automated workflows based on lead changes
        
        Ok(())
    }

    async fn sync_leads(&self) -> Result<i32> {
        log::info!("Syncing leads from AmoCRM");
        
        // TODO: Implement actual leads sync using AmoCRM API
        let leads = vec![
            CRMLead {
                id: 1,
                name: "Test Lead".to_string(),
                status: CRMLeadStatus::FirstContact,
                responsible_user_id: Some(1),
                price: Some(10000.0),
                source_id: Some(1),
                tags: vec!["test".to_string()],
                custom_fields: HashMap::new(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }
        ];

        let sync_count = leads.len();
        log::info!("Synced {} leads from AmoCRM", sync_count);
        
        Ok(sync_count as i32)
    }

    async fn sync_contacts(&self) -> Result<i32> {
        log::info!("Syncing contacts from AmoCRM");
        
        // TODO: Implement actual contacts sync using AmoCRM API
        let contacts = vec![
            CRMContact {
                id: 1,
                name: "Test Contact".to_string(),
                email: Some("test@example.com".to_string()),
                phone: Some("+79001234567".to_string()),
                company: Some("Test Company".to_string()),
                custom_fields: HashMap::new(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }
        ];

        let sync_count = contacts.len();
        log::info!("Synced {} contacts from AmoCRM", sync_count);
        
        Ok(sync_count as i32)
    }

    async fn sync_accounts(&self) -> Result<i32> {
        log::info!("Syncing accounts from AmoCRM");
        
        // TODO: Implement actual accounts sync using AmoCRM API
        let accounts = vec![
            CRMAccount {
                id: 1,
                name: "Test Account".to_string(),
                company_type: "LLC".to_string(),
                country: Some("Russia".to_string()),
                city: Some("Moscow".to_string()),
                responsible_user_id: Some(1),
                custom_fields: HashMap::new(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }
        ];

        let sync_count = accounts.len();
        log::info!("Synced {} accounts from AmoCRM", sync_count);
        
        Ok(sync_count as i32)
    }
}

// AmoCRM-specific webhook event structure
#[derive(Debug, Clone, Deserialize)]
pub struct AmoCRMWebhookEvent {
    pub action: String,
    pub account_id: i64,
    pub entity_type: String,
    pub entity_id: i64,
    pub data: serde_json::Value,
    pub timestamp: i64,
}

// AmoCRM API client
pub struct AmoCRMClient {
    pub config: AmoCRMConfig,
    pub access_token: Option<String>,
}

impl AmoCRMClient {
    pub fn new(config: AmoCRMConfig) -> Self {
        Self {
            config,
            access_token: None,
        }
    }

    pub async fn authenticate(&mut self, auth_code: &str) -> Result<()> {
        // TODO: Implement OAuth2 authentication
        self.access_token = Some("mock-access-token".to_string());
        Ok(())
    }

    pub async fn get_leads(&self) -> Result<Vec<CRMLead>> {
        // TODO: Implement actual API call
        Ok(Vec::new())
    }

    pub async fn get_contacts(&self) -> Result<Vec<CRMContact>> {
        // TODO: Implement actual API call
        Ok(Vec::new())
    }
}
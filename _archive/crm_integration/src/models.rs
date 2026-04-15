use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[async_trait::async_trait]
pub trait CRMHandler: Send + Sync {
    fn name(&self) -> &str;
    async fn handle_webhook(&self, payload: &str) -> Result<CRMResponse>;
    async fn sync_data(&self) -> Result<CRMSyncResult>;
}

#[derive(Debug, Clone, Serialize)]
pub struct CRMResponse {
    pub success: bool,
    pub message: String,
    pub provider: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum CRMDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum CRMStatus {
    New,
    Contact,
    Customer,
    Pending,
    Completed,
    Lost,
    Spam,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum CRMLeadStatus {
    FirstContact,
    Qualified,
    ProposalSent,
    ContractSent,
    Customer,
    Lost,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMSyncResult {
    pub entities_synced: i32,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

// Common CRM entities
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMContact {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub company: Option<String>,
    pub custom_fields: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMLead {
    pub id: i64,
    pub name: String,
    pub status: CRMLeadStatus,
    pub responsible_user_id: Option<i32>,
    pub price: Option<f64>,
    pub source_id: Option<i32>,
    pub tags: Vec<String>,
    pub custom_fields: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMAccount {
    pub id: i64,
    pub name: String,
    pub company_type: String,
    pub country: Option<String>,
    pub city: Option<String>,
    responsible_user_id: Option<i32>,
    pub custom_fields: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMTask {
    pub id: i64,
    pub text: String,
    pub element_type: String,
    pub element_id: i64,
    pub responsible_user_id: Option<i32>,
    pub completed: bool,
    pub date_complete: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMNote {
    pub id: i64,
    pub text: String,
    pub element_type: String,
    pub element_id: i64,
    pub note_type: String,
    pub file: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMPipeline {
    pub id: i64,
    pub name: String,
    pub is_main: bool,
    pub status_ids: Vec<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMStatus {
    pub id: i64,
    name: String,
    is_status: bool,
    sort: i32,
    pipeline_id: i64,
    color: String,
}

// Webhook events
#[derive(Debug, Clone, Deserialize)]
pub struct CRMWebhookEvent {
    pub id: String,
    pub provider: String,
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CRMWebhookResponse {
    pub success: bool,
    pub message: String,
    pub event_id: String,
    pub processed_at: DateTime<Utc>,
}

// Integration configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMIntegrationConfig {
    pub provider: String,
    pub webhook_url: String,
    pub auth_token: Option<String>,
    pub webhook_secret: Option<String>,
    pub sync_frequency: i32,
    pub entities: Vec<String>,
    pub custom_mappings: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMEntityMapping {
    pub local_entity: String,
    pub crm_entity: String,
    pub field_mappings: HashMap<String, String>,
    pub filters: Option<serde_json::Value>,
}

// Sync status
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum CRMSyncStatus {
    Synced,
    Pending,
    Failed,
    Scheduled,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMSyncLog {
    pub id: String,
    pub provider: String,
    pub status: CRMSyncStatus,
    entities_synced: i32,
    errors: Vec<String>,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

// Notification templates
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMNotificationTemplate {
    pub id: String,
    pub name: String,
    pub template: String,
    pub provider: String,
    pub trigger_events: Vec<String>,
    pub target_entity: String,
    pub custom_fields: HashMap<String, String>,
}

// Analytics
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CRMAalytics {
    pub leads_count: i32,
    pub leads_created_today: i32,
    pub leads_completed_today: i32,
    pub customers_count: i32,
    pub revenue_monthly: f64,
    pub conversion_rate: f64,
    pub average_deal_size: f64,
}
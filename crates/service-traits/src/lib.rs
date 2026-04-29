//! FlowLink Service Traits
//!
//! Abstract service interfaces that decouple relay from concrete implementations.
//! Each trait has two implementations:
//! - **Local** (standalone mode): in-process, direct engine access
//! - **Remote** (cloud mode): HTTP calls to separate microservices
//!
//! Relay's AppState holds `Arc<dyn XxxProvider>` instead of concrete types.
//! Switching between cloud/standalone is purely configuration-driven.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════
// Shared Types
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub agent_id: String,
    pub hostname: String,
    pub online: bool,
    pub os: String,
    pub version: Option<String>,
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldAlertInfo {
    pub id: String,
    pub agent_id: String,
    pub risk: String,
    pub command: String,
    pub resolved: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalInfo {
    pub id: String,
    pub agent_id: String,
    pub command: String,
    pub risk: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanInfo {
    pub plan_id: String,
    pub name: String,
    pub price_kopecks: u64,
    pub description: String,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingAccountInfo {
    pub account_id: String,
    pub plan_id: String,
    pub plan_name: String,
    pub status: String,
    pub balance_kopecks: i64,
    pub trial_ends_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    pub agents_connected: u32,
    pub commands_total: u64,
    pub commands_blocked: u64,
    pub storage_used_bytes: u64,
    pub period: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceInfo {
    pub id: String,
    pub amount_kopecks: i64,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthCheckResult {
    pub account_id: String,
    pub is_admin: bool,
    pub org_id: Option<String>,
    pub plan_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenceInfo {
    pub key: String,
    pub customer: String,
    pub tier: String,
    pub max_agents: u32,
    pub max_users: u32,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub features: Vec<String>,
    pub offline_until: chrono::DateTime<chrono::Utc>,
}

// ═══════════════════════════════════════════════
// Billing Provider
// ═══════════════════════════════════════════════

/// Billing service abstraction.
/// Local: uses BillingEngine directly (standalone).
/// Remote: calls billing microservice via HTTP (cloud).
#[async_trait]
pub trait BillingProvider: Send + Sync {
    async fn list_plans(&self) -> anyhow::Result<Vec<PlanInfo>>;
    async fn get_plan(&self, plan_id: &str) -> anyhow::Result<Option<PlanInfo>>;
    async fn get_account_info(&self, account_id: &str) -> anyhow::Result<Option<BillingAccountInfo>>;
    async fn get_or_create_account(&self, account_id: &str) -> anyhow::Result<BillingAccountInfo>;
    async fn change_plan(&self, account_id: &str, plan_id: &str) -> anyhow::Result<()>;
    async fn check_feature(&self, account_id: &str, feature: &str) -> anyhow::Result<bool>;
    async fn track_usage(&self, account_id: &str, tokens: u32) -> anyhow::Result<()>;
    async fn get_usage(&self, account_id: &str) -> anyhow::Result<UsageInfo>;
    async fn list_invoices(&self, account_id: &str) -> anyhow::Result<Vec<InvoiceInfo>>;
    async fn check_agent_limit(&self, account_id: &str) -> anyhow::Result<bool>;
    async fn check_storage_limit(&self, account_id: &str, bytes: u64) -> anyhow::Result<bool>;
}

// ═══════════════════════════════════════════════
// Auth Provider
// ═══════════════════════════════════════════════

#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn validate_token(&self, token: &str) -> anyhow::Result<AuthCheckResult>;
    async fn check_account(&self, account_id: &str) -> anyhow::Result<bool>;
    async fn get_account_orgs(&self, account_id: &str) -> anyhow::Result<Vec<String>>;
    async fn check_org_role(&self, account_id: &str, org_id: &str, required_role: &str) -> anyhow::Result<bool>;
    async fn create_session(&self, account_id: &str, device_info: &str) -> anyhow::Result<String>;
    async fn revoke_session(&self, session_id: &str) -> anyhow::Result<()>;
}

// ═══════════════════════════════════════════════
// Agent Provider
// ═══════════════════════════════════════════════

#[async_trait]
pub trait AgentProvider: Send + Sync {
    async fn list_agents(&self) -> anyhow::Result<Vec<AgentStatus>>;
    async fn get_agent(&self, agent_id: &str) -> anyhow::Result<Option<AgentStatus>>;
    async fn register_agent(&self, agent_id: &str, hostname: &str, os: &str) -> anyhow::Result<()>;
    async fn unregister_agent(&self, agent_id: &str) -> anyhow::Result<()>;
    async fn heartbeat(&self, agent_id: &str) -> anyhow::Result<()>;
    async fn online_count(&self) -> anyhow::Result<u32>;
}

// ═══════════════════════════════════════════════
// Shield Provider
// ═══════════════════════════════════════════════

#[async_trait]
pub trait ShieldProvider: Send + Sync {
    async fn get_alerts(&self) -> anyhow::Result<Vec<ShieldAlertInfo>>;
    async fn get_agent_alerts(&self, agent_id: &str) -> anyhow::Result<Vec<ShieldAlertInfo>>;
    async fn resolve_alert(&self, alert_id: &str, approved: bool) -> anyhow::Result<()>;
    async fn get_pending_approvals(&self) -> anyhow::Result<Vec<ApprovalInfo>>;
    async fn approve_request(&self, approval_id: &str) -> anyhow::Result<()>;
    async fn reject_request(&self, approval_id: &str) -> anyhow::Result<()>;
}

// ═══════════════════════════════════════════════
// Licence Provider (self-hosted only)
// ═══════════════════════════════════════════════

#[async_trait]
pub trait LicenceProvider: Send + Sync {
    async fn verify(&self) -> anyhow::Result<LicenceInfo>;
    fn has_feature(&self, feature: &str) -> bool;
    fn max_agents(&self) -> u32;
    fn max_users(&self) -> u32;
    fn is_expired(&self) -> bool;
    async fn start_periodic_check(&self, interval_secs: u64);
}

// ═══════════════════════════════════════════════
// Notification Provider
// ═══════════════════════════════════════════════

#[async_trait]
pub trait NotificationProvider: Send + Sync {
    async fn send_notification(&self, account_id: &str, title: &str, body: &str, severity: &str) -> anyhow::Result<()>;
    async fn send_org_notification(&self, org_id: &str, title: &str, body: &str, severity: &str) -> anyhow::Result<()>;
    async fn get_preferences(&self, account_id: &str) -> anyhow::Result<HashMap<String, bool>>;
    async fn set_preferences(&self, account_id: &str, prefs: HashMap<String, bool>) -> anyhow::Result<()>;
}

// ═══════════════════════════════════════════════
// Service Mode Configuration
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ServiceMode {
    Standalone,
    Cloud,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEndpoints {
    pub billing_url: Option<String>,
    pub auth_url: Option<String>,
    pub agent_url: Option<String>,
    pub shield_url: Option<String>,
    pub notification_url: Option<String>,
    pub licence_url: Option<String>,
}

impl Default for ServiceEndpoints {
    fn default() -> Self {
        Self {
            billing_url: None,
            auth_url: None,
            agent_url: None,
            shield_url: None,
            notification_url: None,
            licence_url: None,
        }
    }
}

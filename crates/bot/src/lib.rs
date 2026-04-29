//! FlowLink Telegram Bot — commands, notifications, approval handling.
//!
//! This crate provides the Telegram bot interface for FlowLink:
//! - Bot commands (/start, /status, /approve, etc.)
//! - Push notifications via Telegram
//! - Approval workflows via inline keyboards
//!
//! The bot is behind the `tgbot` feature flag (requires `teloxide`).

#[cfg(feature = "tgbot")]
pub mod bot;
#[cfg(feature = "tgbot")]
pub mod commands;
#[cfg(feature = "tgbot")]
pub mod notifications;

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════
// Bot configuration
// ═══════════════════════════════════════════════

/// Bot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    /// Telegram bot token
    pub token: String,
    /// Admin chat IDs for system notifications
    pub admin_chat_ids: Vec<i64>,
    /// Base URL for checkout/pricing links
    pub dashboard_url: String,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            token: String::new(),
            admin_chat_ids: Vec::new(),
            dashboard_url: "http://localhost:3000".to_string(),
        }
    }
}

// ═══════════════════════════════════════════════
// Bot state — self-contained, replaces relay's AppState
// ═══════════════════════════════════════════════

/// Minimal agent info for bot commands
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub agent_id: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub last_heartbeat: i64,
    pub connected_at: i64,
    pub labels: Vec<String>,
    pub capabilities: Vec<String>,
}

/// Minimal agent pool for bot commands
#[derive(Debug, Clone, Default)]
pub struct AgentPool {
    agents: Vec<AgentInfo>,
}

impl AgentPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list(&self) -> Vec<AgentInfo> {
        self.agents.clone()
    }

    pub fn get(&self, id: &str) -> Option<AgentInfo> {
        self.agents.iter().find(|a| a.agent_id == id).cloned()
    }

    pub fn unregister(&self, _id: &str) {
        // In standalone mode, this is a no-op
        // In relay integration, this delegates to the real pool
    }
}

/// Approval decision
#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalDecision {
    Approved,
    Rejected,
}

/// Minimal pending approval
#[derive(Debug, Clone)]
pub struct PendingApproval {
    pub id: String,
    pub agent_id: String,
    pub command: String,
    pub risk_level: String,
    pub created_at: i64,
}

/// Minimal approval queue
#[derive(Debug, Clone, Default)]
pub struct ApprovalQueue {
    pending: Vec<PendingApproval>,
}

impl ApprovalQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list_pending(&self) -> Vec<PendingApproval> {
        self.pending.clone()
    }

    pub fn resolve(&self, id: &str, _decision: ApprovalDecision) -> bool {
        // In standalone mode, this is a simplified version
        true
    }
}

/// Shield alert info
#[derive(Debug, Clone)]
pub struct ShieldAlert {
    pub id: String,
    pub message: String,
    pub active: bool,
}

/// Minimal shield alert manager
#[derive(Debug, Clone, Default)]
pub struct ShieldAlertManager {
    alerts: Vec<ShieldAlert>,
}

impl ShieldAlertManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list_active(&self) -> Vec<ShieldAlert> {
        self.alerts.iter().filter(|a| a.active).cloned().collect()
    }

    pub fn list_all(&self) -> Vec<ShieldAlert> {
        self.alerts.clone()
    }
}

/// Usage stats
#[derive(Debug, Clone, Default)]
pub struct UsageTracker {
    daily_requests: u64,
    daily_tokens: u64,
}

impl UsageTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn today_stats(&self) -> (u64, u64) {
        (self.daily_requests, self.daily_tokens)
    }

    pub async fn get_all_usage(&self) -> std::collections::HashMap<String, u64> {
        std::collections::HashMap::new()
    }
}

/// Bot state — self-contained state for the bot crate.
/// Replaces relay's AppState with a simplified version that can be constructed
/// independently or populated from relay.
#[derive(Clone)]
pub struct BotState {
    /// Database pool
    pub db: Option<std::sync::Arc<flowlink_db::DbPool>>,
    /// Agent pool
    pub pool: std::sync::Arc<AgentPool>,
    /// Billing engine
    pub billing: Option<std::sync::Arc<flowlink_billing::BillingEngine>>,
    /// Approval queue
    pub approvals: std::sync::Arc<ApprovalQueue>,
    /// Shield alerts
    pub shield_alerts: std::sync::Arc<ShieldAlertManager>,
    /// Usage tracker
    pub usage_tracker: std::sync::Arc<UsageTracker>,
    /// Notification router
    pub notification_router: Option<std::sync::Arc<flowlink_notifications::NotificationRouter>>,
    /// Base URL for checkout links
    pub base_url: String,
}

impl Default for BotState {
    fn default() -> Self {
        Self {
            db: None,
            pool: std::sync::Arc::new(AgentPool::new()),
            billing: None,
            approvals: std::sync::Arc::new(ApprovalQueue::new()),
            shield_alerts: std::sync::Arc::new(ShieldAlertManager::new()),
            usage_tracker: std::sync::Arc::new(UsageTracker::new()),
            notification_router: None,
            base_url: "http://localhost:3000".to_string(),
        }
    }
}

impl BotState {
    /// Create a new BotState with minimal defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a BotState with a database pool
    pub fn with_db(db: flowlink_db::DbPool) -> Self {
        Self {
            db: Some(std::sync::Arc::new(db)),
            ..Self::default()
        }
    }

    /// Set billing engine
    pub fn with_billing(mut self, billing: std::sync::Arc<flowlink_billing::BillingEngine>) -> Self {
        self.billing = Some(billing);
        self
    }

    /// Set notification router
    pub fn with_notification_router(mut self, router: std::sync::Arc<flowlink_notifications::NotificationRouter>) -> Self {
        self.notification_router = Some(router);
        self
    }

    /// Set base URL
    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.to_string();
        self
    }
}

// Re-export the approval types for callback handler
// Types are already defined above — no duplicate re-exports needed

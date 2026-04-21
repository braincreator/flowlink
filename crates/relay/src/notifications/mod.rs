//! Notification channel system — extensible, channel-agnostic alert delivery.
//!
//! Architecture:
//!   NotificationRouter (dispatch)
//!     ├── Resolves target channels from DB (user_notification_channels)
//!     ├── TelegramChannel      (implemented)
//!     ├── MaxMessengerChannel  (future)
//!     ├── SlackChannel         (future)
//!     ├── WebhookChannel       (future, generic HTTP POST)
//!     └── EmailChannel         (via existing EmailService)
//!
//! Per-user channel binding:
//!   1. User binds TG → /start in bot → upsert(account_id, "telegram", chat_id)
//!   2. User binds MAX → sends code → upsert(account_id, "max", max_user_id)
//!   3. User binds Slack → OAuth → upsert(account_id, "slack", webhook_url)
//!   4. Each binding has: is_primary, verified, mute_categories, min_severity
//!
//! Router flow:
//!   notification.account_id → DB lookup → user's channels → filter → deliver
//!
//! Configuration (env vars):
//!   FLOWLINK_NOTIFY_TELEGRAM_CHAT_ID   — global admin TG (system-level alerts)
//!   FLOWLINK_NOTIFY_MAX_TOKEN          — enable MAX channel (future)
//!   FLOWLINK_NOTIFY_SLACK_WEBHOOK_URL  — enable Slack channel (future)

mod telegram;

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// Re-exports
pub use telegram::TelegramChannel;

/// Severity level for notifications.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Low-priority informational message
    Info = 0,
    /// Something needs attention
    Warning = 1,
    /// Action required (e.g., shield blocked a command)
    Alert = 2,
    /// Critical failure (payment failed, subscription cancelled)
    Critical = 3,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Alert => "alert",
            Self::Critical => "critical",
        }
    }

    /// Parse from string (DB storage).
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "warning" => Self::Warning,
            "alert" => Self::Alert,
            "critical" => Self::Critical,
            _ => Self::Info,
        }
    }

    /// Emoji for channels that support it (TG, MAX)
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Info => "ℹ️",
            Self::Warning => "⚠️",
            Self::Alert => "🚨",
            Self::Critical => "🔴",
        }
    }
}

/// Category of notification — channels may route/filter by category.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Shield security alerts
    Shield,
    /// Billing / payment events
    Billing,
    /// Agent lifecycle (connect, disconnect, health)
    Agent,
    /// System events (deploy, config reload, health)
    System,
    /// Audit events (approval requested/granted)
    Audit,
}

impl Category {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Shield => "shield",
            Self::Billing => "billing",
            Self::Agent => "agent",
            Self::System => "system",
            Self::Audit => "audit",
        }
    }

    /// Label with emoji for message formatting.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Shield => "🛡️ Shield",
            Self::Billing => "💳 Billing",
            Self::Agent => "🤖 Agent",
            Self::System => "⚙️ System",
            Self::Audit => "📋 Audit",
        }
    }
}

/// A structured notification payload — channel-agnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// Unique identifier (for dedup)
    pub id: String,
    /// Target account_id — router looks up this user's bound channels
    pub account_id: String,
    /// Severity level
    pub severity: Severity,
    /// Category
    pub category: Category,
    /// Plain-text subject line (for channels with subject support: email, Slack)
    pub subject: String,
    /// Message body — supports basic HTML (`<b>`, `<code>`, `<a>`)
    /// Channels that don't support HTML should strip tags.
    pub body: String,
    /// Structured data payload (for webhook channels, logging, etc.)
    pub data: HashMap<String, serde_json::Value>,
    /// Timestamp (RFC3339)
    pub timestamp: String,
    /// Tags for routing/filtering
    pub tags: Vec<String>,
}

impl Notification {
    /// Quick constructor for shield alerts targeting a specific account.
    pub fn shield_alert(account_id: &str, pid: i32, username: &str, command: &str, rule: &str, action: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: account_id.into(),
            severity: Severity::Alert,
            category: Category::Shield,
            subject: format!("Shield Alert: {} blocked {}", rule, command),
            body: format!(
                "<b>Shield Alert</b>\nPID: {}\nUser: {}\nCommand: <code>{}</code>\nRule: {}\nAction: {}",
                pid, username, command, rule, action,
            ),
            data: HashMap::from([
                ("pid".into(), serde_json::json!(pid)),
                ("username".into(), serde_json::json!(username)),
                ("command".into(), serde_json::json!(command)),
                ("rule_name".into(), serde_json::json!(rule)),
                ("action".into(), serde_json::json!(action)),
            ]),
            timestamp: chrono::Utc::now().to_rfc3339(),
            tags: vec!["shield".into(), "security".into()],
        }
    }

    /// Quick constructor for billing events.
    pub fn billing_event(account_id: &str, subject: &str, body: &str, severity: Severity, data: HashMap<String, serde_json::Value>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: account_id.into(),
            severity,
            category: Category::Billing,
            subject: subject.into(),
            body: body.into(),
            data,
            timestamp: chrono::Utc::now().to_rfc3339(),
            tags: vec!["billing".into()],
        }
    }

    /// Quick constructor for system events (no specific account — goes to global channels only).
    pub fn system(subject: &str, body: &str, severity: Severity) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: String::new(), // global
            severity,
            category: Category::System,
            subject: subject.into(),
            body: body.into(),
            data: HashMap::new(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            tags: vec!["system".into()],
        }
    }

    /// Convenience: set severity (builder pattern).
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Convenience: add a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

/// A notification delivery channel.
///
/// Implement this trait to add a new notification channel (MAX, Slack, webhook, etc.).
/// Each channel receives pre-resolved delivery info via `deliver_to()`.
#[async_trait]
pub trait NotificationChannel: Send + Sync + std::fmt::Debug {
    /// Channel type identifier (must match `channel_type` in DB: "telegram", "max", "slack", etc.)
    fn channel_type(&self) -> &str;

    /// Human-readable channel name (for logging).
    fn name(&self) -> &str;

    /// Send the notification to a specific address.
    /// `address` comes from `user_notification_channels.channel_address`.
    /// Errors are logged but don't propagate — one channel failing must not block others.
    async fn deliver_to(&self, address: &str, notification: &Notification) -> anyhow::Result<()>;
}

/// Notification router — resolves per-user channel bindings from DB and dispatches.
///
/// Two tiers of delivery:
/// 1. **Per-user**: notification.account_id → DB lookup → user's verified channels
/// 2. **Global**: notification.account_id is empty → global admin channels (env-configured)
#[derive(Debug)]
pub struct NotificationRouter {
    /// Available channel implementations (keyed by channel_type)
    channels: HashMap<String, Box<dyn NotificationChannel>>,
    /// Global admin channels (for system events, no specific account)
    global_channels: Vec<(String, String)>, // (channel_type, channel_address)
    /// DB pool for looking up user bindings (optional — works without DB for global only)
    pool: Option<sqlx::PgPool>,
}

impl Default for NotificationRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationRouter {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            global_channels: Vec::new(),
            pool: None,
        }
    }

    /// Register a channel implementation.
    pub fn with_channel(mut self, channel: impl NotificationChannel + 'static) -> Self {
        let ctype = channel.channel_type().to_string();
        self.channels.insert(ctype, Box::new(channel));
        self
    }

    /// Set DB pool for per-user channel resolution.
    pub fn with_db(mut self, pool: sqlx::PgPool) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Add a global admin channel (for system-level alerts).
    pub fn with_global(mut self, channel_type: &str, address: &str) -> Self {
        self.global_channels.push((channel_type.into(), address.into()));
        self
    }

    /// Build a router from environment variables.
    ///
    /// Reads `FLOWLINK_NOTIFY_*` env vars and creates matching channels.
    /// This is the standard factory for production use.
    pub fn from_env(pool: Option<sqlx::PgPool>) -> Self {
        let mut router = Self::new().with_db_opt(pool);

        // Telegram channel implementation
        #[cfg(feature = "tgbot")]
        {
            if let Ok(token) = std::env::var("FLOWLINK_TG_BOT_TOKEN") {
                if !token.is_empty() {
                    router = router.with_channel(TelegramChannel::new(token));
                    log::info!("Notification channel impl: Telegram");

                    // Global admin channel from env
                    if let Ok(chat_id) = std::env::var("FLOWLINK_NOTIFY_TELEGRAM_CHAT_ID") {
                        if let Ok(id) = chat_id.parse::<i64>() {
                            if id != 0 {
                                router = router.with_global("telegram", &id.to_string());
                                log::info!("Notification global: Telegram (chat {id})");
                            }
                        }
                    }
                }
            }
        }

        if router.channels.is_empty() && router.global_channels.is_empty() {
            log::info!("No notification channels configured");
        } else {
            log::info!(
                "Notification router: {} impl(s), {} global channel(s)",
                router.channels.len(),
                router.global_channels.len(),
            );
        }

        router
    }

    /// Set DB pool (for use after construction).
    fn with_db_opt(mut self, pool: Option<sqlx::PgPool>) -> Self {
        self.pool = pool;
        self
    }

    /// Send a notification — resolves target channels from DB and delivers.
    /// Returns number of successful deliveries.
    pub async fn send(&self, notification: &Notification) -> usize {
        let mut ok = 0usize;

        // 1. Per-user channels (from DB)
        if !notification.account_id.is_empty() {
            if let Some(ref pool) = self.pool {
                match flowlink_db::notification_channels::UserChannelRepo::list_for_account(pool, &notification.account_id).await {
                    Ok(bindings) => {
                        for binding in bindings {
                            // Check severity filter
                            if let Some(ref min_sev) = binding.min_severity {
                                let threshold = Severity::from_str_lossy(min_sev);
                                if notification.severity < threshold {
                                    continue;
                                }
                            }
                            // Check mute categories
                            if let Some(ref muted) = binding.mute_categories {
                                if let Ok(categories) = serde_json::from_value::<Vec<String>>(muted.clone()) {
                                    if categories.iter().any(|c| c == notification.category.as_str()) {
                                        continue;
                                    }
                                }
                            }
                            // Deliver
                            if let Some(channel) = self.channels.get(&binding.channel_type) {
                                match channel.deliver_to(&binding.channel_address, notification).await {
                                    Ok(()) => ok += 1,
                                    Err(e) => {
                                        log::warn!(
                                            "Notification [{}] → {}@{} failed: {}",
                                            notification.subject,
                                            binding.channel_type,
                                            binding.channel_address,
                                            e,
                                        );
                                    }
                                }
                            } else {
                                log::debug!(
                                    "No channel impl for '{}' (account {})",
                                    binding.channel_type,
                                    binding.account_id,
                                );
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to resolve channels for account {}: {}", notification.account_id, e);
                    }
                }
            }
        }

        // 2. Global channels (for system events, admin alerts, or fallback)
        let goes_global = notification.account_id.is_empty()
            || notification.tags.contains(&"global_fallback".into());
        if goes_global {
            for (channel_type, address) in &self.global_channels {
                if let Some(channel) = self.channels.get(channel_type) {
                    match channel.deliver_to(address, notification).await {
                        Ok(()) => ok += 1,
                        Err(e) => {
                            log::warn!("Notification [{}] → global {}@{} failed: {}", notification.subject, channel_type, address, e);
                        }
                    }
                }
            }
        }

        ok
    }

    /// Number of registered channel implementations.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MockChannel {
        ctype: String,
    }

    impl MockChannel {
        fn new(ctype: &str) -> Self {
            Self { ctype: ctype.into() }
        }
    }

    #[async_trait]
    impl NotificationChannel for MockChannel {
        fn channel_type(&self) -> &str {
            &self.ctype
        }

        fn name(&self) -> &str {
            &self.ctype
        }

        async fn deliver_to(&self, _address: &str, _notification: &Notification) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_router_per_user_without_db() {
        let router = NotificationRouter::new()
            .with_channel(MockChannel::new("telegram"))
            .with_global("telegram", "12345");

        // Global notification (no account_id) → goes to global channels
        let n = Notification::system("test", "body", Severity::Info);
        let delivered = router.send(&n).await;
        assert_eq!(delivered, 1);

        // Per-user notification without DB → 0 deliveries (no DB to resolve channels)
        let n = Notification::shield_alert("acc_1", 1, "root", "ls", "safe", "allowed");
        let delivered = router.send(&n).await;
        assert_eq!(delivered, 0);
    }

    #[test]
    fn test_notification_builder() {
        let n = Notification::shield_alert("acc_1", 1234, "root", "rm -rf /", "destructive", "blocked");
        assert_eq!(n.severity, Severity::Alert);
        assert_eq!(n.category, Category::Shield);
        assert_eq!(n.account_id, "acc_1");
        assert!(n.body.contains("1234"));
        assert!(n.tags.contains(&"shield".into()));
    }

    #[test]
    fn test_billing_notification() {
        let n = Notification::billing_event(
            "acc_2",
            "Payment failed",
            "Card declined",
            Severity::Critical,
            HashMap::from([("order_id".into(), serde_json::json!("ord_123"))]),
        );
        assert_eq!(n.category, Category::Billing);
        assert_eq!(n.data["order_id"], "ord_123");
        assert_eq!(n.severity, Severity::Critical);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Alert);
        assert!(Severity::Alert < Severity::Critical);
    }

    #[test]
    fn test_severity_from_str() {
        assert_eq!(Severity::from_str_lossy("critical"), Severity::Critical);
        assert_eq!(Severity::from_str_lossy("alert"), Severity::Alert);
        assert_eq!(Severity::from_str_lossy("info"), Severity::Info);
        assert_eq!(Severity::from_str_lossy("unknown"), Severity::Info);
    }

    #[test]
    fn test_category_label() {
        assert_eq!(Category::Shield.label(), "🛡️ Shield");
        assert_eq!(Category::Billing.label(), "💳 Billing");
    }
}

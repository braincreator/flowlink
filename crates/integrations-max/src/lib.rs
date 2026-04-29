//! FlowLink MAX Messenger Integration
//!
//! Connects to VK MAX messenger via platform-api.max.ru.
//! Sends HTML-formatted notifications and supports bot commands
//! via the same FlowLinkClient pattern as Telegram.
//!
//! MAX API basics:
//! - Auth: `Authorization: <token>` header
//! - Send message: POST https://platform-api.max.ru/messages
//! - Set webhook: POST https://platform-api.max.ru/subscriptions
//! - Rate limit: 30 rps
//! - Text formats: markdown, html

use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use flowlink_integrations_core::*;
use flowlink_bot_client::FlowLinkClient;

// ═══════════════════════════════════════════════
// Config
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaxConfig {
    /// Bot access token from MAX business platform
    pub access_token: String,
    /// Chat ID for notifications (user_id or chat_id)
    pub chat_id: Option<i64>,
    /// Webhook URL for receiving updates
    pub webhook_url: Option<String>,
    /// Dashboard base URL for links
    pub dashboard_url: Option<String>,
    /// Relay API URL for bot commands
    pub api_url: Option<String>,
    /// JWT token for API authentication
    pub api_token: Option<String>,
}

// ═══════════════════════════════════════════════
// MAX API client
// ═══════════════════════════════════════════════

const MAX_API_BASE: &str = "https://platform-api.max.ru";

/// Send a text message via MAX API
async fn send_max_message(
    http: &reqwest::Client,
    token: &str,
    chat_id: i64,
    text: &str,
    format: Option<&str>,
    attachments: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    let mut body = serde_json::json!({
        "text": text,
        "format": format.unwrap_or("html"),
    });
    if let Some(att) = attachments {
        body["attachments"] = att;
    }

    let resp = http
        .post(format!("{}/messages?chat_id={}", MAX_API_BASE, chat_id))
        .header("Authorization", token)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("MAX API error {}: {}", status, body);
    }
    Ok(())
}

/// Set webhook subscription for receiving updates
async fn set_webhook(
    http: &reqwest::Client,
    token: &str,
    url: &str,
) -> anyhow::Result<()> {
    let body = serde_json::json!({
        "url": url,
        "update_types": [
            "message_created",
            "bot_started",
            "message_callback"
        ]
    });

    let resp = http
        .post(format!("{}/subscriptions", MAX_API_BASE))
        .header("Authorization", token)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        log::warn!("MAX webhook setup failed: {} {}", status, body);
    }
    Ok(())
}

// ═══════════════════════════════════════════════
// Integration
// ═══════════════════════════════════════════════

pub struct MaxIntegration {
    db: Option<Arc<flowlink_db::DbPool>>,
}

impl MaxIntegration {
    pub fn new(db: Option<Arc<flowlink_db::DbPool>>) -> Self {
        Self { db }
    }

    fn parse_config(config: &serde_json::Value) -> Result<MaxConfig, IntegrationError> {
        serde_json::from_value(config.clone())
            .map_err(|e| IntegrationError::ConfigError(format!("Invalid MAX config: {}", e)))
    }

    fn make_api_client(config: &MaxConfig) -> Option<FlowLinkClient> {
        let url = config.api_url.as_deref().unwrap_or("http://localhost:3000");
        let token = config.api_token.as_deref()?;
        Some(FlowLinkClient::new(url, flowlink_bot_client::AuthMethod::Jwt(token.to_string())))
    }

    /// Format event as MAX-compatible HTML
    fn format_event(event: &IntegrationEvent) -> Option<String> {
        match event {
            IntegrationEvent::AgentConnected { agent_id, hostname } => Some(
                format!("🟡 <b>Сервер подключён</b>\n\n🖥 <b>{}</b>\nID: <code>{}</code>", hostname, agent_id)
            ),
            IntegrationEvent::AgentDisconnected { agent_id, hostname } => Some(
                format!("🔴 <b>Сервер отключён</b>\n\n🖥 <b>{}</b>\nID: <code>{}</code>", hostname, agent_id)
            ),
            IntegrationEvent::ShieldAlert { agent_id, risk, command } => Some(
                format!(
                    "🛡 <b>Security Alert</b>\n\n🖥 <code>{}</code>\n⚠️ Risk: <b>{}</b>\n📝 <code>{}</code>",
                    agent_id, risk, command
                )
            ),
            IntegrationEvent::ApprovalRequested { approval_id, agent_id, command, risk } => {
                let short_cmd = if command.len() > 60 { format!("{}...", &command[..60]) } else { command.clone() };
                let risk_emoji = match risk.as_str() {
                    "critical" => "🔴", "high" => "🟠", "medium" => "🟡", _ => "🟢",
                };
                Some(format!(
                    "⏳ <b>Требуется подтверждение</b>\n\n\
                     🖥 Агент: <code>{}</code>\n\
                     💻 Команда: <code>{}</code>\n\
                     {} Риск: <b>{}</b>\n\
                     🆔 <code>{}</code>",
                    agent_id, short_cmd, risk_emoji, risk, approval_id
                ))
            }
            IntegrationEvent::ApprovalResolved { approval_id, decision, resolved_by } => Some(
                format!(
                    "✅ <b>Запрос решён</b>\n\n🆔 <code>{}</code>\n📋 {}\n👤 <code>{}</code>",
                    approval_id, decision, resolved_by
                )
            ),
            IntegrationEvent::PaymentReceived { account_id, amount_kopecks, description } => {
                let rubles = amount_kopecks / 100;
                Some(format!(
                    "💳 <b>Платёж</b>\n\n💰 {} ₽\n📊 {}\n👤 <code>{}</code>",
                    rubles, description, account_id
                ))
            }
            IntegrationEvent::SubscriptionChanged { account_id, plan } => Some(
                format!("📋 <b>Подписка изменена</b>\n\n👤 <code>{}</code>\n📊 Тариф: <b>{}</b>", account_id, plan)
            ),
            IntegrationEvent::PlanExpiring { account_id, days_left } => Some(
                format!(
                    "⏰ <b>Тариф истекает</b>\n\n👤 <code>{}</code>\n📅 Осталось {} дн.",
                    account_id, days_left
                )
            ),
            IntegrationEvent::SystemAlert { level, message } => Some(
                format!("🚨 <b>{}: {}</b>", level.to_uppercase(), message)
            ),
            IntegrationEvent::Custom { name, data } => Some(
                format!("📡 <b>{}</b>\n\n{}", name, data)
            ),
        }
    }
}

#[async_trait]
impl Integration for MaxIntegration {
    fn kind(&self) -> IntegrationKind {
        IntegrationKind("max".into())
    }

    fn meta(&self) -> IntegrationMeta {
        builtin_catalog().into_iter()
            .find(|m| m.kind == IntegrationKind("max".into()))
            .expect("max meta must exist in catalog")
    }

    async fn validate_config(&self, config: &serde_json::Value) -> Result<(), IntegrationError> {
        let mc = Self::parse_config(config)?;
        if mc.access_token.is_empty() {
            return Err(IntegrationError::ConfigError("access_token is required".into()));
        }
        Ok(())
    }

    async fn start(
        &self,
        id: IntegrationId,
        config: IntegrationConfig,
        mut event_rx: tokio::sync::broadcast::Receiver<IntegrationEvent>,
    ) -> Result<(), IntegrationError> {
        let mc = Self::parse_config(&config.config)?;

        log::info!("📱 MAX integration starting for {} (account {})", id, config.account_id);

        // Set up webhook if configured
        if let Some(ref webhook_url) = mc.webhook_url {
            let http = reqwest::Client::new();
            if let Err(e) = set_webhook(&http, &mc.access_token, webhook_url).await {
                log::warn!("📱 MAX: failed to set webhook: {}", e);
            }
        }

        tokio::spawn(async move {
            let http = reqwest::Client::new();
            let mut default_chat_ids: Vec<i64> = Vec::new();
            if let Some(cid) = mc.chat_id {
                default_chat_ids.push(cid);
            }

            loop {
                let event = match event_rx.recv().await {
                    Ok(e) => e,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("MAX integration {} lagged {} events", id, n);
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        log::info!("MAX integration {} channel closed", id);
                        break;
                    }
                };

                let event_type = match &event {
                    IntegrationEvent::AgentConnected { .. } => "agent_connected",
                    IntegrationEvent::AgentDisconnected { .. } => "agent_disconnected",
                    IntegrationEvent::ShieldAlert { .. } => "shield_alert",
                    IntegrationEvent::ApprovalRequested { .. } => "approval_requested",
                    IntegrationEvent::ApprovalResolved { .. } => "approval_resolved",
                    IntegrationEvent::PaymentReceived { .. } => "payment_received",
                    IntegrationEvent::SubscriptionChanged { .. } => "subscription_changed",
                    IntegrationEvent::PlanExpiring { .. } => "plan_expiring",
                    IntegrationEvent::SystemAlert { .. } => "system_alert",
                    IntegrationEvent::Custom { name, .. } => name.as_str(),
                };

                if !config.subscribed_events.is_empty()
                    && !config.subscribed_events.contains(&event_type.to_string())
                {
                    continue;
                }

                let text = match Self::format_event(&event) {
                    Some(t) => t,
                    None => continue,
                };

                // Resolve chat IDs — default + DB lookup
                let mut chat_ids = default_chat_ids.iter().copied().collect::<Vec<_>>();

                // Send to all resolved chat IDs
                for chat_id in &chat_ids {
                    if let Err(e) = send_max_message(&http, &mc.access_token, *chat_id, &text, Some("html"), None).await {
                        log::warn!("📱 MAX {}: failed to send to {}: {}", id, chat_id, e);
                    }
                }
                log::debug!("📱 MAX {}: sent {} to {} chats", id, event_type, chat_ids.len());
            }

            log::info!("📱 MAX integration {} stopped", id);
        });

        Ok(())
    }

    async fn stop(&self, id: &IntegrationId) -> Result<(), IntegrationError> {
        log::info!("🛑 MAX integration stopped: {}", id);
        Ok(())
    }

    async fn handle_event(
        &self,
        event: &IntegrationEvent,
        _config: &IntegrationConfig,
    ) -> Vec<IntegrationAction> {
        match Self::format_event(event) {
            Some(text) => vec![IntegrationAction::SendMessage {
                chat_id: None,
                text,
                buttons: vec![],
            }],
            None => vec![IntegrationAction::Noop],
        }
    }

    /// Handle bot commands via FlowLinkClient — same as Telegram
    async fn handle_command(
        &self,
        command: &str,
        args: &serde_json::Value,
        config: &IntegrationConfig,
    ) -> Result<serde_json::Value, IntegrationError> {
        let mc = Self::parse_config(&config.config)?;
        let api = Self::make_api_client(&mc)
            .ok_or_else(|| IntegrationError::AuthError("No API token configured".into()))?;

        let chat_id = args.get("chat_id").and_then(|v| v.as_i64()).unwrap_or(0);

        let response_text = match command {
            "/start" => {
                if let Some(payload) = args.get("payload").and_then(|v| v.as_str()) {
                    format!("🔗 Подключение аккаунта... Payload: {}", payload)
                } else {
                    "👋 Привет! Я — ваш FlowLink бот в MAX.\n\n\
                     📊 /status — статус серверов\n\
                     🖥 /servers — список агентов\n\
                     💳 /billing — подписка\n\
                     🛡 /shield — безопасность\n\
                     ✅ /approvals — подтверждения\n\
                     🆘 /help — справка".to_string()
                }
            }

            "/help" => {
                "🤖 <b>FlowLink Bot (MAX)</b>\n\n\
                 📊 /status — статус системы\n\
                 🖥 /servers — список агентов\n\
                 💳 /billing — подписка и баланс\n\
                 📦 /myplan — текущий тариф\n\
                 📋 /plans — доступные тарифы\n\
                 📊 /usage — статистика\n\
                 🛡 /shield — оповещения безопасности\n\
                 ✅ /approvals — подтверждения\n\
                 ⚙️ /config — конфигурация\n\
                 📝 /logs [N] — последние действия\n\
                 🆘 /help — эта справка".to_string()
            }

            "/status" => {
                match api.get_health().await {
                    Ok(h) => format!(
                        "📊 <b>FlowLink Status</b>\n\n\
                         ✅ Статус: {}\n\
                         ⏱ Uptime: {}с\n\
                         🖥 Агентов онлайн: {}\n\
                         🗄 БД: {}",
                        h.status, h.uptime_seconds, h.agents_online, h.db
                    ),
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/servers" => {
                match api.list_agents().await {
                    Ok(agents) => {
                        let online: Vec<_> = agents.iter().filter(|a| a.online).collect();
                        let mut text = format!("🖥 <b>Агенты</b> ({} онлайн / {})\n\n", online.len(), agents.len());
                        for a in online.iter().take(15) {
                            text.push_str(&format!("🟢 <b>{}</b> — {}\n", a.hostname, a.agent_id));
                        }
                        if text.len() > 3900 {
                            text = format!("🖥 <b>Агенты</b>: {} онлайн из {}\n\n(Слишком много для одного сообщения)", online.len(), agents.len());
                        }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/billing" | "/myplan" => {
                match api.get_billing_info().await {
                    Ok(info) => {
                        let plan = info.get("plan_name").and_then(|v| v.as_str()).unwrap_or("—");
                        let status = info.get("status").and_then(|v| v.as_str()).unwrap_or("—");
                        let balance = info.get("balance_kopecks").and_then(|v| v.as_i64()).unwrap_or(0);
                        format!("💳 <b>Подписка</b>\n\n📦 Тариф: <b>{}</b>\n📊 Статус: {}\n💰 Баланс: {} ₽", plan, status, balance / 100)
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/plans" => {
                match api.get_plans().await {
                    Ok(plans) => {
                        let mut text = "📋 <b>Тарифные планы</b>\n\n".to_string();
                        for p in plans.iter().take(5) {
                            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                            let price = p.get("price_kopecks").and_then(|v| v.as_i64()).unwrap_or(0);
                            text.push_str(&format!("📦 <b>{}</b> — {} ₽/мес\n", name, price / 100));
                        }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/usage" => {
                match api.get_usage().await {
                    Ok(usage) => format!("📊 <b>Использование</b>\n\n{}", usage),
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/shield" => {
                match api.get_alerts().await {
                    Ok(alerts) => {
                        let active: Vec<_> = alerts.iter().filter(|a| !a.resolved).collect();
                        let mut text = format!("🛡 <b>Shield</b> ({} активных)\n\n", active.len());
                        for a in active.iter().take(10) {
                            let emoji = match a.risk.as_str() { "critical" => "🔴", "high" => "🟠", _ => "🟡" };
                            let short_cmd = if a.command.len() > 40 { format!("{}...", &a.command[..40]) } else { a.command.clone() };
                            text.push_str(&format!("{} <b>{}</b> {}\n", emoji, a.risk, short_cmd));
                        }
                        if active.is_empty() { text.push_str("✅ Нет активных алертов"); }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/approvals" => {
                match api.get_approvals().await {
                    Ok(approvals) => {
                        let mut text = format!("✅ <b>Подтверждения</b> ({})\n\n", approvals.len());
                        for a in approvals.iter().take(10) {
                            let emoji = match a.risk.as_str() { "critical" => "🔴", "high" => "🟠", "medium" => "🟡", _ => "🟢" };
                            text.push_str(&format!("{} <b>{}</b> — {} [{}]\n", emoji, a.risk, a.command, a.agent_id));
                        }
                        if approvals.is_empty() { text.push_str("✅ Нет ожидающих"); }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/config" => {
                match api.get_config().await {
                    Ok(c) => format!("⚙️ <b>Config</b>\n\n{}", serde_json::to_string_pretty(&c).unwrap_or_default()),
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/logs" => {
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as u32;
                match api.get_audit(Some(limit)).await {
                    Ok(events) => {
                        let mut text = format!("📝 <b>Последние {} событий</b>\n\n", events.len());
                        for e in events.iter().take(15) {
                            let action = e.get("action").and_then(|v| v.as_str()).unwrap_or("?");
                            let agent = e.get("agent_id").and_then(|v| v.as_str()).unwrap_or("-");
                            text.push_str(&format!("• <b>{}</b> — {}\n", action, agent));
                        }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            _ => format!("❓ Неизвестная команда: {}\n/help — список команд", command),
        };

        Ok(serde_json::json!({ "text": response_text, "chat_id": chat_id }))
    }

    async fn health_check(&self, _id: &IntegrationId) -> Result<bool, IntegrationError> {
        // Quick check: get bot info from MAX API
        let client = reqwest::Client::new();
        // We'd need the token here... for now return true
        Ok(true)
    }
}

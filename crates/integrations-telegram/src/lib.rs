//! FlowLink Telegram Integration — full implementation.
//!
//! Each user/org installs their own Telegram bot via the marketplace.
//! The bot receives events from the IntegrationManager event bus
//! and sends HTML-formatted notifications to linked chat IDs.
//!
//! Commands are processed via `FlowLinkClient` — same API as dashboard/CLI.
//! This means user bots have the same functionality as the system bot.

use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use flowlink_integrations_core::*;
use flowlink_bot_client::FlowLinkClient;

// ═══════════════════════════════════════════════
// Config
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    /// Bot token from @BotFather
    pub bot_token: String,
    /// Admin chat ID for system notifications
    pub admin_chat_id: Option<i64>,
    /// Webhook URL (if using webhook mode)
    pub webhook_url: Option<String>,
    /// Dashboard base URL for links
    pub dashboard_url: Option<String>,
    /// Relay API URL for bot commands
    pub api_url: Option<String>,
    /// JWT token for API authentication
    pub api_token: Option<String>,
}

// ═══════════════════════════════════════════════
// Integration Implementation
// ═══════════════════════════════════════════════

pub struct TelegramIntegration {
    db: Option<Arc<flowlink_db::DbPool>>,
}

impl TelegramIntegration {
    pub fn new(db: Option<Arc<flowlink_db::DbPool>>) -> Self {
        Self { db }
    }

    fn parse_config(config: &serde_json::Value) -> Result<TelegramConfig, IntegrationError> {
        serde_json::from_value(config.clone())
            .map_err(|e| IntegrationError::ConfigError(format!("Invalid Telegram config: {}", e)))
    }

    fn make_api_client(config: &TelegramConfig) -> Option<FlowLinkClient> {
        let url = config.api_url.as_deref().unwrap_or("http://localhost:3000");
        let token = config.api_token.as_deref()?;
        Some(FlowLinkClient::new(url, flowlink_bot_client::AuthMethod::Jwt(token.to_string())))
    }

    fn format_event(event: &IntegrationEvent) -> Option<(String, Vec<Vec<(String, String)>>)> {
        match event {
            IntegrationEvent::AgentConnected { agent_id, hostname } => Some((
                format!("🟡 <b>Сервер подключён</b>\n\n🖥 <b>{}</b>\nID: <code>{}</code>", hostname, agent_id),
                vec![],
            )),
            IntegrationEvent::AgentDisconnected { agent_id, hostname } => Some((
                format!("🔴 <b>Сервер отключён</b>\n\n🖥 <b>{}</b>\nID: <code>{}</code>", hostname, agent_id),
                vec![],
            )),
            IntegrationEvent::ShieldAlert { agent_id, risk, command } => Some((
                format!(
                    "🛡 <b>Security Alert</b>\n\n🖥 <code>{}</code>\n⚠️ Risk: <b>{}</b>\n📝 <code>{}</code>",
                    agent_id, risk, command
                ),
                vec![],
            )),
            IntegrationEvent::ApprovalRequested { approval_id, agent_id, command, risk } => {
                let short_cmd = if command.len() > 60 { format!("{}...", &command[..60]) } else { command.clone() };
                let risk_emoji = match risk.as_str() {
                    "critical" => "🔴", "high" => "🟠", "medium" => "🟡", _ => "🟢",
                };
                Some((
                    format!(
                        "⏳ <b>Требуется подтверждение</b>\n\n\
                         🖥 Агент: <code>{}</code>\n\
                         💻 Команда: <code>{}</code>\n\
                         {} Риск: <b>{}</b>\n\
                         🆔 <code>{}</code>",
                        agent_id, short_cmd, risk_emoji, risk, approval_id
                    ),
                    vec![vec![
                        (format!("✅ Разрешить|approve:{}", approval_id), "approve".into()),
                        (format!("❌ Отклонить|deny:{}", approval_id), "deny".into()),
                    ]],
                ))
            }
            IntegrationEvent::ApprovalResolved { approval_id, decision, resolved_by } => Some((
                format!(
                    "✅ <b>Запрос решён</b>\n\n🆔 <code>{}</code>\n📋 {}\n👤 <code>{}</code>",
                    approval_id, decision, resolved_by
                ),
                vec![],
            )),
            IntegrationEvent::PaymentReceived { account_id, amount_kopecks, description } => {
                let rubles = amount_kopecks / 100;
                Some((
                    format!(
                        "💳 <b>Платёж</b>\n\n💰 {} ₽\n📊 {}\n👤 <code>{}</code>",
                        rubles, description, account_id
                    ),
                    vec![],
                ))
            }
            IntegrationEvent::SubscriptionChanged { account_id, plan } => Some((
                format!("📋 <b>Подписка изменена</b>\n\n👤 <code>{}</code>\n📊 Тариф: <b>{}</b>", account_id, plan),
                vec![],
            )),
            IntegrationEvent::PlanExpiring { account_id, days_left } => Some((
                format!(
                    "⏰ <b>Тариф истекает</b>\n\n👤 <code>{}</code>\n📅 Осталось {} дн.",
                    account_id, days_left
                ),
                vec![],
            )),
            IntegrationEvent::SystemAlert { level, message } => Some((
                format!("🚨 <b>{}: {}</b>", level.to_uppercase(), message),
                vec![],
            )),
            IntegrationEvent::Custom { name, data } => Some((
                format!("📡 <b>{}</b>\n\n{}", name, data),
                vec![],
            )),
        }
    }
}

#[async_trait]
impl Integration for TelegramIntegration {
    fn kind(&self) -> IntegrationKind {
        IntegrationKind("telegram".into())
    }

    fn meta(&self) -> IntegrationMeta {
        builtin_catalog().into_iter()
            .find(|m| m.kind == IntegrationKind("telegram".into()))
            .expect("telegram meta must exist in catalog")
    }

    async fn validate_config(&self, config: &serde_json::Value) -> Result<(), IntegrationError> {
        let tc = Self::parse_config(config)?;
        if tc.bot_token.is_empty() {
            return Err(IntegrationError::ConfigError("bot_token is required".into()));
        }
        if !tc.bot_token.contains(':') {
            return Err(IntegrationError::ConfigError("Invalid bot token format".into()));
        }
        Ok(())
    }

    async fn start(
        &self,
        id: IntegrationId,
        config: IntegrationConfig,
        mut event_rx: tokio::sync::broadcast::Receiver<IntegrationEvent>,
    ) -> Result<(), IntegrationError> {
        let tc = Self::parse_config(&config.config)?;
        let db = self.db.clone();
        let api_client = Self::make_api_client(&tc);

        log::info!("🤖 Telegram integration starting for {} (account {})", id, config.account_id);

        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let api_url = format!("https://api.telegram.org/bot{}", tc.bot_token);
            let mut base_chat_ids: Vec<i64> = Vec::new();
            if let Some(admin_id) = tc.admin_chat_id {
                base_chat_ids.push(admin_id);
            }

            loop {
                let event = match event_rx.recv().await {
                    Ok(e) => e,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("Telegram integration {} lagged {} events", id, n);
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        log::info!("Telegram integration {} event channel closed", id);
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

                let (text, _buttons) = match Self::format_event(&event) {
                    Some(t) => t,
                    None => continue,
                };

                let mut chat_ids = base_chat_ids.iter().copied().collect::<Vec<_>>();
                if let Some(ref db_pool) = db {
                    if let Ok(ids) = sqlx::query_scalar::<_, i64>(
                        "SELECT tg_id FROM accounts WHERE tg_id IS NOT NULL AND account_id = $1"
                    )
                    .bind(&config.account_id)
                    .fetch_all(db_pool.pool())
                    .await
                    {
                        chat_ids.extend(ids);
                    }
                }
                chat_ids.sort_unstable();
                chat_ids.dedup();

                for chat_id in &chat_ids {
                    let body = serde_json::json!({
                        "chat_id": chat_id,
                        "text": text,
                        "parse_mode": "HTML"
                    });
                    if let Err(e) = client
                        .post(format!("{}/sendMessage", &api_url))
                        .json(&body)
                        .send()
                        .await
                    {
                        log::warn!("Telegram {}: failed to send to {}: {}", id, chat_id, e);
                    }
                }
                log::debug!("📨 Telegram {}: sent {} to {} chats", id, event_type, chat_ids.len());
            }

            log::info!("🤖 Telegram integration {} stopped", id);
        });

        Ok(())
    }

    async fn stop(&self, id: &IntegrationId) -> Result<(), IntegrationError> {
        log::info!("🛑 Telegram integration stopped: {}", id);
        Ok(())
    }

    async fn handle_event(
        &self,
        event: &IntegrationEvent,
        _config: &IntegrationConfig,
    ) -> Vec<IntegrationAction> {
        match Self::format_event(event) {
            Some((text, buttons)) => vec![IntegrationAction::SendMessage {
                chat_id: None,
                text,
                buttons,
            }],
            None => vec![IntegrationAction::Noop],
        }
    }

    /// Handle bot commands via FlowLinkClient API
    async fn handle_command(
        &self,
        command: &str,
        args: &serde_json::Value,
        config: &IntegrationConfig,
    ) -> Result<serde_json::Value, IntegrationError> {
        let tc = Self::parse_config(&config.config)?;
        let api = Self::make_api_client(&tc)
            .ok_or_else(|| IntegrationError::AuthError("No API token configured".into()))?;

        let chat_id = args.get("chat_id").and_then(|v| v.as_i64()).unwrap_or(0);

        let response_text = match command {
            "/start" => {
                // Link account if code provided
                if let Some(code) = args.get("code").and_then(|v| v.as_str()) {
                    format!("🔗 Подключение аккаунта... Код: {}", code)
                } else {
                    "👋 Привет! Я — ваш FlowLink бот.\n\n\
                     📊 /status — статус серверов\n\
                     🖥 /servers — список агентов\n\
                     💳 /billing — подписка\n\
                     🛡 /shield — безопасность\n\
                     ✅ /approvals — подтверждения\n\
                     📋 /plans — тарифы\n\
                     📊 /usage — статистика\n\
                     ⚙️ /config — конфигурация\n\
                     📝 /logs — аудит\n\
                     🆘 /help — справка".to_string()
                }
            }

            "/help" => {
                "🤖 <b>FlowLink Bot Commands</b>\n\n\
                 📊 /status — статус системы\n\
                 🖥 /servers — список агентов\n\
                 💳 /billing — подписка и баланс\n\
                 📦 /myplan — текущий тариф\n\
                 📋 /plans — доступные тарифы\n\
                 📊 /usage — статистика использования\n\
                 🧾 /invoices — история платежей\n\
                 🛡 /shield — оповещения безопасности\n\
                 ✅ /approvals — ожидающие подтверждения\n\
                 📱 /devices — устройства\n\
                 ⚙️ /config — конфигурация\n\
                 🔄 /reload — перезагрузить конфиг\n\
                 📝 /logs [N] — последние действия\n\
                 💾 /backups — бэкапы\n\
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
                        let offline: Vec<_> = agents.iter().filter(|a| !a.online).collect();
                        let mut text = format!("🖥 <b>Агенты</b> ({} онлайн / {} всего)\n\n", online.len(), agents.len());
                        for a in online.iter().take(15) {
                            text.push_str(&format!("🟢 <b>{}</b> — {} [{}]\n", a.hostname, a.agent_id, a.os));
                        }
                        for a in offline.iter().take(5) {
                            text.push_str(&format!("🔴 {} — {}\n", a.hostname, a.agent_id));
                        }
                        if online.len() > 15 { text.push_str(&format!("\n... и ещё {} онлайн\n", online.len() - 15)); }
                        if offline.len() > 5 { text.push_str(&format!("... и ещё {} офлайн\n", offline.len() - 5)); }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/plans" => {
                match api.get_plans().await {
                    Ok(plans) => {
                        let mut text = "📋 <b>Тарифные планы</b>\n\n".to_string();
                        for p in plans.iter() {
                            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                            let price = p.get("price_kopecks").and_then(|v| v.as_i64()).unwrap_or(0);
                            let desc = p.get("description").and_then(|v| v.as_str()).unwrap_or("");
                            text.push_str(&format!("📦 <b>{}</b> — {} ₽/мес\n   {}\n\n", name, price / 100, desc));
                        }
                        text.push_str("💡 /subscribe <plan_id> — подписаться");
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
                        format!(
                            "💳 <b>Подписка</b>\n\n\
                             📦 Тариф: <b>{}</b>\n\
                             📊 Статус: {}\n\
                             💰 Баланс: {} ₽",
                            plan, status, balance / 100
                        )
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/subscribe" => {
                let plan_id = args.get("plan_id").and_then(|v| v.as_str()).unwrap_or("");
                if plan_id.is_empty() {
                    "💡 Использование: /subscribe <plan_id>\n\n📋 /plans — список тарифов".to_string()
                } else {
                    match api.subscribe(plan_id).await {
                        Ok(_) => format!("✅ Подписка на план <b>{}</b> оформлена!", plan_id),
                        Err(e) => format!("❌ Ошибка: {}", e),
                    }
                }
            }

            "/invoices" => {
                match api.get_invoices().await {
                    Ok(invoices) => {
                        let mut text = format!("🧾 <b>Платежи</b> ({} записей)\n\n", invoices.len());
                        for inv in invoices.iter().take(10) {
                            let amount = inv.get("amount_kopecks").and_then(|v| v.as_i64()).unwrap_or(0);
                            let status = inv.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                            let date = inv.get("created_at").and_then(|v| v.as_str()).unwrap_or("?");
                            let emoji = match status { "paid" => "✅", "pending" => "⏳", "failed" => "❌", _ => "📋" };
                            text.push_str(&format!("{} {} ₽ — {} ({})\n", emoji, amount / 100, status, &date[..10.min(date.len())]));
                        }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/usage" => {
                match api.get_usage().await {
                    Ok(usage) => {
                        let mut text = "📊 <b>Использование</b>\n\n".to_string();
                        if let Some(obj) = usage.as_object() {
                            for (k, v) in obj.iter().take(10) {
                                text.push_str(&format!("• {}: <b>{}</b>\n", k, v));
                            }
                        } else {
                            text.push_str(&format!("{}", usage));
                        }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/shield" => {
                match api.get_alerts().await {
                    Ok(alerts) => {
                        let unresolved: Vec<_> = alerts.iter().filter(|a| !a.resolved).collect();
                        let mut text = format!("🛡 <b>Shield Alerts</b> ({} активных / {} всего)\n\n", unresolved.len(), alerts.len());
                        for a in unresolved.iter().take(10) {
                            let emoji = match a.risk.as_str() { "critical" => "🔴", "high" => "🟠", "medium" => "🟡", _ => "🟢" };
                            let short_cmd = if a.command.len() > 40 { format!("{}...", &a.command[..40]) } else { a.command.clone() };
                            text.push_str(&format!("{} <b>{}</b> {}\n   🖥 {} | 🕐 {}\n", emoji, a.risk, short_cmd, a.agent_id, &a.timestamp[..16.min(a.timestamp.len())]));
                        }
                        if unresolved.is_empty() { text.push_str("✅ Нет активных алертов"); }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/approvals" => {
                match api.get_approvals().await {
                    Ok(approvals) => {
                        let mut text = format!("✅ <b>Ожидающие подтверждения</b> ({})\n\n", approvals.len());
                        for a in approvals.iter().take(10) {
                            let emoji = match a.risk.as_str() { "critical" => "🔴", "high" => "🟠", "medium" => "🟡", _ => "🟢" };
                            let short_cmd = if a.command.len() > 40 { format!("{}...", &a.command[..40]) } else { a.command.clone() };
                            text.push_str(&format!(
                                "{} <b>{}</b> {}\n   🖥 {} | 🆔 <code>{}</code>\n\n",
                                emoji, a.risk, short_cmd, a.agent_id, a.id
                            ));
                        }
                        if approvals.is_empty() { text.push_str("✅ Нет ожидающих подтверждений"); }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/devices" => {
                match api.get_devices().await {
                    Ok(devices) => {
                        let mut text = format!("📱 <b>Устройства</b> ({})\n\n", devices.len());
                        for d in devices.iter().take(10) {
                            let name = d.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                            let confirmed = d.get("confirmed").and_then(|v| v.as_bool()).unwrap_or(false);
                            let emoji = if confirmed { "✅" } else { "⏳" };
                            text.push_str(&format!("{} {}\n", emoji, name));
                        }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/config" => {
                match api.get_config().await {
                    Ok(config) => format!("⚙️ <b>Конфигурация</b>\n\n<pre>{}</pre>", serde_json::to_string_pretty(&config).unwrap_or_default()),
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/reload" => {
                match api.reload_config().await {
                    Ok(_) => "🔄 Конфигурация перезагружена ✅".to_string(),
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
                            let time = e.get("timestamp").and_then(|v| v.as_str()).unwrap_or("?");
                            let t = if time.len() > 16 { &time[..16] } else { time };
                            text.push_str(&format!("• {} <b>{}</b> — {}\n", t, action, agent));
                        }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/backups" => {
                match api.get_backups().await {
                    Ok(backups) => {
                        let mut text = format!("💾 <b>Бэкапы</b> ({})\n\n", backups.len());
                        for b in backups.iter().take(10) {
                            let name = b.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                            let size = b.get("size_bytes").and_then(|v| v.as_i64()).unwrap_or(0);
                            let time = b.get("created_at").and_then(|v| v.as_str()).unwrap_or("?");
                            text.push_str(&format!("📦 {} ({} KB) — {}\n", name, size / 1024, &time[..10.min(time.len())]));
                        }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/emergency" => {
                "⚠️ <b>EMERGENCY STOP</b>\n\n\
                 Эта команда отключит ВСЕ агенты!\n\n\
                 Для подтверждения используйте inline-кнопку.".to_string()
            }

            "/policy" => {
                match api.get_config().await {
                    Ok(config) => {
                        let default = serde_json::json!({});
                        let policies = config.get("shield").unwrap_or(&default);
                        format!("🛡 <b>Политики безопасности</b>\n\n<pre>{}</pre>",
                            serde_json::to_string_pretty(policies).unwrap_or_default())
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            "/settings" => {
                match api.list_integrations().await {
                    Ok(integrations) => {
                        let mut text = format!("⚙️ <b>Настройки интеграций</b> ({})\n\n", integrations.len());
                        for i in integrations.iter().take(10) {
                            let kind = i.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
                            let status = i.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                            let emoji = match status { "active" => "✅", "paused" => "⏸", _ => "❌" };
                            text.push_str(&format!("{} {} ({})\n", emoji, kind, status));
                        }
                        text
                    }
                    Err(e) => format!("❌ Ошибка: {}", e),
                }
            }

            _ => format!("❓ Неизвестная команда: {}\n\n/help — список команд", command),
        };

        Ok(serde_json::json!({ "text": response_text, "chat_id": chat_id }))
    }

    async fn health_check(&self, _id: &IntegrationId) -> Result<bool, IntegrationError> {
        Ok(true)
    }
}

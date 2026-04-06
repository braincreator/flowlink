// FlowLink Shield — Notifier
// Sends alerts via webhook (relay → Telegram bot)

use serde::Serialize;
use log::{info, warn};

#[derive(Debug, Serialize)]
struct AlertPayload {
    event: String,
    pid: u32,
    uid: u32,
    username: String,
    command: String,
    rule_name: String,
    action: String,
    snapshot: Option<String>,
    timestamp: String,
}

pub struct Notifier {
    webhook_url: Option<String>,
    client: reqwest::Client,
}

impl Notifier {
    pub fn new(webhook_url: Option<String>) -> Self {
        Self {
            webhook_url,
            client: reqwest::Client::new(),
        }
    }

    /// Send alert to webhook (FlowLink relay → Telegram)
    pub async fn alert(
        &self,
        pid: u32,
        uid: u32,
        username: &str,
        command: &str,
        rule_name: &str,
        action: &str,
        snapshot: Option<&str>,
    ) {
        let url = match &self.webhook_url {
            Some(u) => u,
            None => {
                warn!("No webhook URL configured — alert not sent");
                return;
            }
        };

        let payload = AlertPayload {
            event: "shield_alert".to_string(),
            pid,
            uid,
            username: username.to_string(),
            command: command.to_string(),
            rule_name: rule_name.to_string(),
            action: action.to_string(),
            snapshot: snapshot.map(|s| s.to_string()),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        match self.client.post(url).json(&payload).send().await {
            Ok(resp) => info!("Alert sent (status: {})", resp.status()),
            Err(e) => warn!("Failed to send alert: {}", e),
        }
    }

    /// Format Telegram message for shield alert
    pub fn format_telegram_alert(
        username: &str,
        command: &str,
        rule_name: &str,
        snapshot: Option<&str>,
    ) -> String {
        let snap_text = match snapshot {
            Some(s) => format!("📸 Снапшот: <code>{}</code>", s),
            None => "📸 Снапшот: недоступен".to_string(),
        };

        format!(
            "🚨 <b>FlowLink Shield Alert</b>\n\n\
             👤 Пользователь: <code>{}</code>\n\
             ⌨️ Команда: <code>{}</code>\n\
             📋 Правило: <code>{}</code>\n\
             {}\n\n\
             ⏸ Процесс заморожен (SIGSTOP). Ожидание подтверждения...",
            username, command, rule_name, snap_text
        )
    }
}

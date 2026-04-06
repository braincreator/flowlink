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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_telegram_alert_with_snapshot() {
        let msg = Notifier::format_telegram_alert(
            "alice", "rm -rf /", "rm_rf", Some("tank/data@shield-rm_rf-20260406"),
        );
        assert!(msg.contains("alice"));
        assert!(msg.contains("rm -rf /"));
        assert!(msg.contains("rm_rf"));
        assert!(msg.contains("tank/data@shield-rm_rf-20260406"));
        assert!(msg.contains("FlowLink Shield Alert"));
        assert!(msg.contains("SIGSTOP"));
    }

    #[test]
    fn format_telegram_alert_no_snapshot() {
        let msg = Notifier::format_telegram_alert("root", "mkfs /dev/sda", "format_disk", None);
        assert!(msg.contains("недоступен"));
        assert!(msg.contains("root"));
    }

    #[test]
    fn format_telegram_alert_html_formatting() {
        let msg = Notifier::format_telegram_alert("bob", "echo hi", "test", None);
        assert!(msg.contains("<b>"));
        assert!(msg.contains("<code>"));
    }

    #[test]
    fn notifier_new_with_url() {
        let n = Notifier::new(Some("https://example.com/webhook".into()));
        assert!(n.webhook_url.is_some());
    }

    #[test]
    fn notifier_new_no_url() {
        let n = Notifier::new(None);
        assert!(n.webhook_url.is_none());
    }

    #[tokio::test]
    async fn alert_no_url_does_not_panic() {
        let n = Notifier::new(None);
        n.alert(1234, 1000, "root", "ls", "safe", "allowed", None).await;
    }

    #[test]
    fn alert_payload_serialization() {
        let payload = AlertPayload {
            event: "shield_alert".into(),
            pid: 1234, uid: 1000,
            username: "alice".into(),
            command: "rm -rf /".into(),
            rule_name: "rm_rf".into(),
            action: "intercepted".into(),
            snapshot: Some("snap1".into()),
            timestamp: "2026-04-06T12:00:00Z".into(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("alice"));
        assert!(json.contains("rm -rf /"));
        assert!(json.contains("snap1"));
    }

    #[test]
    fn alert_payload_without_snapshot() {
        let payload = AlertPayload {
            event: "shield_alert".into(),
            pid: 1, uid: 0,
            username: "root".into(),
            command: "shutdown".into(),
            rule_name: "shutdown".into(),
            action: "blocked".into(),
            snapshot: None,
            timestamp: "2026-04-06T12:00:00Z".into(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("null")); // snapshot is null
    }
}

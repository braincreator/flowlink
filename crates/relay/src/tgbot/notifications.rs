//! Proactive Telegram notifications for relay events.

use crate::server::AppState;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::ParseMode;

/// Send notification to all linked Telegram accounts
pub async fn notify(bot: &Bot, state: &Arc<AppState>, message: &str) {
    if let Some(db) = &state.db {
        match sqlx::query_scalar::<_, i64>(
            "SELECT tg_chat_id FROM accounts WHERE tg_chat_id IS NOT NULL"
        )
        .fetch_all(db.pool())
        .await
        {
            Ok(chat_ids) => {
                for chat_id in chat_ids {
                    let chat = teloxide::types::ChatId(chat_id);
                    if let Err(e) = bot.send_message(chat, message).parse_mode(ParseMode::Markdown).await {
                        log::warn!("Failed to send TG notification to {}: {}", chat_id, e);
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to query TG chat IDs: {}", e);
            }
        }
    }
}

/// Notify about agent connection
pub async fn agent_connected(bot: &Bot, state: &Arc<AppState>, agent_id: &str, hostname: &str) {
    notify(bot, state, &format!("🟡 *Сервер подключён*\n\n🖥 *{}*\nID: `{}`", hostname, agent_id)).await;
}

/// Notify about agent disconnection
pub async fn agent_disconnected(bot: &Bot, state: &Arc<AppState>, agent_id: &str, hostname: &str) {
    notify(bot, state, &format!("🔴 *Сервер отключён*\n\n🖥 *{}*\nID: `{}`", hostname, agent_id)).await;
}

/// Notify about security alert
pub async fn shield_alert(bot: &Bot, state: &Arc<AppState>, agent_id: &str, risk: &str, command: &str) {
    notify(bot, state, &format!(
        "🛡 *Security Alert*\n\n🖥 `{}`\n⚠️ Risk: *{}*\n📝 `{}`",
        agent_id, risk, command
    )).await;
}

/// Notify about payment event
pub async fn payment_event(bot: &Bot, state: &Arc<AppState>, account_id: &str, event: &str, amount: &str) {
    notify(bot, state, &format!(
        "💳 *Платёж*\n\n📊 {}\n💰 {}\n👤 `{}`",
        event, amount, account_id
    )).await;
}

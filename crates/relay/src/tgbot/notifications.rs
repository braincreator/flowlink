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
                    if let Err(e) = bot.send_message(chat, message).parse_mode(ParseMode::Html).await {
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
    notify(bot, state, &format!("🟡 <b>Сервер подключён</b>\n\n🖥 <b>{}</b>\nID: <code>{}</code>", hostname, agent_id)).await;
}

/// Notify about agent disconnection
pub async fn agent_disconnected(bot: &Bot, state: &Arc<AppState>, agent_id: &str, hostname: &str) {
    notify(bot, state, &format!("🔴 <b>Сервер отключён</b>\n\n🖥 <b>{}</b>\nID: <code>{}</code>", hostname, agent_id)).await;
}

/// Notify about security alert
pub async fn shield_alert(bot: &Bot, state: &Arc<AppState>, agent_id: &str, risk: &str, command: &str) {
    notify(bot, state, &format!(
        "🛡 <b>Security Alert</b>\n\n🖥 <code>{}</code>\n⚠️ Risk: <b>{}</b>\n📝 <code>{}</code>",
        agent_id, risk, command
    )).await;
}

/// Notify about payment event
pub async fn payment_event(bot: &Bot, state: &Arc<AppState>, account_id: &str, event: &str, amount: &str) {
    notify(bot, state, &format!(
        "💳 <b>Платёж</b>\n\n📊 {}\n💰 {}\n👤 <code>{}</code>",
        event, amount, account_id
    )).await;
}

//! Proactive Telegram notifications for relay events.

use crate::server::AppState;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::ParseMode;

/// Send notification to all linked Telegram accounts
pub async fn notify(bot: &Bot, state: &Arc<AppState>, message: &str) {
    if let Some(db) = &state.db {
        match sqlx::query_scalar::<_, i64>(
            "SELECT tg_id FROM accounts WHERE tg_id IS NOT NULL"
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

/// Notify about pending approval with inline buttons
pub async fn approval_request(bot: &Bot, state: &Arc<AppState>, approval_id: &str, agent_id: &str, command: &str, risk: &str) {
    let risk_emoji = match risk {
        "critical" => "🔴", "high" => "🟠", "medium" => "🟡", _ => "🟢",
    };
    let short_cmd = if command.len() > 60 { format!("{}...", &command[..60]) } else { command.to_string() };

    let text = format!(
        "⏳ <b>Требуется подтверждение</b>\n\n\
         🖥 Агент: <code>{}</code>\n\
         💻 Команда: <code>{}</code>\n\
         {} Риск: <b>{}</b>\n\
         🆔 <code>{}</code>",
        agent_id, short_cmd, risk_emoji, risk, approval_id
    );

    let kb = teloxide::types::InlineKeyboardMarkup::new(vec![
        vec![
            teloxide::types::InlineKeyboardButton::callback("✅ Разрешить", format!("approve:{}", approval_id)),
            teloxide::types::InlineKeyboardButton::callback("❌ Отклонить", format!("deny:{}", approval_id)),
        ],
    ]);

    if let Some(db) = &state.db {
        match sqlx::query_scalar::<_, i64>(
            "SELECT tg_id FROM accounts WHERE tg_id IS NOT NULL"
        )
        .fetch_all(db.pool())
        .await
        {
            Ok(chat_ids) => {
                for chat_id in chat_ids {
                    let chat = teloxide::types::ChatId(chat_id);
                    if let Err(e) = bot.send_message(chat, &text)
                        .parse_mode(ParseMode::Html)
                        .reply_markup(kb.clone())
                        .await
                    {
                        log::warn!("Failed to send TG approval to {}: {}", chat_id, e);
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to query TG chat IDs: {}", e);
            }
        }
    }
}

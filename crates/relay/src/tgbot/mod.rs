//! Telegram bot module for FlowLink relay.
//!
//! Integrated Telegram bot that runs alongside the relay server.

#[cfg(feature = "tgbot")]
pub mod commands;
#[cfg(feature = "tgbot")]
pub mod bot;
#[cfg(feature = "tgbot")]
pub mod notifications;

#[cfg(feature = "tgbot")]
pub use bot::start_tgbot;

/// Process a single Telegram update (used by webhook handler).
#[cfg(feature = "tgbot")]
pub async fn process_update(
    bot: teloxide::Bot,
    update: teloxide::types::Update,
    ctx: commands::BotContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use teloxide::utils::command::BotCommands;
    // Extract message from update
    let msg = match &update {
        teloxide::types::Update {
            kind: teloxide::types::UpdateKind::Message(m),
            ..
        } => Some(m.clone()),
        _ => None,
    };

    // Try as command first
    if let Some(ref msg) = msg {
        if let Some(text) = msg.text() {
            if let Ok(cmd) = bot::Command::parse(text, "flowlinkbot") {
                if let Err(e) = bot::handle_command(bot, msg.clone(), cmd, ctx).await {
                    log::error!("tg_webhook: command error: {}", e);
                }
                return Ok(());
            }
        }
    }

    // Try as callback query
    match &update {
        teloxide::types::Update {
            kind: teloxide::types::UpdateKind::CallbackQuery(q),
            ..
        } => {
            if let Err(e) = bot::handle_callback(bot, q.clone(), ctx).await {
                log::error!("tg_webhook: callback error: {}", e);
            }
        }
        _ => {}
    }

    Ok(())
}

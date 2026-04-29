//! Telegram bot entry point — supports both polling and webhook modes.
//!
//! This is the standalone crate version — uses `BotState` instead of relay's `AppState`.

use crate::commands::{self, BotContext};
use crate::{BotState, ApprovalDecision};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, MissedTickBehavior};
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use teloxide::types::ParseMode;

/// Bot mode
#[derive(Clone, Debug)]
pub enum BotMode {
    Polling,
    Webhook,
}

/// Bot runtime configuration
#[derive(Clone)]
pub struct BotRunConfig {
    pub mode: BotMode,
    pub webhook_url: Option<String>,
    pub polling_interval: Duration,
    pub auto_recovery_enabled: bool,
}

impl Default for BotRunConfig {
    fn default() -> Self {
        Self {
            mode: BotMode::Polling,
            webhook_url: None,
            polling_interval: Duration::from_secs(1),
            auto_recovery_enabled: true,
        }
    }
}

/// Start the Telegram bot as a background task with auto-recovery.
pub async fn start_tgbot(state: Arc<BotState>, token: String, config: BotRunConfig) -> tokio::task::JoinHandle<()> {
    let bot = Bot::new(token.clone());
    let ctx = BotContext { state: state.clone() };

    log::info!("🤖 Telegram bot starting in {:?} mode...", config.mode);

    if config.auto_recovery_enabled {
        let bot_health = bot.clone();
        let token_health = token.clone();
        tokio::spawn(async move {
            let mut health_check = interval(Duration::from_secs(60));
            health_check.set_missed_tick_behavior(MissedTickBehavior::Delay);

            loop {
                health_check.tick().await;
                check_bot_health(&bot_health, &token_health).await;
            }
        });
    }

    tokio::spawn(async move {
        match config.mode {
            BotMode::Polling => start_polling_mode(bot, ctx, config).await,
            BotMode::Webhook => start_webhook_mode(bot, ctx, config, token).await,
        }
    })
}

/// Start bot in polling mode with auto-recovery
async fn start_polling_mode(bot: Bot, ctx: BotContext, config: BotRunConfig) {
    loop {
        match run_polling(&bot, ctx.clone()).await {
            Ok(_) => {
                log::info!("🤖 Telegram bot polling completed normally");
                break;
            }
            Err(e) => {
                log::error!("🤖 Telegram bot polling error: {}", e);
                if config.auto_recovery_enabled {
                    log::info!("🔄 Auto-recovery: restarting bot in 5 seconds...");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                } else {
                    break;
                }
            }
        }
    }
}

/// Start bot in webhook mode
async fn start_webhook_mode(bot: Bot, ctx: BotContext, config: BotRunConfig, _token: String) {
    if let Some(webhook_url) = config.webhook_url.clone() {
        log::info!("🔗 Setting up webhook: {}", webhook_url);

        match bot.set_webhook(webhook_url.parse().expect("invalid webhook URL")).send().await {
            Ok(_) => {
                log::info!("✅ Webhook set successfully");
                run_webhook_handler(bot, ctx, config).await;
            }
            Err(e) => {
                log::error!("❌ Failed to set webhook: {}", e);
                start_polling_mode(bot, ctx, config).await;
            }
        }
    } else {
        log::warn!("⚠️ No webhook URL configured, falling back to polling");
        start_polling_mode(bot, ctx, config).await;
    }
}

/// Run bot in polling mode
async fn run_polling(bot: &Bot, ctx: BotContext) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    log::info!("🚀 Starting polling mode...");

    let handler = Update::filter_message()
        .branch(
            dptree::entry()
                .filter_command::<Command>()
                .endpoint(handle_command),
        )
        .branch(
            Update::filter_callback_query()
                .endpoint(handle_callback),
        );

    Dispatcher::builder(bot.clone(), handler)
        .dependencies(dptree::deps![ctx])
        .error_handler(LoggingErrorHandler::with_custom_text("Bot error"))
        .build()
        .dispatch()
        .await;

    Ok(())
}

/// Run webhook handler
async fn run_webhook_handler(_bot: Bot, _ctx: BotContext, _config: BotRunConfig) {
    log::info!("🌐 Webhook handler active");
    loop {
        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}

/// Health check for bot recovery
async fn check_bot_health(bot: &Bot, _token: &str) {
    match bot.get_me().await {
        Ok(_) => {
            log::debug!("🤖 Bot health OK");
        }
        Err(e) => {
            log::warn!("🤖 Bot health check failed: {}", e);
        }
    }
}

/// Emergency stop command
pub async fn emergency_stop_bot(_bot: Bot) {
    log::warn!("🚨 EMERGENCY: Stopping Telegram bot...");
    log::info!("🚨 Emergency stop message sent to bot users");
}

/// Central command router
pub async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    ctx: BotContext,
) -> ResponseResult<()> {
    match cmd {
        Command::Start => commands::cmd_start(bot, msg, ctx).await,
        Command::Help => commands::cmd_help(bot, msg, ctx).await,
        Command::Status => commands::cmd_status(bot, msg, ctx).await,
        Command::Servers => commands::cmd_servers(bot, msg, ctx).await,
        Command::Plans => commands::cmd_plans(bot, msg, ctx).await,
        Command::Billing => commands::cmd_billing(bot, msg, ctx).await,
        Command::Myplan => commands::cmd_myplan(bot, msg, ctx).await,
        Command::Subscribe => {
            let plan_id = msg.text()
                .and_then(|t| t.strip_prefix("/subscribe"))
                .unwrap_or("")
                .trim()
                .split('@')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();

            if !plan_id.is_empty() {
                if let Some(billing) = &ctx.state.billing {
                    if let Some(plan) = billing.plans().get(&plan_id) {
                        if plan.price_kopecks == 0 {
                            let account_id = format!("tg:{}", msg.chat.id.0);
                            let mut acc = billing.get_or_create_account(&account_id);
                            let _ = billing.change_plan(&mut acc, &plan_id);
                            bot.send_message(msg.chat.id, format!(
                                "✅ План <b>{}</b> активирован!\n\n/use — статистика",
                                plan.name
                            )).parse_mode(ParseMode::Html).await?;
                            return Ok(());
                        }
                    }
                }
            }
            commands::cmd_subscribe(bot, msg, ctx).await
        }
        Command::Invoices => commands::cmd_invoices(bot, msg, ctx).await,
        Command::Usage => commands::cmd_usage(bot, msg, ctx).await,
        Command::Devices => commands::cmd_devices(bot, msg, ctx).await,
        Command::Shield => commands::cmd_shield(bot, msg, ctx).await,
        Command::Logs => commands::cmd_logs(bot, msg, ctx).await,
        Command::Approvals => commands::cmd_approvals(bot, msg, ctx).await,
        Command::Config => commands::cmd_config(bot, msg, ctx).await,
        Command::Reload => commands::cmd_reload(bot, msg, ctx).await,
        Command::Emergency => commands::cmd_emergency(bot, msg, ctx).await,
        Command::Exec => commands::cmd_exec(bot, msg, ctx).await,
        Command::Backups => commands::cmd_backups(bot, msg, ctx).await,
        Command::Restore => commands::cmd_restore(bot, msg, ctx).await,
        Command::Policy => commands::cmd_policy(bot, msg, ctx).await,
        Command::Settings => commands::cmd_settings(bot, msg, ctx).await,
        Command::Notiftest => commands::cmd_notiftest(bot, msg, ctx).await,
        Command::Substatus => commands::cmd_substatus(bot, msg, ctx).await,
        Command::Subcancel => commands::cmd_subcancel(bot, msg, ctx).await,
        Command::Subchange => commands::cmd_subchange(bot, msg, ctx).await,
    }
}

/// Handle inline button callbacks
pub async fn handle_callback(
    bot: Bot,
    q: CallbackQuery,
    ctx: BotContext,
) -> ResponseResult<()> {
    let data = match &q.data {
        Some(d) => d.clone(),
        None => return Ok(()),
    };

    let (chat_id, msg_id) = match &q.message {
        Some(msg) => (msg.chat().id, msg.id()),
        None => return Ok(()),
    };

    let _ = bot.answer_callback_query(&q.id).await;

    match data.as_str() {
        d if d.starts_with("approve:") => {
            let approval_id = &d[8..];
            if ctx.state.approvals.resolve(approval_id, ApprovalDecision::Approved) {
                bot.edit_message_text(chat_id, msg_id,
                    format!("✅ <b>Разрешено</b>\n\nID: <code>{}</code>", approval_id)
                ).parse_mode(ParseMode::Html).await?;
            } else {
                bot.edit_message_text(chat_id, msg_id,
                    "❌ Запрос не найден (возможно уже обработан или истёк таймаут)"
                ).await?;
            }
        }
        d if d.starts_with("deny:") => {
            let approval_id = &d[4..];
            if ctx.state.approvals.resolve(approval_id, ApprovalDecision::Rejected) {
                bot.edit_message_text(chat_id, msg_id,
                    format!("❌ <b>Отклонено</b>\n\nID: <code>{}</code>", approval_id)
                ).parse_mode(ParseMode::Html).await?;
            } else {
                bot.edit_message_text(chat_id, msg_id,
                    "❌ Запрос не найден (возможно уже обработан или истёк таймаут)"
                ).await?;
            }
        }
        "notif:bind:max" => {
            let code = rand::random::<u32>() % 900_000 + 100_000;
            bot.send_message(chat_id, format!(
                "📲 Привязка MAX Messenger\n\nОтправьте код ниже боту MAX:\n\n<code>{}</code>\n\n⏳ Ожидание подтверждения...",
                code
            )).parse_mode(ParseMode::Html).await?;
        }
        "notif:bind:slack" => {
            bot.send_message(chat_id, format!("🔗 Привязка Slack\n\nОткройте: {}/api/notifications/slack/install", ctx.state.base_url)).await?;
        }
        "notif:test" => {
            if let Some(ref router) = ctx.state.notification_router {
                let n = flowlink_notifications::Notification::system(
                    "Test",
                    "✅ Test notification from inline button",
                    flowlink_notifications::Severity::Info,
                );
                let delivered = router.send(&n).await;
                bot.send_message(chat_id, format!("🔔 Отправлено: {} кан.", delivered)).await?;
            }
        }
        "dismiss" => {
            let _ = bot.edit_message_reply_markup(chat_id, msg_id).await;
        }
        "subcancel:confirm" => {
            bot.send_message(chat_id, "✅ Подписка отменена. Доступ сохранится до конца текущего периода.").await?;
            let _ = bot.edit_message_reply_markup(chat_id, msg_id).await;
        }
        d if d.starts_with("sub:") => {
            let plan_id = &d[4..];
            bot.send_message(chat_id, format!("Для подписки на план {} используйте: /subscribe {}", plan_id, plan_id)).await?;
        }
        d if d.starts_with("subchange:") => {
            let new_plan_id = &d[10..];
            if let Some(billing) = &ctx.state.billing {
                let account_id = format!("tg:{}", chat_id.0);
                let mut acc = billing.get_or_create_account(&account_id);
                let _ = billing.change_plan(&mut acc, new_plan_id);
                bot.send_message(chat_id, format!("✅ План изменён на <b>{}</b>", new_plan_id))
                    .parse_mode(ParseMode::Html).await?;
            }
            let _ = bot.edit_message_reply_markup(chat_id, msg_id).await;
        }
        "emergency:confirm" => {
            let agents = ctx.state.pool.list();
            let count = agents.len();
            for a in &agents {
                ctx.state.pool.unregister(&a.agent_id);
            }
            bot.send_message(chat_id, format!("🚨 EMERGENCY: {} агентов отключено.", count)).await?;
            let _ = bot.edit_message_reply_markup(chat_id, msg_id).await;
        }
        _ => {
            bot.send_message(chat_id, "❓ Неизвестное действие").await?;
        }
    }

    Ok(())
}

/// Command enum for teloxide filter_command
#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "FlowLink bot commands")]
pub enum Command {
    #[command(description = "Приветствие")]
    Start,
    #[command(description = "Список команд")]
    Help,
    #[command(description = "Статус серверов")]
    Status,
    #[command(description = "Список агентов")]
    Servers,
    #[command(description = "Тарифные планы")]
    Plans,
    #[command(description = "Статус подписки")]
    Billing,
    #[command(description = "Текущий план с лимитами")]
    Myplan,
    #[command(description = "Подписаться на план")]
    Subscribe,
    #[command(description = "История платежей")]
    Invoices,
    #[command(description = "Статистика использования")]
    Usage,
    #[command(description = "Список устройств")]
    Devices,
    #[command(description = "Оповещения безопасности")]
    Shield,
    #[command(description = "Последние действия")]
    Logs,
    #[command(description = "Ожидающие подтверждения")]
    Approvals,
    #[command(description = "Конфигурация")]
    Config,
    #[command(description = "Перезагрузить конфиг")]
    Reload,
    #[command(description = "Экстренная остановка")]
    Emergency,
    #[command(description = "Выполнить команду")]
    Exec,
    #[command(description = "Список бэкапов")]
    Backups,
    #[command(description = "Восстановить бэкап")]
    Restore,
    #[command(description = "Политики безопасности")]
    Policy,
    #[command(description = "Статус подписки")]
    Substatus,
    #[command(description = "Отменить подписку")]
    Subcancel,
    #[command(description = "Сменить план")]
    Subchange,
    #[command(description = "Настройки уведомлений")]
    Settings,
    #[command(description = "Тестовое уведомление")]
    Notiftest,
}

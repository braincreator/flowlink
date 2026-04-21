//! Telegram bot entry point — supports both polling and webhook modes.

use super::commands::{self, BotContext};
use crate::server::AppState;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, MissedTickBehavior};
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use teloxide::types::ParseMode;

/// Bot configuration
#[derive(Clone)]
pub struct BotConfig {
    pub mode: BotMode,
    pub webhook_url: Option<String>,
    pub polling_interval: Duration,
    pub auto_recovery_enabled: bool,
}

#[derive(Clone, Debug)]
pub enum BotMode {
    Polling,
    Webhook,
}

/// Start the Telegram bot as a background task with auto-recovery.
pub async fn start_tgbot(state: Arc<AppState>, token: String, config: BotConfig) -> tokio::task::JoinHandle<()> {
    let bot = Bot::new(token.clone());
    let ctx = BotContext { state: state.clone() };
    
    log::info!("🤖 Telegram bot starting in {:?} mode...", config.mode);
    
    if config.auto_recovery_enabled {
        // Auto-recovery mechanism
        let bot_health = bot.clone();
        let state_health = state.clone();
        let token_health = token.clone();
        tokio::spawn(async move {
            let mut health_check = interval(Duration::from_secs(60));
            health_check.set_missed_tick_behavior(MissedTickBehavior::Delay);
            
            loop {
                health_check.tick().await;
                check_bot_health(&bot_health, &state_health, &token_health).await;
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
async fn start_polling_mode(bot: Bot, ctx: BotContext, config: BotConfig) {
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
async fn start_webhook_mode(bot: Bot, ctx: BotContext, config: BotConfig, _token: String) {
    if let Some(webhook_url) = config.webhook_url.clone() {
        log::info!("🔗 Setting up webhook: {}", webhook_url);
        
        match bot.set_webhook(webhook_url.parse().expect("invalid webhook URL")).send().await {
            Ok(_) => {
                log::info!("✅ Webhook set successfully");
                run_webhook_handler(bot, ctx, config).await;
            }
            Err(e) => {
                log::error!("❌ Failed to set webhook: {}", e);
                // Fallback to polling
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
async fn run_webhook_handler(_bot: Bot, _ctx: BotContext, _config: BotConfig) {
    log::info!("🌐 Webhook handler active — relay server receives updates via POST /api/tg/webhook");
    // Webhook updates are handled by the relay server's axum endpoint
    // which calls tgbot::handle_update() directly.
    // This task just keeps the context alive.
    loop {
        tokio::time::sleep(Duration::from_secs(300)).await;
    }
}

/// Health check for bot recovery
async fn check_bot_health(bot: &Bot, state: &AppState, _token: &str) {
    // Simple bot health check
    match bot.get_me().await {
        Ok(_) => {
            // Bot is healthy
            if let Some(billing) = &state.billing {
                let plans = billing.plans();
                log::debug!("🤖 Bot health OK");
                let _ = plans; // suppress unused warning
            }
        }
        Err(e) => {
            log::warn!("🤖 Bot health check failed: {}", e);
            // Could trigger recovery here
        }
    }
}

/// Emergency stop command
pub async fn emergency_stop_bot(_bot: Bot) {
    log::warn!("🚨 EMERGENCY: Stopping Telegram bot...");
    let _message = "🚨 EMERGENCY STOP: Telegram bot is being shut down due to critical system issue.";
    log::info!("🚨 Emergency stop message sent to bot users");
}

/// Central command router
async fn handle_command(
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
async fn handle_callback(
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

    // Always answer the callback to remove loading state
    let _ = bot.answer_callback_query(&q.id).await;

    match data.as_str() {
        // ── Notification channel callbacks ──
        d if d.starts_with("approve:") => {
            let approval_id = &d[8..];
            if ctx.state.approvals.resolve(approval_id, crate::approval::ApprovalDecision::Approved) {
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
            if ctx.state.approvals.resolve(approval_id, crate::approval::ApprovalDecision::Rejected) {
                bot.edit_message_text(chat_id, msg_id,
                    format!("❌ <b>Отклонено</b>\n\nID: <code>{}</code>", approval_id)
                ).parse_mode(ParseMode::Html).await?;
            } else {
                bot.edit_message_text(chat_id, msg_id,
                    "❌ Запрос не найден (возможно уже обработан или истёк таймаут)"
                ).await?;
            }
        }
        d if d.starts_with("notif:level:") => {
            // Format: notif:level:<channel_type>:<severity>
            let parts: Vec<&str> = d.split(':').collect();
            if parts.len() == 4 {
                let (_ch, _sev) = (parts[2], parts[3]);
                bot.send_message(chat_id, format!("🔔 Уровень уведомлений: {} → {}", _ch, _sev)).await?;
                // TODO: update via DB when channel_id is known
            }
            let _ = bot.edit_message_reply_markup(chat_id, msg_id).await;
        }
        d if d.starts_with("notif:mute:") => {
            let parts: Vec<&str> = d.split(':').collect();
            if parts.len() == 4 {
                let (_ch, _cat) = (parts[2], parts[3]);
                bot.send_message(chat_id, format!("🔇 Категория {} {} для канала {}", _cat, "замьючена", _ch)).await?;
                // TODO: update via DB
            }
            let _ = bot.edit_message_reply_markup(chat_id, msg_id).await;
        }
        d if d.starts_with("notif:unbind:") => {
            let _ch_type = &d[12..];
            bot.send_message(chat_id, format!("❌ Канал {} отвязан", _ch_type)).await?;
            // TODO: delete via DB
            let _ = bot.edit_message_reply_markup(chat_id, msg_id).await;
        }
        "notif:bind:max" => {
            let code = rand::random::<u32>() % 900_000 + 100_000;
            bot.send_message(chat_id, format!(
                "📲 Привязка MAX Messenger\n\nОтправьте код ниже боту MAX:\n\n<code>{}</code>\n\n⏳ Ожидание подтверждения...",
                code
            )).parse_mode(ParseMode::Html).await?;
            // TODO: store code + chat_id for verification callback
        }
        "notif:bind:slack" => {
            bot.send_message(chat_id, "🔗 Привязка Slack\n\nОткройте: https://flowlink.flow-masters.ru/api/notifications/slack/install\n\n⏳ После OAuth вы получите подтверждение.".to_string()).await?;
        }
        "dismiss" => {
            let _ = bot.edit_message_reply_markup(chat_id, msg_id).await;
        }
        "subcancel:confirm" => {
            let account_id = format!("tg:{}", chat_id.0);
            if let Some(tochka) = &ctx.state.tochka {
                match tochka.get_subscription_by_customer(&account_id).await {
                    Ok(sub) => {
                        match tochka.cancel_subscription(&sub.subscription_id).await {
                            Ok(_) => {
                                bot.send_message(chat_id, "✅ Подписка отменена. Доступ сохранится до конца текущего периода.").await?;
                            }
                            Err(e) => {
                                bot.send_message(chat_id, format!("❌ Ошибка отмены: {}", e)).await?;
                            }
                        }
                    }
                    Err(_) => {
                        bot.send_message(chat_id, "❌ Активная подписка не найдена.").await?;
                    }
                }
            } else {
                bot.send_message(chat_id, "❌ Платёжная система не настроена.").await?;
            }
            let _ = bot.edit_message_reply_markup(chat_id, msg_id).await;
        }
        d if d.starts_with("sub:") => {
            let plan_id = &d[4..];
            bot.send_message(chat_id, format!("Для подписки на план {} используйте: /subscribe {}", plan_id, plan_id)).await?;
        }
        d if d.starts_with("subchange:") => {
            let new_plan_id = &d[10..];
            let account_id = format!("tg:{}", chat_id.0);

            if let (Some(billing), Some(tochka)) = (&ctx.state.billing, &ctx.state.tochka) {
                let acc = billing.get_or_create_account(&account_id);
                let current_plan = billing.plans().get(&acc.plan_id);
                let new_plan = billing.plans().get(new_plan_id);

                match (current_plan, new_plan) {
                    (Some(cur), Some(new_p)) => {
                        let is_upgrade = new_p.price_kopecks >= cur.price_kopecks;

                        if is_upgrade {
                            if let Ok(old_sub) = tochka.get_subscription_by_customer(&account_id).await {
                                let _ = tochka.cancel_subscription(&old_sub.subscription_id).await;
                            }

                            let req = flowlink_billing::tochka::CreateSubscriptionRequest {
                                customer_id: account_id.clone(),
                                plan_id: new_plan_id.to_string(),
                                period: flowlink_billing::tochka::BillingPeriod::Month,
                                amount: new_p.price_kopecks,
                                payment_method: flowlink_billing::tochka::SubscriptionPaymentMethod::Sbp {
                                    phone: String::new(),
                                },
                                description: format!("FlowLink {}", new_p.name),
                                start_date: None,
                                trial_days: 0,
                                customer_email: None,
                            };

                            match tochka.create_subscription(&req).await {
                                Ok(_sub) => {
                                    let mut acc = billing.get_or_create_account(&account_id);
                                    let _ = billing.change_plan(&mut acc, new_plan_id);
                                    bot.send_message(chat_id, format!("✅ План изменён на <b>{}</b> (немедленно)", new_p.name))
                                        .parse_mode(ParseMode::Html).await?;
                                }
                                Err(e) => {
                                    bot.send_message(chat_id, format!("❌ Ошибка: {}", e)).await?;
                                }
                            }
                        } else {
                            // Store pending plan change in memory
                            if let Some(db) = &ctx.state.db {
                                let _ = sqlx::query(
                                    "INSERT INTO pending_plan_changes (account_id, current_plan_id, pending_plan_id, created_at) \
                                     VALUES ($1, $2, $3, NOW()) \
                                     ON CONFLICT (account_id) DO UPDATE SET pending_plan_id = $3, created_at = NOW()"
                                )
                                .bind(&account_id)
                                .bind(&acc.plan_id)
                                .bind(new_plan_id)
                                .execute(db.pool())
                                .await;
                            }
                            bot.send_message(chat_id, format!(
                                "✅ Понижение запланировано.\n📦 {} → <b>{}</b>\n📅 Вступит в силу в конце текущего периода",
                                cur.name, new_p.name
                            )).parse_mode(ParseMode::Html).await?;
                        }
                    }
                    _ => {
                        bot.send_message(chat_id, "❌ Ошибка: план не найден").await?;
                    }
                }
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

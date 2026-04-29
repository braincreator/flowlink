//! Telegram bot commands for FlowLink.
//!
//! Each handler receives BotContext (wrapping BotState) and replies via teloxide.
//! This is the standalone crate version — uses `BotState` instead of relay's `AppState`.

use crate::BotState;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};

/// Shared context for bot commands — wraps BotState
#[derive(Clone)]
pub struct BotContext {
    pub state: Arc<BotState>,
}

// ═══════════════════════════════════════════════
// Formatting helpers
// ═══════════════════════════════════════════════

/// Format kopecks to "1 990 ₽"
pub fn format_kopecks(kopecks: u64) -> String {
    let rubles = kopecks / 100;
    let s = format!("{}", rubles);
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            result.push(' ');
        }
        result.push(c);
    }
    result.push_str(" ₽");
    result
}

// ═══════════════════════════════════════════════
// Command handlers
// ═══════════════════════════════════════════════

/// /start — greeting, optionally handles Telegram link code
pub async fn cmd_start(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let name = msg.from.as_ref()
        .map(|u| u.first_name.clone())
        .unwrap_or_else(|| "друг".to_string());
    let tg_chat_id = msg.chat.id.0;

    // ── Phase 1: Check if already linked ──
    if let Some(db) = &ctx.state.db {
        let account_id = format!("tg:{}", tg_chat_id);
        if let Ok(Some(_account)) = flowlink_db::accounts::AccountRepo::get(db.pool(), &account_id).await {
            let text = format!(
                "👋 Привет, <b>{}</b>!\n\nАккаунт уже привязан ✅\n\n📊 /status — статус\n💳 /billing — подписка\n🛡 /shield — безопасность\n📢 /settings — уведомления\n\n/help — все команды",
                name,
            );
            bot.send_message(msg.chat.id, text).parse_mode(ParseMode::Html).await?;
            return Ok(());
        }

        let link_text = msg.text().unwrap_or("");
        let arg = link_text
            .strip_prefix("/start")
            .map(|s| s.trim().split('@').next().unwrap_or("").trim())
            .filter(|s| !s.is_empty());

        if let Some(code) = arg {
            // Legacy linking: /start <account_id>
            match flowlink_db::accounts::AccountRepo::get(db.pool(), code).await {
                Ok(Some(account)) => {
                    // Link: update account's tg_id AND bind notification channel
                    match flowlink_db::accounts::AccountRepo::update_tg_id(
                        db.pool(), &account.account_id, tg_chat_id,
                    ).await {
                        Ok(()) => {
                            // Auto-bind notification channel
                            let _ = flowlink_db::notification_channels::UserChannelRepo::upsert(
                                db.pool(),
                                &account.account_id,
                                "telegram",
                                &tg_chat_id.to_string(),
                                Some(name.as_str()),
                                true, // set as primary if first
                            ).await;
                            log::info!(
                                "TG linked: account={}, tg_id={}, name={}",
                                account.account_id, tg_chat_id, name
                            );
                            bot.send_message(msg.chat.id, format!(
                                "✅ Telegram привязан к аккаунту!\n\nПривет, <b>{}</b>!\n\n📊 /status — статус\n💳 /billing — подписка\n🛡 /shield — безопасность\n📢 /settings — уведомления",
                                name,
                            )).parse_mode(ParseMode::Html).await?;
                        }
                        Err(e) => {
                            log::warn!("TG link failed: {e}");
                            bot.send_message(msg.chat.id, "❌ Ошибка привязки.").await?;
                        }
                    }
                }
                Ok(None) => {
                    bot.send_message(msg.chat.id,
                        "❌ Код не найден.\n\nВведите код из настройки профиля FlowLink:\n<code>/start &lt;код&gt;</code>"
                    ).parse_mode(ParseMode::Html).await?;
                }
                Err(e) => {
                    log::warn!("Account lookup failed: {e}");
                    bot.send_message(msg.chat.id, "❌ Ошибка.").await?;
                }
            }
        } else {
            bot.send_message(msg.chat.id, format!(
                "👋 Привет, <b>{}</b>!\n\nПривяжите Telegram к аккаунту FlowLink:\n\n1. Откройте настройки профиля\n2. Скопируйте ваш код привязки\n3. Отправьте: <code>/start &lt;код&gt;</code>\n\n⏳ Код действует 10 минут\n\n💡 Или сгенерируйте код в веб-дашборде:\n<i>Настройки → Уведомления → Привязать Telegram</i>",
                name,
            )).parse_mode(ParseMode::Html).await?;
        }
    } else {
        let text = format!(
            "👋 Привет, <b>{}</b>!\n\nБаза данных недоступна.\n/help — команды",
            name,
        );
        bot.send_message(msg.chat.id, text).parse_mode(ParseMode::Html).await?;
    }
    Ok(())
}

/// /help — list all commands
pub async fn cmd_help(bot: Bot, msg: Message, _ctx: BotContext) -> ResponseResult<()> {
    let text = "\
\u{1f4d6} <b>Справка FlowLink</b>\n\n\
\u{1f5a5} <b>Серверы:</b>\n\
/status \u{2014} статус серверов\n\
/servers \u{2014} список агентов\n\
/logs \u{2014} последние действия\n\
/approvals \u{2014} ожидающие подтверждения\n\
/devices \u{2014} устройства\n\n\
\u{1f6e1} <b>Безопасность:</b>\n\
/shield \u{2014} оповещения\n\
/reload \u{2014} перезагрузить конфиг\n\
/emergency \u{2014} экстренная остановка\n\n\
\u{1f4b3} <b>Биллинг:</b>\n\
/plans \u{2014} тарифы\n\
/billing \u{2014} статус подписки\n\
/myplan \u{2014} фичи и лимиты плана\n\
/subscribe &lt;plan&gt; \u{2014} подписаться\n\
/usage \u{2014} статистика с лимитами\n\n\
\u{2699} <b>Система:</b>\n\
/config \u{2014} конфигурация\n\
/settings \u{2014} каналы уведомлений";

    bot.send_message(msg.chat.id, text).parse_mode(ParseMode::Html).await?;
    Ok(())
}

/// /status — server status
pub async fn cmd_status(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let agents = ctx.state.pool.list();

    let text = if agents.is_empty() {
        "📭 Нет подключённых серверов.".to_string()
    } else {
        let mut sb = format!("<b>📊 Статус серверов ({})</b>\n\n", agents.len());
        for a in &agents {
            let short = if a.agent_id.len() > 12 { &a.agent_id[..12] } else { &a.agent_id };
            let heartbeat = if a.last_heartbeat > 0 {
                let secs = (chrono::Utc::now().timestamp() - a.last_heartbeat).max(0);
                if secs < 60 { format!("{}с назад", secs) }
                else if secs < 3600 { format!("{}м назад", secs / 60) }
                else { format!("{}ч назад", secs / 3600) }
            } else { "never".to_string() };

            sb.push_str(&format!(
                "<b>{}</b> (<code>{}</code>)\n  {} | {}\n\n",
                a.hostname, short, a.os, heartbeat,
            ));
        }
        sb
    };

    bot.send_message(msg.chat.id, text).parse_mode(ParseMode::Html).await?;
    Ok(())
}

/// /servers — list agents
pub async fn cmd_servers(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let agents = ctx.state.pool.list();

    let text = if agents.is_empty() {
        "📭 Нет подключённых агентов.".to_string()
    } else {
        let mut sb = format!("<b>🖥 Серверы ({})</b>\n\n", agents.len());
        for (i, a) in agents.iter().enumerate() {
            let short = if a.agent_id.len() > 12 { &a.agent_id[..12] } else { &a.agent_id };
            sb.push_str(&format!(
                "{}. <b>{}</b>\n   ID: <code>{}</code> | {}\n\n",
                i + 1, a.hostname, short, a.os,
            ));
        }
        sb
    };

    bot.send_message(msg.chat.id, text).parse_mode(ParseMode::Html).await?;
    Ok(())
}

/// /plans — available plans (dynamic from DB)
pub async fn cmd_plans(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    const PLAN_DESC: &[(&str, &str)] = &[
        ("plan_free_desc", "Базовая защита — 1 агент, щиты, согласование, логи"),
        ("plan_starter_desc", "Для фрилансеров и небольших команд — 3 агента"),
        ("plan_team_desc", "Для команд разработчиков — RBAC, обучение паттернам, SIEM"),
        ("plan_business_desc", "Для агентств — SSO, AI Ops, каталог сервисов"),
        ("plan_enterprise_desc", "Безлимит — on-premise, SLA, выделенная поддержка"),
    ];
    fn resolve_desc(key: &str) -> &str {
        PLAN_DESC.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or(key)
    }
    let plans = ctx.state.billing.as_ref()
        .map(|e| e.plans().list_available())
        .unwrap_or_default();

    if plans.is_empty() {
        bot.send_message(msg.chat.id, "Нет доступных планов.").await?;
        return Ok(());
    }

    let account_id = format!("tg:{}", msg.chat.id);
    let current_plan_id = ctx.state.billing.as_ref()
        .map(|b| b.get_or_create_account(&account_id).plan_id.clone());

    let mut sb = String::from("\u{1f4cb} <b>Тарифные планы FlowLink</b>\n\n");
    for p in &plans {
        let is_current = current_plan_id.as_deref() == Some(&p.id);
        let badge = if is_current { " \u{2705} <i>текущий</i>" } else { "" };
        sb.push_str(&format!("\u{1f4e6} <b>{}{}</b>\n", p.name, badge));
        if p.price_kopecks == 0 {
            sb.push_str("   \u{1f4b0} Бесплатно");
            if let Some(days) = p.trial_days {
                sb.push_str(&format!(" ({} дн.)", days));
            }
            sb.push('\n');
        } else {
            sb.push_str(&format!("   \u{1f4b0} <b>{} \u{20bd}</b>/мес\n", format_kopecks(p.price_kopecks)));
        }
        if !p.description.is_empty() {
            sb.push_str(&format!("   {}\n", resolve_desc(&p.description)));
        }
        let features = [
            ("Shield", p.features.shield),
            ("MCP Gateway", p.features.mcp_gateway),
            ("Approval", p.features.approval),
            ("RBAC", p.features.rbac),
            ("Policy Engine", p.features.policy_engine),
            ("Pattern Learning", p.features.pattern_learning),
            ("Webhooks", p.features.webhooks),
            ("SIEM Export", p.features.siem_export),
            ("SSO", p.features.sso),
            ("E2EE", p.features.e2ee),
            ("On-premise", p.features.on_premise),
        ];
        for (name, enabled) in &features {
            if *enabled {
                sb.push_str(&format!("   \u{2705} {}\n", name));
            }
        }
        sb.push_str("   \u{1f4cf} ");
        let mut limit_parts = Vec::new();
        if p.limits.max_agents > 0 {
            limit_parts.push(format!("{} аг.", p.limits.max_agents));
        }
        if p.limits.max_users > 0 {
            limit_parts.push(format!("{} польз.", p.limits.max_users));
        }
        if p.limits.audit_retention_days > 0 {
            limit_parts.push(format!("{} дн. логов", p.limits.audit_retention_days));
        }
        if limit_parts.is_empty() {
            sb.push_str("Без ограничений");
        } else {
            sb.push_str(&limit_parts.join(" | "));
        }
        sb.push_str(&format!("\n   \u{1f6e0} Поддержка: {}\n", p.limits.support_tier));
        sb.push('\n');
    }
    sb.push_str("\u{1f4a1} /subscribe <code>&lt;plan&gt;</code> \u{2014} подписаться");

    bot.send_message(msg.chat.id, &sb).parse_mode(ParseMode::Html).await?;
    Ok(())
}

/// /billing — current billing status
pub async fn cmd_billing(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let account_id = format!("tg:{}", msg.chat.id);
    let billing = match &ctx.state.billing {
        Some(b) => b,
        None => {
            bot.send_message(msg.chat.id, "❌ Биллинг не настроен.").await?;
            return Ok(());
        }
    };

    let acc = billing.get_or_create_account(&account_id);
    let plan = billing.plans().get(&acc.plan_id);
    let plan_name = plan.as_ref().map(|p| p.name.clone()).unwrap_or_default();

    let status = if acc.active { "✅ Активна" } else { "❌ Неактивна" };

    let text = format!(
        "💳 <b>Биллинг</b>\n\n\
         {} Статус: *{}*\n\
         📦 Текущий план: *{}*\n\
         💰 Баланс: *{}*\n\n\
         💡 /plans — тарифы | /myplan — детали",
        status, status, plan_name,
        flowlink_billing::payment::PaymentConfig::format_rub(acc.balance_kopecks),
    );

    bot.send_message(msg.chat.id, text).parse_mode(ParseMode::Html).await?;
    Ok(())
}

/// /myplan — current plan details with all features and limits
pub async fn cmd_myplan(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let account_id = format!("tg:{}", msg.chat.id);
    let billing = match &ctx.state.billing {
        Some(b) => b,
        None => {
            bot.send_message(msg.chat.id, "Биллинг не настроен.").await?;
            return Ok(());
        }
    };

    let acc = billing.get_or_create_account(&account_id);
    let plan = match billing.plans().get(&acc.plan_id) {
        Some(p) => p,
        None => {
            bot.send_message(msg.chat.id, "План не найден.").await?;
            return Ok(());
        }
    };

    let mut text = format!("\u{1f4e6} <b>Текущий план: {}</b>\n\n", plan.name);

    text.push_str("\u{1f513} <b>Функции:</b>\n");
    let features = [
        ("Shield", plan.features.shield, Some(&plan.features.shield_level)),
        ("MCP Gateway", plan.features.mcp_gateway, None),
        ("Approval", plan.features.approval, None),
        ("RBAC", plan.features.rbac, None),
        ("Policy Engine", plan.features.policy_engine, None),
        ("Pattern Learning", plan.features.pattern_learning, None),
        ("E2EE", plan.features.e2ee, None),
        ("Audit Log", plan.features.audit_log, None),
        ("Webhooks", plan.features.webhooks, None),
        ("SIEM Export", plan.features.siem_export, None),
        ("SSO", plan.features.sso, None),
        ("On-premise", plan.features.on_premise, None),
    ];
    for (name, enabled, detail) in &features {
        let icon = if *enabled { "\u{2705}" } else { "\u{274c}" };
        let detail_str = detail.map(|d| format!(" ({})", d)).unwrap_or_default();
        text.push_str(&format!("   {} {}{}\n", icon, name, detail_str));
    }

    text.push_str("\n\u{1f4cf} <b>Лимиты:</b>\n");
    let limits = [
        ("Агенты", plan.limits.max_agents, "шт."),
        ("Пользователи", plan.limits.max_users, "шт."),
        ("Хранение логов", plan.limits.audit_retention_days, "дн."),
        ("API rate limit", plan.limits.api_rate_limit as u64, "зап./мин."),
        ("Кастомных правил", plan.limits.max_custom_rules, "шт."),
        ("Политик", plan.limits.max_policies, "шт."),
    ];
    for (name, val, unit) in &limits {
        let display = if *val == 0 { "\u{221e}".to_string() } else { val.to_string() };
        text.push_str(&format!("   {} / {} {}\n", name, display, unit));
    }
    if !plan.limits.approval_channels.is_empty() {
        text.push_str(&format!("   Каналы одобрения: {}\n", plan.limits.approval_channels.join(", ")));
    }
    if !plan.limits.siem_formats.is_empty() {
        text.push_str(&format!("   SIEM форматы: {}\n", plan.limits.siem_formats.join(", ")));
    }
    text.push_str(&format!("   Поддержка: <b>{}</b>\n", plan.limits.support_tier));

    text.push_str("\n\u{1f4a1} /plans \u{2014} все тарифы | /usage \u{2014} статистика");
    bot.send_message(msg.chat.id, &text).parse_mode(ParseMode::Html).await?;
    Ok(())
}

/// /subscribe <plan_id> — redirect to checkout page
pub async fn cmd_subscribe(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let plan_id = msg.text()
        .and_then(|t| t.strip_prefix("/subscribe"))
        .unwrap_or("")
        .trim()
        .split('@')
        .next()
        .unwrap_or("")
        .trim();

    if plan_id.is_empty() {
        cmd_plans(bot.clone(), msg.clone(), ctx.clone()).await?;
        if let Some(billing) = &ctx.state.billing {
            let plans = billing.plans().list_available();
            let buttons: Vec<Vec<InlineKeyboardButton>> = plans.iter()
                .filter(|p| p.price_kopecks > 0)
                .map(|p| vec![InlineKeyboardButton::callback(
                    format!("📦 {} — {} ₽/мес", p.name, format_kopecks(p.price_kopecks)),
                    format!("sub:{}", p.id),
                )])
                .collect();
            if !buttons.is_empty() {
                let kb: InlineKeyboardMarkup = InlineKeyboardMarkup::new(buttons);
                bot.send_message(msg.chat.id, "Выберите план для подписки:")
                    .reply_markup(kb).await?;
            }
        }
        return Ok(());
    }

    let billing = match &ctx.state.billing {
        Some(b) => b,
        None => {
            bot.send_message(msg.chat.id, "❌ Биллинг не настроен.").await?;
            return Ok(());
        }
    };

    let plan = match billing.plans().get(plan_id) {
        Some(p) => p,
        None => {
            bot.send_message(msg.chat.id, format!("❌ План '{}' не найден. /plans", plan_id)).await?;
            return Ok(());
        }
    };

    // Free plan — switch directly
    if plan.price_kopecks == 0 {
        return Ok(());
    }

    let checkout_url = format!("{}/checkout/{}", ctx.state.base_url, plan_id);
    let kb: InlineKeyboardMarkup = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::url(
            "💳 Оплатить",
            reqwest::Url::parse(&checkout_url).unwrap(),
        )],
    ]);

    bot.send_message(msg.chat.id, format!(
        "💳 <b>Оплата {}</b>\n\n📦 План: <b>{}</b>\n💰 Сумма: <b>{} ₽</b>\n\nНажмите кнопку для перехода к оплате:",
        plan.name, plan.name, format_kopecks(plan.price_kopecks),
    ))
    .parse_mode(ParseMode::Html)
    .reply_markup(kb)
    .await?;

    Ok(())
}

/// /invoices — billing history
pub async fn cmd_invoices(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let account_id = format!("tg:{}", msg.chat.id);

    let invoices = ctx.state.billing.as_ref()
        .map(|e| e.invoices().list_for_account(&account_id))
        .unwrap_or_default();

    if invoices.is_empty() {
        bot.send_message(msg.chat.id, "📭 Нет счетов.").await?;
        return Ok(());
    }

    let mut sb = format!("🧾 <b>Счета ({})</b>\n\n", invoices.len());
    for inv in &invoices {
        let short = if inv.id.len() > 8 { &inv.id[..8] } else { &inv.id };
        let status = match inv.status {
            flowlink_billing::invoice::InvoiceStatus::Paid => "✅",
            flowlink_billing::invoice::InvoiceStatus::Pending => "⏳",
            _ => "❓",
        };
        sb.push_str(&format!("{} <code>{}</code> — <b>{}</b>\n", status, short, format_kopecks(inv.total_kopecks)));
    }

    bot.send_message(msg.chat.id, &sb).parse_mode(ParseMode::Html).await?;
    Ok(())
}

/// /usage — usage statistics
pub async fn cmd_usage(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let (daily_requests, daily_tokens) = ctx.state.usage_tracker.today_stats().await;
    let all_usage = ctx.state.usage_tracker.get_all_usage().await;
    let active_agents = all_usage.len();

    let account_id = format!("tg:{}", msg.chat.id);
    let plan_info = ctx.state.billing.as_ref().map(|b| {
        let acc = b.get_or_create_account(&account_id);
        let plan = b.plans().get(&acc.plan_id);
        plan.map(|p| (p.name.clone(), p.limits.clone()))
    });

    let mut text = format!(
        "\u{1f4ca} <b>Использование</b>\n\n\u{1f4c8} Запросов сегодня: {}\n\u{1f524} Токенов сегодня: {}\n\u{1f5a5} Активных агентов: {}",
        daily_requests, daily_tokens, active_agents,
    );

    if let Some(Some((plan_name, limits))) = plan_info {
        text.push_str(&format!("\n\n\u{1f4e6} План: <b>{}</b>\n\n\u{1f4cf} <b>Лимиты:</b>", plan_name));
        let agent_pct = if limits.max_agents > 0 {
            format!(" ({}/{}, {}%)", active_agents, limits.max_agents,
                (active_agents as f64 / limits.max_agents as f64 * 100.0).min(100.0) as u64)
        } else { " (\u{221e})".to_string() };
        text.push_str(&format!("\n   \u{1f916} Агенты:{}", agent_pct));
        if limits.api_rate_limit > 0 {
            let rate_pct = (daily_requests as f64 / limits.api_rate_limit as f64 * 100.0).min(999.0) as u64;
            let warn_str = if rate_pct > 80 { " \u{26a0}" } else { "" };
            text.push_str(&format!("\n   \u{1f4e1} API: {}/{} зап/мин{}", daily_requests, limits.api_rate_limit, warn_str));
        }
        if limits.max_custom_rules > 0 {
            text.push_str(&format!("\n   \u{1f9e9} Правил: /{}", limits.max_custom_rules));
        }
        if limits.max_policies > 0 {
            text.push_str(&format!("\n   \u{1f6e1} Политик: /{}", limits.max_policies));
        }
        text.push_str(&format!("\n\n\u{1f6e0} Поддержка: {}", limits.support_tier));
    }

    bot.send_message(msg.chat.id, text).parse_mode(ParseMode::Html).await?;
    Ok(())
}

/// /shield — security alerts
pub async fn cmd_shield(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let active = ctx.state.shield_alerts.list_active();
    let all = ctx.state.shield_alerts.list_all();

    let text = format!(
        "🛡 <b>Shield Status</b>\n\n\
         📊 Всего оповещений: {}\n\
         ⚠️ Активных: {}\n\
         ✅ Разрешено: {}",
        all.len(), active.len(), all.len() - active.len(),
    );

    bot.send_message(msg.chat.id, text).parse_mode(ParseMode::Html).await?;
    Ok(())
}

/// /settings — notification channel management
pub async fn cmd_settings(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let chat_id = msg.chat.id;
    let account_id = format!("tg:{}", chat_id.0);

    let channels_text = if let Some(ref db) = ctx.state.db {
        match flowlink_db::notification_channels::UserChannelRepo::list_for_account(db.pool(), &account_id).await {
            Ok(channels) if !channels.is_empty() => {
                channels.iter().enumerate().map(|(i, ch)| {
                    let icon = match ch.channel_type.as_str() {
                        "telegram" => "🔵",
                        "max" => "💬",
                        "slack" => "🟣",
                        "webhook" => "🌐",
                        _ => "⚪",
                    };
                    let primary = if ch.is_primary { " ✅" } else { "" };
                    let verified = if ch.verified { "" } else { " ⏳" };
                    let severity = ch.min_severity.as_deref().unwrap_or("info");
                    let name = ch.display_name.as_deref().unwrap_or(&ch.channel_type);
                    format!("{}. {} {} — {}{}{}", i + 1, icon, name, severity, primary, verified)
                }).collect::<Vec<_>>().join("\n")
            }
            _ => "Нет привязанных каналов".into(),
        }
    } else {
        "База данных не подключена".into()
    };

    let kb: InlineKeyboardMarkup = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("💬 MAX", "notif:bind:max"),
            InlineKeyboardButton::callback("🟣 Slack", "notif:bind:slack"),
        ],
        vec![InlineKeyboardButton::callback("🔔 Тест", "notif:test")],
    ]);

    bot.send_message(chat_id, format!("📢 <b>Уведомления</b>\n\n{}\n\n<i>Shield, Approvals, Billing → ваши каналы</i>", channels_text))
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await?;
    Ok(())
}

/// /config — show current configuration
pub async fn cmd_config(bot: Bot, msg: Message, _ctx: BotContext) -> ResponseResult<()> {
    bot.send_message(msg.chat.id, "⚙ Конфигурация доступна через веб-дашборд.").await?;
    Ok(())
}

/// /reload — reload configuration
pub async fn cmd_reload(bot: Bot, msg: Message, _ctx: BotContext) -> ResponseResult<()> {
    bot.send_message(msg.chat.id, "🔄 Перезагрузка конфигурации через веб-дашборд или API.").await?;
    Ok(())
}

/// /emergency — emergency stop
pub async fn cmd_emergency(bot: Bot, msg: Message, _ctx: BotContext) -> ResponseResult<()> {
    let kb: InlineKeyboardMarkup = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🔴 STOP ALL", "emergency:confirm".to_string()),
            InlineKeyboardButton::callback("❌ Отмена", "dismiss".to_string()),
        ],
    ]);

    bot.send_message(msg.chat.id,
        "🚨 <b>ЭКСТРЕННАЯ ОСТАНОВКА</b>\n\nВсе серверы будут немедленно остановлены.")
    .parse_mode(ParseMode::Html)
    .reply_markup(kb)
    .await?;

    Ok(())
}

/// /logs — recent audit entries
pub async fn cmd_logs(bot: Bot, msg: Message, _ctx: BotContext) -> ResponseResult<()> {
    bot.send_message(msg.chat.id, "📋 Audit log доступен через веб-дашборд.").await?;
    Ok(())
}

/// /approvals — list pending approvals
pub async fn cmd_approvals(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let approvals = ctx.state.approvals.list_pending();
    if approvals.is_empty() {
        bot.send_message(msg.chat.id, "\u{2705} Нет ожидающих подтверждений.").await?;
        return Ok(());
    }

    for a in &approvals {
        let cmd = if a.command.len() > 60 { format!("{}...", &a.command[..60]) } else { a.command.clone() };
        let risk_emoji = match a.risk_level.as_str() {
            "critical" => "🔴", "high" => "🟠", "medium" => "🟡", _ => "🟢",
        };

        let text = format!(
            "⏳ <b>Запрос на выполнение</b>\n\n\
             🖥 Агент: <code>{}</code>\n\
             💻 Команда: <code>{}</code>\n\
             {} Риск: <b>{}</b>\n\
             🆔 ID: <code>{}</code>",
            a.agent_id, cmd, risk_emoji, a.risk_level, a.id
        );

        let kb = InlineKeyboardMarkup::new(vec![
            vec![
                InlineKeyboardButton::callback("✅ Разрешить", format!("approve:{}", a.id)),
                InlineKeyboardButton::callback("❌ Отклонить", format!("deny:{}", a.id)),
            ],
        ]);

        bot.send_message(msg.chat.id, &text)
            .parse_mode(ParseMode::Html)
            .reply_markup(kb)
            .await?;
    }

    Ok(())
}

/// /notiftest — send test notification
pub async fn cmd_notiftest(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let chat_id = msg.chat.id;
    if let Some(ref router) = ctx.state.notification_router {
        let n = flowlink_notifications::Notification::system(
            "Test",
            &format!("✅ FlowLink уведомления работают!\n{}", chrono::Utc::now().format("%H:%M:%S")),
            flowlink_notifications::Severity::Info,
        );
        let delivered = router.send(&n).await;
        bot.send_message(chat_id, format!("🔔 Отправлено: {} кан.", delivered)).await?;
    } else {
        bot.send_message(chat_id, "⚠️ Router не настроен").await?;
    }
    Ok(())
}

/// /substatus — show current subscription info
pub async fn cmd_substatus(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let account_id = format!("tg:{}", msg.chat.id);
    let billing = match &ctx.state.billing {
        Some(b) => b,
        None => { bot.send_message(msg.chat.id, "❌ Биллинг не настроен.").await?; return Ok(()); }
    };
    let acc = billing.get_or_create_account(&account_id);
    let plan = billing.plans().get(&acc.plan_id);
    let plan_name = plan.as_ref().map(|p| p.name.clone()).unwrap_or_default();

    let mut text = format!("💳 <b>Подписка</b>\n\n📦 План: <b>{}</b>\n", plan_name);

    text.push_str("\n💡 /subcancel — отменить | /subchange — сменить план");
    bot.send_message(msg.chat.id, text).parse_mode(ParseMode::Html).await?;
    Ok(())
}

/// /subcancel — cancel subscription with confirmation
pub async fn cmd_subcancel(bot: Bot, msg: Message, _ctx: BotContext) -> ResponseResult<()> {
    let kb: InlineKeyboardMarkup = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🔴 Да, отменить", "subcancel:confirm".to_string()),
            InlineKeyboardButton::callback("❌ Нет", "dismiss".to_string()),
        ],
    ]);
    bot.send_message(msg.chat.id,
        "⚠️ <b>Отмена подписки</b>\n\nВы уверены? После отмены доступ к платным функциям прекратится в конце текущего периода.")
    .parse_mode(ParseMode::Html)
    .reply_markup(kb)
    .await?;
    Ok(())
}

/// /subchange <plan_id> — change plan
pub async fn cmd_subchange(bot: Bot, msg: Message, ctx: BotContext) -> ResponseResult<()> {
    let new_plan_id = msg.text()
        .and_then(|t| t.strip_prefix("/subchange"))
        .unwrap_or("")
        .trim()
        .split('@')
        .next()
        .unwrap_or("")
        .trim();

    if new_plan_id.is_empty() {
        cmd_plans(bot.clone(), msg.clone(), ctx.clone()).await?;
        bot.send_message(msg.chat.id, "💡 Используйте: /subchange <code><plan_id></code>").parse_mode(ParseMode::Html).await?;
        return Ok(());
    }

    let account_id = format!("tg:{}", msg.chat.id);
    let billing = match &ctx.state.billing {
        Some(b) => b,
        None => { bot.send_message(msg.chat.id, "❌ Биллинг не настроен.").await?; return Ok(()); }
    };
    let acc = billing.get_or_create_account(&account_id);
    let current = match billing.plans().get(&acc.plan_id) {
        Some(p) => p,
        None => { bot.send_message(msg.chat.id, "❌ Текущий план не найден.").await?; return Ok(()); }
    };
    let new_plan = match billing.plans().get(new_plan_id) {
        Some(p) => p,
        None => { bot.send_message(msg.chat.id, format!("❌ План '{}' не найден. /plans", new_plan_id)).await?; return Ok(()); }
    };

    let is_upgrade = new_plan.price_kopecks >= current.price_kopecks;
    let change_type = if is_upgrade { "⬆️ Повышение" } else { "⬇️ Понижение" };
    let effective = if is_upgrade { "немедленно" } else { "в конце текущего периода" };

    let text = format!(
        "🔄 <b>Смена плана</b>\n\n{}: {} → <b>{}</b>\n💰 {} → {} ₽/мес\n📅 Вступит в силу: <b>{}</b>",
        change_type, current.name, new_plan.name,
        format_kopecks(current.price_kopecks), format_kopecks(new_plan.price_kopecks),
        effective,
    );

    let kb: InlineKeyboardMarkup = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "✅ Подтвердить",
            format!("subchange:{}", new_plan_id),
        )],
        vec![InlineKeyboardButton::callback("❌ Отмена", "dismiss".to_string())],
    ]);

    bot.send_message(msg.chat.id, text).parse_mode(ParseMode::Html).reply_markup(kb).await?;
    Ok(())
}

/// /exec <agent_id> <command> — execute command (info only)
pub async fn cmd_exec(bot: Bot, msg: Message, _ctx: BotContext) -> ResponseResult<()> {
    let text = "⚠ Выполнение команд доступно через relay API.\n\n\
                Используйте: POST /api/exec/`<agent_id>`";
    bot.send_message(msg.chat.id, text).parse_mode(ParseMode::Html).await?;
    Ok(())
}

/// /devices — list registered devices
pub async fn cmd_devices(bot: Bot, msg: Message, _ctx: BotContext) -> ResponseResult<()> {
    bot.send_message(msg.chat.id, "📱 Управление устройствами через веб-дашборд.").await?;
    Ok(())
}

/// /backups — placeholder
pub async fn cmd_backups(bot: Bot, msg: Message, _ctx: BotContext) -> ResponseResult<()> {
    bot.send_message(msg.chat.id, "📦 Бэкапы через API агента.").await?;
    Ok(())
}

/// /restore — placeholder
pub async fn cmd_restore(bot: Bot, msg: Message, _ctx: BotContext) -> ResponseResult<()> {
    bot.send_message(msg.chat.id, "📦 Восстановление через API агента.").await?;
    Ok(())
}

/// /policy — placeholder
pub async fn cmd_policy(bot: Bot, msg: Message, _ctx: BotContext) -> ResponseResult<()> {
    bot.send_message(msg.chat.id, "🛡 /shield — статус безопасности").await?;
    Ok(())
}

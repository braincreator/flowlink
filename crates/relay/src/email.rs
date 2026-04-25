//! Email service — SMTP sending via Postal
//!
//! Bilingual (EN/RU), dark theme matching flow-masters.ru.
//! All templates accept `lang: &str` ("en" | "ru").

use anyhow::{Result, Context};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{header::ContentType, MultiPart},
    transport::smtp::authentication::Credentials,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct EmailService {
    transport: Arc<AsyncSmtpTransport<Tokio1Executor>>,
    from: String,
}

impl EmailService {
    pub fn new(host: &str, port: u16, username: &str, password: &str, from: &str) -> Result<Self> {
        let creds = Credentials::new(username.to_string(), password.to_string());
        // Use unencrypted SMTP for local Postal (no STARTTLS on localhost)
        // For external SMTP servers, use starttls instead
        let transport = if host == "127.0.0.1" || host == "localhost" {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
                .port(port)
                .credentials(creds)
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
                .map_err(|e| anyhow::anyhow!("SMTP relay error: {e}"))?
                .port(port)
                .credentials(creds)
                .build()
        };
        Ok(Self { transport: Arc::new(transport), from: from.to_string() })
    }

    async fn send_email(&self, to: &str, subject: &str, html_body: &str, text_body: &str) -> Result<()> {
        let base = std::env::var("SERVER_URL").unwrap_or_else(|_| "https://flowlink.flow-masters.ru".to_string());
        log::info!("SMTP: building email from={} to={}", self.from, to);
        let email = Message::builder()
            .from(self.from.parse().context("Invalid FROM")?)
            .to(to.parse().context("Invalid TO")?)
            .subject(subject)
            .multipart(MultiPart::alternative()
                .singlepart(lettre::message::SinglePart::builder().header(ContentType::TEXT_PLAIN).body(text_body.replace("__BASE_URL__", &base)))
                .singlepart(lettre::message::SinglePart::builder().header(ContentType::TEXT_HTML).body(html_body.replace("__BASE_URL__", &base))),
            )?;
        log::info!("SMTP: sending via transport...");
        let result = self.transport.send(email).await;
        match result {
            Ok(_) => { log::info!("SMTP: sent successfully"); Ok(()) }
            Err(e) => {
                log::error!("SMTP transport error: {e:?}");
                Err(anyhow::anyhow!("SMTP send failed: {e:?}"))
            }
        }
    }

    pub async fn send_verification_code(&self, email: &str, code: &str, lang: &str) -> Result<()> {
        let (subject, body_text, footer_note) = if lang == "en" {
            ("FlowLink Verification Code", "Enter this code to complete sign-in:", "Code is valid for 10 minutes. If you didn't request it, ignore this email.")
        } else {
            ("Код подтверждения FlowLink", "Введите этот код для завершения входа:", "Код действителен 10 минут. Если вы не запрашивали код, проигнорируйте это письмо.")
        };
        let html = format_verification_html(code, lang, body_text, footer_note);
        let text = format!("FlowLink verification code: {code}\n\n{body_text}\n\n{footer_note}");
        self.send_email(email, subject, &html, &text).await
    }

    pub async fn send_welcome(&self, email: &str, name: &str, lang: &str) -> Result<()> {
        self.send_welcome_email1(email, name, lang).await
    }

    pub async fn send_welcome_email1(&self, email: &str, name: &str, lang: &str) -> Result<()> {
        let (subject, text) = if lang == "en" {
            ("Welcome to FlowLink", format!("Hi {name}!\n\nYour FlowLink account has been created.\n\n1. Connect the Telegram bot\n2. Configure your AI agents\n3. Choose a plan\n\nDocs: __BASE_URL__/docs\nSupport: support@flow-masters.ru"))
        } else {
            ("Добро пожаловать в FlowLink", format!("Привет, {name}!\n\nВаш аккаунт FlowLink создан.\n\n1. Подключите Telegram бота\n2. Настройте AI-агентов\n3. Выберите тариф\n\nДокументация: __BASE_URL__/docs\nПоддержка: support@flow-masters.ru"))
        };
        let html = format_welcome1_html(name, lang);
        self.send_email(email, subject, &html, &text).await
    }

    pub async fn send_welcome_email2(&self, email: &str, name: &str, lang: &str) -> Result<()> {
        let (subject, text) = if lang == "en" {
            ("Connect your first server — FlowLink", format!("Hi {name}!\n\nIt's time to connect your first server.\n\n1. Install FlowLink Agent\n2. Get an API key\n3. Connect the agent"))
        } else {
            ("Подключите первый сервер — FlowLink", format!("Привет, {name}!\n\nСамое время подключить первый сервер.\n\n1. Установите FlowLink Agent\n2. Получите API-ключ\n3. Подключите агента"))
        };
        let html = format_welcome2_html(name, lang);
        self.send_email(email, subject, &html, &text).await
    }

    pub async fn send_welcome_email3(&self, email: &str, name: &str, lang: &str) -> Result<()> {
        let (subject, text) = if lang == "en" {
            ("FlowLink trial active", format!("{name}, you've been with us for 3 days!\n\nTry all FlowLink features and choose a plan when ready."))
        } else {
            ("Пробный период FlowLink активен", format!("{name}, вы уже с нами 3 дня!\n\nПопробуйте все возможности FlowLink и выберите подходящий тариф."))
        };
        let html = format_welcome3_html(name, lang);
        self.send_email(email, subject, &html, &text).await
    }

    pub async fn send_payment_success(&self, email: &str, name: &str, plan_name: &str, amount: &str, lang: &str) -> Result<()> {
        let (subject, text) = if lang == "en" {
            ("Payment successful — FlowLink", format!("Thank you, {name}!\n\nPayment received.\nPlan: {plan_name}\nAmount: {amount}"))
        } else {
            ("Оплата прошла успешно — FlowLink", format!("Спасибо, {name}!\n\nОплата получена.\nПлан: {plan_name}\nСумма: {amount}"))
        };
        let html = format_payment_success_html(name, plan_name, amount, lang);
        self.send_email(email, subject, &html, &text).await
    }

    pub async fn send_payment_failed(&self, email: &str, name: &str, plan_name: &str, lang: &str) -> Result<()> {
        let (subject, text) = if lang == "en" {
            ("Payment issue — FlowLink", format!("Hi {name}!\n\nPayment for plan {plan_name} failed. Update your payment method."))
        } else {
            ("Проблема с оплатой — FlowLink", format!("Привет, {name}!\n\nНе удалось списать средства за план {plan_name}. Обновите способ оплаты."))
        };
        let html = format_payment_failed_html(name, plan_name, lang);
        self.send_email(email, subject, &html, &text).await
    }

    pub async fn send_renewal_reminder(&self, email: &str, name: &str, plan_name: &str, renewal_date: &str, lang: &str) -> Result<()> {
        let (subject, text) = if lang == "en" {
            ("Subscription renewal — FlowLink", format!("Hi {name}!\n\nYour {plan_name} subscription renews in 3 days ({renewal_date})."))
        } else {
            ("Продление подписки — FlowLink", format!("Привет, {name}!\n\nЧерез 3 дня продлевается подписка {plan_name} ({renewal_date})."))
        };
        let html = format_renewal_reminder_html(name, plan_name, renewal_date, lang);
        self.send_email(email, subject, &html, &text).await
    }

    pub async fn send_subscription_cancelled(&self, email: &str, name: &str, access_until: &str, lang: &str) -> Result<()> {
        let until = if access_until.is_empty() { if lang == "en" { "end of current period" } else { "окончания текущего периода" } } else { access_until };
        let (subject, text) = if lang == "en" {
            ("Subscription cancelled — FlowLink", format!("Hi {name}!\n\nYour FlowLink subscription is cancelled.\nAccess until: {until}"))
        } else {
            ("Подписка отменена — FlowLink", format!("Привет, {name}!\n\nПодписка на FlowLink отменена.\nДоступ сохранён до: {until}"))
        };
        let html = format_subscription_cancelled_html(name, until, lang);
        self.send_email(email, subject, &html, &text).await
    }

    pub async fn send_new_login(&self, email: &str, name: &str, ip: &str, country: &str, time: &str, lang: &str) -> Result<()> {
        let loc = if country.is_empty() { "—" } else { country };
        let (subject, text) = if lang == "en" {
            ("New sign-in — FlowLink", format!("Hi {name}!\n\nNew sign-in detected.\nIP: {ip}\nCountry: {loc}\nTime: {time}\n\nIf this wasn't you, change your password."))
        } else {
            ("Новый вход в аккаунт — FlowLink", format!("Привет, {name}!\n\nНовый вход в аккаунт.\nIP: {ip}\nСтрана: {loc}\nВремя: {time}\n\nЕсли это не вы — смените пароль."))
        };
        let html = format_new_login_html(name, ip, country, time, lang);
        self.send_email(email, subject, &html, &text).await
    }

    pub async fn send_password_changed(&self, email: &str, name: &str, lang: &str) -> Result<()> {
        let (subject, text) = if lang == "en" {
            ("Password changed — FlowLink", format!("Hi {name}!\n\nYour password has been changed.\nIf this wasn't you, contact support."))
        } else {
            ("Пароль изменён — FlowLink", format!("Привет, {name}!\n\nПароль успешно изменён.\nЕсли это не вы — обратитесь в поддержку."))
        };
        let html = format_password_changed_html(name, lang);
        self.send_email(email, subject, &html, &text).await
    }

    pub async fn send_api_key_created(&self, email: &str, name: &str, key_name: &str, lang: &str) -> Result<()> {
        let (subject, text) = if lang == "en" {
            ("New API key — FlowLink", format!("Hi {name}!\n\nAPI key \"{key_name}\" created.\nIf this wasn't you, delete it and contact support."))
        } else {
            ("Новый API-ключ — FlowLink", format!("Привет, {name}!\n\nAPI-ключ \"{key_name}\" создан.\nЕсли это не вы — удалите ключ и обратитесь в поддержку."))
        };
        let html = format_api_key_html(name, key_name, true, lang);
        self.send_email(email, subject, &html, &text).await
    }

    pub async fn send_api_key_deleted(&self, email: &str, name: &str, key_name: &str, lang: &str) -> Result<()> {
        let (subject, text) = if lang == "en" {
            ("API key deleted — FlowLink", format!("Hi {name}!\n\nAPI key \"{key_name}\" deleted."))
        } else {
            ("API-ключ удалён — FlowLink", format!("Привет, {name}!\n\nAPI-ключ \"{key_name}\" удалён."))
        };
        let html = format_api_key_html(name, key_name, false, lang);
        self.send_email(email, subject, &html, &text).await
    }

    pub async fn send_plan_changed(&self, email: &str, name: &str, old_plan: &str, new_plan: &str, lang: &str) -> Result<()> {
        let (subject, text) = if lang == "en" {
            ("Plan changed — FlowLink", format!("Hi {name}!\n\nPlan changed: {old_plan} → {new_plan}\n\nEffective immediately."))
        } else {
            ("Тарифный план изменён — FlowLink", format!("Привет, {name}!\n\nПлан изменён: {old_plan} → {new_plan}\n\nВступает в силу немедленно."))
        };
        let html = format_plan_changed_html(name, old_plan, new_plan, lang);
        self.send_email(email, subject, &html, &text).await
    }
}

// ═══════════════════════════════════════════════════════════════
// Templates — dark theme, bilingual
// ═══════════════════════════════════════════════════════════════

fn fmt_email(lang: &str, header_color: &str, sub: &str, body_html: &str) -> String {
    fmt_email_with_base(lang, header_color, sub, body_html, "__BASE_URL__")
}

fn fmt_email_with_base(lang: &str, header_color: &str, sub: &str, body_html: &str, base_url: &str) -> String {
    let docs = if lang == "en" { "Documentation" } else { "Документация" };
    format!(r#"<!DOCTYPE html>
<html lang="{lang}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1.0"></head>
<body style="margin:0;padding:0;background:#0a0a0a;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">
<table width="100%" cellpadding="0" cellspacing="0" style="padding:40px 0;"><tr><td align="center">
<table width="520" cellpadding="0" cellspacing="0" style="background:#111;border:1px solid #1a1a1a;border-radius:16px;overflow:hidden;">
<tr><td style="background:linear-gradient(135deg,{hc});padding:28px 32px;text-align:center;">
<h1 style="margin:0;color:#fff;font-size:22px;font-weight:700;letter-spacing:-0.02em;">FlowLink</h1>
<p style="margin:6px 0 0;color:rgba(255,255,255,.8);font-size:13px;font-weight:500;">{sub}</p>
</td></tr>
<tr><td style="padding:36px 32px;color:#ededed;">{body}</td></tr>
<tr><td style="padding:20px 32px;border-top:1px solid #1a1a1a;text-align:center;">
<p style="margin:0 0 10px;font-size:11px;color:#555;line-height:1.8;">
<a href="__BASE_URL__/docs" style="color:#0070f3;text-decoration:none;">{docs}</a>
&nbsp;&middot;&nbsp;
<a href="mailto:support@flow-masters.ru" style="color:#0070f3;text-decoration:none;">support@flow-masters.ru</a>
&nbsp;&middot;&nbsp;
<a href="https://t.me/flowlink_ai_sales_bot" style="color:#0070f3;text-decoration:none;">Telegram</a>
</p>
<p style="margin:0;font-size:10px;color:#333;">FlowLink &middot; AI Agent Security Gateway &middot; <a href="__BASE_URL__" style="color:#333;text-decoration:none;">FlowLink</a></p>
</td></tr>
</table>
</td></tr></table>
</body></html>"#, hc = header_color, sub = sub, body = body_html, lang = lang, docs = docs).replace("__BASE_URL__", base_url)
}

fn fmt_verification_standalone(code: &str, lang: &str, body_text: &str, footer_note: &str) -> String {
    let (header_sub, docs) = if lang == "en" { ("Verification Code", "Documentation") } else { ("Код подтверждения", "Документация") };
    format!(r#"<!DOCTYPE html>
<html lang="{lang}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1.0"></head>
<body style="margin:0;padding:0;background:#0a0a0a;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">
<table width="100%" cellpadding="0" cellspacing="0" style="padding:40px 0;"><tr><td align="center">
<table width="520" cellpadding="0" cellspacing="0" style="background:#111;border:1px solid #1a1a1a;border-radius:16px;overflow:hidden;">
<tr><td style="background:linear-gradient(135deg,#6366f1,#8b5cf6);padding:28px 32px;text-align:center;">
<h1 style="margin:0;color:#fff;font-size:22px;font-weight:700;letter-spacing:-0.02em;">FlowLink</h1>
<p style="margin:6px 0 0;color:rgba(255,255,255,.8);font-size:13px;font-weight:500;">{header_sub}</p>
</td></tr>
<tr><td style="padding:40px 32px;text-align:center;">
<p style="margin:0 0 28px;color:#888;font-size:15px;">{body_text}</p>
<div style="background:#0a0a0a;border:1px solid #222;border-radius:12px;padding:20px 36px;display:inline-block;">
<span style="font-size:36px;font-weight:700;letter-spacing:10px;color:#fff;">{code}</span>
</div>
<p style="margin:28px 0 0;color:#555;font-size:13px;">{footer_note}</p>
</td></tr>
<tr><td style="padding:20px 32px;border-top:1px solid #1a1a1a;text-align:center;">
<p style="margin:0 0 10px;font-size:11px;color:#555;line-height:1.8;">
<a href="__BASE_URL__/docs" style="color:#0070f3;text-decoration:none;">{docs}</a>
&nbsp;&middot;&nbsp;
<a href="mailto:support@flow-masters.ru" style="color:#0070f3;text-decoration:none;">support@flow-masters.ru</a>
&nbsp;&middot;&nbsp;
<a href="https://t.me/flowlink_ai_sales_bot" style="color:#0070f3;text-decoration:none;">Telegram</a>
</p>
<p style="margin:0;font-size:10px;color:#333;">FlowLink &middot; AI Agent Security Gateway &middot; <a href="__BASE_URL__" style="color:#333;text-decoration:none;">flow-masters.ru</a></p>
</td></tr>
</table>
</td></tr></table>
</body></html>"#, code = code, lang = lang, header_sub = header_sub, body_text = body_text, footer_note = footer_note, docs = docs)
}

fn format_verification_html(code: &str, lang: &str, body_text: &str, footer_note: &str) -> String {
    fmt_verification_standalone(code, lang, body_text, footer_note)
}

fn format_welcome1_html(name: &str, lang: &str) -> String {
    let (sub, greeting, desc, s1, s2, s3, cta_text) = if lang == "en" {
        ("Welcome", "Hi, {name}!", "Your FlowLink account has been created. Next steps:",
         "Connect the Telegram bot for notifications", "Configure your AI agents", "Choose a plan", "Open documentation")
    } else {
        ("Добро пожаловать", "Привет, {name}!", "Ваш аккаунт FlowLink создан. Вот что можно сделать дальше:",
         "Подключите Telegram бота для уведомлений", "Настройте своих AI-агентов", "Выберите тарифный план", "Открыть документацию")
    };
    let greeting = greeting.replace("{name}", name);
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#fff;font-size:20px;font-weight:600;">{greeting}</h2>
<p style="margin:0 0 24px;color:#888;font-size:15px;line-height:1.7;">{desc}</p>
<table width="100%" cellpadding="0" cellspacing="0" style="margin-bottom:28px;">
<tr><td style="padding:10px 0;border-bottom:1px solid #1a1a1a;color:#ededed;font-size:14px;">
<span style="color:#0070f3;font-weight:600;margin-right:10px;">1</span>{s1}
</td></tr>
<tr><td style="padding:10px 0;border-bottom:1px solid #1a1a1a;color:#ededed;font-size:14px;">
<span style="color:#0070f3;font-weight:600;margin-right:10px;">2</span>{s2}
</td></tr>
<tr><td style="padding:10px 0;color:#ededed;font-size:14px;">
<span style="color:#0070f3;font-weight:600;margin-right:10px;">3</span>{s3}
</td></tr>
</table>
<div style="background:#0070f3/8;border:1px solid #0070f3/20;border-radius:10px;padding:16px;text-align:center;">
<a href="__BASE_URL__/docs" style="color:#0070f3;font-size:14px;font-weight:600;text-decoration:none;">{cta_text} →</a>
</div>"#, greeting = greeting, desc = desc, s1 = s1, s2 = s2, s3 = s3, cta_text = cta_text);
    fmt_email(lang, "#6366f1,#8b5cf6", sub, &body)
}

fn format_welcome2_html(name: &str, lang: &str) -> String {
    let (sub, greeting, desc, st1, st2, st3, cta_text) = if lang == "en" {
        ("Connect your first server", "Hi, {name}!", "It's time to connect your first server to FlowLink:",
         "Install FlowLink Agent on your server", "Get an API key from the dashboard", "Connect the agent to your workflow", "Full documentation")
    } else {
        ("Подключите первый сервер", "Привет, {name}!", "Самое время подключить первый сервер к FlowLink:",
         "Установите FlowLink Agent на сервер", "Получите API-ключ в личном кабинете", "Подключите агента к рабочему процессу", "Подробная документация")
    };
    let greeting = greeting.replace("{name}", name);
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#fff;font-size:20px;font-weight:600;">{greeting}</h2>
<p style="margin:0 0 24px;color:#888;font-size:15px;line-height:1.7;">{desc}</p>
<div style="background:#0a0a0a;border:1px solid #1a1a1a;border-radius:10px;padding:20px;margin-bottom:24px;">
<table width="100%" cellpadding="0" cellspacing="0">
<tr><td style="padding:6px 0;color:#ededed;font-size:14px;"><span style="color:#0070f3;font-weight:700;margin-right:8px;">01</span>{st1}</td></tr>
<tr><td style="padding:6px 0;color:#ededed;font-size:14px;"><span style="color:#0070f3;font-weight:700;margin-right:8px;">02</span>{st2}</td></tr>
<tr><td style="padding:6px 0;color:#ededed;font-size:14px;"><span style="color:#0070f3;font-weight:700;margin-right:8px;">03</span>{st3}</td></tr>
</table>
</div>
<p style="margin:0;color:#888;font-size:14px;"><a href="__BASE_URL__/docs/getting-started/" style="color:#0070f3;text-decoration:none;font-weight:500;">{cta_text} →</a></p>"#, greeting = greeting, desc = desc, st1 = st1, st2 = st2, st3 = st3, cta_text = cta_text);
    fmt_email(lang, "#3b82f6,#6366f1", sub, &body)
}

fn format_welcome3_html(name: &str, lang: &str) -> String {
    let (sub, title, desc, note, cta_text) = if lang == "en" {
        ("Trial period active", "{name}, you've been with us for 3 days!",
         "The trial period lets you try all FlowLink features without limits.",
         "Choose a plan when you're ready", "View plans")
    } else {
        ("Пробный период активен", "{name}, вы уже с нами 3 дня!",
         "Пробный период позволяет попробовать все возможности FlowLink без ограничений.",
         "Выберите подходящий тарифный план, когда будете готовы", "Смотреть тарифы")
    };
    let title = title.replace("{name}", name);
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#fff;font-size:20px;font-weight:600;">{title}</h2>
<p style="margin:0 0 24px;color:#888;font-size:15px;line-height:1.7;">{desc}</p>
<div style="background:#f59e0b/8;border:1px solid #f59e0b/20;border-radius:10px;padding:16px 20px;margin-bottom:24px;">
<p style="margin:0;color:#f59e0b;font-size:14px;font-weight:500;">{note}</p>
</div>
<p style="margin:0;color:#888;font-size:14px;"><a href="__BASE_URL__/pricing" style="color:#0070f3;text-decoration:none;font-weight:500;">{cta_text} →</a></p>"#, title = title, desc = desc, note = note, cta_text = cta_text);
    fmt_email(lang, "#f59e0b,#d97706", sub, &body)
}

fn format_payment_success_html(name: &str, plan_name: &str, amount: &str, lang: &str) -> String {
    let (sub, greeting, msg, plan_label, amount_label, footnote) = if lang == "en" {
        ("Payment successful", "Thank you, {name}!", "Payment received. Subscription activated.", "Plan", "Amount", "Enjoy FlowLink!")
    } else {
        ("Оплата прошла успешно", "Спасибо, {name}!", "Оплата получена. Подписка активирована.", "План", "Сумма", "Приятного использования FlowLink!")
    };
    let greeting = greeting.replace("{name}", name);
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#fff;font-size:20px;font-weight:600;">{greeting}</h2>
<p style="margin:0 0 24px;color:#888;font-size:15px;">{msg}</p>
<table width="100%" cellpadding="12" cellspacing="0" style="background:#0a0a0a;border:1px solid #1a1a1a;border-radius:10px;margin-bottom:24px;">
<tr><td style="color:#888;font-size:14px;border-bottom:1px solid #1a1a1a;">{plan_label}</td><td style="color:#fff;font-size:14px;font-weight:600;text-align:right;border-bottom:1px solid #1a1a1a;">{plan_name}</td></tr>
<tr><td style="color:#888;font-size:14px;">{amount_label}</td><td style="color:#fff;font-size:14px;font-weight:600;text-align:right;">{amount}</td></tr>
</table>
<p style="margin:0;color:#555;font-size:13px;">{footnote}</p>"#, greeting = greeting, msg = msg, plan_label = plan_label, amount_label = amount_label, plan_name = plan_name, amount = amount, footnote = footnote);
    fmt_email(lang, "#10b981,#059669", sub, &body)
}

fn format_payment_failed_html(name: &str, plan_name: &str, lang: &str) -> String {
    let (sub, greeting, alert, cta_text) = if lang == "en" {
        ("Payment issue", "Hi, {name}!",
         "Payment for plan <strong style=\"color:#fff;\">{plan_name}</strong> failed. Update your payment method to avoid service suspension.",
         "Go to billing")
    } else {
        ("Проблема с оплатой", "Привет, {name}!",
         "Не удалось списать средства за план <strong style=\"color:#fff;\">{plan_name}</strong>. Обновите способ оплаты, чтобы избежать приостановки сервиса.",
         "Перейти в биллинг")
    };
    let greeting = greeting.replace("{name}", name);
    let alert = alert.replace("{plan_name}", plan_name);
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#fff;font-size:20px;font-weight:600;">{greeting}</h2>
<div style="background:#ef4444/8;border-left:3px solid #ef4444;border-radius:0 10px 10px 0;padding:16px;margin-bottom:24px;">
<p style="margin:0;color:#fca5a5;font-size:14px;line-height:1.7;">{alert}</p>
</div>
<p style="margin:0;color:#888;font-size:14px;"><a href="__BASE_URL__/dashboard/billing" style="color:#0070f3;text-decoration:none;font-weight:500;">{cta_text} →</a></p>"#, greeting = greeting, alert = alert, cta_text = cta_text);
    fmt_email(lang, "#ef4444,#dc2626", sub, &body)
}

fn format_renewal_reminder_html(name: &str, plan_name: &str, renewal_date: &str, lang: &str) -> String {
    let (sub, greeting, desc, plan_label, date_label, footnote) = if lang == "en" {
        ("Subscription renewal", "Hi, {name}!", "Your subscription renews in 3 days:", "Plan", "Renewal date", "Make sure your payment method is up to date.")
    } else {
        ("Продление подписки", "Привет, {name}!", "Через 3 дня продлевается ваша подписка:", "План", "Дата продления", "Убедитесь, что способ оплаты актуален.")
    };
    let greeting = greeting.replace("{name}", name);
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#fff;font-size:20px;font-weight:600;">{greeting}</h2>
<p style="margin:0 0 24px;color:#888;font-size:15px;line-height:1.7;">{desc}</p>
<table width="100%" cellpadding="12" cellspacing="0" style="background:#0a0a0a;border:1px solid #1a1a1a;border-radius:10px;margin-bottom:24px;">
<tr><td style="color:#888;font-size:14px;border-bottom:1px solid #1a1a1a;">{plan_label}</td><td style="color:#fff;font-size:14px;font-weight:600;text-align:right;border-bottom:1px solid #1a1a1a;">{plan_name}</td></tr>
<tr><td style="color:#888;font-size:14px;">{date_label}</td><td style="color:#fff;font-size:14px;font-weight:600;text-align:right;">{renewal_date}</td></tr>
</table>
<p style="margin:0;color:#888;font-size:14px;">{footnote}</p>"#, greeting = greeting, desc = desc, plan_label = plan_label, date_label = date_label, plan_name = plan_name, renewal_date = renewal_date, footnote = footnote);
    fmt_email(lang, "#6366f1,#8b5cf6", sub, &body)
}

fn format_subscription_cancelled_html(name: &str, access_until: &str, lang: &str) -> String {
    let (sub, greeting, desc, access_text, restore_text) = if lang == "en" {
        ("Subscription cancelled", "Hi, {name}!", "Your FlowLink subscription has been cancelled.",
         "Access is preserved until: <strong style=\"color:#fff;\">{until}</strong>", "You can reactivate anytime from your <a href=\"__BASE_URL__/dashboard/billing\" style=\"color:#0070f3;text-decoration:none;font-weight:500;\">dashboard</a>.")
    } else {
        ("Подписка отменена", "Привет, {name}!", "Подписка на FlowLink отменена по вашему запросу.",
         "Доступ к сервису сохранён до: <strong style=\"color:#fff;\">{until}</strong>", "Возобновить подписку можно в любой момент из <a href=\"__BASE_URL__/dashboard/billing\" style=\"color:#0070f3;text-decoration:none;font-weight:500;\">личного кабинета</a>.")
    };
    let greeting = greeting.replace("{name}", name);
    let until_display = if access_until.is_empty() { if lang == "en" { "end of current period" } else { "окончания текущего периода" } } else { access_until };
    let access_text = access_text.replace("{until}", until_display);
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#fff;font-size:20px;font-weight:600;">{greeting}</h2>
<p style="margin:0 0 24px;color:#888;font-size:15px;line-height:1.7;">{desc}</p>
<div style="background:#0a0a0a;border:1px solid #1a1a1a;border-radius:10px;padding:16px;margin-bottom:24px;">
<p style="margin:0;color:#ededed;font-size:14px;">{access_text}</p>
</div>
<p style="margin:0;color:#888;font-size:14px;">{restore_text}</p>"#, greeting = greeting, desc = desc, access_text = access_text, restore_text = restore_text);
    fmt_email(lang, "#64748b,#475569", sub, &body)
}

fn format_new_login_html(name: &str, ip: &str, country: &str, time: &str, lang: &str) -> String {
    let location = if !country.is_empty() { country.to_string() } else { "—".to_string() };
    let (sub, greeting, desc, ip_label, country_label, time_label, warning) = if lang == "en" {
        ("New sign-in", "Hi, {name}!", "New sign-in to your FlowLink account detected:",
         "IP Address", "Country", "Time",
         "If this wasn't you — change your password immediately and contact <a href=\"mailto:support@flow-masters.ru\" style=\"color:#0070f3;text-decoration:none;\">support</a>.")
    } else {
        ("Новый вход в аккаунт", "Привет, {name}!", "Зафиксирован новый вход в ваш аккаунт FlowLink:",
         "IP-адрес", "Страна", "Время",
         "Если это не вы — немедленно смените пароль и обратитесь в <a href=\"mailto:support@flow-masters.ru\" style=\"color:#0070f3;text-decoration:none;\">поддержку</a>.")
    };
    let greeting = greeting.replace("{name}", name);
    let ip_display = if ip.is_empty() { "unknown" } else { ip };
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#fff;font-size:20px;font-weight:600;">{greeting}</h2>
<p style="margin:0 0 24px;color:#888;font-size:15px;">{desc}</p>
<table width="100%" cellpadding="12" cellspacing="0" style="background:#0a0a0a;border:1px solid #1a1a1a;border-radius:10px;margin-bottom:24px;">
<tr><td style="color:#888;font-size:14px;border-bottom:1px solid #1a1a1a;">{ip_label}</td><td style="color:#fff;font-size:14px;font-weight:600;text-align:right;font-family:monospace;">{ip_display}</td></tr>
<tr><td style="color:#888;font-size:14px;border-bottom:1px solid #1a1a1a;">{country_label}</td><td style="color:#fff;font-size:14px;font-weight:600;text-align:right;">{location}</td></tr>
<tr><td style="color:#888;font-size:14px;">{time_label}</td><td style="color:#fff;font-size:14px;font-weight:600;text-align:right;">{time}</td></tr>
</table>
<div style="background:#ef4444/8;border-left:3px solid #ef4444;border-radius:0 10px 10px 0;padding:14px 16px;margin-bottom:24px;">
<p style="margin:0;color:#fca5a5;font-size:14px;">{warning}</p>
</div>"#, greeting = greeting, desc = desc, ip_label = ip_label, country_label = country_label, time_label = time_label, ip_display = ip_display, location = location, time = time, warning = warning);
    fmt_email(lang, "#6366f1,#8b5cf6", sub, &body)
}

fn format_password_changed_html(name: &str, lang: &str) -> String {
    let (sub, greeting, desc, confirm, footnote) = if lang == "en" {
        ("Password changed", "Hi, {name}!", "Your FlowLink password has been changed.", "Change confirmed",
         "If you didn't change your password — contact <a href=\"mailto:support@flow-masters.ru\" style=\"color:#0070f3;text-decoration:none;\">support</a> immediately.")
    } else {
        ("Пароль изменён", "Привет, {name}!", "Пароль вашего аккаунта FlowLink успешно изменён.", "Изменение подтверждено",
         "Если вы не меняли пароль — немедленно обратитесь в <a href=\"mailto:support@flow-masters.ru\" style=\"color:#0070f3;text-decoration:none;\">поддержку</a>.")
    };
    let greeting = greeting.replace("{name}", name);
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#fff;font-size:20px;font-weight:600;">{greeting}</h2>
<p style="margin:0 0 24px;color:#888;font-size:15px;line-height:1.7;">{desc}</p>
<div style="background:#10b981/8;border-left:3px solid #10b981;border-radius:0 10px 10px 0;padding:14px 16px;margin-bottom:24px;">
<p style="margin:0;color:#6ee7b7;font-size:14px;">{confirm}</p>
</div>
<p style="margin:0;color:#555;font-size:13px;">{footnote}</p>"#, greeting = greeting, desc = desc, confirm = confirm, footnote = footnote);
    fmt_email(lang, "#22c55e,#16a34a", sub, &body)
}

fn format_api_key_html(name: &str, key_name: &str, created: bool, lang: &str) -> String {
    let (action, color, sub, greeting, desc, footnote) = if created {
        if lang == "en" {
            ("created", "#6366f1,#8b5cf6", "New API key",
             "Hi, {name}!", "API key <strong style=\"color:#fff;\">{key_name}</strong> created in your account.",
             "If this wasn't you — delete the key and contact <a href=\"mailto:support@flow-masters.ru\" style=\"color:#0070f3;text-decoration:none;\">support</a>.")
        } else {
            ("создан", "#6366f1,#8b5cf6", "Новый API-ключ",
             "Привет, {name}!", "API-ключ <strong style=\"color:#fff;\">{key_name}</strong> создан в вашем аккаунте.",
             "Если это не вы — удалите ключ и обратитесь в <a href=\"mailto:support@flow-masters.ru\" style=\"color:#0070f3;text-decoration:none;\">поддержку</a>.")
        }
    } else {
        if lang == "en" {
            ("deleted", "#64748b,#475569", "API key deleted",
             "Hi, {name}!", "API key <strong style=\"color:#fff;\">{key_name}</strong> deleted from your account.",
             "If this wasn't you — contact <a href=\"mailto:support@flow-masters.ru\" style=\"color:#0070f3;text-decoration:none;\">support</a>.")
        } else {
            ("удалён", "#64748b,#475569", "API-ключ удалён",
             "Привет, {name}!", "API-ключ <strong style=\"color:#fff;\">{key_name}</strong> удалён из вашего аккаунта.",
             "Если это не вы — обратитесь в <a href=\"mailto:support@flow-masters.ru\" style=\"color:#0070f3;text-decoration:none;\">поддержку</a>.")
        }
    };
    let greeting = greeting.replace("{name}", name);
    let desc = desc.replace("{key_name}", key_name);
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#fff;font-size:20px;font-weight:600;">{greeting}</h2>
<p style="margin:0 0 24px;color:#888;font-size:15px;">{desc}</p>
<p style="margin:0;color:#555;font-size:13px;">{footnote}</p>"#, greeting = greeting, desc = desc, footnote = footnote);
    fmt_email(lang, color, sub, &body)
}

fn format_plan_changed_html(name: &str, old_plan: &str, new_plan: &str, lang: &str) -> String {
    let (sub, greeting, desc, footnote) = if lang == "en" {
        ("Plan changed", "Hi, {name}!", "Your FlowLink plan has been changed:", "Changes take effect immediately.")
    } else {
        ("Тарифный план изменён", "Привет, {name}!", "Ваш тарифный план FlowLink изменён:", "Изменения вступают в силу немедленно.")
    };
    let greeting = greeting.replace("{name}", name);
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#fff;font-size:20px;font-weight:600;">{greeting}</h2>
<p style="margin:0 0 24px;color:#888;font-size:15px;">{desc}</p>
<div style="background:#0a0a0a;border:1px solid #1a1a1a;border-radius:10px;padding:20px;margin-bottom:24px;text-align:center;">
<p style="margin:0;color:#888;font-size:16px;">{old_plan} <span style="color:#0070f3;font-size:20px;margin:0 8px;">→</span> {new_plan}</p>
</div>
<p style="margin:0;color:#555;font-size:13px;">{footnote}</p>"#, greeting = greeting, desc = desc, old_plan = old_plan, new_plan = new_plan, footnote = footnote);
    fmt_email(lang, "#6366f1,#8b5cf6", sub, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_ru() {
        let html = format_verification_html("123456", "ru", "Введите этот код", "Код действителен");
        assert!(html.contains("123456"));
        assert!(html.contains("Введите этот код"));
        assert!(html.contains("Код подтверждения"));
        assert!(html.contains("Документация"));
        assert!(html.contains(r#"lang="ru""#));
    }

    #[test]
    fn verification_en() {
        let html = format_verification_html("654321", "en", "Enter this code", "Code is valid");
        assert!(html.contains("654321"));
        assert!(html.contains("Enter this code"));
        assert!(html.contains("Verification Code"));
        assert!(html.contains("Documentation"));
        assert!(html.contains(r#"lang="en""#));
        assert!(!html.contains("Код подтверждения"));
    }

    #[test]
    fn welcome1_ru() {
        let html = format_welcome1_html("Алексей", "ru");
        assert!(html.contains("Алексей"));
        assert!(html.contains("Добро пожаловать"));
        assert!(html.contains("Telegram бота"));
    }

    #[test]
    fn welcome1_en() {
        let html = format_welcome1_html("John", "en");
        assert!(html.contains("John"));
        assert!(html.contains("Welcome"));
        assert!(html.contains("Telegram bot"));
        assert!(!html.contains("Добро пожаловать"));
    }

    #[test]
    fn welcome2_ru() {
        let html = format_welcome2_html("Иван", "ru");
        assert!(html.contains("Подключите первый сервер"));
        assert!(html.contains("API-ключ"));
    }

    #[test]
    fn welcome2_en() {
        let html = format_welcome2_html("Jane", "en");
        assert!(html.contains("Connect your first server"));
        assert!(html.contains("API key"));
        assert!(!html.contains("Подключите первый сервер"));
    }

    #[test]
    fn welcome3_ru() {
        let html = format_welcome3_html("Олег", "ru");
        assert!(html.contains("Пробный период активен"));
        assert!(html.contains("Смотреть тарифы"));
    }

    #[test]
    fn welcome3_en() {
        let html = format_welcome3_html("Bob", "en");
        assert!(html.contains("Trial period active"));
        assert!(html.contains("View plans"));
        assert!(!html.contains("Пробный период"));
    }

    #[test]
    fn payment_success_ru() {
        let html = format_payment_success_html("Маша", "Professional", "1 990 ₽", "ru");
        assert!(html.contains("Оплата прошла успешно"));
        assert!(html.contains("Сумма"));
    }

    #[test]
    fn payment_success_en() {
        let html = format_payment_success_html("Alice", "Scale", "$49", "en");
        assert!(html.contains("Payment successful"));
        assert!(html.contains("Amount"));
        assert!(!html.contains("Оплата"));
    }

    #[test]
    fn payment_failed_ru() {
        let html = format_payment_failed_html("Петр", "Scale", "ru");
        assert!(html.contains("Проблема с оплатой"));
    }

    #[test]
    fn payment_failed_en() {
        let html = format_payment_failed_html("Dave", "Enterprise", "en");
        assert!(html.contains("Payment issue"));
        assert!(!html.contains("Проблема"));
    }

    #[test]
    fn renewal_reminder_ru() {
        let html = format_renewal_reminder_html("Анна", "Professional", "2025-02-01", "ru");
        assert!(html.contains("Продление подписки"));
        assert!(html.contains("Дата продления"));
    }

    #[test]
    fn renewal_reminder_en() {
        let html = format_renewal_reminder_html("Tom", "Scale", "2025-03-15", "en");
        assert!(html.contains("Subscription renewal"));
        assert!(html.contains("Renewal date"));
        assert!(!html.contains("Продление"));
    }

    #[test]
    fn subscription_cancelled_ru() {
        let html = format_subscription_cancelled_html("Сергей", "2025-03-01", "ru");
        assert!(html.contains("Подписка отменена"));
        assert!(html.contains("2025-03-01"));
    }

    #[test]
    fn subscription_cancelled_en() {
        let html = format_subscription_cancelled_html("Kate", "2025-04-01", "en");
        assert!(html.contains("Subscription cancelled"));
        assert!(!html.contains("Подписка отменена"));
    }

    #[test]
    fn subscription_cancelled_empty_until_ru() {
        let html = format_subscription_cancelled_html("Тест", "", "ru");
        assert!(html.contains("окончания текущего периода"));
    }

    #[test]
    fn subscription_cancelled_empty_until_en() {
        let html = format_subscription_cancelled_html("Test", "", "en");
        assert!(html.contains("end of current period"));
    }

    #[test]
    fn new_login_ru_with_country() {
        let html = format_new_login_html("Вася", "1.2.3.4", "RU", "2025-01-15 10:00", "ru");
        assert!(html.contains("IP-адрес"));
        assert!(html.contains("Страна"));
        assert!(html.contains("RU"));
        // Verify IP is displayed in monospace table cell
        assert!(html.contains("monospace"));
    }

    #[test]
    fn new_login_en_with_country() {
        let html = format_new_login_html("Mike", "10.0.0.1", "US", "2025-06-01", "en");
        assert!(html.contains("IP Address"));
        assert!(html.contains("Country"));
        assert!(html.contains("New sign-in"));
        assert!(!html.contains("IP-адрес"));
    }

    #[test]
    fn new_login_empty_ip_shows_unknown() {
        let html = format_new_login_html("X", "", "RU", "now", "en");
        assert!(html.contains("unknown"));
    }

    #[test]
    fn new_login_empty_country_shows_dash() {
        let html = format_new_login_html("X", "1.1.1.1", "", "now", "ru");
        assert!(html.contains("—"));
    }

    #[test]
    fn new_login_both_empty() {
        let html = format_new_login_html("X", "", "", "now", "en");
        assert!(html.contains("unknown"));
        assert!(html.contains("—"));
    }

    #[test]
    fn new_login_ipv6() {
        let html = format_new_login_html("Test", "2a02:6ea0:d50c:1::2", "DE", "now", "en");
        assert!(html.contains("2a02:6ea0:d50c:1::2"));
        assert!(html.contains("DE"));
    }

    #[test]
    fn password_changed_ru() {
        let html = format_password_changed_html("Оля", "ru");
        assert!(html.contains("Пароль изменён"));
        assert!(html.contains("Изменение подтверждено"));
    }

    #[test]
    fn password_changed_en() {
        let html = format_password_changed_html("Mark", "en");
        assert!(html.contains("Password changed"));
        assert!(html.contains("Change confirmed"));
        assert!(!html.contains("Пароль"));
    }

    #[test]
    fn api_key_created_ru() {
        let html = format_api_key_html("Дима", "prod-key", true, "ru");
        assert!(html.contains("Новый API-ключ"));
        assert!(html.contains("prod-key"));
    }

    #[test]
    fn api_key_created_en() {
        let html = format_api_key_html("Lisa", "dev-key", true, "en");
        assert!(html.contains("New API key"));
        assert!(!html.contains("Новый API-ключ"));
    }

    #[test]
    fn api_key_deleted_ru() {
        let html = format_api_key_html("Никита", "old-key", false, "ru");
        assert!(html.contains("API-ключ удалён"));
    }

    #[test]
    fn api_key_deleted_en() {
        let html = format_api_key_html("Sarah", "stale-key", false, "en");
        assert!(html.contains("API key deleted"));
        assert!(!html.contains("удалён"));
    }

    #[test]
    fn plan_changed_ru() {
        let html = format_plan_changed_html("Женя", "Starter", "Professional", "ru");
        assert!(html.contains("Тарифный план изменён"));
        assert!(html.contains("→"));
    }

    #[test]
    fn plan_changed_en() {
        let html = format_plan_changed_html("Chris", "Professional", "Scale", "en");
        assert!(html.contains("Plan changed"));
        assert!(!html.contains("Тарифный план"));
    }

    #[test]
    fn all_templates_dark_theme_and_footer() {
        let templates: Vec<String> = vec![
            format_verification_html("000000", "ru", "t", "t"),
            format_welcome1_html("N", "en"),
            format_welcome2_html("N", "ru"),
            format_welcome3_html("N", "en"),
            format_payment_success_html("N", "P", "$0", "ru"),
            format_payment_failed_html("N", "P", "en"),
            format_renewal_reminder_html("N", "P", "D", "ru"),
            format_subscription_cancelled_html("N", "D", "en"),
            format_new_login_html("N", "1.1.1.1", "RU", "T", "ru"),
            format_password_changed_html("N", "en"),
            format_api_key_html("N", "K", true, "ru"),
            format_api_key_html("N", "K", false, "en"),
            format_plan_changed_html("N", "A", "B", "ru"),
        ];
        for html in &templates {
            assert!(html.contains("#0a0a0a"), "missing dark bg");
            assert!(html.contains("#111"), "missing card bg");
            assert!(html.contains("support@flow-masters.ru"), "missing support");
            assert!(html.contains("t.me/flowlink_ai_sales_bot"), "missing telegram");
            assert!(html.contains("flow-masters.ru/docs"), "missing docs");
            assert!(html.contains("AI Agent Security Gateway"), "missing branding");
        }
    }

    #[test]
    fn footer_ru_label() {
        let html = fmt_email("ru", "#000,#000", "Test", "<p>body</p>");
        assert!(html.contains("Документация"));
        assert!(html.contains(r#"lang="ru""#));
    }

    #[test]
    fn footer_en_label() {
        let html = fmt_email("en", "#000,#000", "Test", "<p>body</p>");
        assert!(html.contains("Documentation"));
        assert!(html.contains(r#"lang="en""#));
    }

    #[test]
    fn unknown_lang_defaults_to_ru() {
        let html = format_welcome1_html("X", "fr");
        assert!(html.contains("Добро пожаловать"));
    }

    #[test]
    fn empty_lang_defaults_to_ru() {
        let html = format_welcome1_html("X", "");
        assert!(html.contains("Добро пожаловать"));
    }

    #[test]
    fn special_chars_in_name() {
        let html = format_new_login_html("O'Brien", "1.1.1.1", "IE", "now", "en");
        assert!(html.contains("O'Brien"));
    }

    #[test]
    fn empty_name_renders() {
        let html = format_welcome1_html("", "en");
        assert!(html.contains("Welcome"));
    }

    #[test]
    fn verification_purple_gradient() {
        assert!(format_verification_html("0", "ru", "t", "t").contains("#6366f1,#8b5cf6"));
    }

    #[test]
    fn payment_success_green_gradient() {
        assert!(format_payment_success_html("N", "P", "$0", "en").contains("#10b981,#059669"));
    }

    #[test]
    fn payment_failed_red_gradient() {
        assert!(format_payment_failed_html("N", "P", "en").contains("#ef4444,#dc2626"));
    }

    #[test]
    fn password_changed_green_gradient() {
        assert!(format_password_changed_html("N", "en").contains("#22c55e,#16a34a"));
    }

    #[test]
    fn welcome3_yellow_gradient() {
        assert!(format_welcome3_html("N", "en").contains("#f59e0b,#d97706"));
    }

    #[test]
    fn api_key_deleted_gray_gradient() {
        assert!(format_api_key_html("N", "K", false, "en").contains("#64748b,#475569"));
    }
}

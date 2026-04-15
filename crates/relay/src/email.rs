//! Email service — SMTP sending via Postal
//!
//! Supports verification codes, welcome emails, payment receipts, and
//! a full chain of transactional emails (security, billing, onboarding).

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
    /// Create email service from SMTP config
    pub fn new(host: &str, port: u16, username: &str, password: &str, from: &str) -> Result<Self> {
        let creds = Credentials::new(username.to_string(), password.to_string());
        // Use plain SMTP (no TLS) for local Postal relay on port 25
        let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(port)
            .credentials(creds)
            .build();

        Ok(Self {
            transport: Arc::new(transport),
            from: from.to_string(),
        })
    }

    /// Create a no-op email service (logs instead of sending)
    #[allow(dead_code)]
    pub fn noop(from: &str) -> Self {
        Self::new("localhost", 587, "", "", from).unwrap_or_else(|_| Self {
            transport: Arc::new(AsyncSmtpTransport::<Tokio1Executor>::relay("localhost").unwrap_or_else(|_| AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("localhost")).build()),
            from: from.to_string(),
        })
    }

    /// Send an email with both HTML and plain text parts
    pub async fn send_email(&self, to: &str, subject: &str, html_body: &str, text_body: &str) -> Result<()> {
        let email = Message::builder()
            .from(self.from.parse().context("Invalid from address")?)
            .to(to.parse().context("Invalid to address")?)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text_body.to_string()),
                    )
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html_body.to_string()),
                    ),
            )?;

        self.transport.send(email).await
            .map_err(|e| {
                log::error!("SMTP error for {to}: {e}");
                e
            })
            .context("Failed to send email")?;

        log::info!("📧 Email sent to {}", to);
        Ok(())
    }

    /// Send a 6-digit verification code
    pub async fn send_verification_code(&self, email: &str, code: &str) -> Result<()> {
        let subject = "FlowLink — Код подтверждения";
        let html = format_verification_html(code);
        let text = format!("Ваш код подтверждения FlowLink: {}\n\nКод действителен 10 минут. Если вы не запрашивали код, проигнорируйте это письмо.", code);
        self.send_email(email, subject, &html, &text).await
    }

    // ═══════════════════════════════════════════
    // Welcome series (3 emails)
    // ═══════════════════════════════════════════

    /// Legacy compat — delegates to send_welcome_email1
    pub async fn send_welcome(&self, email: &str, name: &str) -> Result<()> {
        self.send_welcome_email1(email, name).await
    }

    /// Email 1 (immediate): registration confirmation
    pub async fn send_welcome_email1(&self, email: &str, name: &str) -> Result<()> {
        let subject = "Добро пожаловать в FlowLink! 🚀";
        let html = format_welcome1_html(name);
        let text = format!("Добро пожаловать в FlowLink, {}!\n\nВаш аккаунт создан. Вот что можно сделать:\n1. Подключите Telegram бота для уведомлений\n2. Настройте агенты и рабочие процессы\n3. Выберите тарифный план", name);
        self.send_email(email, subject, &html, &text).await
    }

    /// Email 2 (day 1): first steps guide
    pub async fn send_welcome_email2(&self, email: &str, name: &str) -> Result<()> {
        let subject = "Начните с подключения первого сервера 🖥️";
        let html = format_welcome2_html(name);
        let text = format!("Привет, {}!\n\nТеперь, когда ваш аккаунт готов, самое время подключить первый сервер.\n\n1. Установите FlowLink Agent на ваш сервер\n2. Получите API-ключ в личном кабинете\n3. Подключите агента к вашему рабочему процессу\n\nПодробная документация: https://docs.flow-masters.ru", name);
        self.send_email(email, subject, &html, &text).await
    }

    /// Email 3 (day 3): trial reminder
    pub async fn send_welcome_email3(&self, email: &str, name: &str) -> Result<()> {
        let subject = "Ваш пробный период активен ⏰";
        let html = format_welcome3_html(name);
        let text = format!("Привет, {}!\n\nВы уже 3 дня с нами. Пробный период бесплатного плана позволяет попробовать все возможности FlowLink.\n\nКогда будете готовы — выберите тарифный план, который подходит вам.\n\nЕсли у вас есть вопросы — мы всегда на связи!", name);
        self.send_email(email, subject, &html, &text).await
    }

    // ═══════════════════════════════════════════
    // Payment emails
    // ═══════════════════════════════════════════

    /// Payment success receipt
    pub async fn send_payment_success(&self, email: &str, name: &str, plan_name: &str, amount: &str) -> Result<()> {
        let subject = "FlowLink — Оплата прошла успешно ✅";
        let html = format_payment_success_html(name, plan_name, amount);
        let text = format!("Оплата прошла успешно!\n\nПлан: {}\nСумма: {}\n\nСпасибо за использование FlowLink!", plan_name, amount);
        self.send_email(email, subject, &html, &text).await
    }

    /// Payment failed notification
    pub async fn send_payment_failed(&self, email: &str, name: &str, plan_name: &str) -> Result<()> {
        let subject = "Не удалось списать средства за FlowLink ⚠️";
        let html = format_payment_failed_html(name, plan_name);
        let text = format!("Привет, {}!\n\nК сожалению, не удалось списать средства за план {}.\n\nПожалуйста, обновите способ оплаты в личном кабинете, чтобы не потерять доступ к сервису.", name, plan_name);
        self.send_email(email, subject, &html, &text).await
    }

    /// Subscription renewal reminder
    pub async fn send_renewal_reminder(&self, email: &str, name: &str, plan_name: &str, renewal_date: &str) -> Result<()> {
        let subject = "Скоро продление подписки FlowLink 📅";
        let html = format_renewal_reminder_html(name, plan_name, renewal_date);
        let text = format!("Привет, {}!\n\nЧерез 3 дня ({}) продлевается подписка на план {}.\n\nУбедитесь, что способ оплаты актуален.", name, renewal_date, plan_name);
        self.send_email(email, subject, &html, &text).await
    }

    /// Subscription cancelled confirmation
    pub async fn send_subscription_cancelled(&self, email: &str, name: &str, access_until: &str) -> Result<()> {
        let subject = "Подписка FlowLink отменена";
        let html = format_subscription_cancelled_html(name, access_until);
        let text = format!("Привет, {}!\n\nВаша подписка отменена. Доступ к сервису сохранён до {}.\n\nВы можете возобновить подписку в любой момент.", name, access_until);
        self.send_email(email, subject, &html, &text).await
    }

    // ═══════════════════════════════════════════
    // Security emails
    // ═══════════════════════════════════════════

    /// New login notification
    pub async fn send_new_login(&self, email: &str, name: &str, ip: &str, time: &str) -> Result<()> {
        let subject = "Новый вход в аккаунт FlowLink 🔐";
        let html = format_new_login_html(name, ip, time);
        let text = format!("Привет, {}!\n\nЗафиксирован новый вход в ваш аккаунт.\nIP: {}\nВремя: {}\n\nЕсли это не вы — немедленно смените пароль.", name, ip, time);
        self.send_email(email, subject, &html, &text).await
    }

    /// Password changed confirmation
    pub async fn send_password_changed(&self, email: &str, name: &str) -> Result<()> {
        let subject = "Пароль FlowLink успешно изменён";
        let html = format_password_changed_html(name);
        let text = format!("Привет, {}!\n\nВаш пароль успешно изменён. Если это не вы — немедленно обратитесь в поддержку.", name);
        self.send_email(email, subject, &html, &text).await
    }

    /// API key created notification
    pub async fn send_api_key_created(&self, email: &str, name: &str, key_name: &str) -> Result<()> {
        let subject = "Создан новый API-ключ FlowLink";
        let html = format_api_key_html(name, key_name, true);
        let text = format!("Привет, {}!\n\nВ вашем аккаунте создан API-ключ: {}\n\nЕсли это не вы — удалите ключ и обратитесь в поддержку.", name, key_name);
        self.send_email(email, subject, &html, &text).await
    }

    /// API key deleted notification
    pub async fn send_api_key_deleted(&self, email: &str, name: &str, key_name: &str) -> Result<()> {
        let subject = "API-ключ FlowLink удалён";
        let html = format_api_key_html(name, key_name, false);
        let text = format!("Привет, {}!\n\nAPI-ключ '{}' был удалён из вашего аккаунта.\n\nЕсли это не вы — немедленно обратитесь в поддержку.", name, key_name);
        self.send_email(email, subject, &html, &text).await
    }

    /// Plan changed notification
    pub async fn send_plan_changed(&self, email: &str, name: &str, old_plan: &str, new_plan: &str) -> Result<()> {
        let subject = "Тарифный план FlowLink изменён";
        let html = format_plan_changed_html(name, old_plan, new_plan);
        let text = format!("Привет, {}!\n\nВаш тарифный план изменён: {} → {}\n\nИзменения вступают в силу немедленно.", name, old_plan, new_plan);
        self.send_email(email, subject, &html, &text).await
    }
}

// ═══════════════════════════════════════════════
// HTML Templates (inline)
// ═══════════════════════════════════════════════

fn fmt_email(_name: &str, header_color: &str, sub: &str, body_html: &str) -> String {
    format!(r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1.0"></head>
<body style="margin:0;padding:0;background:#f5f5f5;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">
<table width="100%" cellpadding="0" cellspacing="0" style="padding:40px 0;"><tr><td align="center">
<table width="480" cellpadding="0" cellspacing="0" style="background:#fff;border-radius:16px;overflow:hidden;box-shadow:0 2px 8px rgba(0,0,0,.08);">
<tr><td style="background:linear-gradient(135deg,{hc});padding:32px;text-align:center;">
<h1 style="margin:0;color:#fff;font-size:24px;font-weight:700;">FlowLink</h1>
<p style="margin:8px 0 0;color:rgba(255,255,255,.85);font-size:14px;">{sub}</p>
</td></tr>
<tr><td style="padding:40px 32px;">{body}</td></tr>
<tr><td style="padding:16px 32px;border-top:1px solid #f1f5f9;text-align:center;">
<p style="margin:0;color:#cbd5e1;font-size:12px;">FlowLink — Платформа для AI-агентов</p>
</td></tr>
</table>
</td></tr></table>
</body></html>"#, hc = header_color, sub = sub, body = body_html)
}

fn format_verification_html(code: &str) -> String {
    format!(r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
<body style="margin:0;padding:0;background:#f5f5f5;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">
  <table width="100%" cellpadding="0" cellspacing="0" style="padding:40px 0;">
    <tr><td align="center">
      <table width="480" cellpadding="0" cellspacing="0" style="background:#fff;border-radius:16px;overflow:hidden;box-shadow:0 2px 8px rgba(0,0,0,0.08);">
        <tr><td style="background:linear-gradient(135deg,#6366f1,#8b5cf6);padding:32px;text-align:center;">
          <h1 style="margin:0;color:#fff;font-size:24px;font-weight:700;">FlowLink</h1>
          <p style="margin:8px 0 0;color:rgba(255,255,255,0.85);font-size:14px;">Код подтверждения</p>
        </td></tr>
        <tr><td style="padding:40px 32px;text-align:center;">
          <p style="margin:0 0 24px;color:#64748b;font-size:16px;">Введите этот код для завершения входа:</p>
          <div style="background:#f1f5f9;border-radius:12px;padding:20px 32px;display:inline-block;">
            <span style="font-size:36px;font-weight:700;letter-spacing:8px;color:#1e293b;">{code}</span>
          </div>
          <p style="margin:24px 0 0;color:#94a3b8;font-size:13px;">Код действителен 10 минут. Если вы не запрашивали код, проигнорируйте это письмо.</p>
        </td></tr>
        <tr><td style="padding:16px 32px;border-top:1px solid #f1f5f9;text-align:center;">
          <p style="margin:0;color:#cbd5e1;font-size:12px;">FlowLink — Платформа для AI-агентов</p>
        </td></tr>
      </table>
    </td></tr>
  </table>
</body>
</html>"#, code = code)
}

fn format_welcome1_html(name: &str) -> String {
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#1e293b;font-size:20px;">Привет, {name}!</h2>
<p style="margin:0 0 20px;color:#64748b;font-size:15px;line-height:1.6;">Ваш аккаунт FlowLink создан. Вот что можно сделать дальше:</p>
<ul style="margin:0 0 24px;padding-left:20px;color:#475569;font-size:15px;line-height:2;">
<li>🔗 Подключите Telegram бота для уведомлений</li>
<li>🤖 Настройте своих AI-агентов</li>
<li>📋 Выберите тарифный план</li>
</ul>
<p style="margin:0;color:#94a3b8;font-size:13px;">Если у вас есть вопросы, напишите нам в поддержку.</p>"#, name = name);
    fmt_email(name, "#6366f1,#8b5cf6", "Добро пожаловать! 🚀", &body)
}

fn format_welcome2_html(name: &str) -> String {
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#1e293b;font-size:20px;">Привет, {name}!</h2>
<p style="margin:0 0 20px;color:#64748b;font-size:15px;line-height:1.6;">Теперь, когда ваш аккаунт готов, самое время подключить первый сервер.</p>
<div style="background:#f8fafc;border-radius:8px;padding:20px;margin-bottom:24px;">
<p style="margin:0 0 12px;color:#1e293b;font-size:14px;font-weight:600;">Первые шаги:</p>
<ol style="margin:0;padding-left:20px;color:#475569;font-size:14px;line-height:2;">
<li>Установите FlowLink Agent на ваш сервер</li>
<li>Получите API-ключ в личном кабинете</li>
<li>Подключите агента к вашему рабочему процессу</li>
</ol>
</div>
<p style="margin:0;color:#64748b;font-size:14px;">📖 <a href="https://docs.flow-masters.ru" style="color:#6366f1;text-decoration:none;">Подробная документация</a></p>"#, name = name);
    fmt_email(name, "#3b82f6,#6366f1", "Подключите первый сервер 🖥️", &body)
}

fn format_welcome3_html(name: &str) -> String {
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#1e293b;font-size:20px;">{name}, вы уже с нами 3 дня!</h2>
<p style="margin:0 0 20px;color:#64748b;font-size:15px;line-height:1.6;">Пробный период бесплатного плана позволяет попробовать все возможности FlowLink без ограничений.</p>
<div style="background:linear-gradient(135deg,#fef3c7,#fde68a);border-radius:8px;padding:20px;margin-bottom:24px;">
<p style="margin:0;color:#92400e;font-size:14px;font-weight:600;">⏰ Попробуйте и выберите подходящий тарифный план, когда будете готовы</p>
</div>
<p style="margin:0;color:#94a3b8;font-size:13px;">Если у вас есть вопросы — мы всегда на связи!</p>"#, name = name);
    fmt_email(name, "#f59e0b,#d97706", "Ваш пробный период активен ⏰", &body)
}

fn format_payment_success_html(name: &str, plan_name: &str, amount: &str) -> String {
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#1e293b;font-size:20px;">Спасибо, {name}!</h2>
<table width="100%" cellpadding="12" cellspacing="0" style="background:#f8fafc;border-radius:8px;margin-bottom:24px;">
<tr><td style="color:#64748b;font-size:14px;border-bottom:1px solid #e2e8f0;">План</td><td style="color:#1e293b;font-size:14px;font-weight:600;text-align:right;border-bottom:1px solid #e2e8f0;">{plan_name}</td></tr>
<tr><td style="color:#64748b;font-size:14px;">Сумма</td><td style="color:#1e293b;font-size:14px;font-weight:600;text-align:right;">{amount}</td></tr>
</table>
<p style="margin:0;color:#94a3b8;font-size:13px;">Подписка активирована. Приятного использования FlowLink!</p>"#, name = name, plan_name = plan_name, amount = amount);
    fmt_email(name, "#10b981,#059669", "Оплата прошла успешно ✅", &body)
}

fn format_payment_failed_html(name: &str, plan_name: &str) -> String {
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#1e293b;font-size:20px;">Привет, {name}!</h2>
<div style="background:#fef2f2;border-left:4px solid #ef4444;border-radius:4px;padding:16px;margin-bottom:24px;">
<p style="margin:0;color:#991b1b;font-size:14px;line-height:1.6;">Не удалось списать средства за план <strong>{plan_name}</strong>. Пожалуйста, обновите способ оплаты, чтобы избежать приостановки сервиса.</p>
</div>
<p style="margin:0;color:#64748b;font-size:14px;">Вы можете обновить платёжные данные в личном кабинете.</p>"#, name = name, plan_name = plan_name);
    fmt_email(name, "#ef4444,#dc2626", "Проблема с оплатой ⚠️", &body)
}

fn format_renewal_reminder_html(name: &str, plan_name: &str, renewal_date: &str) -> String {
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#1e293b;font-size:20px;">Привет, {name}!</h2>
<p style="margin:0 0 20px;color:#64748b;font-size:15px;line-height:1.6;">Через 3 дня продлевается ваша подписка:</p>
<table width="100%" cellpadding="12" cellspacing="0" style="background:#f8fafc;border-radius:8px;margin-bottom:24px;">
<tr><td style="color:#64748b;font-size:14px;">План</td><td style="color:#1e293b;font-size:14px;font-weight:600;text-align:right;">{plan_name}</td></tr>
<tr><td style="color:#64748b;font-size:14px;">Дата продления</td><td style="color:#1e293b;font-size:14px;font-weight:600;text-align:right;">{renewal_date}</td></tr>
</table>
<p style="margin:0;color:#64748b;font-size:14px;">Пожалуйста, убедитесь, что способ оплаты актуален.</p>"#, name = name, plan_name = plan_name, renewal_date = renewal_date);
    fmt_email(name, "#6366f1,#8b5cf6", "Продление подписки 📅", &body)
}

fn format_subscription_cancelled_html(name: &str, access_until: &str) -> String {
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#1e293b;font-size:20px;">Привет, {name}!</h2>
<p style="margin:0 0 20px;color:#64748b;font-size:15px;line-height:1.6;">Ваша подписка на FlowLink отменена по вашему запросу.</p>
<div style="background:#f8fafc;border-radius:8px;padding:16px;margin-bottom:24px;">
<p style="margin:0;color:#1e293b;font-size:14px;">Доступ к сервису сохранён до: <strong>{access_until}</strong></p>
</div>
<p style="margin:0;color:#64748b;font-size:14px;">Вы можете возобновить подписку в любой момент из личного кабинета.</p>"#, name = name, access_until = if access_until.is_empty() { "окончания текущего периода" } else { access_until });
    fmt_email(name, "#64748b,#475569", "Подписка отменена", &body)
}

fn format_new_login_html(name: &str, ip: &str, time: &str) -> String {
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#1e293b;font-size:20px;">Привет, {name}!</h2>
<p style="margin:0 0 20px;color:#64748b;font-size:15px;">Зафиксирован новый вход в ваш аккаунт FlowLink:</p>
<table width="100%" cellpadding="12" cellspacing="0" style="background:#f8fafc;border-radius:8px;margin-bottom:24px;">
<tr><td style="color:#64748b;font-size:14px;">IP-адрес</td><td style="color:#1e293b;font-size:14px;font-weight:600;text-align:right;font-family:monospace;">{ip}</td></tr>
<tr><td style="color:#64748b;font-size:14px;">Время</td><td style="color:#1e293b;font-size:14px;font-weight:600;text-align:right;">{time}</td></tr>
</table>
<p style="margin:0;color:#ef4444;font-size:14px;font-weight:500;">Если это не вы — немедленно смените пароль и обратитесь в поддержку.</p>"#, name = name, ip = if ip.is_empty() { "неизвестно" } else { ip }, time = if time.is_empty() { "только что" } else { time });
    fmt_email(name, "#6366f1,#8b5cf6", "Новый вход в аккаунт 🔐", &body)
}

fn format_password_changed_html(name: &str) -> String {
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#1e293b;font-size:20px;">Привет, {name}!</h2>
<p style="margin:0 0 20px;color:#64748b;font-size:15px;line-height:1.6;">Пароль вашего аккаунта FlowLink успешно изменён.</p>
<div style="background:#f0fdf4;border-left:4px solid #22c55e;border-radius:4px;padding:16px;margin-bottom:24px;">
<p style="margin:0;color:#166534;font-size:14px;">✅ Изменение подтверждено</p>
</div>
<p style="margin:0;color:#94a3b8;font-size:13px;">Если вы не меняли пароль — немедленно обратитесь в поддержку.</p>"#, name = name);
    fmt_email(name, "#22c55e,#16a34a", "Пароль изменён", &body)
}

fn format_api_key_html(name: &str, key_name: &str, created: bool) -> String {
    let (action, color, sub) = if created {
        ("создан", "#6366f1,#8b5cf6", "Новый API-ключ 🔑")
    } else {
        ("удалён", "#64748b,#475569", "API-ключ удалён 🗑️")
    };
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#1e293b;font-size:20px;">Привет, {name}!</h2>
<p style="margin:0 0 20px;color:#64748b;font-size:15px;">API-ключ <strong style="color:#1e293b;">{key_name}</strong> {action} в вашем аккаунте.</p>
<p style="margin:0;color:#94a3b8;font-size:13px;">Если это не вы — немедленно обратитесь в поддержку.</p>"#, name = name, key_name = key_name, action = action);
    fmt_email(name, color, sub, &body)
}

fn format_plan_changed_html(name: &str, old_plan: &str, new_plan: &str) -> String {
    let body = format!(r#"
<h2 style="margin:0 0 16px;color:#1e293b;font-size:20px;">Привет, {name}!</h2>
<p style="margin:0 0 20px;color:#64748b;font-size:15px;">Ваш тарифный план изменён:</p>
<div style="background:#f8fafc;border-radius:8px;padding:16px;margin-bottom:24px;text-align:center;">
<p style="margin:0;color:#64748b;font-size:16px;">{old_plan} <span style="color:#6366f1;font-size:20px;">→</span> {new_plan}</p>
</div>
<p style="margin:0;color:#94a3b8;font-size:13px;">Изменения вступают в силу немедленно.</p>"#, name = name, old_plan = old_plan, new_plan = new_plan);
    fmt_email(name, "#6366f1,#8b5cf6", "Тарифный план изменён", &body)
}

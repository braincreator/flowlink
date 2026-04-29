//! Email queue — DB-backed scheduled email system
//!
//! Persists scheduled emails in PostgreSQL and processes them via a background worker.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::email::EmailService;

// ═══════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EmailType {
    Welcome1,
    Welcome2,
    Welcome3,
    PaymentSuccess,
    PaymentFailed,
    RenewalReminder,
    SubscriptionCancelled,
    NewLogin,
    PasswordChanged,
    ApiKeyCreated,
    ApiKeyDeleted,
    PlanChanged,
}

impl EmailType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Welcome1 => "welcome1",
            Self::Welcome2 => "welcome2",
            Self::Welcome3 => "welcome3",
            Self::PaymentSuccess => "payment_success",
            Self::PaymentFailed => "payment_failed",
            Self::RenewalReminder => "renewal_reminder",
            Self::SubscriptionCancelled => "subscription_cancelled",
            Self::NewLogin => "new_login",
            Self::PasswordChanged => "password_changed",
            Self::ApiKeyCreated => "api_key_created",
            Self::ApiKeyDeleted => "api_key_deleted",
            Self::PlanChanged => "plan_changed",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "welcome1" => Some(Self::Welcome1),
            "welcome2" => Some(Self::Welcome2),
            "welcome3" => Some(Self::Welcome3),
            "payment_success" => Some(Self::PaymentSuccess),
            "payment_failed" => Some(Self::PaymentFailed),
            "renewal_reminder" => Some(Self::RenewalReminder),
            "subscription_cancelled" => Some(Self::SubscriptionCancelled),
            "new_login" => Some(Self::NewLogin),
            "password_changed" => Some(Self::PasswordChanged),
            "api_key_created" => Some(Self::ApiKeyCreated),
            "api_key_deleted" => Some(Self::ApiKeyDeleted),
            "plan_changed" => Some(Self::PlanChanged),
            _ => None,
        }
    }
}

#[allow(dead_code)]
#[allow(dead_code)]
struct ScheduledEmail {
    id: uuid::Uuid,
    account_id: String,
    email_type: EmailType,
    recipient: String,
    scheduled_at: chrono::DateTime<Utc>,
    template_vars: HashMap<String, String>,
    attempts: i16,
    max_attempts: i16,
}

// ═══════════════════════════════════════════════
// EmailQueue
// ═══════════════════════════════════════════════

pub struct EmailQueue {
    email_service: Arc<EmailService>,
    pool: Arc<PgPool>,
}

impl EmailQueue {
    pub fn new(email_service: Arc<EmailService>, pool: PgPool) -> Self {
        Self { email_service, pool: Arc::new(pool) }
    }

    /// Schedule a single email
    pub async fn schedule(
        &self,
        account_id: &str,
        email_type: EmailType,
        recipient: &str,
        scheduled_at: chrono::DateTime<Utc>,
        template_vars: HashMap<String, String>,
    ) -> Result<()> {
        let vars = serde_json::to_value(&template_vars).unwrap_or_default();
        sqlx::query(
            r#"INSERT INTO email_queue (account_id, email_type, recipient, scheduled_at, template_vars)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(account_id)
        .bind(email_type.as_str())
        .bind(recipient)
        .bind(scheduled_at)
        .bind(&vars)
        .execute(self.pool.as_ref())
        .await
        .context("Failed to schedule email")?;

        log::info!(
            "📧 Scheduled {:?} for {} at {}",
            email_type, recipient, scheduled_at,
        );
        Ok(())
    }

    /// Schedule the full welcome series (3 emails)
    pub async fn schedule_welcome_series(&self, account_id: &str, email: &str) -> Result<()> {
        let now = Utc::now();

        // Email 1 — immediate
        let mut vars1 = HashMap::new();
        vars1.insert("email".into(), email.into());
        self.schedule(account_id, EmailType::Welcome1, email, now, vars1).await?;

        // Email 2 — day 1
        let mut vars2 = HashMap::new();
        vars2.insert("email".into(), email.into());
        self.schedule(account_id, EmailType::Welcome2, email, now + Duration::days(1), vars2).await?;

        // Email 3 — day 3
        let mut vars3 = HashMap::new();
        vars3.insert("email".into(), email.into());
        self.schedule(account_id, EmailType::Welcome3, email, now + Duration::days(3), vars3).await?;

        Ok(())
    }

    /// Process all pending emails that are due
    pub async fn process_pending(&self) {
        let rows = match sqlx::query_as::<_, (uuid::Uuid, String, String, String, serde_json::Value, i16, i16)>(
            r#"SELECT id, account_id, email_type, recipient, template_vars, attempts, max_attempts
               FROM email_queue
               WHERE sent_at IS NULL AND scheduled_at <= NOW()
               ORDER BY scheduled_at
               LIMIT 50"#,
        )
        .fetch_all(self.pool.as_ref())
        .await
        {
            Ok(r) => r,
            Err(e) => {
                log::warn!("email_queue: failed to fetch pending: {e}");
                return;
            }
        };

        for (id, account_id, type_str, recipient, vars, attempts, max_attempts) in rows {
            if attempts >= max_attempts {
                log::warn!("email_queue: skipping {} (max attempts reached)", id);
                continue;
            }

            let email_type = match EmailType::from_str(&type_str) {
                Some(t) => t,
                None => {
                    log::warn!("email_queue: unknown type '{}', skipping {}", type_str, id);
                    continue;
                }
            };

            let vars_map: HashMap<String, String> = serde_json::from_value(vars).unwrap_or_default();

            // Get account language preference
            let lang = if let Ok(Some(account)) = flowlink_db::accounts::AccountRepo::get(self.pool.as_ref(), &account_id).await {
                account.preferred_language.as_deref().unwrap_or("ru").to_string()
            } else {
                "ru".to_string()
            };

            let result = self.send_typed_email(&email_type, &recipient, &vars_map, &lang).await;

            match result {
                Ok(()) => {
                    let _ = sqlx::query("UPDATE email_queue SET sent_at = NOW() WHERE id = $1")
                        .bind(id)
                        .execute(self.pool.as_ref())
                        .await;
                    log::info!("email_queue: sent {:?} to {}", email_type, recipient);
                }
                Err(e) => {
                    let new_attempts = attempts + 1;
                    let _ = sqlx::query(
                        "UPDATE email_queue SET attempts = $1 WHERE id = $2",
                    )
                    .bind(new_attempts)
                    .bind(id)
                    .execute(self.pool.as_ref())
                    .await;
                    log::warn!(
                        "email_queue: failed {:?} to {} (attempt {}/{}): {e}",
                        email_type, recipient, new_attempts, max_attempts,
                    );
                }
            }
        }
    }

    async fn send_typed_email(
        &self,
        email_type: &EmailType,
        recipient: &str,
        vars: &HashMap<String, String>,
        lang: &str,
    ) -> Result<()> {
        let email = &self.email_service;
        match email_type {
            EmailType::Welcome1 => {
                let name = vars.get("email").map(|s| s.split('@').next().unwrap_or(s)).unwrap_or("пользователь");
                email.send_welcome_email1(recipient, name, lang).await
            }
            EmailType::Welcome2 => {
                let name = vars.get("email").map(|s| s.split('@').next().unwrap_or(s)).unwrap_or("пользователь");
                email.send_welcome_email2(recipient, name, lang).await
            }
            EmailType::Welcome3 => {
                let name = vars.get("email").map(|s| s.split('@').next().unwrap_or(s)).unwrap_or("пользователь");
                email.send_welcome_email3(recipient, name, lang).await
            }
            EmailType::PaymentSuccess => {
                let name = vars.get("name").map(|s| s.as_str()).unwrap_or("пользователь");
                let plan = vars.get("plan_name").map(|s| s.as_str()).unwrap_or("план");
                let amount = vars.get("amount").map(|s| s.as_str()).unwrap_or("0 ₽");
                email.send_payment_success(recipient, name, plan, amount, lang).await
            }
            EmailType::PaymentFailed => {
                let name = vars.get("name").map(|s| s.as_str()).unwrap_or("пользователь");
                let plan = vars.get("plan_name").map(|s| s.as_str()).unwrap_or("план");
                email.send_payment_failed(recipient, name, plan, lang).await
            }
            EmailType::RenewalReminder => {
                let name = vars.get("name").map(|s| s.as_str()).unwrap_or("пользователь");
                let plan = vars.get("plan_name").map(|s| s.as_str()).unwrap_or("план");
                let date = vars.get("renewal_date").map(|s| s.as_str()).unwrap_or("скоро");
                email.send_renewal_reminder(recipient, name, plan, date, lang).await
            }
            EmailType::SubscriptionCancelled => {
                let name = vars.get("name").map(|s| s.as_str()).unwrap_or("пользователь");
                let until = vars.get("access_until").map(|s| s.as_str()).unwrap_or("");
                email.send_subscription_cancelled(recipient, name, until, lang).await
            }
            EmailType::NewLogin => {
                let name = vars.get("name").map(|s| s.as_str()).unwrap_or("пользователь");
                let ip = vars.get("ip").map(|s| s.as_str()).unwrap_or("");
                let country = vars.get("country").map(|s| s.as_str()).unwrap_or("");
                let time = vars.get("time").map(|s| s.as_str()).unwrap_or("");
                email.send_new_login(recipient, name, ip, country, time, lang).await
            }
            EmailType::PasswordChanged => {
                let name = vars.get("name").map(|s| s.as_str()).unwrap_or("пользователь");
                email.send_password_changed(recipient, name, lang).await
            }
            EmailType::ApiKeyCreated => {
                let name = vars.get("name").map(|s| s.as_str()).unwrap_or("пользователь");
                let key_name = vars.get("key_name").map(|s| s.as_str()).unwrap_or("API ключ");
                email.send_api_key_created(recipient, name, key_name, lang).await
            }
            EmailType::ApiKeyDeleted => {
                let name = vars.get("name").map(|s| s.as_str()).unwrap_or("пользователь");
                let key_name = vars.get("key_name").map(|s| s.as_str()).unwrap_or("API ключ");
                email.send_api_key_deleted(recipient, name, key_name, lang).await
            }
            EmailType::PlanChanged => {
                let name = vars.get("name").map(|s| s.as_str()).unwrap_or("пользователь");
                let old_plan = vars.get("old_plan").map(|s| s.as_str()).unwrap_or("");
                let new_plan = vars.get("new_plan").map(|s| s.as_str()).unwrap_or("");
                email.send_plan_changed(recipient, name, old_plan, new_plan, lang).await
            }
        }
    }

    /// Spawn a background worker that processes pending emails every 60 seconds.
    /// Also calls `cleanup_fn` to purge expired rate-limit windows.
    pub fn start_worker<F>(self: Arc<Self>, cleanup_fn: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            // Skip first immediate tick
            interval.tick().await;
            loop {
                interval.tick().await;
                self.process_pending().await;
                cleanup_fn();
            }
        });
        log::info!("📧 Email queue worker started (60s interval)");
    }
}

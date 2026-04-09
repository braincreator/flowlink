//! Подписки — CRUD для рекуррентных подписок через Точка Банк

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// Строка подписки из БД
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct SubscriptionRow {
    pub id: String,
    pub account_id: String,
    pub plan_id: String,
    pub status: String,
    pub period: String,
    pub amount_kopecks: i64,
    pub tochka_subscription_id: Option<String>,
    pub payment_method: Option<String>,
    pub started_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub next_billing_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct SubscriptionRepo;

impl SubscriptionRepo {
    /// Создать новую подписку
    pub async fn create(
        pool: &PgPool,
        id: &str,
        account_id: &str,
        plan_id: &str,
        period: &str,
        amount_kopecks: i64,
        tochka_id: Option<&str>,
    ) -> Result<SubscriptionRow> {
        let row = sqlx::query_as::<_, SubscriptionRow>(
            r#"INSERT INTO subscriptions (id, account_id, plan_id, period, amount_kopecks, tochka_subscription_id)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id, account_id, plan_id, status, period, amount_kopecks,
                         tochka_subscription_id, payment_method, started_at, expires_at,
                         trial_ends_at, next_billing_at, cancelled_at, created_at, updated_at"#,
        )
        .bind(id)
        .bind(account_id)
        .bind(plan_id)
        .bind(period)
        .bind(amount_kopecks)
        .bind(tochka_id)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    /// Получить активную подписку для аккаунта
    pub async fn get_active(pool: &PgPool, account_id: &str) -> Result<Option<SubscriptionRow>> {
        let row = sqlx::query_as::<_, SubscriptionRow>(
            r#"SELECT id, account_id, plan_id, status, period, amount_kopecks,
                      tochka_subscription_id, payment_method, started_at, expires_at,
                      trial_ends_at, next_billing_at, cancelled_at, created_at, updated_at
               FROM subscriptions
               WHERE account_id = $1 AND status = 'active'
               ORDER BY created_at DESC LIMIT 1"#,
        )
        .bind(account_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Обновить статус подписки
    pub async fn update_status(pool: &PgPool, id: &str, status: &str) -> Result<()> {
        sqlx::query(
            "UPDATE subscriptions SET status = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Отменить подписку
    pub async fn cancel(pool: &PgPool, id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE subscriptions SET status = 'cancelled', cancelled_at = NOW(), updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Получить все подписки аккаунта
    pub async fn list_for_account(pool: &PgPool, account_id: &str) -> Result<Vec<SubscriptionRow>> {
        let rows = sqlx::query_as::<_, SubscriptionRow>(
            r#"SELECT id, account_id, plan_id, status, period, amount_kopecks,
                      tochka_subscription_id, payment_method, started_at, expires_at,
                      trial_ends_at, next_billing_at, cancelled_at, created_at, updated_at
               FROM subscriptions
               WHERE account_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(account_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}

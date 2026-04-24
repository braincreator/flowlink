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
    /// Get subscription by ID
    pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<SubscriptionRow>> {
        let row = sqlx::query_as::<_, SubscriptionRow>(
            "SELECT * FROM subscriptions WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

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
        sqlx::query("UPDATE subscriptions SET status = $1, updated_at = NOW() WHERE id = $2")
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn now() -> DateTime<Utc> { Utc::now() }

    fn make_subscription() -> SubscriptionRow {
        SubscriptionRow {
            id: "sub_001".to_string(),
            account_id: "acct_001".to_string(),
            plan_id: "plan_pro".to_string(),
            status: "active".to_string(),
            period: "monthly".to_string(),
            amount_kopecks: 19900,
            tochka_subscription_id: Some("ts_001".to_string()),
            payment_method: Some("card".to_string()),
            started_at: now(),
            expires_at: Some(now() + chrono::Duration::days(30)),
            trial_ends_at: Some(now() + chrono::Duration::days(14)),
            next_billing_at: Some(now() + chrono::Duration::days(30)),
            cancelled_at: None,
            created_at: now(),
            updated_at: now(),
        }
    }

    #[test]
    fn subscription_row_construction() {
        let s = make_subscription();
        assert_eq!(s.id, "sub_001");
        assert_eq!(s.status, "active");
        assert_eq!(s.period, "monthly");
        assert_eq!(s.amount_kopecks, 19900);
    }

    #[test]
    fn subscription_row_all_options_none() {
        let s = SubscriptionRow {
            id: "sub_min".to_string(),
            account_id: "acct".to_string(),
            plan_id: "plan".to_string(),
            status: "active".to_string(),
            period: "monthly".to_string(),
            amount_kopecks: 100,
            tochka_subscription_id: None,
            payment_method: None,
            started_at: now(),
            expires_at: None,
            trial_ends_at: None,
            next_billing_at: None,
            cancelled_at: None,
            created_at: now(),
            updated_at: now(),
        };
        assert!(s.tochka_subscription_id.is_none());
        assert!(s.payment_method.is_none());
        assert!(s.expires_at.is_none());
        assert!(s.trial_ends_at.is_none());
        assert!(s.next_billing_at.is_none());
        assert!(s.cancelled_at.is_none());
    }

    #[test]
    fn subscription_row_all_options_some() {
        let s = make_subscription();
        assert!(s.tochka_subscription_id.is_some());
        assert!(s.payment_method.is_some());
        assert!(s.expires_at.is_some());
        assert!(s.trial_ends_at.is_some());
        assert!(s.next_billing_at.is_some());
    }

    #[test]
    fn subscription_row_clone() {
        let s = make_subscription();
        let cloned = s.clone();
        assert_eq!(cloned.id, s.id);
        assert_eq!(cloned.account_id, s.account_id);
    }

    #[test]
    fn subscription_row_debug() {
        let s = make_subscription();
        let debug = format!("{:?}", s);
        assert!(debug.contains("sub_001"));
    }

    #[test]

    #[test]
    fn subscription_row_statuses() {
        for status in &["active", "cancelled", "expired", "trial", "past_due"] {
            let mut s = make_subscription();
            s.status = status.to_string();
            assert_eq!(s.status, *status);
        }
    }

    #[test]
    fn subscription_row_periods() {
        for period in &["monthly", "yearly", "weekly"] {
            let mut s = make_subscription();
            s.period = period.to_string();
            assert_eq!(s.period, *period);
        }
    }

    #[test]
    fn subscription_row_zero_amount() {
        let mut s = make_subscription();
        s.amount_kopecks = 0;
        assert_eq!(s.amount_kopecks, 0);
    }

    #[test]
    fn subscription_row_large_amount() {
        let mut s = make_subscription();
        s.amount_kopecks = 999_999_99;
        assert_eq!(s.amount_kopecks, 999_999_99);
    }

    #[test]
    fn subscription_row_timestamps() {
        let s = make_subscription();
        assert!(s.started_at <= Utc::now());
        assert!(s.expires_at.is_some());
        assert!(s.cancelled_at.is_none());
    }

    #[test]
    fn subscription_row_empty_strings() {
        let s = SubscriptionRow {
            id: String::new(),
            account_id: String::new(),
            plan_id: String::new(),
            status: String::new(),
            period: String::new(),
            amount_kopecks: 0,
            tochka_subscription_id: None,
            payment_method: None,
            started_at: now(),
            expires_at: None,
            trial_ends_at: None,
            next_billing_at: None,
            cancelled_at: None,
            created_at: now(),
            updated_at: now(),
        };
        assert!(s.id.is_empty());
        assert!(s.account_id.is_empty());
        assert!(s.plan_id.is_empty());
    }

    #[test]
    fn subscription_row_cancelled_at_some() {
        let mut s = make_subscription();
        s.cancelled_at = Some(now());
        s.status = "cancelled".to_string();
        assert!(s.cancelled_at.is_some());
    }

    #[test]
    fn subscription_repo_exists() {
        let _repo = SubscriptionRepo;
    }
}

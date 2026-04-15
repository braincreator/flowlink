use anyhow::Result;
use sqlx::postgres::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;
use super::models::*;

pub struct SubscriptionStorage {
    pub pool: Arc<PgPool>,
}

impl SubscriptionStorage {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub async fn create_tables(&self) -> Result<()> {
        let create_tables_sql = r#"
            CREATE TABLE IF NOT EXISTS payment_subscriptions (
                id TEXT PRIMARY KEY,
                provider TEXT NOT NULL,
                customer_id TEXT NOT NULL,
                plan_id TEXT NOT NULL,
                amount REAL,
                currency TEXT NOT NULL,
                period TEXT NOT NULL,
                status TEXT NOT NULL,
                current_period_start TIMESTAMP NOT NULL,
                current_period_end TIMESTAMP NOT NULL,
                trial_end TIMESTAMP,
                auto_renew BOOLEAN NOT NULL DEFAULT TRUE,
                cancel_at_period_end BOOLEAN NOT NULL DEFAULT FALSE,
                cancelled_at TIMESTAMP,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMP NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_payment_subscriptions_customer_id ON payment_subscriptions(customer_id);
            CREATE INDEX IF NOT EXISTS idx_payment_subscriptions_status ON payment_subscriptions(status);
            CREATE INDEX IF NOT EXISTS idx_payment_subscriptions_end ON payment_subscriptions(current_period_end);
        "#;

        sqlx::query(create_tables_sql)
            .execute(self.pool.clone())
            .await?;

        log::info!("Subscription storage tables created successfully");
        Ok(())
    }

    pub async fn save_subscription(&self, subscription: &Subscription) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO payment_subscriptions (
                id, provider, customer_id, plan_id, amount, currency, period,
                status, current_period_start, current_period_end, trial_end,
                auto_renew, cancel_at_period_end, cancelled_at, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            ON CONFLICT (id) DO UPDATE SET
                status = EXCLUDED.status,
                amount = EXCLUDED.amount,
                period = EXCLUDED.period,
                current_period_end = EXCLUDED.current_period_end,
                auto_renew = EXCLUDED.auto_renew,
                cancel_at_period_end = EXCLUDED.cancel_at_period_end,
                updated_at = NOW()
        "#;

        sqlx::query(insert_sql)
            .bind(&subscription.id)
            .bind(&subscription.provider)
            .bind(&subscription.customer_id)
            .bind(&subscription.plan_id)
            .bind(subscription.amount)
            .bind(&subscription.currency)
            .bind(format!("{:?}", subscription.period))
            .bind(subscription.status.as_str())
            .bind(subscription.current_period_start)
            .bind(subscription.current_period_end)
            .bind(subscription.trial_end)
            .bind(subscription.auto_renew)
            .bind(subscription.cancel_at_period_end)
            .bind(subscription.cancelled_at)
            .bind(subscription.created_at)
            .bind(subscription.updated_at)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_subscription(&self, subscription_id: &str) -> Result<Option<Subscription>> {
        let query = r#"
            SELECT id, provider, customer_id, plan_id, amount, currency, period,
                   status, current_period_start, current_period_end, trial_end,
                   auto_renew, cancel_at_period_end, cancelled_at, created_at, updated_at
            FROM payment_subscriptions
            WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(subscription_id)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                let period: String = row.try_get("period")?;
                let period = match period.as_str() {
                    "Monthly" => SubscriptionPeriod::Monthly,
                    "Quarterly" => SubscriptionPeriod::Quarterly,
                    "Annually" => SubscriptionPeriod::Annually,
                    _ => SubscriptionPeriod::Monthly,
                };

                Ok(Some(Subscription {
                    id: row.try_get("id")?,
                    provider: row.try_get("provider")?,
                    customer_id: row.try_get("customer_id")?,
                    plan_id: row.try_get("plan_id")?,
                    amount: row.try_get("amount"),
                    currency: row.try_get("currency")?,
                    period,
                    status: row.try_get("status")?,
                    current_period_start: row.try_get("current_period_start")?,
                    current_period_end: row.try_get("current_period_end")?,
                    trial_end: row.try_get("trial_end"),
                    auto_renew: row.try_get("auto_renew")?,
                    cancel_at_period_end: row.try_get("cancel_at_period_end")?,
                    cancelled_at: row.try_get("cancelled_at"),
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn get_subscriptions_by_customer(&self, customer_id: &str) -> Result<Vec<Subscription>> {
        let query = r#"
            SELECT id, provider, customer_id, plan_id, amount, currency, period,
                   status, current_period_start, current_period_end, trial_end,
                   auto_renew, cancel_at_period_end, cancelled_at, created_at, updated_at
            FROM payment_subscriptions
            WHERE customer_id = $1
            ORDER BY created_at DESC
        "#;

        let rows = sqlx::query(query)
            .bind(customer_id)
            .fetch_all(self.pool.clone())
            .await?;

        let mut subscriptions = Vec::new();

        for row in rows {
            let period: String = row.try_get("period")?;
            let period = match period.as_str() {
                "Monthly" => SubscriptionPeriod::Monthly,
                "Quarterly" => SubscriptionPeriod::Quarterly,
                "Annually" => SubscriptionPeriod::Annually,
                _ => SubscriptionPeriod::Monthly,
            };

            subscriptions.push(Subscription {
                id: row.try_get("id")?,
                provider: row.try_get("provider")?,
                customer_id: row.try_get("customer_id")?,
                plan_id: row.try_get("plan_id")?,
                amount: row.try_get("amount"),
                currency: row.try_get("currency")?,
                period,
                status: row.try_get("status")?,
                current_period_start: row.try_get("current_period_start")?,
                current_period_end: row.try_get("current_period_end")?,
                trial_end: row.try_get("trial_end"),
                auto_renew: row.try_get("auto_renew")?,
                cancel_at_period_end: row.try_get("cancel_at_period_end")?,
                cancelled_at: row.try_get("cancelled_at"),
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }

        Ok(subscriptions)
    }

    pub async fn get_all_subscriptions(&self) -> Result<Vec<Subscription>> {
        let query = r#"
            SELECT id, provider, customer_id, plan_id, amount, currency, period,
                   status, current_period_start, current_period_end, trial_end,
                   auto_renew, cancel_at_period_end, cancelled_at, created_at, updated_at
            FROM payment_subscriptions
            ORDER BY created_at DESC
        "#;

        let rows = sqlx::query(query).fetch_all(self.pool.clone()).await?;

        let mut subscriptions = Vec::new();

        for row in rows {
            let period: String = row.try_get("period")?;
            let period = match period.as_str() {
                "Monthly" => SubscriptionPeriod::Monthly,
                "Quarterly" => SubscriptionPeriod::Quarterly,
                "Annually" => SubscriptionPeriod::Annually,
                _ => SubscriptionPeriod::Monthly,
            };

            subscriptions.push(Subscription {
                id: row.try_get("id")?,
                provider: row.try_get("provider")?,
                customer_id: row.try_get("customer_id")?,
                plan_id: row.try_get("plan_id")?,
                amount: row.try_get("amount"),
                currency: row.try_get("currency")?,
                period,
                status: row.try_get("status")?,
                current_period_start: row.try_get("current_period_start")?,
                current_period_end: row.try_get("current_period_end")?,
                trial_end: row.try_get("trial_end"),
                auto_renew: row.try_get("auto_renew")?,
                cancel_at_period_end: row.try_get("cancel_at_period_end")?,
                cancelled_at: row.try_get("cancelled_at"),
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            });
        }

        Ok(subscriptions)
    }

    pub async fn update_subscription(&self, subscription: &Subscription) -> Result<()> {
        let update_sql = r#"
            UPDATE payment_subscriptions
            SET status = $1,
                amount = $2,
                period = $3,
                current_period_end = $4,
                auto_renew = $5,
                cancel_at_period_end = $6,
                updated_at = NOW()
            WHERE id = $7
        "#;

        sqlx::query(update_sql)
            .bind(subscription.status.as_str())
            .bind(subscription.amount)
            .bind(format!("{:?}", subscription.period))
            .bind(subscription.current_period_end)
            .bind(subscription.auto_renew)
            .bind(subscription.cancel_at_period_end)
            .bind(&subscription.id)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn delete_subscription(&self, subscription_id: &str) -> Result<()> {
        let query = "DELETE FROM payment_subscriptions WHERE id = $1";

        sqlx::query(query)
            .bind(subscription_id)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn cleanup_expired_subscriptions(&self) -> Result<i64> {
        let query = r#"
            UPDATE payment_subscriptions
            SET status = 'expired', updated_at = NOW()
            WHERE status != 'cancelled' AND current_period_end < NOW()
        "#;

        let result = sqlx::query(query)
            .execute(self.pool.clone())
            .await?;

        let updated_count = result.rows_affected();
        log::info!("Cleaned up {} expired subscriptions", updated_count);

        Ok(updated_count)
    }
}
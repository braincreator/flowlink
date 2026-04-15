use anyhow::Result;
use sqlx::postgres::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;
use super::models::*;

pub struct RefundStorage {
    pub pool: Arc<PgPool>,
}

impl RefundStorage {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub async fn create_tables(&self) -> Result<()> {
        let create_tables_sql = r#"
            CREATE TABLE IF NOT EXISTS payment_refunds (
                id TEXT PRIMARY KEY,
                invoice_id TEXT NOT NULL,
                provider TEXT NOT NULL,
                amount REAL,
                status TEXT NOT NULL,
                reason TEXT,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                processed_at TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_payment_refunds_invoice_id ON payment_refunds(invoice_id);
            CREATE INDEX IF NOT EXISTS idx_payment_refunds_status ON payment_refunds(status);
            CREATE INDEX IF NOT EXISTS idx_payment_refunds_created_at ON payment_refunds(created_at DESC);
        "#;

        sqlx::query(create_tables_sql)
            .execute(self.pool.clone())
            .await?;

        log::info!("Refund storage tables created successfully");
        Ok(())
    }

    pub async fn save_refund(&self, refund: &Refund) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO payment_refunds (
                id, invoice_id, provider, amount, status, reason, created_at, processed_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                status = EXCLUDED.status,
                reason = EXCLUDED.reason,
                processed_at = EXCLUDED.processed_at,
                updated_at = NOW()
        "#;

        sqlx::query(insert_sql)
            .bind(&refund.id)
            .bind(&refund.invoice_id)
            .bind(&refund.provider)
            .bind(refund.amount)
            .bind(refund.status.as_str())
            .bind(&refund.reason)
            .bind(refund.created_at)
            .bind(refund.processed_at)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_refund(&self, refund_id: &str) -> Result<Option<Refund>> {
        let query = r#"
            SELECT id, invoice_id, provider, amount, status, reason, created_at, processed_at
            FROM payment_refunds
            WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(refund_id)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                Ok(Some(Refund {
                    id: row.try_get("id")?,
                    invoice_id: row.try_get("invoice_id")?,
                    provider: row.try_get("provider")?,
                    amount: row.try_get("amount"),
                    status: row.try_get("status")?,
                    reason: row.try_get("reason"),
                    created_at: row.try_get("created_at")?,
                    processed_at: row.try_get("processed_at"),
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn get_refunds_by_invoice(&self, invoice_id: &str) -> Result<Vec<Refund>> {
        let query = r#"
            SELECT id, invoice_id, provider, amount, status, reason, created_at, processed_at
            FROM payment_refunds
            WHERE invoice_id = $1
            ORDER BY created_at DESC
        "#;

        let rows = sqlx::query(query)
            .bind(invoice_id)
            .fetch_all(self.pool.clone())
            .await?;

        let mut refunds = Vec::new();

        for row in rows {
            refunds.push(Refund {
                id: row.try_get("id")?,
                invoice_id: row.try_get("invoice_id")?,
                provider: row.try_get("provider")?,
                amount: row.try_get("amount"),
                status: row.try_get("status")?,
                reason: row.try_get("reason"),
                created_at: row.try_get("created_at")?,
                processed_at: row.try_get("processed_at"),
            });
        }

        Ok(refunds)
    }

    pub async fn update_refund_status(&self, refund_id: &str, status: RefundStatus) -> Result<()> {
        let update_sql = r#"
            UPDATE payment_refunds
            SET status = $1,
                processed_at = CASE WHEN $1 = 'processed' THEN NOW() ELSE processed_at END,
                updated_at = NOW()
            WHERE id = $2
        "#;

        sqlx::query(update_sql)
            .bind(status.as_str())
            .bind(refund_id)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn delete_refund(&self, refund_id: &str) -> Result<()> {
        let query = "DELETE FROM payment_refunds WHERE id = $1";

        sqlx::query(query)
            .bind(refund_id)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn cleanup_old_refunds(&self, days: i32) -> Result<i64> {
        let query = r#"
            DELETE FROM payment_refunds
            WHERE created_at < NOW() - INTERVAL '1 day' * $1
        "#;

        let result = sqlx::query(query)
            .bind(days)
            .execute(self.pool.clone())
            .await?;

        let deleted_count = result.rows_affected();
        log::info!("Cleaned up {} old refunds", deleted_count);

        Ok(deleted_count)
    }
}

// In-memory storage for testing
pub struct InMemoryRefundStorage {
    pub refunds: Arc<RwLock<HashMap<String, Refund>>>,
}

impl InMemoryRefundStorage {
    pub fn new() -> Self {
        Self {
            refunds: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn save_refund(&self, refund: &Refund) -> Result<()> {
        self.refunds.write().await.insert(refund.id.clone(), refund.clone());
        Ok(())
    }

    pub async fn get_refund(&self, refund_id: &str) -> Result<Option<Refund>> {
        Ok(self.refunds.read().await.get(refund_id).cloned())
    }

    pub async fn get_refunds_by_invoice(&self, invoice_id: &str) -> Result<Vec<Refund>> {
        Ok(self.refunds.read().await.values()
            .filter(|r| r.invoice_id == invoice_id)
            .cloned()
            .collect())
    }

    pub async fn update_refund_status(&self, refund_id: &str, status: RefundStatus) -> Result<()> {
        if let Some(refund) = self.refunds.write().await.get_mut(refund_id) {
            refund.status = status;
            if status == RefundStatus::Processed {
                refund.processed_at = Some(chrono::Utc::now());
            }
        }
        Ok(())
    }
}
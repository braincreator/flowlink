use anyhow::Result;
use sqlx::postgres::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::*;
use super::models::*;

pub struct InvoiceStorage {
    pub pool: Arc<PgPool>,
}

impl InvoiceStorage {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub async fn create_tables(&self) -> Result<()> {
        let create_tables_sql = r#"
            CREATE TABLE IF NOT EXISTS payment_invoices (
                id TEXT PRIMARY KEY,
                provider TEXT NOT NULL,
                customer_id TEXT NOT NULL,
                amount REAL,
                currency TEXT NOT NULL,
                description TEXT NOT NULL,
                status TEXT NOT NULL,
                payment_id TEXT,
                payment_url TEXT,
                metadata JSONB,
                created_at TIMESTAMP NOT NULL DEFAULT NOW(),
                paid_at TIMESTAMP,
                failed_at TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_payment_invoices_customer_id ON payment_invoices(customer_id);
            CREATE INDEX IF NOT EXISTS idx_payment_invoices_status ON payment_invoices(status);
            CREATE INDEX IF NOT EXISTS idx_payment_invoices_created_at ON payment_invoices(created_at DESC);
        "#;

        sqlx::query(create_tables_sql)
            .execute(self.pool.clone())
            .await?;

        log::info!("Invoice storage tables created successfully");
        Ok(())
    }

    pub async fn save_invoice(&self, invoice: &PaymentInvoice) -> Result<()> {
        let insert_sql = r#"
            INSERT INTO payment_invoices (
                id, provider, customer_id, amount, currency, description,
                status, payment_id, payment_url, metadata, created_at, paid_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (id) DO UPDATE SET
                status = EXCLUDED.status,
                payment_id = EXCLUDED.payment_id,
                payment_url = EXCLUDED.payment_url,
                paid_at = EXCLUDED.paid_at,
                updated_at = NOW()
        "#;

        let metadata_json = serde_json::to_value(&invoice.metadata).unwrap_or(serde_json::Value::Null);

        sqlx::query(insert_sql)
            .bind(&invoice.id)
            .bind(&invoice.provider)
            .bind(&invoice.customer_id)
            .bind(invoice.amount)
            .bind(&invoice.currency)
            .bind(&invoice.description)
            .bind(invoice.status.as_str())
            .bind(&invoice.payment_id)
            .bind(&invoice.payment_url)
            .bind(&metadata_json)
            .bind(invoice.created_at)
            .bind(invoice.paid_at)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn get_invoice(&self, invoice_id: &str) -> Result<Option<PaymentInvoice>> {
        let query = r#"
            SELECT id, provider, customer_id, amount, currency, description,
                   status, payment_id, payment_url, metadata, created_at, paid_at
            FROM payment_invoices
            WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(invoice_id)
            .fetch_optional(self.pool.clone())
            .await?;

        match row {
            Some(row) => {
                let metadata_json: serde_json::Value = row.try_get("metadata")?;
                let metadata: HashMap<String, String> = serde_json::from_value(metadata_json)?;

                Ok(Some(PaymentInvoice {
                    id: row.try_get("id")?,
                    provider: row.try_get("provider")?,
                    customer_id: row.try_get("customer_id")?,
                    amount: row.try_get("amount"),
                    currency: row.try_get("currency")?,
                    description: row.try_get("description")?,
                    status: row.try_get("status")?,
                    payment_id: row.try_get("payment_id"),
                    payment_url: row.try_get("payment_url"),
                    metadata,
                    created_at: row.try_get("created_at")?,
                    paid_at: row.try_get("paid_at"),
                    failed_at: None,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn get_invoices_by_customer(&self, customer_id: &str) -> Result<Vec<PaymentInvoice>> {
        let query = r#"
            SELECT id, provider, customer_id, amount, currency, description,
                   status, payment_id, payment_url, metadata, created_at, paid_at
            FROM payment_invoices
            WHERE customer_id = $1
            ORDER BY created_at DESC
        "#;

        let rows = sqlx::query(query)
            .bind(customer_id)
            .fetch_all(self.pool.clone())
            .await?;

        let mut invoices = Vec::new();

        for row in rows {
            let metadata_json: serde_json::Value = row.try_get("metadata")?;
            let metadata: HashMap<String, String> = serde_json::from_value(metadata_json)?;

            invoices.push(PaymentInvoice {
                id: row.try_get("id")?,
                provider: row.try_get("provider")?,
                customer_id: row.try_get("customer_id")?,
                amount: row.try_get("amount"),
                currency: row.try_get("currency")?,
                description: row.try_get("description")?,
                status: row.try_get("status")?,
                payment_id: row.try_get("payment_id"),
                payment_url: row.try_get("payment_url"),
                metadata,
                created_at: row.try_get("created_at")?,
                paid_at: row.try_get("paid_at"),
                failed_at: None,
            });
        }

        Ok(invoices)
    }

    pub async fn get_invoices_by_status(&self, status: PaymentStatus) -> Result<Vec<PaymentInvoice>> {
        let query = r#"
            SELECT id, provider, customer_id, amount, currency, description,
                   status, payment_id, payment_url, metadata, created_at, paid_at
            FROM payment_invoices
            WHERE status = $1
            ORDER BY created_at DESC
        "#;

        let rows = sqlx::query(query)
            .bind(status.as_str())
            .fetch_all(self.pool.clone())
            .await?;

        let mut invoices = Vec::new();

        for row in rows {
            let metadata_json: serde_json::Value = row.try_get("metadata")?;
            let metadata: HashMap<String, String> = serde_json::from_value(metadata_json)?;

            invoices.push(PaymentInvoice {
                id: row.try_get("id")?,
                provider: row.try_get("provider")?,
                customer_id: row.try_get("customer_id")?,
                amount: row.try_get("amount"),
                currency: row.try_get("currency")?,
                description: row.try_get("description")?,
                status: row.try_get("status")?,
                payment_id: row.try_get("payment_id"),
                payment_url: row.try_get("payment_url"),
                metadata,
                created_at: row.try_get("created_at")?,
                paid_at: row.try_get("paid_at"),
                failed_at: None,
            });
        }

        Ok(invoices)
    }

    pub async fn get_all_invoices(&self) -> Result<Vec<PaymentInvoice>> {
        let query = r#"
            SELECT id, provider, customer_id, amount, currency, description,
                   status, payment_id, payment_url, metadata, created_at, paid_at
            FROM payment_invoices
            ORDER BY created_at DESC
        "#;

        let rows = sqlx::query(query).fetch_all(self.pool.clone()).await?;

        let mut invoices = Vec::new();

        for row in rows {
            let metadata_json: serde_json::Value = row.try_get("metadata")?;
            let metadata: HashMap<String, String> = serde_json::from_value(metadata_json)?;

            invoices.push(PaymentInvoice {
                id: row.try_get("id")?,
                provider: row.try_get("provider")?,
                customer_id: row.try_get("customer_id")?,
                amount: row.try_get("amount"),
                currency: row.try_get("currency")?,
                description: row.try_get("description")?,
                status: row.try_get("status")?,
                payment_id: row.try_get("payment_id"),
                payment_url: row.try_get("payment_url"),
                metadata,
                created_at: row.try_get("created_at")?,
                paid_at: row.try_get("paid_at"),
                failed_at: None,
            });
        }

        Ok(invoices)
    }

    pub async fn update_invoice_status(&self, invoice_id: &str, status: PaymentStatus) -> Result<()> {
        let update_sql = r#"
            UPDATE payment_invoices
            SET status = $1,
                updated_at = NOW()
            WHERE id = $2
        "#;

        if status == PaymentStatus::Paid {
            let update_with_paid = r#"
                UPDATE payment_invoices
                SET status = $1, paid_at = NOW(), updated_at = NOW()
                WHERE id = $2
            "#;
            sqlx::query(update_with_paid)
                .bind(status.as_str())
                .bind(invoice_id)
                .execute(self.pool.clone())
                .await?;
        } else {
            sqlx::query(update_sql)
                .bind(status.as_str())
                .bind(invoice_id)
                .execute(self.pool.clone())
                .await?;
        }

        Ok(())
    }

    pub async fn delete_invoice(&self, invoice_id: &str) -> Result<()> {
        let query = "DELETE FROM payment_invoices WHERE id = $1";

        sqlx::query(query)
            .bind(invoice_id)
            .execute(self.pool.clone())
            .await?;

        Ok(())
    }

    pub async fn cleanup_expired_invoices(&self) -> Result<i64> {
        let query = r#"
            DELETE FROM payment_invoices
            WHERE status IN ('failed', 'cancelled') AND created_at < NOW() - INTERVAL '30 days'
        "#;

        let result = sqlx::query(query)
            .execute(self.pool.clone())
            .await?;

        let deleted_count = result.rows_affected();
        log::info!("Cleaned up {} expired invoices", deleted_count);

        Ok(deleted_count)
    }
}
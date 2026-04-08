//! Invoice persistence

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;


#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct InvoiceRow {
    pub id: String,
    pub account_id: String,
    pub number: String,
    pub status: String,
    pub subtotal_kopecks: i64,
    pub tax_kopecks: i64,
    pub total_kopecks: i64,
    pub currency: String,
    pub payment_method: Option<String>,
    pub created_at: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
    pub due_at: DateTime<Utc>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct InvoiceItemRow {
    pub id: i64,
    pub invoice_id: String,
    pub description: String,
    pub quantity: i64,
    pub unit_price_kopecks: i64,
    pub total_kopecks: i64,
    pub sort_order: i32,
}

pub struct InvoiceRepo;

impl InvoiceRepo {
    /// Create invoice with items (transactional)
    pub async fn create(
        pool: &PgPool,
        invoice: &InvoiceRow,
        items: &[InvoiceItemRow],
    ) -> Result<String> {
        // Use a transaction
        let mut tx = pool.begin().await?;

        sqlx::query(
            "INSERT INTO invoices (id, account_id, number, status, subtotal_kopecks,
             tax_kopecks, total_kopecks, currency, payment_method, due_at, notes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
        )
        .bind(&invoice.id)
        .bind(&invoice.account_id)
        .bind(&invoice.number)
        .bind(&invoice.status)
        .bind(invoice.subtotal_kopecks)
        .bind(invoice.tax_kopecks)
        .bind(invoice.total_kopecks)
        .bind(&invoice.currency)
        .bind(&invoice.payment_method)
        .bind(invoice.due_at)
        .bind(&invoice.notes)
        .execute(&mut *tx)
        .await?;

        for item in items {
            sqlx::query(
                "INSERT INTO invoice_items (invoice_id, description, quantity,
                 unit_price_kopecks, total_kopecks, sort_order)
                 VALUES ($1, $2, $3, $4, $5, $6)"
            )
            .bind(&item.invoice_id)
            .bind(&item.description)
            .bind(item.quantity)
            .bind(item.unit_price_kopecks)
            .bind(item.total_kopecks)
            .bind(item.sort_order)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(invoice.id.clone())
    }

    /// Get invoice by ID
    pub async fn get(pool: &PgPool, id: &str) -> Result<Option<InvoiceRow>> {
        let row = sqlx::query_as::<_, InvoiceRow>(
            "SELECT * FROM invoices WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// List invoices for an account
    pub async fn list_for_account(pool: &PgPool, account_id: &str) -> Result<Vec<InvoiceRow>> {
        let rows = sqlx::query_as::<_, InvoiceRow>(
            "SELECT * FROM invoices WHERE account_id = $1 ORDER BY created_at DESC"
        )
        .bind(account_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Get invoice items
    pub async fn get_items(pool: &PgPool, invoice_id: &str) -> Result<Vec<InvoiceItemRow>> {
        let rows = sqlx::query_as::<_, InvoiceItemRow>(
            "SELECT * FROM invoice_items WHERE invoice_id = $1 ORDER BY sort_order"
        )
        .bind(invoice_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Update invoice status
    pub async fn update_status(pool: &PgPool, id: &str, status: &str) -> Result<()> {
        let paid_at = if status == "paid" { "paid_at = NOW()," } else { "" };

        sqlx::query(&format!(
            "UPDATE invoices SET status = $1, {paid_at} payment_method = COALESCE(payment_method, $2)
             WHERE id = $3"
        ))
        .bind(status)
        .bind(None::<String>)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Mark as paid
    pub async fn mark_paid(pool: &PgPool, id: &str, method: &str) -> Result<()> {
        sqlx::query(
            "UPDATE invoices SET status = 'paid', paid_at = NOW(), payment_method = $1 WHERE id = $2"
        )
        .bind(method)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// List pending invoices
    pub async fn list_pending(pool: &PgPool) -> Result<Vec<InvoiceRow>> {
        let rows = sqlx::query_as::<_, InvoiceRow>(
            "SELECT * FROM invoices WHERE status = 'pending' ORDER BY due_at"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// List overdue invoices
    pub async fn list_overdue(pool: &PgPool) -> Result<Vec<InvoiceRow>> {
        let rows = sqlx::query_as::<_, InvoiceRow>(
            "SELECT * FROM invoices WHERE status = 'pending' AND due_at < NOW()"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Total revenue (sum of paid invoices)
    pub async fn total_revenue(pool: &PgPool) -> Result<i64> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total_kopecks), 0) FROM invoices WHERE status = 'paid'"
        )
        .fetch_one(pool)
        .await?;
        Ok(total)
    }

    /// Revenue for a specific account
    pub async fn account_revenue(pool: &PgPool, account_id: &str) -> Result<i64> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total_kopecks), 0) FROM invoices
             WHERE account_id = $1 AND status = 'paid'"
        )
        .bind(account_id)
        .fetch_one(pool)
        .await?;
        Ok(total)
    }

    /// Generate next invoice number
    pub async fn next_number(pool: &PgPool) -> Result<String> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM invoices"
        )
        .fetch_one(pool)
        .await?;
        Ok(format!("INV-{:04}", count + 1))
    }
}

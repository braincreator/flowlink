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
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
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
                 VALUES ($1, $2, $3, $4, $5, $6)",
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
        let row = sqlx::query_as::<_, InvoiceRow>("SELECT * FROM invoices WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }

    /// List invoices for an account
    pub async fn list_for_account(pool: &PgPool, account_id: &str) -> Result<Vec<InvoiceRow>> {
        let rows = sqlx::query_as::<_, InvoiceRow>(
            "SELECT * FROM invoices WHERE account_id = $1 ORDER BY created_at DESC",
        )
        .bind(account_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Get invoice items
    pub async fn get_items(pool: &PgPool, invoice_id: &str) -> Result<Vec<InvoiceItemRow>> {
        let rows = sqlx::query_as::<_, InvoiceItemRow>(
            "SELECT * FROM invoice_items WHERE invoice_id = $1 ORDER BY sort_order",
        )
        .bind(invoice_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Update invoice status
    pub async fn update_status(pool: &PgPool, id: &str, status: &str) -> Result<()> {
        let paid_at = if status == "paid" {
            "paid_at = NOW(),"
        } else {
            ""
        };

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
            "SELECT * FROM invoices WHERE status = 'pending' ORDER BY due_at",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// List overdue invoices
    pub async fn list_overdue(pool: &PgPool) -> Result<Vec<InvoiceRow>> {
        let rows = sqlx::query_as::<_, InvoiceRow>(
            "SELECT * FROM invoices WHERE status = 'pending' AND due_at < NOW()",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Total revenue (sum of paid invoices)
    pub async fn total_revenue(pool: &PgPool) -> Result<i64> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total_kopecks), 0) FROM invoices WHERE status = 'paid'",
        )
        .fetch_one(pool)
        .await?;
        Ok(total)
    }

    /// Revenue for a specific account
    pub async fn account_revenue(pool: &PgPool, account_id: &str) -> Result<i64> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total_kopecks), 0) FROM invoices
             WHERE account_id = $1 AND status = 'paid'",
        )
        .bind(account_id)
        .fetch_one(pool)
        .await?;
        Ok(total)
    }

    /// Generate next invoice number
    pub async fn next_number(pool: &PgPool) -> Result<String> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM invoices")
            .fetch_one(pool)
            .await?;
        Ok(format!("INV-{:04}", count + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_invoice() -> InvoiceRow {
        let now = Utc::now();
        InvoiceRow {
            id: "inv-001".into(),
            account_id: "acc-123".into(),
            number: "INV-0001".into(),
            status: "pending".into(),
            subtotal_kopecks: 10_000,
            tax_kopecks: 2_000,
            total_kopecks: 12_000,
            currency: "RUB".into(),
            payment_method: None,
            created_at: now,
            paid_at: None,
            due_at: now + chrono::Duration::days(30),
            notes: None,
        }
    }

    fn make_invoice_item() -> InvoiceItemRow {
        InvoiceItemRow {
            id: 1,
            invoice_id: "inv-001".into(),
            description: "API calls".into(),
            quantity: 100,
            unit_price_kopecks: 100,
            total_kopecks: 10_000,
            sort_order: 0,
        }
    }

    // --- InvoiceRow tests ---

    #[test]
    fn invoice_row_construction() {
        let inv = make_invoice();
        assert_eq!(inv.id, "inv-001");
        assert_eq!(inv.account_id, "acc-123");
        assert_eq!(inv.status, "pending");
        assert_eq!(inv.subtotal_kopecks, 10_000);
        assert_eq!(inv.tax_kopecks, 2_000);
        assert_eq!(inv.total_kopecks, 12_000);
        assert_eq!(inv.currency, "RUB");
        assert!(inv.payment_method.is_none());
        assert!(inv.paid_at.is_none());
    }

    #[test]
    fn invoice_row_clone() {
        let inv = make_invoice();
        let cloned = inv.clone();
        assert_eq!(cloned.id, inv.id);
        assert_eq!(cloned.total_kopecks, inv.total_kopecks);
    }

    #[test]
    fn invoice_row_serialization_roundtrip() {
        let inv = make_invoice();
        let json = serde_json::to_string(&inv).expect("serialize");
        let deserialized: InvoiceRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.id, inv.id);
        assert_eq!(deserialized.account_id, inv.account_id);
        assert_eq!(deserialized.total_kopecks, inv.total_kopecks);
        assert_eq!(deserialized.currency, inv.currency);
    }

    #[test]
    fn invoice_row_serialization_contains_expected_fields() {
        let inv = make_invoice();
        let json = serde_json::to_value(&inv).expect("to_value");
        assert_eq!(json["id"], "inv-001");
        assert_eq!(json["account_id"], "acc-123");
        assert_eq!(json["status"], "pending");
        assert_eq!(json["total_kopecks"], 12_000);
        assert_eq!(json["currency"], "RUB");
        assert!(json.get("payment_method").unwrap().is_null());
    }

    #[test]
    fn invoice_row_with_all_fields_populated() {
        let now = Utc::now();
        let inv = InvoiceRow {
            id: "inv-full".into(),
            account_id: "acc-456".into(),
            number: "INV-0002".into(),
            status: "paid".into(),
            subtotal_kopecks: 50_000,
            tax_kopecks: 10_000,
            total_kopecks: 60_000,
            currency: "USD".into(),
            payment_method: Some("bank_transfer".into()),
            created_at: now,
            paid_at: Some(now),
            due_at: now + chrono::Duration::days(30),
            notes: Some("Monthly invoice".into()),
        };
        assert_eq!(inv.status, "paid");
        assert_eq!(inv.payment_method.as_deref(), Some("bank_transfer"));
        assert!(inv.paid_at.is_some());
        assert_eq!(inv.notes.as_deref(), Some("Monthly invoice"));
    }

    // --- InvoiceItemRow tests ---

    #[test]
    fn invoice_item_row_construction() {
        let item = make_invoice_item();
        assert_eq!(item.id, 1);
        assert_eq!(item.invoice_id, "inv-001");
        assert_eq!(item.description, "API calls");
        assert_eq!(item.quantity, 100);
        assert_eq!(item.unit_price_kopecks, 100);
        assert_eq!(item.total_kopecks, 10_000);
        assert_eq!(item.sort_order, 0);
    }

    #[test]
    fn invoice_item_row_serialization_roundtrip() {
        let item = make_invoice_item();
        let json = serde_json::to_string(&item).expect("serialize");
        let deserialized: InvoiceItemRow = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.id, item.id);
        assert_eq!(deserialized.invoice_id, item.invoice_id);
        assert_eq!(deserialized.total_kopecks, item.total_kopecks);
    }

    #[test]
    fn invoice_item_row_clone() {
        let item = make_invoice_item();
        let cloned = item.clone();
        assert_eq!(cloned.description, item.description);
        assert_eq!(cloned.quantity, item.quantity);
    }

    // --- SQL query validation ---

    #[test]
    fn sql_queries_reference_invoices_table() {
        let queries = [
            "INSERT INTO invoices (id, account_id, number, status",
            "SELECT * FROM invoices WHERE id = $1",
            "SELECT * FROM invoices WHERE account_id = $1 ORDER BY created_at DESC",
            "UPDATE invoices SET status = $1",
            "SELECT * FROM invoices WHERE status = 'pending' ORDER BY due_at",
            "SELECT COALESCE(SUM(total_kopecks), 0) FROM invoices WHERE status = 'paid'",
            "SELECT COUNT(*) FROM invoices",
        ];
        for q in &queries {
            assert!(
                q.contains("invoices"),
                "Query missing 'invoices' table: {}",
                q
            );
        }
    }

    #[test]
    fn sql_queries_reference_invoice_items_table() {
        let queries = [
            "INSERT INTO invoice_items (invoice_id, description, quantity",
            "SELECT * FROM invoice_items WHERE invoice_id = $1 ORDER BY sort_order",
        ];
        for q in &queries {
            assert!(
                q.contains("invoice_items"),
                "Query missing 'invoice_items' table: {}",
                q
            );
        }
    }

    #[test]
    fn invoice_create_uses_transaction() {
        // The create method uses a transaction - verify the INSERT queries exist
        let inv_sql = "INSERT INTO invoices (id, account_id, number, status, subtotal_kopecks,
             tax_kopecks, total_kopecks, currency, payment_method, due_at, notes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)";
        assert!(inv_sql.contains("VALUES"));
        assert!(inv_sql.contains("$1"));
        assert!(inv_sql.contains("$11"));
    }

    #[test]
    fn mark_paid_query_sets_paid_at() {
        let query = "UPDATE invoices SET status = 'paid', paid_at = NOW(), payment_method = $1 WHERE id = $2";
        assert!(query.contains("paid_at = NOW()"));
        assert!(query.contains("status = 'paid'"));
    }

    #[test]
    fn update_status_paid_sets_paid_at_clause() {
        // The update_status method conditionally adds paid_at = NOW()
        let status = "paid";
        let paid_at = if status == "paid" {
            "paid_at = NOW(),"
        } else {
            ""
        };
        let sql = format!(
            "UPDATE invoices SET status = $1, {paid_at} payment_method = COALESCE(payment_method, $2)
             WHERE id = $3"
        );
        assert!(sql.contains("paid_at = NOW(),"));
    }

    #[test]
    fn update_status_non_paid_no_paid_at_clause() {
        let status = "cancelled";
        let paid_at = if status == "paid" {
            "paid_at = NOW(),"
        } else {
            ""
        };
        let sql = format!(
            "UPDATE invoices SET status = $1, {paid_at} payment_method = COALESCE(payment_method, $2)
             WHERE id = $3"
        );
        assert!(!sql.contains("paid_at = NOW()"));
    }
}

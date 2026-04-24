//! Платёжные заказы — CRUD для разовых платежей через Точка Банк

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// Строка заказа из БД
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct OrderRow {
    pub id: String,
    pub account_id: String,
    pub invoice_id: Option<String>,
    pub amount_kopecks: i64,
    pub description: Option<String>,
    pub status: String,
    pub payment_method: String,
    pub tochka_payment_id: Option<String>,
    pub payment_url: Option<String>,
    pub plan_id: Option<String>,
    pub paid_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub struct OrderRepo;

impl OrderRepo {
    /// Создать новый заказ
    pub async fn create(
        pool: &PgPool,
        id: &str,
        account_id: &str,
        amount_kopecks: i64,
        description: Option<&str>,
        payment_method: &str,
    ) -> Result<OrderRow> {
        let row = sqlx::query_as::<_, OrderRow>(
            r#"INSERT INTO orders (id, account_id, amount_kopecks, description, payment_method)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id, account_id, invoice_id, amount_kopecks, description,
                         status, payment_method, tochka_payment_id, payment_url,
                         plan_id, paid_at, failed_at, created_at"#,
        )
        .bind(id)
        .bind(account_id)
        .bind(amount_kopecks)
        .bind(description)
        .bind(payment_method)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    /// Отметить заказ как оплаченный
    pub async fn update_paid(pool: &PgPool, id: &str, tochka_payment_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE orders SET status = 'paid', tochka_payment_id = $1, paid_at = NOW() WHERE id = $2",
        )
        .bind(tochka_payment_id)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Отметить заказ как failed
    pub async fn update_failed(pool: &PgPool, id: &str) -> Result<()> {
        sqlx::query("UPDATE orders SET status = 'failed', failed_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Получить заказ по ID
    pub async fn get(pool: &PgPool, id: &str) -> Result<Option<OrderRow>> {
        let row = sqlx::query_as::<_, OrderRow>(
            r#"SELECT id, account_id, invoice_id, amount_kopecks, description,
                      status, payment_method, tochka_payment_id, payment_url,
                      plan_id, paid_at, failed_at, created_at
               FROM orders WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Получить все заказы аккаунта
    pub async fn list_for_account(pool: &PgPool, account_id: &str) -> Result<Vec<OrderRow>> {
        let rows = sqlx::query_as::<_, OrderRow>(
            r#"SELECT id, account_id, invoice_id, amount_kopecks, description,
                      status, payment_method, tochka_payment_id, payment_url,
                      plan_id, paid_at, failed_at, created_at
               FROM orders
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

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    fn make_order() -> OrderRow {
        OrderRow {
            id: "ord_001".to_string(),
            account_id: "acct_001".to_string(),
            invoice_id: Some("inv_001".to_string()),
            amount_kopecks: 9900,
            description: Some("Pro plan monthly".to_string()),
            status: "paid".to_string(),
            payment_method: "card".to_string(),
            tochka_payment_id: Some("tp_001".to_string()),
            payment_url: Some("https://pay.example.com/ord_001".to_string()),
            plan_id: Some("plan_pro".to_string()),
            paid_at: Some(now()),
            failed_at: None,
            created_at: now(),
        }
    }

    #[test]
    fn order_row_construction() {
        let o = make_order();
        assert_eq!(o.id, "ord_001");
        assert_eq!(o.account_id, "acct_001");
        assert_eq!(o.amount_kopecks, 9900);
        assert_eq!(o.status, "paid");
    }

    #[test]
    fn order_row_all_options_none() {
        let o = OrderRow {
            id: "ord_min".to_string(),
            account_id: "acct".to_string(),
            invoice_id: None,
            amount_kopecks: 100,
            description: None,
            status: "pending".to_string(),
            payment_method: "bank".to_string(),
            tochka_payment_id: None,
            payment_url: None,
            plan_id: None,
            paid_at: None,
            failed_at: None,
            created_at: now(),
        };
        assert!(o.invoice_id.is_none());
        assert!(o.description.is_none());
        assert!(o.tochka_payment_id.is_none());
        assert!(o.payment_url.is_none());
        assert!(o.plan_id.is_none());
        assert!(o.paid_at.is_none());
        assert!(o.failed_at.is_none());
    }

    #[test]
    fn order_row_all_options_some() {
        let o = make_order();
        assert!(o.invoice_id.is_some());
        assert!(o.description.is_some());
        assert!(o.tochka_payment_id.is_some());
        assert!(o.payment_url.is_some());
        assert!(o.plan_id.is_some());
        assert!(o.paid_at.is_some());
    }

    #[test]
    fn order_row_clone() {
        let o = make_order();
        let cloned = o.clone();
        assert_eq!(cloned.id, o.id);
        assert_eq!(cloned.amount_kopecks, o.amount_kopecks);
    }

    #[test]
    fn order_row_debug() {
        let o = make_order();
        let debug = format!("{:?}", o);
        assert!(debug.contains("ord_001"));
    }

    #[test]

    #[test]
    fn order_row_statuses() {
        for status in &["pending", "paid", "failed", "refunded", "expired"] {
            let mut o = make_order();
            o.status = status.to_string();
            assert_eq!(o.status, *status);
        }
    }

    #[test]
    fn order_row_payment_methods() {
        for method in &["card", "bank_transfer", "sbp"] {
            let mut o = make_order();
            o.payment_method = method.to_string();
            assert_eq!(o.payment_method, *method);
        }
    }

    #[test]
    fn order_row_zero_amount() {
        let mut o = make_order();
        o.amount_kopecks = 0;
        assert_eq!(o.amount_kopecks, 0);
    }

    #[test]
    fn order_row_large_amount() {
        let mut o = make_order();
        o.amount_kopecks = 999_999_99;
        assert_eq!(o.amount_kopecks, 999_999_99);
    }

    #[test]
    fn order_row_negative_amount() {
        let mut o = make_order();
        o.amount_kopecks = -500;
        assert_eq!(o.amount_kopecks, -500);
    }

    #[test]
    fn order_row_empty_description() {
        let mut o = make_order();
        o.description = Some(String::new());
        assert_eq!(o.description.as_deref(), Some(""));
    }

    #[test]
    fn order_row_empty_strings() {
        let o = OrderRow {
            id: String::new(),
            account_id: String::new(),
            invoice_id: None,
            amount_kopecks: 0,
            description: None,
            status: String::new(),
            payment_method: String::new(),
            tochka_payment_id: None,
            payment_url: None,
            plan_id: None,
            paid_at: None,
            failed_at: None,
            created_at: now(),
        };
        assert!(o.id.is_empty());
        assert!(o.account_id.is_empty());
    }

    #[test]
    fn order_row_timestamps() {
        let o = make_order();
        assert!(o.paid_at.is_some());
        assert!(o.failed_at.is_none());
        assert!(o.created_at <= Utc::now());
    }

    #[test]
    fn order_row_with_invoice_id() {
        let o = make_order();
        assert_eq!(o.invoice_id.as_deref(), Some("inv_001"));
    }

    #[test]
    fn order_row_with_plan_id() {
        let o = make_order();
        assert_eq!(o.plan_id.as_deref(), Some("plan_pro"));
    }

    #[test]
    fn order_row_with_tochka_and_url() {
        let o = make_order();
        assert!(o.tochka_payment_id.as_ref().unwrap().starts_with("tp_"));
        assert!(o.payment_url.as_ref().unwrap().starts_with("https://"));
    }

    #[test]
    fn order_repo_exists() {
        let _repo = OrderRepo;
    }

    #[test]
    fn order_row_failed_at_some() {
        let mut o = make_order();
        o.failed_at = Some(now());
        o.paid_at = None;
        assert!(o.failed_at.is_some());
        assert!(o.paid_at.is_none());
    }

    #[test]
    fn order_row_both_timestamps_some() {
        let mut o = make_order();
        o.paid_at = Some(now());
        o.failed_at = Some(now());
        assert!(o.paid_at.is_some());
        assert!(o.failed_at.is_some());
    }
}

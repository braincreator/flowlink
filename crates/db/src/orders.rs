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
                         paid_at, failed_at, created_at"#,
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
        sqlx::query(
            "UPDATE orders SET status = 'failed', failed_at = NOW() WHERE id = $1",
        )
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
                      paid_at, failed_at, created_at
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
                      paid_at, failed_at, created_at
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

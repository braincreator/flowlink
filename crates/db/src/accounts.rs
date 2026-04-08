//! Account persistence — CRUD for account billing state

use anyhow::Result;
use sqlx::PgPool;
use chrono::{DateTime, Utc};


/// Account row from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AccountRow {
    pub account_id: String,
    pub plan_id: String,
    pub active: bool,
    pub balance_kopecks: i64,
    pub payment_method: Option<String>,
    pub activated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub cycle_start: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct AccountRepo;

impl AccountRepo {
    /// Get account by ID
    pub async fn get(pool: &PgPool, account_id: &str) -> Result<Option<AccountRow>> {
        let row = sqlx::query_as::<_, AccountRow>(
            "SELECT account_id, plan_id, active, balance_kopecks, payment_method,
                    activated_at, expires_at, cycle_start, created_at, updated_at
             FROM accounts WHERE account_id = $1"
        )
        .bind(account_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Create a new account (skip if exists)
    pub async fn create(pool: &PgPool, account_id: &str, plan_id: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO accounts (account_id, plan_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"
        )
        .bind(account_id)
        .bind(plan_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Update plan
    pub async fn update_plan(pool: &PgPool, account_id: &str, plan_id: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE accounts SET plan_id = $1, activated_at = NOW(), cycle_start = NOW(),
             expires_at = CASE WHEN $1 = 'free' THEN NULL ELSE NOW() + INTERVAL '30 days' END,
             updated_at = NOW()
             WHERE account_id = $2"
        )
        .bind(plan_id)
        .bind(account_id)
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            anyhow::bail!("Account not found: {}", account_id);
        }
        Ok(())
    }

    /// Update balance (returns new balance)
    pub async fn update_balance(pool: &PgPool, account_id: &str, delta_kopecks: i64) -> Result<i64> {
        sqlx::query_scalar(
            "UPDATE accounts SET balance_kopecks = balance_kopecks + $1, updated_at = NOW()
             WHERE account_id = $2 RETURNING balance_kopecks"
        )
        .bind(delta_kopecks)
        .bind(account_id)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
    }

    /// Set active/inactive
    pub async fn set_active(pool: &PgPool, account_id: &str, active: bool) -> Result<()> {
        sqlx::query(
            "UPDATE accounts SET active = $1, updated_at = NOW() WHERE account_id = $2"
        )
        .bind(active)
        .bind(account_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Set payment method
    pub async fn set_payment_method(pool: &PgPool, account_id: &str, method: Option<&str>) -> Result<()> {
        sqlx::query(
            "UPDATE accounts SET payment_method = $1, updated_at = NOW() WHERE account_id = $2"
        )
        .bind(method)
        .bind(account_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// List all accounts
    pub async fn list(pool: &PgPool) -> Result<Vec<AccountRow>> {
        let rows = sqlx::query_as::<_, AccountRow>(
            "SELECT account_id, plan_id, active, balance_kopecks, payment_method,
                    activated_at, expires_at, cycle_start, created_at, updated_at
             FROM accounts ORDER BY created_at"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Count accounts by plan
    pub async fn count_by_plan(pool: &PgPool) -> Result<Vec<(String, i64)>> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT plan_id, COUNT(*) FROM accounts GROUP BY plan_id"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Get or create account
    pub async fn get_or_create(pool: &PgPool, account_id: &str, default_plan: &str) -> Result<AccountRow> {
        if let Some(acc) = Self::get(pool, account_id).await? {
            return Ok(acc);
        }
        Self::create(pool, account_id, default_plan).await?;
        Self::get(pool, account_id).await?
            .ok_or_else(|| anyhow::anyhow!("Account disappeared after create"))
    }
}

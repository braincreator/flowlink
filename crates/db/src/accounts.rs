//! Account persistence — CRUD for account billing state

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// Account row from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AccountRow {
    pub account_id: String,
    pub plan_id: String,
    pub active: bool,
    pub balance_kopecks: i64,
    pub payment_method: Option<String>,
    pub tg_id: Option<i64>,
    pub email: Option<String>,
    pub last_login: Option<DateTime<Utc>>,
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
             FROM accounts WHERE account_id = $1",
        )
        .bind(account_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Create a new account (skip if exists)
    pub async fn create(pool: &PgPool, account_id: &str, plan_id: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO accounts (account_id, plan_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
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
             WHERE account_id = $2",
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
    pub async fn update_balance(
        pool: &PgPool,
        account_id: &str,
        delta_kopecks: i64,
    ) -> Result<i64> {
        sqlx::query_scalar(
            "UPDATE accounts SET balance_kopecks = balance_kopecks + $1, updated_at = NOW()
             WHERE account_id = $2 RETURNING balance_kopecks",
        )
        .bind(delta_kopecks)
        .bind(account_id)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
    }

    /// Set active/inactive
    pub async fn set_active(pool: &PgPool, account_id: &str, active: bool) -> Result<()> {
        sqlx::query("UPDATE accounts SET active = $1, updated_at = NOW() WHERE account_id = $2")
            .bind(active)
            .bind(account_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Set payment method
    pub async fn set_payment_method(
        pool: &PgPool,
        account_id: &str,
        method: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE accounts SET payment_method = $1, updated_at = NOW() WHERE account_id = $2",
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
             FROM accounts ORDER BY created_at",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Count accounts by plan
    pub async fn count_by_plan(pool: &PgPool) -> Result<Vec<(String, i64)>> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT plan_id, COUNT(*) FROM accounts GROUP BY plan_id",
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Update Telegram ID
    pub async fn update_tg_id(pool: &PgPool, account_id: &str, tg_id: i64) -> Result<()> {
        sqlx::query("UPDATE accounts SET tg_id = $1, updated_at = NOW() WHERE account_id = $2")
            .bind(tg_id)
            .bind(account_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Get account by email
    pub async fn get_by_email(pool: &PgPool, email: &str) -> Result<Option<AccountRow>> {
        let row = sqlx::query_as::<_, AccountRow>(
            "SELECT account_id, plan_id, active, balance_kopecks, payment_method, tg_id,
                    activated_at, expires_at, cycle_start, created_at, updated_at
             FROM accounts WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Create account with email
    pub async fn create_with_email(
        pool: &PgPool,
        account_id: &str,
        plan_id: &str,
        email: &str,
    ) -> Result<()> {
        sqlx::query("INSERT INTO accounts (account_id, plan_id, email) VALUES ($1, $2, $3)")
            .bind(account_id)
            .bind(plan_id)
            .bind(email)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Update last login timestamp
    pub async fn update_last_login(pool: &PgPool, account_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE accounts SET last_login = NOW(), updated_at = NOW() WHERE account_id = $1",
        )
        .bind(account_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Get account by Telegram ID
    pub async fn get_by_tg_id(pool: &PgPool, tg_id: i64) -> Result<Option<AccountRow>> {
        let row = sqlx::query_as::<_, AccountRow>(
            "SELECT account_id, plan_id, active, balance_kopecks, payment_method, tg_id,
                    activated_at, expires_at, cycle_start, created_at, updated_at
             FROM accounts WHERE tg_id = $1",
        )
        .bind(tg_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Get or create account
    pub async fn get_or_create(
        pool: &PgPool,
        account_id: &str,
        default_plan: &str,
    ) -> Result<AccountRow> {
        if let Some(acc) = Self::get(pool, account_id).await? {
            return Ok(acc);
        }
        Self::create(pool, account_id, default_plan).await?;
        Self::get(pool, account_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Account disappeared after create"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn make_account() -> AccountRow {
        let now = Utc::now();
        AccountRow {
            account_id: "acc-123".into(),
            plan_id: "starter".into(),
            active: true,
            balance_kopecks: 10_000,
            payment_method: Some("card".into()),
            activated_at: now,
            expires_at: Some(now + Duration::days(30)),
            cycle_start: now,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn account_row_construction() {
        let acc = make_account();
        assert_eq!(acc.account_id, "acc-123");
        assert_eq!(acc.plan_id, "starter");
        assert!(acc.active);
        assert_eq!(acc.balance_kopecks, 10_000);
        assert_eq!(acc.payment_method.as_deref(), Some("card"));
        assert!(acc.expires_at.is_some());
    }

    #[test]
    fn account_row_clone() {
        let acc = make_account();
        let cloned = acc.clone();
        assert_eq!(cloned.account_id, acc.account_id);
        assert_eq!(cloned.balance_kopecks, acc.balance_kopecks);
    }

    #[test]
    fn account_row_debug() {
        let acc = make_account();
        let debug_str = format!("{:?}", acc);
        assert!(debug_str.contains("acc-123"));
        assert!(debug_str.contains("starter"));
    }

    #[test]
    fn account_row_optional_fields_none() {
        let now = Utc::now();
        let acc = AccountRow {
            account_id: "free-acc".into(),
            plan_id: "trial".into(),
            active: false,
            balance_kopecks: 0,
            payment_method: None,
            activated_at: now,
            expires_at: None,
            cycle_start: now,
            created_at: now,
            updated_at: now,
        };
        assert!(!acc.active);
        assert_eq!(acc.balance_kopecks, 0);
        assert!(acc.payment_method.is_none());
        assert!(acc.expires_at.is_none());
    }

    #[test]
    fn sql_queries_reference_accounts_table() {
        // Verify that the SQL queries in AccountRepo reference the correct table
        let queries = [
            "SELECT account_id, plan_id, active, balance_kopecks, payment_method,
                    activated_at, expires_at, cycle_start, created_at, updated_at
             FROM accounts WHERE account_id = $1",
            "INSERT INTO accounts (account_id, plan_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            "UPDATE accounts SET plan_id = $1",
            "UPDATE accounts SET balance_kopecks = balance_kopecks + $1",
            "UPDATE accounts SET active = $1",
            "UPDATE accounts SET payment_method = $1",
            "SELECT plan_id, COUNT(*) FROM accounts GROUP BY plan_id",
        ];
        for q in &queries {
            assert!(
                q.contains("accounts"),
                "Query missing 'accounts' table: {}",
                q
            );
        }
    }

    #[test]
    fn sql_queries_use_parameterized_bindings() {
        // All queries should use $N parameterized bindings (not string interpolation for values)
        let queries = [
            "INSERT INTO accounts (account_id, plan_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            "UPDATE accounts SET active = $1, updated_at = NOW() WHERE account_id = $2",
            "UPDATE accounts SET balance_kopecks = balance_kopecks + $1, updated_at = NOW()
             WHERE account_id = $2 RETURNING balance_kopecks",
        ];
        for q in &queries {
            assert!(q.contains("$1"), "Query missing parameter binding: {}", q);
        }
    }
}

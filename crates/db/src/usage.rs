//! Daily usage persistence

use anyhow::Result;
use chrono::{Utc, NaiveDate};
use sqlx::PgPool;


#[derive(Debug, Clone, Default)]
pub struct UsageRow {
    pub api_requests: i64,
    pub tokens: i64,
    pub active_agents: i64,
    pub storage_bytes: i64,
    pub api_requests_total: i64,
    pub tokens_total: i64,
}

pub struct UsageRepo;

impl UsageRepo {
    /// Increment a counter for today
    pub async fn increment(
        pool: &PgPool,
        account_id: &str,
        field: &str,
        amount: i64,
    ) -> Result<()> {
        let today = Utc::now().date_naive();

        sqlx::query(&format!(
            "INSERT INTO usage_daily (account_id, date, {field}) VALUES ($1, $2, $3)
             ON CONFLICT (account_id, date) DO UPDATE SET {field} = usage_daily.{field} + $3"
        ))
        .bind(account_id)
        .bind(today)
        .bind(amount)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Get today's usage
    pub async fn get_today(pool: &PgPool, account_id: &str) -> Result<UsageRow> {
        let today = Utc::now().date_naive();

        let row: Option<(i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT api_requests, tokens, active_agents, storage_bytes,
                    api_requests_total, tokens_total
             FROM usage_daily WHERE account_id = $1 AND date = $2"
        )
        .bind(account_id)
        .bind(today)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|(a, t, ag, s, at, tt)| UsageRow {
            api_requests: a, tokens: t, active_agents: ag,
            storage_bytes: s, api_requests_total: at, tokens_total: tt,
        }).unwrap_or_default())
    }

    /// Get usage for a date range
    pub async fn get_range(
        pool: &PgPool,
        account_id: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<(NaiveDate, UsageRow)>> {
        let rows: Vec<(NaiveDate, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT date, api_requests, tokens, active_agents, storage_bytes,
                    api_requests_total, tokens_total
             FROM usage_daily
             WHERE account_id = $1 AND date BETWEEN $2 AND $3
             ORDER BY date"
        )
        .bind(account_id)
        .bind(from)
        .bind(to)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|(d, a, t, ag, s, at, tt)| {
            (d, UsageRow {
                api_requests: a, tokens: t, active_agents: ag,
                storage_bytes: s, api_requests_total: at, tokens_total: tt,
            })
        }).collect())
    }

    /// Top accounts by API requests today (admin)
    pub async fn top_by_requests(pool: &PgPool, limit: i64) -> Result<Vec<(String, i64)>> {
        let today = Utc::now().date_naive();

        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT account_id, api_requests FROM usage_daily
             WHERE date = $1 ORDER BY api_requests DESC LIMIT $2"
        )
        .bind(today)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Reset daily counters (call at midnight)
    pub async fn reset_daily(pool: &PgPool) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE usage_daily SET api_requests = 0, tokens = 0"
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}

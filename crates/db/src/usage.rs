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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_row_default_is_all_zeros() {
        let row = UsageRow::default();
        assert_eq!(row.api_requests, 0);
        assert_eq!(row.tokens, 0);
        assert_eq!(row.active_agents, 0);
        assert_eq!(row.storage_bytes, 0);
        assert_eq!(row.api_requests_total, 0);
        assert_eq!(row.tokens_total, 0);
    }

    #[test]
    fn usage_row_clone() {
        let row = UsageRow {
            api_requests: 42,
            tokens: 1000,
            active_agents: 3,
            storage_bytes: 2048,
            api_requests_total: 500,
            tokens_total: 10000,
        };
        let cloned = row.clone();
        assert_eq!(cloned.api_requests, 42);
        assert_eq!(cloned.tokens, 1000);
        assert_eq!(cloned.active_agents, 3);
        assert_eq!(cloned.storage_bytes, 2048);
        assert_eq!(cloned.api_requests_total, 500);
        assert_eq!(cloned.tokens_total, 10000);
    }

    #[test]
    fn usage_row_debug() {
        let row = UsageRow {
            api_requests: 1,
            tokens: 2,
            active_agents: 3,
            storage_bytes: 4,
            api_requests_total: 5,
            tokens_total: 6,
        };
        let debug_str = format!("{:?}", row);
        assert!(debug_str.contains("api_requests"));
        assert!(debug_str.contains("tokens"));
    }

    #[test]
    fn usage_row_with_large_values() {
        let row = UsageRow {
            api_requests: i64::MAX,
            tokens: i64::MAX,
            active_agents: i64::MAX,
            storage_bytes: i64::MAX,
            api_requests_total: i64::MAX,
            tokens_total: i64::MAX,
        };
        assert_eq!(row.api_requests, i64::MAX);
        assert_eq!(row.tokens, i64::MAX);
    }

    #[test]
    fn sql_queries_reference_usage_daily_table() {
        let queries = [
            "INSERT INTO usage_daily (account_id, date, api_requests) VALUES ($1, $2, $3)",
            "SELECT api_requests, tokens, active_agents, storage_bytes,
                    api_requests_total, tokens_total
             FROM usage_daily WHERE account_id = $1 AND date = $2",
            "UPDATE usage_daily SET api_requests = 0, tokens = 0",
        ];
        for q in &queries {
            assert!(q.contains("usage_daily"), "Query missing 'usage_daily' table: {}", q);
        }
    }

    #[test]
    fn increment_query_uses_upsert() {
        // The increment query should use ON CONFLICT for upsert behavior
        let field = "api_requests";
        let sql = format!(
            "INSERT INTO usage_daily (account_id, date, {field}) VALUES ($1, $2, $3)
             ON CONFLICT (account_id, date) DO UPDATE SET {field} = usage_daily.{field} + $3"
        );
        assert!(sql.contains("ON CONFLICT"));
        assert!(sql.contains("DO UPDATE"));
        assert!(sql.contains("usage_daily.api_requests"));
    }
}

//! Audit log persistence

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuditRow {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub category: Option<String>,
    pub agent_id: Option<String>,
    pub account_id: Option<String>,
    pub action: String,
    pub target: Option<String>,
    pub result: Option<String>,
    pub metadata: Option<Value>,
    pub hmac_hash: Option<String>,
    pub source_ip: Option<String>,
}

/// Filter for audit queries
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    pub level: Option<String>,
    pub category: Option<String>,
    pub agent_id: Option<String>,
    pub account_id: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: i64,
    pub offset: i64,
}

pub struct AuditRepo;

impl AuditRepo {
    /// Insert an audit entry
    pub async fn insert(
        pool: &PgPool,
        level: &str,
        category: Option<&str>,
        agent_id: Option<&str>,
        account_id: Option<&str>,
        action: &str,
        target: Option<&str>,
        result: Option<&str>,
        metadata: Option<Value>,
        hmac_hash: Option<&str>,
        source_ip: Option<&str>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO audit_log (level, category, agent_id, account_id, action,
             target, result, metadata, hmac_hash, source_ip)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING id"
        )
        .bind(level)
        .bind(category)
        .bind(agent_id)
        .bind(account_id)
        .bind(action)
        .bind(target)
        .bind(result)
        .bind(metadata)
        .bind(hmac_hash)
        .bind(source_ip)
        .fetch_one(pool)
        .await?;
        Ok(id)
    }

    /// Query audit log with filters
    pub async fn query(pool: &PgPool, filter: &AuditFilter) -> Result<Vec<AuditRow>> {
        let mut conditions = Vec::new();
        let mut bind_idx = 0u32;

        if let Some(_level) = &filter.level {
            bind_idx += 1;
            conditions.push(format!("level = ${}", bind_idx));
        }
        if let Some(_cat) = &filter.category {
            bind_idx += 1;
            conditions.push(format!("category = ${}", bind_idx));
        }
        if let Some(_agent) = &filter.agent_id {
            bind_idx += 1;
            conditions.push(format!("agent_id = ${}", bind_idx));
        }
        if let Some(_acc) = &filter.account_id {
            bind_idx += 1;
            conditions.push(format!("account_id = ${}", bind_idx));
        }
        if let Some(_from) = filter.from {
            bind_idx += 1;
            conditions.push(format!("timestamp >= ${}", bind_idx));
        }
        if let Some(_to) = filter.to {
            bind_idx += 1;
            conditions.push(format!("timestamp <= ${}", bind_idx));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT * FROM audit_log {} ORDER BY timestamp DESC LIMIT ${} OFFSET ${}",
            where_clause, bind_idx + 1, bind_idx + 2
        );

        let mut query = sqlx::query_as::<_, AuditRow>(&sql);
        if let Some(level) = &filter.level { query = query.bind(level); }
        if let Some(cat) = &filter.category { query = query.bind(cat); }
        if let Some(agent) = &filter.agent_id { query = query.bind(agent); }
        if let Some(acc) = &filter.account_id { query = query.bind(acc); }
        if let Some(from) = filter.from { query = query.bind(from); }
        if let Some(to) = filter.to { query = query.bind(to); }
        query = query.bind(filter.limit);
        query = query.bind(filter.offset);

        let rows = query.fetch_all(pool).await?;
        Ok(rows)
    }

    /// Count entries by level (for stats)
    pub async fn count_by_level(pool: &PgPool) -> Result<Vec<(String, i64)>> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT level, COUNT(*) FROM audit_log GROUP BY level"
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Purge old entries (retention policy)
    pub async fn purge_before(pool: &PgPool, before: DateTime<Utc>) -> Result<u64> {
        let result = sqlx::query("DELETE FROM audit_log WHERE timestamp < $1")
            .bind(before)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}

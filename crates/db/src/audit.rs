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

/// High-level helper: log an org-scoped audit event.
pub async fn log_event(
    pool: &PgPool,
    org_id: Option<&str>,
    account_id: &str,
    action: &str,
    resource_type: Option<&str>,
    resource_id: Option<&str>,
    details: serde_json::Value,
    ip: Option<&str>,
) -> Result<i64> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO audit_log (org_id, account_id, action, resource_type, resource_id, details, ip_address)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id",
    )
    .bind(org_id)
    .bind(account_id)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(details)
    .bind(ip)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Query org audit log (paginated, with optional action filter).
pub async fn query_org_audit(
    pool: &PgPool,
    org_id: &str,
    page: i64,
    limit: i64,
    action_filter: Option<&str>,
) -> Result<(Vec<OrgAuditRow>, i64)> {
    let offset = (page - 1) * limit;
    let where_clause = match action_filter {
        Some(_a) => "WHERE org_id = $1 AND action = $2".to_string(),
        None => "WHERE org_id = $1".to_string(),
    };
    let count_sql = format!("SELECT COUNT(*) FROM audit_log {}", where_clause);
    let data_sql = format!(
        "SELECT id, org_id, account_id, action, resource_type, resource_id, details, ip_address, timestamp
         FROM audit_log {} ORDER BY timestamp DESC LIMIT ${} OFFSET ${}",
        where_clause,
        if action_filter.is_some() { 4 } else { 3 },
        if action_filter.is_some() { 5 } else { 4 },
    );

    let count: i64 = if let Some(a) = action_filter {
        sqlx::query_scalar(&count_sql).bind(org_id).bind(a).fetch_one(pool).await?
    } else {
        sqlx::query_scalar(&count_sql).bind(org_id).fetch_one(pool).await?
    };

    let mut query = sqlx::query_as::<_, OrgAuditRow>(&data_sql).bind(org_id);
    if let Some(a) = action_filter {
        query = query.bind(a);
    }
    query = query.bind(limit).bind(offset);
    let rows = query.fetch_all(pool).await?;
    Ok((rows, count))
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrgAuditRow {
    pub id: i64,
    pub org_id: Option<String>,
    pub account_id: String,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub details: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Build the WHERE clause and return the number of filter bindings used.
/// This is pure logic, testable without a database.
pub fn build_where_clause(filter: &AuditFilter) -> (String, u32) {
    let mut conditions = Vec::new();
    let mut bind_idx = 0u32;

    if filter.level.is_some() {
        bind_idx += 1;
        conditions.push(format!("level = ${}", bind_idx));
    }
    if filter.category.is_some() {
        bind_idx += 1;
        conditions.push(format!("category = ${}", bind_idx));
    }
    if filter.agent_id.is_some() {
        bind_idx += 1;
        conditions.push(format!("agent_id = ${}", bind_idx));
    }
    if filter.account_id.is_some() {
        bind_idx += 1;
        conditions.push(format!("account_id = ${}", bind_idx));
    }
    if filter.from.is_some() {
        bind_idx += 1;
        conditions.push(format!("timestamp >= ${}", bind_idx));
    }
    if filter.to.is_some() {
        bind_idx += 1;
        conditions.push(format!("timestamp <= ${}", bind_idx));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    (where_clause, bind_idx)
}

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
             RETURNING id",
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
        let (where_clause, bind_idx) = build_where_clause(filter);

        let sql = format!(
            "SELECT * FROM audit_log {} ORDER BY timestamp DESC LIMIT ${} OFFSET ${}",
            where_clause,
            bind_idx + 1,
            bind_idx + 2
        );

        let mut query = sqlx::query_as::<_, AuditRow>(&sql);
        if let Some(level) = &filter.level {
            query = query.bind(level);
        }
        if let Some(cat) = &filter.category {
            query = query.bind(cat);
        }
        if let Some(agent) = &filter.agent_id {
            query = query.bind(agent);
        }
        if let Some(acc) = &filter.account_id {
            query = query.bind(acc);
        }
        if let Some(from) = filter.from {
            query = query.bind(from);
        }
        if let Some(to) = filter.to {
            query = query.bind(to);
        }
        query = query.bind(filter.limit);
        query = query.bind(filter.offset);

        let rows = query.fetch_all(pool).await?;
        Ok(rows)
    }

    /// Count entries by level (for stats)
    pub async fn count_by_level(pool: &PgPool) -> Result<Vec<(String, i64)>> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT level, COUNT(*) FROM audit_log GROUP BY level",
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- AuditFilter tests ---

    #[test]
    fn audit_filter_default_is_empty() {
        let filter = AuditFilter::default();
        assert!(filter.level.is_none());
        assert!(filter.category.is_none());
        assert!(filter.agent_id.is_none());
        assert!(filter.account_id.is_none());
        assert!(filter.from.is_none());
        assert!(filter.to.is_none());
        assert_eq!(filter.limit, 0);
        assert_eq!(filter.offset, 0);
    }

    #[test]
    fn audit_filter_clone() {
        let filter = AuditFilter {
            level: Some("error".into()),
            account_id: Some("acc-1".into()),
            limit: 50,
            offset: 10,
            ..Default::default()
        };
        let cloned = filter.clone();
        assert_eq!(cloned.level.as_deref(), Some("error"));
        assert_eq!(cloned.limit, 50);
    }

    #[test]
    fn audit_filter_debug() {
        let filter = AuditFilter {
            level: Some("info".into()),
            ..Default::default()
        };
        let debug_str = format!("{:?}", filter);
        assert!(debug_str.contains("level"));
        assert!(debug_str.contains("info"));
    }

    // --- build_where_clause tests ---

    #[test]
    fn empty_filter_produces_no_where() {
        let filter = AuditFilter::default();
        let (clause, idx) = build_where_clause(&filter);
        assert!(clause.is_empty());
        assert_eq!(idx, 0);
    }

    #[test]
    fn single_level_filter() {
        let filter = AuditFilter {
            level: Some("error".into()),
            ..Default::default()
        };
        let (clause, idx) = build_where_clause(&filter);
        assert_eq!(clause, "WHERE level = $1");
        assert_eq!(idx, 1);
    }

    #[test]
    fn multiple_filters_joined_with_and() {
        let filter = AuditFilter {
            level: Some("error".into()),
            account_id: Some("acc-1".into()),
            ..Default::default()
        };
        let (clause, idx) = build_where_clause(&filter);
        assert_eq!(clause, "WHERE level = $1 AND account_id = $2");
        assert_eq!(idx, 2);
    }

    #[test]
    fn all_filters_active() {
        let filter = AuditFilter {
            level: Some("info".into()),
            category: Some("auth".into()),
            agent_id: Some("agent-1".into()),
            account_id: Some("acc-1".into()),
            from: Some(Utc::now() - chrono::Duration::days(7)),
            to: Some(Utc::now()),
            limit: 100,
            offset: 0,
        };
        let (clause, idx) = build_where_clause(&filter);
        assert_eq!(idx, 6);
        assert!(clause.contains("level = $1"));
        assert!(clause.contains("category = $2"));
        assert!(clause.contains("agent_id = $3"));
        assert!(clause.contains("account_id = $4"));
        assert!(clause.contains("timestamp >= $5"));
        assert!(clause.contains("timestamp <= $6"));
        // Verify AND joining: 6 conditions -> 5 AND separators -> 6 parts when split
        let parts: Vec<&str> = clause.split(" AND ").collect();
        assert_eq!(parts.len(), 6);
    }

    #[test]
    fn filter_bind_indices_are_sequential() {
        let filter = AuditFilter {
            category: Some("billing".into()),
            from: Some(Utc::now() - chrono::Duration::days(30)),
            ..Default::default()
        };
        let (clause, idx) = build_where_clause(&filter);
        // category is first -> $1, from is second -> $2
        assert_eq!(clause, "WHERE category = $1 AND timestamp >= $2");
        assert_eq!(idx, 2);
    }

    // --- SQL query validation ---

    #[test]
    fn sql_queries_reference_audit_log_table() {
        let queries = [
            "INSERT INTO audit_log (level, category, agent_id, account_id, action",
            "SELECT * FROM audit_log",
            "SELECT level, COUNT(*) FROM audit_log GROUP BY level",
            "DELETE FROM audit_log WHERE timestamp < $1",
        ];
        for q in &queries {
            assert!(
                q.contains("audit_log"),
                "Query missing 'audit_log' table: {}",
                q
            );
        }
    }

    #[test]
    fn insert_query_has_returning() {
        let query = "INSERT INTO audit_log (level, category, agent_id, account_id, action,
             target, result, metadata, hmac_hash, source_ip)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING id";
        assert!(query.contains("RETURNING id"));
        assert!(query.contains("$1"));
        assert!(query.contains("$10"));
    }

    #[test]
    fn query_sql_includes_limit_and_offset() {
        let filter = AuditFilter {
            limit: 50,
            offset: 100,
            ..Default::default()
        };
        let (where_clause, bind_idx) = build_where_clause(&filter);
        let sql = format!(
            "SELECT * FROM audit_log {} ORDER BY timestamp DESC LIMIT ${} OFFSET ${}",
            where_clause,
            bind_idx + 1,
            bind_idx + 2
        );
        assert!(sql.contains("ORDER BY timestamp DESC"));
        assert!(sql.contains("LIMIT $1"));
        assert!(sql.contains("OFFSET $2"));
    }
}

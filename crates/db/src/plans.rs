//! Plans CRUD — PostgreSQL-backed plan storage
//!
//! Plans are loaded from DB at startup and cached in PlanRegistry.
//! Admin can update prices/features via DB or admin API.

use sqlx::PgPool;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Plan limits stored as JSONB
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlanLimits {
    pub api_requests_per_day: u64,
    pub tokens_per_day: u64,
    pub max_agents: u64,
    pub storage_mb: u64,
    pub max_payload_kb: u64,
    pub max_agents_total: u64,
    pub webhook_rate_per_min: u64,
    pub mcp_tools_per_agent: u64,
    pub audit_retention_days: u64,
    pub priority_support: bool,
    pub custom_domain: bool,
}

/// A billing plan stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbPlan {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tier: i32,
    pub price_kopecks: i64,
    pub annual_price_kopecks: Option<i64>,
    pub period: String,
    pub currency: String,
    pub limits: PlanLimits,
    pub features: Vec<String>,
    pub is_active: bool,
    pub sort_order: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl DbPlan {
    /// Get all active plans ordered by sort_order
    pub async fn list_active(pool: &PgPool) -> Result<Vec<Self>> {
        let rows = sqlx::query_as::<_, PlanRow>(
            "SELECT id, name, description, tier, price_kopecks, annual_price_kopecks,
                    period, currency, limits, features, is_active, sort_order,
                    created_at, updated_at
             FROM plans
             WHERE is_active = true
             ORDER BY sort_order ASC"
        )
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Get a plan by ID (including inactive)
    pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<Self>> {
        let row = sqlx::query_as::<_, PlanRow>(
            "SELECT id, name, description, tier, price_kopecks, annual_price_kopecks,
                    period, currency, limits, features, is_active, sort_order,
                    created_at, updated_at
             FROM plans WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    /// Upsert a plan (insert or update)
    pub async fn upsert(pool: &PgPool, plan: &DbPlan) -> Result<()> {
        let limits_json = serde_json::to_value(&plan.limits)?;
        let features_json = serde_json::to_value(&plan.features)?;

        sqlx::query(
            r#"INSERT INTO plans (id, name, description, tier, price_kopecks, annual_price_kopecks,
                                  period, currency, limits, features, is_active, sort_order)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
             ON CONFLICT (id) DO UPDATE SET
               name = EXCLUDED.name,
               description = EXCLUDED.description,
               tier = EXCLUDED.tier,
               price_kopecks = EXCLUDED.price_kopecks,
               annual_price_kopecks = EXCLUDED.annual_price_kopecks,
               period = EXCLUDED.period,
               currency = EXCLUDED.currency,
               limits = EXCLUDED.limits,
               features = EXCLUDED.features,
               is_active = EXCLUDED.is_active,
               sort_order = EXCLUDED.sort_order,
               updated_at = NOW()"#
        )
        .bind(&plan.id)
        .bind(&plan.name)
        .bind(&plan.description)
        .bind(plan.tier)
        .bind(plan.price_kopecks)
        .bind(plan.annual_price_kopecks)
        .bind(&plan.period)
        .bind(&plan.currency)
        .bind(&limits_json)
        .bind(&features_json)
        .bind(plan.is_active)
        .bind(plan.sort_order)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Deactivate a plan
    pub async fn deactivate(pool: &PgPool, id: &str) -> Result<bool> {
        let result = sqlx::query("UPDATE plans SET is_active = false, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

// Internal row type for sqlx
#[derive(sqlx::FromRow)]
struct PlanRow {
    id: String,
    name: String,
    description: String,
    tier: i32,
    price_kopecks: i64,
    annual_price_kopecks: Option<i64>,
    period: String,
    currency: String,
    limits: serde_json::Value,
    features: serde_json::Value,
    is_active: bool,
    sort_order: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<PlanRow> for DbPlan {
    fn from(r: PlanRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            description: r.description,
            tier: r.tier,
            price_kopecks: r.price_kopecks,
            annual_price_kopecks: r.annual_price_kopecks,
            period: r.period,
            currency: r.currency,
            limits: serde_json::from_value(r.limits).unwrap_or_default(),
            features: serde_json::from_value(r.features).unwrap_or_default(),
            is_active: r.is_active,
            sort_order: r.sort_order,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_limits_default() {
        let limits = PlanLimits::default();
        assert_eq!(limits.api_requests_per_day, 0);
        assert!(!limits.priority_support);
    }

    #[test]
    fn test_plan_limits_serialization() {
        let limits = PlanLimits {
            api_requests_per_day: 100,
            tokens_per_day: 50000,
            max_agents: 1,
            storage_mb: 100,
            max_payload_kb: 512,
            max_agents_total: 1,
            webhook_rate_per_min: 10,
            mcp_tools_per_agent: 5,
            audit_retention_days: 7,
            priority_support: false,
            custom_domain: false,
        };
        let json = serde_json::to_value(&limits).unwrap();
        let back: PlanLimits = serde_json::from_value(json).unwrap();
        assert_eq!(back.api_requests_per_day, 100);
        assert_eq!(back.max_agents, 1);
    }
}

//! Plans CRUD — PostgreSQL-backed plan storage
//!
//! Plans are loaded from DB at startup and cached in PlanRegistry.
//! Plans are the single source of truth — change plan = UPDATE in DB.
//! Admin can update prices/features/limits via DB or admin API.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Plan limits stored as JSONB
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PlanLimits {
    #[serde(default)]
    pub max_agents: u64,
    #[serde(default)]
    pub max_users: u64,
    #[serde(default)]
    pub audit_retention_days: u64,
    #[serde(default)]
    pub api_rate_limit: u32,
    #[serde(default)]
    pub api_rate_window_secs: u32,
    #[serde(default)]
    pub max_custom_rules: u64,
    #[serde(default)]
    pub max_policies: u64,
    #[serde(default, deserialize_with = "deserialize_null_u64")]
    pub max_webhooks: u64,
    #[serde(default)]
    pub approval_channels: Vec<String>,
    #[serde(default)]
    pub siem_formats: Vec<String>,
    #[serde(default)]
    pub allowed_shield_levels: Vec<String>,
    #[serde(default)]
    pub support_tier: String,
}

fn deserialize_null_u64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    let opt: Option<u64> = serde::Deserialize::deserialize(d)?;
    Ok(opt.unwrap_or_default())
}

/// Plan features stored as JSONB (object with boolean/string fields)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PlanFeatures {
    pub shield: bool,
    pub shield_level: String,
    pub mcp_gateway: bool,
    pub policy_engine: bool,
    pub approval: bool,
    pub rbac: bool,
    pub pattern_learning: bool,
    pub e2ee: bool,
    pub audit_log: bool,
    pub webhooks: bool,
    pub siem_export: bool,
    pub sso: bool,
    pub on_premise: bool,
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
    pub annual_discount_percent: i32,
    pub period: String,
    pub currency: String,
    pub limits: PlanLimits,
    pub features: PlanFeatures,
    pub is_active: bool,
    pub sort_order: i32,
    pub trial_days: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl DbPlan {
    /// Get all plans ordered by sort_order
    pub async fn list_all(pool: &PgPool) -> Result<Vec<Self>> {
        let rows = sqlx::query_as::<_, PlanRow>(
            "SELECT id, name, description, tier, price_kopecks, annual_price_kopecks,
                    annual_discount_percent, period, currency, limits, features,
                    is_active, sort_order, trial_days,
                    created_at, updated_at
             FROM plans ORDER BY sort_order ASC",
        )
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Get all active plans ordered by sort_order
    pub async fn list_active(pool: &PgPool) -> Result<Vec<Self>> {
        let rows = sqlx::query_as::<_, PlanRow>(
            "SELECT id, name, description, tier, price_kopecks, annual_price_kopecks,
                    annual_discount_percent, period, currency, limits, features,
                    is_active, sort_order, trial_days,
                    created_at, updated_at
             FROM plans
             WHERE is_active = true
             ORDER BY sort_order ASC",
        )
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Get a plan by ID (including inactive)
    pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<Self>> {
        let row = sqlx::query_as::<_, PlanRow>(
            "SELECT id, name, description, tier, price_kopecks, annual_price_kopecks,
                    annual_discount_percent, period, currency, limits, features,
                    is_active, sort_order, trial_days,
                    created_at, updated_at
             FROM plans WHERE id = $1",
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
                                  annual_discount_percent, period, currency, limits, features,
                                  is_active, sort_order, trial_days)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
             ON CONFLICT (id) DO UPDATE SET
               name = EXCLUDED.name,
               description = EXCLUDED.description,
               tier = EXCLUDED.tier,
               price_kopecks = EXCLUDED.price_kopecks,
               annual_price_kopecks = EXCLUDED.annual_price_kopecks,
               annual_discount_percent = EXCLUDED.annual_discount_percent,
               period = EXCLUDED.period,
               currency = EXCLUDED.currency,
               limits = EXCLUDED.limits,
               features = EXCLUDED.features,
               is_active = EXCLUDED.is_active,
               sort_order = EXCLUDED.sort_order,
               trial_days = EXCLUDED.trial_days,
               updated_at = NOW()"#,
        )
        .bind(&plan.id)
        .bind(&plan.name)
        .bind(&plan.description)
        .bind(plan.tier)
        .bind(plan.price_kopecks)
        .bind(plan.annual_price_kopecks)
        .bind(plan.annual_discount_percent)
        .bind(&plan.period)
        .bind(&plan.currency)
        .bind(&limits_json)
        .bind(&features_json)
        .bind(plan.is_active)
        .bind(plan.sort_order)
        .bind(plan.trial_days)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Set active status of a plan
    pub async fn set_active(pool: &PgPool, id: &str, active: bool) -> Result<bool> {
        let result =
            sqlx::query("UPDATE plans SET is_active = $1, updated_at = NOW() WHERE id = $2")
                .bind(active)
                .bind(id)
                .execute(pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Deactivate a plan
    pub async fn deactivate(pool: &PgPool, id: &str) -> Result<bool> {
        Self::set_active(pool, id, false).await
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
    annual_discount_percent: i32,
    period: String,
    currency: String,
    limits: serde_json::Value,
    features: serde_json::Value,
    is_active: bool,
    sort_order: i32,
    trial_days: i32,
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
            annual_discount_percent: r.annual_discount_percent,
            period: r.period,
            currency: r.currency,
            limits: serde_json::from_value(r.limits).unwrap_or_default(),
            features: serde_json::from_value(r.features).unwrap_or_default(),
            is_active: r.is_active,
            sort_order: r.sort_order,
            trial_days: r.trial_days,
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
        assert_eq!(limits.max_agents, 0);
        assert_eq!(limits.support_tier, "");
    }

    #[test]
    fn test_plan_limits_serialization() {
        let limits = PlanLimits {
            max_agents: 5,
            max_users: 5,
            audit_retention_days: 60,
            api_rate_limit: 500,
            api_rate_window_secs: 60,
            max_custom_rules: 50,
            max_policies: 5,
            max_webhooks: 3,
            approval_channels: vec!["telegram".to_string()],
            siem_formats: vec!["json".to_string()],
            allowed_shield_levels: vec!["basic".to_string(), "advanced".to_string()],
            support_tier: "email".to_string(),
        };
        let json = serde_json::to_value(&limits).unwrap();
        let back: PlanLimits = serde_json::from_value(json).unwrap();
        assert_eq!(back.max_agents, 5);
        assert_eq!(back.max_users, 5);
        assert_eq!(back.approval_channels, vec!["telegram"]);
        assert_eq!(back.support_tier, "email");
    }

    #[test]
    fn test_plan_features_serialization() {
        let features = PlanFeatures {
            shield: true,
            shield_level: "advanced".to_string(),
            mcp_gateway: true,
            policy_engine: true,
            approval: true,
            rbac: true,
            e2ee: true,
            audit_log: true,
            webhooks: true,
            siem_export: true,
            ..Default::default()
        };
        let json = serde_json::to_value(&features).unwrap();
        let back: PlanFeatures = serde_json::from_value(json).unwrap();
        assert!(back.shield);
        assert!(back.approval);
        assert!(!back.pattern_learning);
        assert!(!back.sso);
        assert_eq!(back.shield_level, "advanced");
    }

    #[test]
    fn test_plan_features_default_false() {
        let features = PlanFeatures::default();
        assert!(!features.shield);
        assert!(!features.approval);
        assert!(!features.sso);
    }
}

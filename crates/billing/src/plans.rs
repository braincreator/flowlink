//! Plan definitions, feature gates, and registry
//!
//! Plans are the single source of truth loaded from PostgreSQL.
//! Each plan has:
//! - `features` (JSONB): which capabilities are enabled (shield, approval, rbac, etc.)
//! - `limits` (JSONB): numeric constraints (max_agents, max_users, etc.)
//!
//! Change a plan = UPDATE in DB → all levels pick up automatically.
//!
//! All prices in RUB (kopecks). 0 = free / unlimited.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// Built-in plan IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanId {
    Starter,
    Professional,
    Scale,
    Enterprise,
}

impl PlanId {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanId::Starter => "starter",
            PlanId::Professional => "professional",
            PlanId::Scale => "scale",
            PlanId::Enterprise => "enterprise",
        }
    }
}

impl std::fmt::Display for PlanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Plan features — which capabilities are enabled.
/// Deserialized from `features` JSONB column.
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
    pub serverguard: bool,
    pub serverguard_level: String,
    pub forensics: bool,
    pub service_catalog: bool,
    pub ai_ops: bool,
    pub change_management: bool,
}

/// Plan limits — numeric and structural constraints.
/// Deserialized from `limits` JSONB column.
/// 0 means unlimited (for numeric fields).
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

/// Plan gate errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum PlanGateError {
    #[error("Feature '{feature}' is not available on your plan ({plan_id}). Upgrade to {min_plan} or higher.")]
    FeatureNotAvailable {
        feature: String,
        plan_id: String,
        min_plan: String,
    },
    #[error("Limit '{limit}' exceeded: {current}/{max}.")]
    LimitExceeded {
        limit: String,
        current: u64,
        max: u64,
    },
    #[error("Plan not found: {0}")]
    PlanNotFound(String),
}

/// Minimum plan tier required for each feature.
/// If a plan's features don't include the key → FeatureNotAvailable.
/// The "min_plan" hint tells the user what to upgrade to.
static FEATURE_MIN_TIER: &[(&str, &str, u32)] = &[
    ("shield", "Starter", 0),
    ("shield_level", "Starter", 0),
    ("mcp_gateway", "Starter", 0),
    ("policy_engine", "Starter", 0),
    ("e2ee", "Starter", 0),
    ("audit_log", "Starter", 0),
    ("webhooks", "Starter", 0),
    ("approval", "Professional", 1),
    ("rbac", "Professional", 1),
    ("siem_export", "Professional", 1),
    ("pattern_learning", "Professional", 1),
    ("forensics", "Professional", 1),
    ("service_catalog", "Professional", 1),
    ("policy_engine", "Professional", 1),
    ("serverguard", "Professional", 1),
    ("sso", "Business", 2),
    ("ai_ops", "Business", 2),
    ("change_management", "Business", 2),
    ("on_premise", "Enterprise", 3),
];

/// A billing plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// Plan ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Description
    pub description: String,
    /// Tier level (higher = more resources)
    pub tier: u32,
    /// Price per month in kopecks (1/100 RUB). 0 = free
    pub price_kopecks: u64,
    /// Price per year in kopecks (None = no annual option)
    pub annual_price_kopecks: Option<u64>,
    /// Annual discount percent
    pub annual_discount_percent: u32,
    /// Plan features
    pub features: PlanFeatures,
    /// Plan limits
    pub limits: PlanLimits,
    /// Is this plan available for new signups
    pub available: bool,
    /// Is this a legacy plan
    pub legacy: bool,
    /// Trial days (None = no trial)
    pub trial_days: Option<u16>,
    /// Billing period: "month" or "year"
    pub billing_period: String,
}

impl Plan {
    /// Check if a feature is available on this plan.
    /// Returns Ok(()) if the feature exists in the plan's features, or Err with upgrade hint.
    pub fn require_feature(&self, feature: &str) -> Result<(), PlanGateError> {
        // Check boolean features
        let has_feature = match feature {
            "shield" => self.features.shield,
            "mcp_gateway" => self.features.mcp_gateway,
            "policy_engine" => self.features.policy_engine,
            "approval" => self.features.approval,
            "rbac" => self.features.rbac,
            "pattern_learning" => self.features.pattern_learning,
            "e2ee" => self.features.e2ee,
            "audit_log" => self.features.audit_log,
            "webhooks" => self.features.webhooks,
            "siem_export" => self.features.siem_export,
            "sso" => self.features.sso,
            "on_premise" => self.features.on_premise,
            "serverguard" => self.features.serverguard,
            "forensics" => self.features.forensics,
            "service_catalog" => self.features.service_catalog,
            "ai_ops" => self.features.ai_ops,
            "change_management" => self.features.change_management,
            _ => {
                // Unknown feature — allow by default (don't break new features)
                tracing::warn!("Unknown feature check: {}", feature);
                return Ok(());
            }
        };

        if has_feature {
            return Ok(());
        }

        // Find min plan tier for this feature
        let (min_plan, _) = FEATURE_MIN_TIER
            .iter()
            .find(|(f, _, _)| *f == feature)
            .map(|(_, p, _)| (*p, true))
            .unwrap_or(("Professional", true));

        Err(PlanGateError::FeatureNotAvailable {
            feature: feature.to_string(),
            plan_id: self.id.clone(),
            min_plan: min_plan.to_string(),
        })
    }

    /// Check if a numeric limit is respected.
    /// `current` is the current usage count. Returns Ok(()) if under limit.
    pub fn check_limit(&self, limit: &str, current: u64) -> Result<(), PlanGateError> {
        let (max, is_unlimited) = match limit {
            "max_agents" => (self.limits.max_agents, self.limits.max_agents == 0),
            "max_users" => (self.limits.max_users, self.limits.max_users == 0),
            "max_custom_rules" => (self.limits.max_custom_rules, self.limits.max_custom_rules == 0),
            "max_policies" => (self.limits.max_policies, self.limits.max_policies == 0),
            "max_webhooks" => (self.limits.max_webhooks, self.limits.max_webhooks == 0),
            _ => {
                // Unknown limit — allow by default
                tracing::warn!("Unknown limit check: {}", limit);
                return Ok(());
            }
        };

        if is_unlimited {
            return Ok(());
        }

        if current >= max {
            return Err(PlanGateError::LimitExceeded {
                limit: limit.to_string(),
                current,
                max,
            });
        }

        Ok(())
    }

    /// Check if a value is allowed in a list-type limit.
    /// e.g. check_channel("slack") checks if "slack" is in approval_channels.
    pub fn check_allowed(&self, limit: &str, value: &str) -> Result<(), PlanGateError> {
        let allowed = match limit {
            "approval_channels" => &self.limits.approval_channels,
            "siem_formats" => &self.limits.siem_formats,
            "allowed_shield_levels" => &self.limits.allowed_shield_levels,
            _ => return Ok(()),
        };

        if allowed.is_empty() {
            return Err(PlanGateError::FeatureNotAvailable {
                feature: limit.to_string(),
                plan_id: self.id.clone(),
                min_plan: "Professional".to_string(),
            });
        }

        if allowed.iter().any(|a| a.eq_ignore_ascii_case(value)) {
            Ok(())
        } else {
            Err(PlanGateError::FeatureNotAvailable {
                feature: format!("{}:{}", limit, value),
                plan_id: self.id.clone(),
                min_plan: "Professional".to_string(),
            })
        }
    }

    /// Check if a limit is effectively unlimited (0 means unlimited)
    pub fn is_unlimited(limit: u64) -> bool {
        limit == 0
    }

    /// Format price in RUB
    pub fn format_price(kopecks: u64) -> String {
        let rubles = kopecks as f64 / 100.0;
        format!("{:.0} ₽", rubles)
    }

    /// Format price per month
    pub fn format_monthly(&self) -> String {
        Self::format_price(self.price_kopecks)
    }

    /// Convert from database plan row
    pub fn from_db_plan(db: flowlink_db::plans::DbPlan) -> Self {
        Self {
            id: db.id,
            name: db.name,
            description: db.description,
            tier: db.tier as u32,
            price_kopecks: db.price_kopecks as u64,
            annual_price_kopecks: db.annual_price_kopecks.map(|v| v as u64),
            annual_discount_percent: db.annual_discount_percent.max(0) as u32,
            features: serde_json::from_value(serde_json::to_value(&db.features).unwrap_or_default()).unwrap_or_default(),
            limits: serde_json::from_value(serde_json::to_value(&db.limits).unwrap_or_default()).unwrap_or_default(),
            available: db.is_active,
            legacy: false,
            trial_days: if db.trial_days > 0 {
                Some(db.trial_days as u16)
            } else {
                None
            },
            billing_period: db.period.clone(),
        }
    }
}

/// Plan registry — stores all available plans, loaded from database.
///
/// Loads from database on startup, falls back to built-in defaults if DB unavailable.
pub struct PlanRegistry {
    plans: RwLock<HashMap<String, Plan>>,
    /// Last time plans were loaded from DB
    last_loaded: std::sync::Mutex<std::time::Instant>,
}

impl PlanRegistry {
    /// Create with empty registry (plans loaded from DB on startup)
    pub fn new() -> Self {
        Self {
            plans: RwLock::new(HashMap::new()),
            last_loaded: std::sync::Mutex::new(std::time::Instant::now()),
        }
    }

    /// Load plans from database, replacing in-memory cache.
    /// Falls back to built-in defaults if DB query fails.
    pub async fn load_from_db(&self, pool: &flowlink_db::DbPool) {
        match flowlink_db::plans::DbPlan::list_active(pool.write_pool()).await {
            Ok(db_plans) if !db_plans.is_empty() => {
                let mut plans = self.plans.write().unwrap();
                plans.clear();
                for dp in db_plans {
                    plans.insert(dp.id.clone(), Plan::from_db_plan(dp));
                }
                tracing::info!("📦 Loaded {} plans from database", plans.len());
                *self.last_loaded.lock().unwrap() = std::time::Instant::now();
            }
            Ok(_) => {
                tracing::warn!("📦 No plans in database, using built-in defaults");
                self.seed_defaults();
            }
            Err(e) => {
                tracing::warn!("📦 Failed to load plans from DB: {e}. Using built-in defaults.");
                self.seed_defaults();
            }
        }
    }

    /// Seed with built-in default plans (used as fallback)
    pub fn seed_defaults(&self) {
        let mut plans = self.plans.write().unwrap();
        if plans.is_empty() {
            plans.insert(PlanId::Starter.as_str().to_string(), Plan {
                id: PlanId::Starter.as_str().to_string(),
                name: "Starter".to_string(),
                description: "For individuals and small projects".to_string(),
                tier: 0,
                price_kopecks: 299_000,
                annual_price_kopecks: Some(2_990_000),
                annual_discount_percent: 17,
                features: PlanFeatures {
                    shield: true,
                    shield_level: "basic".to_string(),
                    mcp_gateway: true,
                    policy_engine: true,
                    e2ee: true,
                    audit_log: true,
                    webhooks: true,
                    ..Default::default()
                },
                limits: PlanLimits {
                    max_agents: 5,
                    max_users: 2,
                    audit_retention_days: 14,
                    api_rate_limit: 180,
                    api_rate_window_secs: 60,
                    approval_channels: vec!["telegram".to_string()],
                    support_tier: "community".to_string(),
                    ..Default::default()
                },
                available: true,
                legacy: false,
                trial_days: Some(14),
                billing_period: "month".to_string(),
            });
            plans.insert(PlanId::Professional.as_str().to_string(), Plan {
                id: PlanId::Professional.as_str().to_string(),
                name: "Pro".to_string(),
                description: "For growing teams".to_string(),
                tier: 1,
                price_kopecks: 1_999_000,
                annual_price_kopecks: Some(19_990_000),
                annual_discount_percent: 17,
                features: PlanFeatures {
                    shield: true,
                    shield_level: "advanced".to_string(),
                    mcp_gateway: true,
                    policy_engine: true,
                    approval: true,
                    rbac: true,
                    pattern_learning: true,
                    e2ee: true,
                    audit_log: true,
                    webhooks: true,
                    siem_export: true,
                    serverguard: true,
                    serverguard_level: "basic".to_string(),
                    forensics: true,
                    service_catalog: true,
                    ..Default::default()
                },
                limits: PlanLimits {
                    max_agents: 15,
                    max_users: 10,
                    audit_retention_days: 90,
                    api_rate_limit: 500,
                    api_rate_window_secs: 60,
                    approval_channels: vec!["telegram".to_string(), "email".to_string()],
                    siem_formats: vec!["json".to_string()],
                    support_tier: "email".to_string(),
                    ..Default::default()
                },
                available: true,
                legacy: false,
                trial_days: Some(14),
                billing_period: "month".to_string(),
            });
            plans.insert(PlanId::Scale.as_str().to_string(), Plan {
                id: PlanId::Scale.as_str().to_string(),
                name: "Business".to_string(),
                description: "For agencies and multi-cluster setups".to_string(),
                tier: 2,
                price_kopecks: 4_999_000,
                annual_price_kopecks: Some(49_990_000),
                annual_discount_percent: 17,
                features: PlanFeatures {
                    shield: true,
                    shield_level: "full".to_string(),
                    mcp_gateway: true,
                    policy_engine: true,
                    approval: true,
                    rbac: true,
                    pattern_learning: true,
                    e2ee: true,
                    audit_log: true,
                    webhooks: true,
                    siem_export: true,
                    sso: true,
                    serverguard: true,
                    serverguard_level: "full".to_string(),
                    forensics: true,
                    service_catalog: true,
                    ai_ops: true,
                    change_management: true,
                    ..Default::default()
                },
                limits: PlanLimits {
                    max_agents: 50,
                    max_users: 50,
                    audit_retention_days: 365,
                    api_rate_limit: 2000,
                    api_rate_window_secs: 60,
                    approval_channels: vec!["telegram".to_string(), "email".to_string(), "slack".to_string()],
                    siem_formats: vec!["json".to_string(), "cef".to_string(), "leef".to_string()],
                    support_tier: "priority".to_string(),
                    ..Default::default()
                },
                available: true,
                legacy: false,
                trial_days: None,
                billing_period: "month".to_string(),
            });
            plans.insert(PlanId::Enterprise.as_str().to_string(), Plan {
                id: PlanId::Enterprise.as_str().to_string(),
                name: "Enterprise".to_string(),
                description: "For large orgs with dedicated support".to_string(),
                tier: 3,
                price_kopecks: 0,
                annual_price_kopecks: None,
                annual_discount_percent: 0,
                features: PlanFeatures {
                    shield: true,
                    shield_level: "full".to_string(),
                    mcp_gateway: true,
                    policy_engine: true,
                    approval: true,
                    rbac: true,
                    pattern_learning: true,
                    e2ee: true,
                    audit_log: true,
                    webhooks: true,
                    siem_export: true,
                    sso: true,
                    on_premise: true,
                    serverguard: true,
                    serverguard_level: "full".to_string(),
                    forensics: true,
                    service_catalog: true,
                    ai_ops: true,
                    change_management: true,
                },
                limits: PlanLimits {
                    audit_retention_days: 365,
                    approval_channels: vec!["telegram".to_string(), "email".to_string(), "slack".to_string(), "webhook".to_string()],
                    siem_formats: vec!["json".to_string(), "cef".to_string(), "leef".to_string(), "syslog".to_string()],
                    support_tier: "dedicated".to_string(),
                    ..Default::default()
                },
                available: true,
                legacy: false,
                trial_days: None,
                billing_period: "month".to_string(),
            });
        }
    }

    /// Get a plan by ID
    pub fn get(&self, id: &str) -> Option<Plan> {
        self.plans.read().unwrap().get(id).cloned()
    }

    /// Get all available plans (for public listing)
    pub fn list_available(&self) -> Vec<Plan> {
        self.plans
            .read()
            .unwrap()
            .values()
            .filter(|p| p.available && !p.legacy)
            .cloned()
            .collect()
    }

    /// Register a custom plan
    pub fn register(&self, plan: Plan) {
        self.plans.write().unwrap().insert(plan.id.clone(), plan);
    }

    /// Remove a plan (marks as legacy)
    pub fn deprecate(&self, id: &str) {
        if let Some(plan) = self.plans.write().unwrap().get_mut(id) {
            plan.available = false;
            plan.legacy = true;
        }
    }
}

impl Default for PlanRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry() -> PlanRegistry {
        let r = PlanRegistry::new();
        r.seed_defaults();
        r
    }

    #[test]
    fn test_default_plans() {
        let registry = make_registry();
        assert!(registry.get("starter").is_some());
        assert!(registry.get("professional").is_some());
        assert!(registry.get("scale").is_some());
        assert!(registry.get("enterprise").is_some());
    }

    #[test]
    fn test_starter_features() {
        let starter = make_registry().get("starter").unwrap();
        assert!(starter.features.shield);
        assert!(starter.features.mcp_gateway);
        assert!(starter.features.policy_engine);
        assert!(!starter.features.approval);
        assert!(!starter.features.rbac);
        assert!(!starter.features.pattern_learning);
        assert!(starter.features.webhooks);
        assert!(!starter.features.sso);
    }

    #[test]
    fn test_professional_features() {
        let pro = make_registry().get("professional").unwrap();
        assert!(pro.features.approval);
        assert!(pro.features.rbac);
        assert!(pro.features.webhooks);
        assert!(pro.features.siem_export);
        assert!(pro.features.pattern_learning);
        assert!(!pro.features.sso);
    }

    #[test]
    fn test_scale_features() {
        let scale = make_registry().get("scale").unwrap();
        assert!(scale.features.pattern_learning);
        assert!(scale.features.sso);
        assert!(!scale.features.on_premise);
    }

    #[test]
    fn test_enterprise_features() {
        let ent = make_registry().get("enterprise").unwrap();
        assert!(ent.features.sso);
        assert!(ent.features.on_premise);
        assert!(ent.features.pattern_learning);
        assert!(ent.features.webhooks);
    }

    #[test]
    fn test_starter_limits() {
        let starter = make_registry().get("starter").unwrap();
        assert_eq!(starter.limits.max_agents, 5);
        assert_eq!(starter.limits.max_users, 2);
        assert_eq!(starter.limits.max_custom_rules, 0);
        assert_eq!(starter.limits.max_policies, 0);
        assert_eq!(starter.limits.max_webhooks, 0);
        assert_eq!(starter.limits.support_tier, "community");
    }

    #[test]
    fn test_professional_limits() {
        let pro = make_registry().get("professional").unwrap();
        assert_eq!(pro.limits.max_agents, 15);
        assert_eq!(pro.limits.max_users, 10);
        assert_eq!(pro.limits.max_webhooks, 0);
        assert_eq!(pro.limits.support_tier, "email");
    }

    #[test]
    fn test_enterprise_unlimited() {
        let ent = make_registry().get("enterprise").unwrap();
        assert_eq!(ent.limits.max_agents, 0); // 0 = unlimited
        assert!(Plan::is_unlimited(ent.limits.max_agents));
        assert!(Plan::is_unlimited(ent.limits.max_policies));
    }

    #[test]
    fn test_require_feature_ok() {
        let pro = make_registry().get("professional").unwrap();
        assert!(pro.require_feature("approval").is_ok());
        assert!(pro.require_feature("rbac").is_ok());
    }

    #[test]
    fn test_require_feature_rejected() {
        let starter = make_registry().get("starter").unwrap();
        let err = starter.require_feature("approval").unwrap_err();
        match err {
            PlanGateError::FeatureNotAvailable { feature, min_plan, .. } => {
                assert_eq!(feature, "approval");
                assert_eq!(min_plan, "Professional");
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_check_limit_ok() {
        let starter = make_registry().get("starter").unwrap();
        assert!(starter.check_limit("max_agents", 0).is_ok());
    }

    #[test]
    fn test_check_limit_exceeded() {
        let starter = make_registry().get("starter").unwrap();
        let err = starter.check_limit("max_agents", 6).unwrap_err();
        match err {
            PlanGateError::LimitExceeded { limit, current, max } => {
                assert_eq!(limit, "max_agents");
                assert_eq!(current, 6);
                assert_eq!(max, 5);
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_check_limit_unlimited() {
        let ent = make_registry().get("enterprise").unwrap();
        assert!(ent.check_limit("max_agents", 999999).is_ok());
    }

    #[test]
    fn test_check_allowed_ok() {
        let pro = make_registry().get("professional").unwrap();
        assert!(pro.check_allowed("approval_channels", "telegram").is_ok());
    }

    #[test]
    fn test_check_allowed_rejected() {
        let pro = make_registry().get("professional").unwrap();
        assert!(pro.check_allowed("approval_channels", "slack").is_err());
    }

    #[test]
    fn test_features_differ_between_plans() {
        let starter = make_registry().get("starter").unwrap();
        let pro = make_registry().get("professional").unwrap();
        let scale = make_registry().get("scale").unwrap();
        let ent = make_registry().get("enterprise").unwrap();

        // Features are NOT identical — that's the whole point
        assert_ne!(starter.features.approval, pro.features.approval);
        assert_ne!(starter.features.pattern_learning, pro.features.pattern_learning);
        assert_ne!(scale.features.on_premise, ent.features.on_premise);
    }

    #[test]
    fn test_list_available() {
        let registry = make_registry();
        let available = registry.list_available();
        assert_eq!(available.len(), 4);
    }

    #[test]
    fn test_register_custom_plan() {
        let registry = make_registry();
        let custom = Plan {
            id: "custom-1".to_string(),
            name: "Custom".to_string(),
            description: "Custom plan".to_string(),
            tier: 1,
            price_kopecks: 19_990,
            annual_price_kopecks: None,
            annual_discount_percent: 0,
            features: PlanFeatures::default(),
            limits: PlanLimits::default(),
            available: true,
            legacy: false,
            trial_days: None,
            billing_period: "month".to_string(),
        };
        registry.register(custom);
        assert!(registry.get("custom-1").is_some());
        assert_eq!(registry.list_available().len(), 5);
    }

    #[test]
    fn test_deprecate_plan() {
        let registry = make_registry();
        registry.deprecate("professional");
        let available = registry.list_available();
        assert_eq!(available.len(), 3); // Starter + Scale + Enterprise
        let pro = registry.get("professional").unwrap();
        assert!(!pro.available);
        assert!(pro.legacy);
    }

    #[test]
    fn test_format_price() {
        assert_eq!(Plan::format_price(299_000), "2990 ₽");
        assert_eq!(Plan::format_price(0), "0 ₽");
        assert_eq!(Plan::format_price(1_999_000), "19990 ₽");
    }

    #[test]
    fn test_plan_id_display() {
        assert_eq!(PlanId::Starter.to_string(), "starter");
        assert_eq!(PlanId::Professional.to_string(), "professional");
        assert_eq!(PlanId::Scale.to_string(), "scale");
        assert_eq!(PlanId::Enterprise.to_string(), "enterprise");
    }

    #[test]
    fn test_plan_limits_default() {
        let limits = PlanLimits::default();
        assert_eq!(limits.max_agents, 0);
        assert_eq!(limits.max_users, 0);
        assert!(Plan::is_unlimited(limits.max_agents));
    }

    #[test]
    fn test_tier_ordering() {
        let starter = make_registry().get("starter").unwrap();
        let pro = make_registry().get("professional").unwrap();
        let scale = make_registry().get("scale").unwrap();
        let ent = make_registry().get("enterprise").unwrap();
        assert!(starter.tier < pro.tier);
        assert!(pro.tier < scale.tier);
        assert!(scale.tier < ent.tier);
    }

    #[test]
    fn test_annual_discount() {
        let pro = make_registry().get("professional").unwrap();
        let annual = pro.annual_price_kopecks.unwrap();
        let monthly_x12 = pro.price_kopecks * 12;
        assert!(annual < monthly_x12, "Annual should be cheaper than 12 months");
        assert_eq!(annual, 19_990_000);
        assert_eq!(monthly_x12, 23_988_000);
        assert_eq!(pro.annual_discount_percent, 17);
    }

    #[test]
    fn test_format_monthly() {
        let starter = make_registry().get("starter").unwrap();
        let pro = make_registry().get("professional").unwrap();
        assert_eq!(starter.format_monthly(), "2990 ₽");
        assert_eq!(pro.format_monthly(), "19990 ₽");
    }

    #[test]
    fn test_default_registry() {
        let registry = PlanRegistry::default();
        // Initially empty, seed_defaults must be called
        assert_eq!(registry.list_available().len(), 0);
        registry.seed_defaults();
        assert_eq!(registry.list_available().len(), 4);
    }
}

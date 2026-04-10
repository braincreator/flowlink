//! Plan definitions and registry
//!
//! Three tiers: Trial, Starter, Pro
//! All prices in RUB (Russian market)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// Built-in plan IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanId {
    Trial,
    Starter,
    Pro,
}

impl PlanId {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanId::Trial => "trial",
            PlanId::Starter => "starter",
            PlanId::Pro => "pro",
        }
    }
}

impl std::fmt::Display for PlanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Plan limits
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlanLimits {
    /// Max hosts (0 = unlimited)
    pub max_hosts: u64,
    /// Max users (0 = unlimited)
    pub max_users: u64,
    /// Backup storage in MB (0 = unlimited)
    pub backup_storage_mb: u64,
    /// Max snapshots (0 = unlimited)
    pub max_snapshots: u64,
    /// Backup retention in days
    pub retention_days: u16,
    /// Audit log retention in days
    pub audit_retention_days: u64,
    /// Max file size in MB (0 = configurable)
    pub max_file_size_mb: u64,
    /// Execution timeout in seconds (0 = configurable)
    pub exec_timeout_sec: u64,
    /// Shield level: "basic", "advanced", "enterprise"
    pub shield_level: String,
}

/// A billing plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    /// Plan ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Description
    pub description: String,
    /// Tier level (higher = more features)
    pub tier: u32,
    /// Price per month in kopecks (1/100 RUB). 0 = free
    pub price_kopecks: u64,
    /// Price per year in kopecks (None = no annual discount)
    pub annual_price_kopecks: Option<u64>,
    /// Plan limits
    pub limits: PlanLimits,
    /// Features list (for display)
    pub features: Vec<String>,
    /// Is this plan available for new signups
    pub available: bool,
    /// Is this a legacy plan (can't signup, existing users keep it)
    pub legacy: bool,
    /// Trial days (None = no trial)
    pub trial_days: Option<u16>,
    /// Billing period: "month" or "year"
    pub billing_period: String,
}

impl Plan {
    /// Trial plan — free for 7 days
    pub fn trial() -> Self {
        Self {
            id: PlanId::Trial.as_str().to_string(),
            name: "Trial".to_string(),
            description: "Попробуйте FlowLink бесплатно".to_string(),
            tier: 0,
            price_kopecks: 0,
            annual_price_kopecks: None,
            limits: PlanLimits {
                max_hosts: 1,
                max_users: 1,
                backup_storage_mb: 500,
                max_snapshots: 5,
                retention_days: 3,
                audit_retention_days: 3,
                max_file_size_mb: 10,
                exec_timeout_sec: 60,
                shield_level: "basic".to_string(),
            },
            features: vec![
                "1 host".to_string(),
                "1 user".to_string(),
                "3 day logs".to_string(),
                "Pattern blocking".to_string(),
                "Manual backup".to_string(),
                "E2EE".to_string(),
            ],
            available: true,
            legacy: false,
            trial_days: Some(7),
            billing_period: "month".to_string(),
        }
    }

    /// Starter plan — 990 ₽/мес
    pub fn starter() -> Self {
        Self {
            id: PlanId::Starter.as_str().to_string(),
            name: "Starter".to_string(),
            description: "Для фрилансеров и small teams".to_string(),
            tier: 1,
            price_kopecks: 99_000, // 990 RUB/month
            annual_price_kopecks: Some(950_400), // 9 504 RUB/year (~20% discount)
            limits: PlanLimits {
                max_hosts: 3,
                max_users: 3,
                backup_storage_mb: 5120,
                max_snapshots: 50,
                retention_days: 14,
                audit_retention_days: 14,
                max_file_size_mb: 100,
                exec_timeout_sec: 300,
                shield_level: "advanced".to_string(),
            },
            features: vec![
                "3 сервера".to_string(),
                "3 пользователя".to_string(),
                "Telegram бот".to_string(),
                "Web dashboard".to_string(),
                "E2EE шифрование".to_string(),
                "Device trust".to_string(),
                "MCP protocol".to_string(),
                "Email поддержка".to_string(),
            ],
            available: true,
            legacy: false,
            trial_days: None,
            billing_period: "month".to_string(),
        }
    }

    /// Pro plan — 4 990 ₽/мес
    pub fn pro() -> Self {
        Self {
            id: PlanId::Pro.as_str().to_string(),
            name: "Pro".to_string(),
            description: "Для стартапов, IT-отделов и DevOps teams".to_string(),
            tier: 2,
            price_kopecks: 499_000, // 4 990 RUB/month
            annual_price_kopecks: Some(4_790_400), // 47 904 RUB/year (~20% discount)
            limits: PlanLimits {
                max_hosts: 25,
                max_users: 10,
                backup_storage_mb: 0, // unlimited
                max_snapshots: 0, // unlimited
                retention_days: 90,
                audit_retention_days: 90,
                max_file_size_mb: 0, // configurable
                exec_timeout_sec: 0, // configurable
                shield_level: "enterprise".to_string(),
            },
            features: vec![
                "25 серверов".to_string(),
                "10 пользователей".to_string(),
                "K8s operator".to_string(),
                "SIEM export".to_string(),
                "RBAC".to_string(),
                "Approval workflow".to_string(),
                "Forensics".to_string(),
                "Audit log + HMAC".to_string(),
                "Priority поддержка".to_string(),
            ],
            available: true,
            legacy: false,
            trial_days: None,
            billing_period: "month".to_string(),
        }
    }

    /// Check if a limit is effectively unlimited (0 means unlimited)
    pub fn is_unlimited(limit: u64) -> bool {
        limit == 0
    }

    /// Format price in RUB
    pub fn format_price(kopecks: u64) -> String {
        let rubles = kopecks as f64 / 100.0;
        format!("{:.2} ₽", rubles)
    }

    /// Format price per month
    pub fn format_monthly(&self) -> String {
        Self::format_price(self.price_kopecks)
    }

    /// Convert from database plan
    pub fn from_db_plan(db: flowlink_db::plans::DbPlan) -> Self {
        Self {
            id: db.id,
            name: db.name,
            description: db.description,
            tier: db.tier as u32,
            price_kopecks: db.price_kopecks as u64,
            annual_price_kopecks: db.annual_price_kopecks.map(|v| v as u64),
            limits: PlanLimits {
                max_hosts: db.limits.max_hosts,
                max_users: db.limits.max_users,
                backup_storage_mb: db.limits.backup_storage_mb,
                max_snapshots: db.limits.max_snapshots,
                retention_days: db.limits.retention_days,
                audit_retention_days: db.limits.audit_retention_days,
                max_file_size_mb: db.limits.max_file_size_mb,
                exec_timeout_sec: db.limits.exec_timeout_sec,
                shield_level: db.limits.shield_level.clone(),
            },
            features: db.features,
            available: db.is_active,
            legacy: false,
            trial_days: None,
            billing_period: db.period.clone(),
        }
    }
}

/// Plan registry — stores all available plans
///
/// Loads from database on startup, falls back to built-in defaults if DB unavailable.
pub struct PlanRegistry {
    plans: RwLock<HashMap<String, Plan>>,
    /// Last time plans were loaded from DB
    last_loaded: std::sync::Mutex<std::time::Instant>,
}

impl PlanRegistry {
    /// Create with default plans (Trial, Starter, Pro)
    pub fn new() -> Self {
        let mut plans = HashMap::new();
        plans.insert(PlanId::Trial.as_str().to_string(), Plan::trial());
        plans.insert(PlanId::Starter.as_str().to_string(), Plan::starter());
        plans.insert(PlanId::Pro.as_str().to_string(), Plan::pro());

        Self {
            plans: RwLock::new(plans),
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
            }
            Err(e) => {
                tracing::warn!("📦 Failed to load plans from DB: {e}. Using built-in defaults.");
            }
        }
    }

    /// Get a plan by ID
    pub fn get(&self, id: &str) -> Option<Plan> {
        self.plans.read().unwrap().get(id).cloned()
    }

    /// Get all available plans
    pub fn list_available(&self) -> Vec<Plan> {
        self.plans.read().unwrap()
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

    #[test]
    fn test_default_plans() {
        let registry = PlanRegistry::new();
        assert!(registry.get("trial").is_some());
        assert!(registry.get("starter").is_some());
        assert!(registry.get("pro").is_some());
    }

    #[test]
    fn test_free_plan_limits() {
        let free = Plan::trial();
        assert_eq!(free.limits.max_hosts, 1);
        assert_eq!(free.limits.max_users, 1);
        assert_eq!(free.limits.backup_storage_mb, 500);
        assert_eq!(free.price_kopecks, 0);
        assert_eq!(free.limits.shield_level, "basic");
        assert_eq!(free.trial_days, Some(7));
    }

    #[test]
    fn test_individual_plan_limits() {
        let individual = Plan::starter();
        assert_eq!(individual.limits.max_hosts, 3);
        assert_eq!(individual.limits.max_users, 3);
        assert_eq!(individual.limits.backup_storage_mb, 5120);
        assert_eq!(individual.price_kopecks, 99_000);
        assert_eq!(individual.limits.shield_level, "advanced");
        assert_eq!(individual.trial_days, None);
        assert_eq!(individual.annual_price_kopecks, Some(950_400));
    }

    #[test]
    fn test_business_unlimited() {
        let business = Plan::pro();
        assert!(Plan::is_unlimited(business.limits.max_snapshots));
        assert!(Plan::is_unlimited(business.limits.max_file_size_mb));
        assert!(Plan::is_unlimited(business.limits.exec_timeout_sec));
        assert_eq!(business.limits.max_hosts, 25);
        assert_eq!(business.limits.max_users, 10);
        assert_eq!(business.limits.audit_retention_days, 90);
        assert_eq!(business.price_kopecks, 499_000);
        assert_eq!(business.trial_days, None);
    }

    #[test]
    fn test_list_available() {
        let registry = PlanRegistry::new();
        let available = registry.list_available();
        assert_eq!(available.len(), 3);
    }

    #[test]
    fn test_register_custom_plan() {
        let registry = PlanRegistry::new();
        let custom = Plan {
            id: "custom-1".to_string(),
            name: "Custom".to_string(),
            description: "Custom plan".to_string(),
            tier: 1,
            price_kopecks: 19_990,
            annual_price_kopecks: None,
            limits: PlanLimits::default(),
            features: vec![],
            available: true,
            legacy: false,
            trial_days: None,
            billing_period: "month".to_string(),
        };
        registry.register(custom);
        assert!(registry.get("custom-1").is_some());
        assert_eq!(registry.list_available().len(), 4);
    }

    #[test]
    fn test_deprecate_plan() {
        let registry = PlanRegistry::new();
        registry.deprecate("starter");
        let available = registry.list_available();
        assert_eq!(available.len(), 2); // Free + Business
        let individual = registry.get("starter").unwrap();
        assert!(!individual.available);
        assert!(individual.legacy);
    }

    #[test]
    fn test_format_price() {
        assert_eq!(Plan::format_price(199_900), "1999.00 ₽");
        assert_eq!(Plan::format_price(0), "0.00 ₽");
        assert_eq!(Plan::format_price(499_000), "4990.00 ₽");
    }

    #[test]
    fn test_plan_id_display() {
        assert_eq!(PlanId::Trial.to_string(), "trial");
        assert_eq!(PlanId::Starter.to_string(), "starter");
        assert_eq!(PlanId::Pro.to_string(), "pro");
    }
}

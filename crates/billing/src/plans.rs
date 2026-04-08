//! Plan definitions and registry
//!
//! Three tiers: Free, Pro, Enterprise
//! All prices in RUB (Russian market)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// Built-in plan IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanId {
    Free,
    Pro,
    Enterprise,
}

impl PlanId {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanId::Free => "free",
            PlanId::Pro => "pro",
            PlanId::Enterprise => "enterprise",
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
    /// API requests per day (0 = unlimited)
    pub api_requests_per_day: u64,
    /// LLM tokens per day (0 = unlimited)
    pub tokens_per_day: u64,
    /// Max concurrent agents (0 = unlimited)
    pub max_agents: u64,
    /// Storage limit in MB (0 = unlimited)
    pub storage_mb: u64,
    /// Max payload size in KB (0 = unlimited)
    pub max_payload_kb: u64,
    /// Max agents total (not concurrent)
    pub max_agents_total: u64,
    /// Webhook rate limit per minute
    pub webhook_rate_per_min: u64,
    /// MCP tools per agent
    pub mcp_tools_per_agent: u64,
    /// Audit log retention in days
    pub audit_retention_days: u64,
    /// Priority support
    pub priority_support: bool,
    /// Custom domain
    pub custom_domain: bool,
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
}

impl Plan {
    /// Free plan
    pub fn free() -> Self {
        Self {
            id: PlanId::Free.as_str().to_string(),
            name: "Free".to_string(),
            description: "Для знакомства с FlowLink".to_string(),
            tier: 0,
            price_kopecks: 0,
            annual_price_kopecks: None,
            limits: PlanLimits {
                api_requests_per_day: 100,
                tokens_per_day: 50_000,
                max_agents: 1,
                storage_mb: 100,
                max_payload_kb: 512,
                max_agents_total: 1,
                webhook_rate_per_min: 10,
                mcp_tools_per_agent: 5,
                audit_retention_days: 7,
                priority_support: false,
                custom_domain: false,
            },
            features: vec![
                "1 агент".to_string(),
                "100 запросов/день".to_string(),
                "50K токенов/день".to_string(),
                "100 MB хранилище".to_string(),
                "5 MCP инструментов".to_string(),
                "Базовый мониторинг".to_string(),
            ],
            available: true,
            legacy: false,
        }
    }

    /// Pro plan
    pub fn pro() -> Self {
        Self {
            id: PlanId::Pro.as_str().to_string(),
            name: "Pro".to_string(),
            description: "Для продвинутых пользователей и малого бизнеса".to_string(),
            tier: 1,
            price_kopecks: 29_990, // 299.90 RUB/month
            annual_price_kopecks: Some(299_900), // 2999 RUB/year (~17% discount)
            limits: PlanLimits {
                api_requests_per_day: 10_000,
                tokens_per_day: 5_000_000,
                max_agents: 10,
                storage_mb: 10_240, // 10 GB
                max_payload_kb: 5_120,
                max_agents_total: 25,
                webhook_rate_per_min: 100,
                mcp_tools_per_agent: 50,
                audit_retention_days: 90,
                priority_support: true,
                custom_domain: false,
            },
            features: vec![
                "До 10 агентов".to_string(),
                "10K запросов/день".to_string(),
                "5M токенов/день".to_string(),
                "10 GB хранилище".to_string(),
                "50 MCP инструментов".to_string(),
                "Приоритетная поддержка".to_string(),
                "90 дней аудита".to_string(),
                "Shield защита".to_string(),
                "ServerGuard мониторинг".to_string(),
            ],
            available: true,
            legacy: false,
        }
    }

    /// Enterprise plan
    pub fn enterprise() -> Self {
        Self {
            id: PlanId::Enterprise.as_str().to_string(),
            name: "Enterprise".to_string(),
            description: "Для компаний с высокими требованиями".to_string(),
            tier: 2,
            price_kopecks: 99_990, // 999.90 RUB/month
            annual_price_kopecks: Some(999_900), // 9999 RUB/year
            limits: PlanLimits {
                api_requests_per_day: 0, // unlimited
                tokens_per_day: 0,
                max_agents: 0,
                storage_mb: 0,
                max_payload_kb: 0,
                max_agents_total: 0,
                webhook_rate_per_min: 0,
                mcp_tools_per_agent: 0,
                audit_retention_days: 365,
                priority_support: true,
                custom_domain: true,
            },
            features: vec![
                "Безлимит".to_string(),
                "Неограниченные агенты".to_string(),
                "Неограниченные токены".to_string(),
                "Неограниченное хранилище".to_string(),
                "Неограниченные MCP инструменты".to_string(),
                "Приоритетная поддержка 24/7".to_string(),
                "365 дней аудита".to_string(),
                "Shield + ServerGuard".to_string(),
                "Кастомный домен".to_string(),
                "SLA 99.9%".to_string(),
                "Dedicated менеджер".to_string(),
            ],
            available: true,
            legacy: false,
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
}

/// Plan registry — stores all available plans
pub struct PlanRegistry {
    plans: RwLock<HashMap<String, Plan>>,
}

impl PlanRegistry {
    /// Create with default plans (Free, Pro, Enterprise)
    pub fn new() -> Self {
        let mut plans = HashMap::new();
        plans.insert(PlanId::Free.as_str().to_string(), Plan::free());
        plans.insert(PlanId::Pro.as_str().to_string(), Plan::pro());
        plans.insert(PlanId::Enterprise.as_str().to_string(), Plan::enterprise());

        Self {
            plans: RwLock::new(plans),
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
        assert!(registry.get("free").is_some());
        assert!(registry.get("pro").is_some());
        assert!(registry.get("enterprise").is_some());
    }

    #[test]
    fn test_free_plan_limits() {
        let free = Plan::free();
        assert_eq!(free.limits.api_requests_per_day, 100);
        assert_eq!(free.limits.max_agents, 1);
        assert_eq!(free.price_kopecks, 0);
    }

    #[test]
    fn test_pro_plan_limits() {
        let pro = Plan::pro();
        assert_eq!(pro.limits.api_requests_per_day, 10_000);
        assert_eq!(pro.limits.max_agents, 10);
        assert_eq!(pro.price_kopecks, 29_990);
        assert!(pro.limits.priority_support);
    }

    #[test]
    fn test_enterprise_unlimited() {
        let ent = Plan::enterprise();
        assert!(Plan::is_unlimited(ent.limits.api_requests_per_day));
        assert!(Plan::is_unlimited(ent.limits.tokens_per_day));
        assert!(Plan::is_unlimited(ent.limits.max_agents));
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
        };
        registry.register(custom);
        assert!(registry.get("custom-1").is_some());
        assert_eq!(registry.list_available().len(), 4);
    }

    #[test]
    fn test_deprecate_plan() {
        let registry = PlanRegistry::new();
        registry.deprecate("pro");
        let available = registry.list_available();
        assert_eq!(available.len(), 2); // Free + Enterprise
        let pro = registry.get("pro").unwrap();
        assert!(!pro.available);
        assert!(pro.legacy);
    }

    #[test]
    fn test_format_price() {
        assert_eq!(Plan::format_price(29_990), "299.90 ₽");
        assert_eq!(Plan::format_price(0), "0.00 ₽");
        assert_eq!(Plan::format_price(99_990), "999.90 ₽");
    }

    #[test]
    fn test_plan_id_display() {
        assert_eq!(PlanId::Free.to_string(), "free");
        assert_eq!(PlanId::Pro.to_string(), "pro");
        assert_eq!(PlanId::Enterprise.to_string(), "enterprise");
    }
}

//! FlowLink Billing Engine
//!
//! Revenue-critical module: plans, usage tracking, invoices, payments.
//!
//! # Supported Payment Methods (Russia)
//! - SBP (Система Быстрых Платежей)
//! - Т-Банк (ex-Тинькофф)
//! - СберБанк
//! - Точка Банк (for business)
//!
//! # Plans
//! - Free: 100 req/day, 1 agent, 100MB storage
//! - Pro: 10K req/day, 10 agents, 10GB storage
//! - Enterprise: unlimited, custom limits

pub mod plans;
pub mod usage;
pub mod invoice;
pub mod payment;

use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Billing engine — central struct for all billing operations
pub struct BillingEngine {
    /// Plan definitions
    plans: Arc<plans::PlanRegistry>,
    /// Usage tracking
    usage: Arc<usage::UsageTracker>,
    /// Invoice storage
    invoices: Arc<invoice::InvoiceStore>,
    /// Payment configuration
    payments: Arc<payment::PaymentConfig>,
}

/// Account billing state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBilling {
    /// Account ID
    pub account_id: String,
    /// Current plan ID
    pub plan_id: String,
    /// When the current plan was activated
    pub activated_at: DateTime<Utc>,
    /// When the current plan expires (None = never for enterprise)
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether billing is enabled for this account
    pub active: bool,
    /// Payment method on file
    pub payment_method: Option<payment::PaymentMethod>,
    /// Account balance in kopecks (1/100 RUB)
    pub balance_kopecks: i64,
    /// Current billing cycle start
    pub cycle_start: DateTime<Utc>,
}

impl AccountBilling {
    /// Create a new account billing with the Free plan
    pub fn new(account_id: &str) -> Self {
        let now = Utc::now();
        Self {
            account_id: account_id.to_string(),
            plan_id: plans::PlanId::Free.as_str().to_string(),
            activated_at: now,
            expires_at: None,
            active: true,
            payment_method: None,
            balance_kopecks: 0,
            cycle_start: now,
        }
    }
}

/// Billing check result
#[derive(Debug, Clone)]
pub struct BillingCheck {
    /// Whether the operation is allowed
    pub allowed: bool,
    /// Reason if not allowed
    pub reason: Option<String>,
    /// Current usage after this operation
    pub usage_after: Option<usage::UsageSnapshot>,
}

impl BillingEngine {
    /// Create a new billing engine
    pub fn new(payments: payment::PaymentConfig) -> Self {
        Self {
            plans: Arc::new(plans::PlanRegistry::default()),
            usage: Arc::new(usage::UsageTracker::new()),
            invoices: Arc::new(invoice::InvoiceStore::new()),
            payments: Arc::new(payments),
        }
    }

    /// Get the plan registry
    pub fn plans(&self) -> &plans::PlanRegistry {
        &self.plans
    }

    /// Get the usage tracker
    pub fn usage(&self) -> &usage::UsageTracker {
        &self.usage
    }

    /// Get the invoice store
    pub fn invoices(&self) -> &invoice::InvoiceStore {
        &self.invoices
    }

    /// Get payment config
    pub fn payments(&self) -> &payment::PaymentConfig {
        &self.payments
    }

    /// Get or create account billing state
    pub fn get_or_create_account(&self, account_id: &str) -> AccountBilling {
        AccountBilling::new(account_id)
    }

    /// Update account billing state
    ///
    /// Note: persistence is handled at the API layer (billing_api.rs) which
    /// writes to the database. This method is intentionally a no-op for the
    /// in-memory engine.
    pub fn update_account(&self, _billing: &AccountBilling) {
        // Persistence is handled by billing_api.rs → db crate
    }

    /// Check if an operation is allowed under the current plan
    ///
    /// Returns BillingCheck with allowed/denied status.
    /// Automatically tracks usage if allowed.
    pub fn check_and_track(
        &self,
        billing: &AccountBilling,
        operation: usage::UsageOperation,
    ) -> BillingCheck {
        if !billing.active {
            return BillingCheck {
                allowed: false,
                reason: Some("Billing is not active".to_string()),
                usage_after: None,
            };
        }

        let plan = match self.plans.get(&billing.plan_id) {
            Some(p) => p,
            None => {
                return BillingCheck {
                    allowed: false,
                    reason: Some(format!("Unknown plan: {}", billing.plan_id)),
                    usage_after: None,
                };
            }
        };

        // Get current usage
        let current = self.usage.get_snapshot(&billing.account_id);

        // Check limits
        let limit = match operation {
            usage::UsageOperation::ApiRequest => plan.limits.api_requests_per_day,
            usage::UsageOperation::Tokens(_n) => plan.limits.tokens_per_day,
            usage::UsageOperation::AgentConnect => plan.limits.max_agents,
            usage::UsageOperation::StorageBytes(_n) => plan.limits.storage_mb * 1_048_576,
        };

        let current_value = match operation {
            usage::UsageOperation::ApiRequest => current.api_requests_today as u64 + 1,
            usage::UsageOperation::Tokens(n) => current.tokens_today as u64 + n as u64,
            usage::UsageOperation::AgentConnect => current.active_agents as u64 + 1,
            usage::UsageOperation::StorageBytes(n) => current.storage_bytes as u64 + n as u64,
        };

        if limit > 0 && current_value > limit {
            return BillingCheck {
                allowed: false,
                reason: Some(format!(
                    "Plan limit exceeded: {}/{}",
                    current_value, limit
                )),
                usage_after: None,
            };
        }

        // Track usage
        self.usage.track(&billing.account_id, &operation);

        let snapshot = self.usage.get_snapshot(&billing.account_id);
        BillingCheck {
            allowed: true,
            reason: None,
            usage_after: Some(snapshot),
        }
    }

    /// Upgrade an account to a new plan
    pub fn upgrade_plan(
        &self,
        billing: &mut AccountBilling,
        new_plan_id: &str,
    ) -> Result<()> {
        let plan = self.plans.get(new_plan_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown plan: {}", new_plan_id))?;

        // Don't allow downgrade
        let current_tier = self.plans.get(&billing.plan_id)
            .map(|p| p.tier)
            .unwrap_or(0);
        if plan.tier < current_tier {
            anyhow::bail!("Cannot downgrade from {} to {} (use change_plan for downgrades)",
                billing.plan_id, new_plan_id);
        }

        let now = Utc::now();
        billing.plan_id = new_plan_id.to_string();
        billing.activated_at = now;
        billing.cycle_start = now;
        billing.expires_at = if plan.tier == 0 {
            None // Free plan never expires
        } else {
            Some(now + chrono::Duration::days(30))
        };

        Ok(())
    }

    /// Change plan (allows downgrades)
    pub fn change_plan(
        &self,
        billing: &mut AccountBilling,
        new_plan_id: &str,
    ) -> Result<()> {
        let plan = self.plans.get(new_plan_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown plan: {}", new_plan_id))?;

        let now = Utc::now();
        billing.plan_id = new_plan_id.to_string();
        billing.activated_at = now;
        billing.cycle_start = now;
        billing.expires_at = if plan.tier == 0 {
            None
        } else {
            Some(now + chrono::Duration::days(30))
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_engine() -> BillingEngine {
        BillingEngine::new(payment::PaymentConfig::default())
    }

    #[test]
    fn test_account_billing_new() {
        let billing = AccountBilling::new("acc-1");
        assert_eq!(billing.plan_id, "free");
        assert!(billing.active);
        assert!(billing.payment_method.is_none());
    }

    #[test]
    fn test_check_api_request_free() {
        let engine = test_engine();
        let billing = AccountBilling::new("acc-1");

        // Free plan: 100 req/day — first 100 should be allowed
        for i in 0..100 {
            let check = engine.check_and_track(&billing, usage::UsageOperation::ApiRequest);
            assert!(check.allowed, "Request {} should be allowed", i);
        }

        // 101st should be denied
        let check = engine.check_and_track(&billing, usage::UsageOperation::ApiRequest);
        assert!(!check.allowed);
        assert!(check.reason.unwrap().contains("limit exceeded"));
    }

    #[test]
    fn test_check_agent_connect_free() {
        let engine = test_engine();
        let billing = AccountBilling::new("acc-1");

        // Free plan: 1 agent
        let check = engine.check_and_track(&billing, usage::UsageOperation::AgentConnect);
        assert!(check.allowed);

        let check = engine.check_and_track(&billing, usage::UsageOperation::AgentConnect);
        assert!(!check.allowed);
    }

    #[test]
    fn test_upgrade_plan() {
        let engine = test_engine();
        let mut billing = AccountBilling::new("acc-1");

        engine.upgrade_plan(&mut billing, "pro").unwrap();
        assert_eq!(billing.plan_id, "pro");
        assert!(billing.expires_at.is_some());
    }

    #[test]
    fn test_upgrade_denies_downgrade() {
        let engine = test_engine();
        let mut billing = AccountBilling::new("acc-1");

        engine.upgrade_plan(&mut billing, "pro").unwrap();
        let result = engine.upgrade_plan(&mut billing, "free");
        assert!(result.is_err());
        assert_eq!(billing.plan_id, "pro"); // unchanged
    }

    #[test]
    fn test_change_plan_allows_downgrade() {
        let engine = test_engine();
        let mut billing = AccountBilling::new("acc-1");

        engine.upgrade_plan(&mut billing, "pro").unwrap();
        engine.change_plan(&mut billing, "free").unwrap();
        assert_eq!(billing.plan_id, "free");
    }

    #[test]
    fn test_inactive_billing_blocks() {
        let engine = test_engine();
        let mut billing = AccountBilling::new("acc-1");
        billing.active = false;

        let check = engine.check_and_track(&billing, usage::UsageOperation::ApiRequest);
        assert!(!check.allowed);
        assert!(check.reason.unwrap().contains("not active"));
    }

    #[test]
    fn test_unknown_plan_blocks() {
        let engine = test_engine();
        let mut billing = AccountBilling::new("acc-1");
        billing.plan_id = "nonexistent".to_string();

        let check = engine.check_and_track(&billing, usage::UsageOperation::ApiRequest);
        assert!(!check.allowed);
        assert!(check.reason.unwrap().contains("Unknown plan"));
    }
}

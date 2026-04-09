//! FlowLink Billing Engine
//!
//! Revenue-critical module: plans, usage tracking, invoices, payments.
//!
//! # Payment Gateway (Russia)
//! - Точка Банк acquiring (SBP + bank cards)
//! - Subscriptions API (рекуррентные автосписания)
//!
//! # Plans
//! - Trial: 1 host, pattern blocking (7 days)
//! - Starter: 3 hosts, AST analysis
//! - Pro: 20 hosts, eBPF

pub mod plans;
pub mod usage;
pub mod invoice;
pub mod payment;
pub mod tochka;
pub mod persist;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
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
    /// In-memory account billing state (account_id → AccountBilling)
    accounts: DashMap<String, AccountBilling>,
    /// Optional persistence backend (None = memory-only)
    persist: Option<Arc<dyn persist::BillingPersist>>,
    /// Whether initial load from persist has been done
    loaded: AtomicBool,
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
    /// Whether account is in trial mode
    pub is_trial: bool,
    /// Trial start date
    pub trial_start: Option<DateTime<Utc>>,
    /// Trial end date
    pub trial_end: Option<DateTime<Utc>>,
}

impl AccountBilling {
    /// Create a new account billing with the Free plan
    pub fn new(account_id: &str) -> Self {
        let now = Utc::now();
        Self {
            account_id: account_id.to_string(),
            plan_id: plans::PlanId::Trial.as_str().to_string(),
            activated_at: now,
            expires_at: None,
            active: true,
            payment_method: None,
            balance_kopecks: 0,
            cycle_start: now,
            is_trial: false,
            trial_start: None,
            trial_end: None,
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
    /// Create a new billing engine (memory-only, no persistence)
    pub fn new(payments: payment::PaymentConfig) -> Self {
        Self {
            plans: Arc::new(plans::PlanRegistry::default()),
            usage: Arc::new(usage::UsageTracker::new()),
            invoices: Arc::new(invoice::InvoiceStore::new()),
            payments: Arc::new(payments),
            accounts: DashMap::new(),
            persist: None,
            loaded: AtomicBool::new(true),
        }
    }

    /// Create with persistence backend
    pub fn with_persist(
        payments: payment::PaymentConfig,
        persist: Arc<dyn persist::BillingPersist>,
    ) -> Self {
        Self {
            plans: Arc::new(plans::PlanRegistry::default()),
            usage: Arc::new(usage::UsageTracker::new()),
            invoices: Arc::new(invoice::InvoiceStore::new()),
            payments: Arc::new(payments),
            accounts: DashMap::new(),
            persist: Some(persist),
            loaded: AtomicBool::new(false),
        }
    }

    /// Load all accounts from persistence into memory.
    /// Must be called once at startup when using a persist backend.
    /// Idempotent — safe to call multiple times.
    pub async fn load_all(&self) -> Result<usize> {
        if self.loaded.swap(true, Ordering::SeqCst) {
            return Ok(self.accounts.len());
        }

        let persist = match &self.persist {
            Some(p) => p,
            None => return Ok(0),
        };

        let accounts = persist.load_all().await?;
        let count = accounts.len();
        for acc in accounts {
            self.accounts.insert(acc.account_id.clone(), acc);
        }

        tracing::info!(count, "Loaded billing accounts from persistence");
        Ok(count)
    }

    /// Persist a single account to the backend (fire-and-forget with logging).
    /// Called automatically on mutations. Non-blocking.
    fn persist_account(&self, account: &AccountBilling) {
        if let Some(persist) = &self.persist {
            let account = account.clone();
            let persist = persist.clone();
            tokio::spawn(async move {
                if let Err(e) = persist.save_account(&account).await {
                    tracing::error!(
                        account_id = %account.account_id,
                        error = %e,
                        "Failed to persist billing account"
                    );
                }
            });
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
        self.accounts
            .entry(account_id.to_string())
            .or_insert_with(|| AccountBilling::new(account_id))
            .clone()
    }

    /// Update account billing state in memory + persist
    pub fn update_account(&self, billing: &AccountBilling) {
        self.accounts.insert(billing.account_id.clone(), billing.clone());
        self.persist_account(billing);
    }

    /// Remove an account from memory + persist
    pub fn remove_account(&self, account_id: &str) {
        self.accounts.remove(account_id);
        if let Some(persist) = &self.persist {
            let account_id = account_id.to_string();
            let persist = persist.clone();
            tokio::spawn(async move {
                if let Err(e) = persist.delete_account(&account_id).await {
                    tracing::error!(account_id, error = %e, "Failed to delete account from persistence");
                }
            });
        }
    }

    /// List all account IDs
    pub fn list_accounts(&self) -> Vec<String> {
        self.accounts.iter().map(|r| r.key().clone()).collect()
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

        // Check limits — only AgentConnect has a limit (max_hosts)
        // ApiRequest and Tokens are not limited per PRD
        match operation {
            usage::UsageOperation::AgentConnect => {
                let limit = plan.limits.max_hosts;
                let current_value = current.active_agents + 1;
                if limit > 0 && current_value > limit {
                    return BillingCheck {
                        allowed: false,
                        reason: Some(format!(
                            "Host limit exceeded: {}/{}",
                            current_value, limit
                        )),
                        usage_after: None,
                    };
                }
            }
            usage::UsageOperation::ApiRequest => {
                // No limit on API requests per PRD — always allow
            }
            usage::UsageOperation::Tokens(_n) => {
                // No limit on tokens per PRD — always allow
            }
            usage::UsageOperation::StorageBytes(_n) => {
                // No storage limit per PRD — always allow
            }
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
    ) -> Result<Option<invoice::Invoice>> {
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

        // Use billing_period from plan config to determine expiry
        let days = if plan.billing_period == "year" { 365 } else { 30 };
        billing.expires_at = if plan.tier == 0 {
            None // Free plan never expires
        } else {
            Some(now + chrono::Duration::days(days))
        };

        // Set trial if plan has trial_days
        if let Some(trial_days) = plan.trial_days {
            billing.is_trial = true;
            billing.trial_start = Some(now);
            billing.trial_end = Some(now + chrono::Duration::days(trial_days as i64));
        }

        self.update_account(billing);

        // Generate invoice for paid plans
        let created_invoice = if plan.price_kopecks > 0 {
            let inv = self.invoices.create(invoice::Invoice::for_plan(&billing.account_id, &plan));
            Some(inv)
        } else {
            None
        };

        Ok(created_invoice)
    }

    /// Change plan (allows downgrades)
    pub fn change_plan(
        &self,
        billing: &mut AccountBilling,
        new_plan_id: &str,
    ) -> Result<Option<invoice::Invoice>> {
        let plan = self.plans.get(new_plan_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown plan: {}", new_plan_id))?;

        let now = Utc::now();
        billing.plan_id = new_plan_id.to_string();
        billing.activated_at = now;
        billing.cycle_start = now;

        // Use billing_period from plan config to determine expiry
        let days = if plan.billing_period == "year" { 365 } else { 30 };
        billing.expires_at = if plan.tier == 0 {
            None
        } else {
            Some(now + chrono::Duration::days(days))
        };

        // Set trial if plan has trial_days
        if let Some(trial_days) = plan.trial_days {
            billing.is_trial = true;
            billing.trial_start = Some(now);
            billing.trial_end = Some(now + chrono::Duration::days(trial_days as i64));
        }

        self.update_account(billing);

        // Generate invoice for paid plans
        let created_invoice = if plan.price_kopecks > 0 {
            let inv = self.invoices.create(invoice::Invoice::for_plan(&billing.account_id, &plan));
            Some(inv)
        } else {
            None
        };

        Ok(created_invoice)
    }

    /// Record a payment against an invoice
    pub fn record_payment(
        &self,
        invoice_id: &str,
        method: payment::PaymentMethod,
    ) -> Result<()> {
        let mut invoice = self.invoices.get(invoice_id)
            .ok_or_else(|| anyhow::anyhow!("Invoice not found: {}", invoice_id))?;

        invoice.mark_paid(method);
        self.invoices.update(invoice.clone());

        // Credit account balance
        if let Some(mut billing) = self.accounts.get_mut(&invoice.account_id) {
            billing.balance_kopecks += invoice.subtotal_kopecks as i64;
        }

        Ok(())
    }

    /// Generate an overage invoice for an account
    pub fn generate_overage_invoice(
        &self,
        account_id: &str,
        extra_requests: u64,
        extra_tokens: u64,
    ) -> Option<invoice::Invoice> {
        if extra_requests == 0 && extra_tokens == 0 {
            return None;
        }

        let inv = self.invoices.create(invoice::Invoice::for_overage(
            account_id,
            extra_requests,
            extra_tokens,
            self.payments.overage_request_price_kopecks,
            self.payments.overage_token_price_kopecks,
        ));
        Some(inv)
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
        assert_eq!(billing.plan_id, "trial");
        assert!(billing.active);
        assert!(billing.payment_method.is_none());
        assert!(!billing.is_trial);
        assert!(billing.trial_start.is_none());
        assert!(billing.trial_end.is_none());
    }

    #[test]
    fn test_check_api_request_free() {
        let engine = test_engine();
        let billing = AccountBilling::new("acc-1");

        // Free plan: no API request limit per PRD — all allowed
        for i in 0..200 {
            let check = engine.check_and_track(&billing, usage::UsageOperation::ApiRequest);
            assert!(check.allowed, "Request {} should be allowed", i);
        }
    }

    #[test]
    fn test_check_agent_connect_free() {
        let engine = test_engine();
        let billing = AccountBilling::new("acc-1");

        // Free plan: 1 host (max_hosts = 1)
        let check = engine.check_and_track(&billing, usage::UsageOperation::AgentConnect);
        assert!(check.allowed);

        let check = engine.check_and_track(&billing, usage::UsageOperation::AgentConnect);
        assert!(!check.allowed);
        assert!(check.reason.unwrap().contains("Host limit exceeded"));
    }

    #[test]
    fn test_upgrade_plan() {
        let engine = test_engine();
        let mut billing = AccountBilling::new("acc-1");

        // Upgrade from trial to starter — no trial flag on paid plan
        engine.upgrade_plan(&mut billing, "starter").unwrap();
        assert_eq!(billing.plan_id, "starter");
        assert!(billing.expires_at.is_some());
        assert!(!billing.is_trial);
    }

    #[test]
    fn test_upgrade_denies_downgrade() {
        let engine = test_engine();
        let mut billing = AccountBilling::new("acc-1");

        engine.upgrade_plan(&mut billing, "starter").unwrap();
        let result = engine.upgrade_plan(&mut billing, "trial");
        assert!(result.is_err());
        assert_eq!(billing.plan_id, "starter"); // unchanged
    }

    #[test]
    fn test_change_plan_allows_downgrade() {
        let engine = test_engine();
        let mut billing = AccountBilling::new("acc-1");

        engine.upgrade_plan(&mut billing, "starter").unwrap();
        engine.change_plan(&mut billing, "trial").unwrap();
        assert_eq!(billing.plan_id, "trial");
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

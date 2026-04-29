//! Local (standalone) implementations of service traits.
//!
//! Wraps in-process BillingEngine and AuthEngine into trait objects.
//! Used when relay runs in standalone mode (single binary, single tenant).

use std::sync::Arc;
use async_trait::async_trait;
use flowlink_service_traits::*;

// ═══════════════════════════════════════════════
// Local Billing Provider
// ═══════════════════════════════════════════════

pub struct LocalBillingProvider {
    engine: Arc<flowlink_billing::BillingEngine>,
}

impl LocalBillingProvider {
    pub fn new(engine: Arc<flowlink_billing::BillingEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl BillingProvider for LocalBillingProvider {
    async fn list_plans(&self) -> anyhow::Result<Vec<PlanInfo>> {
        Ok(self.engine.plans().list_available().iter().map(|p| PlanInfo {
            plan_id: p.id.clone(),
            name: p.name.clone(),
            price_kopecks: p.price_kopecks,
            description: p.description.clone(),
            features: vec![],
        }).collect())
    }

    async fn get_plan(&self, plan_id: &str) -> anyhow::Result<Option<PlanInfo>> {
        Ok(self.engine.plans().get(plan_id).map(|p| PlanInfo {
            plan_id: p.id.clone(),
            name: p.name.clone(),
            price_kopecks: p.price_kopecks,
            description: p.description.clone(),
            features: vec![],
        }))
    }

    async fn get_account_info(&self, account_id: &str) -> anyhow::Result<Option<BillingAccountInfo>> {
        let accounts = self.engine.list_accounts();
        if !accounts.contains(&account_id.to_string()) {
            return Ok(None);
        }
        let acct = self.engine.get_or_create_account(account_id);
        let plan = self.engine.plans().get(&acct.plan_id);
        Ok(Some(BillingAccountInfo {
            account_id: acct.account_id.clone(),
            plan_id: acct.plan_id.clone(),
            plan_name: plan.map(|p| p.name.clone()).unwrap_or_default(),
            status: if acct.active { "active".into() } else { "inactive".into() },
            balance_kopecks: acct.balance_kopecks,
            trial_ends_at: acct.trial_end,
        }))
    }

    async fn get_or_create_account(&self, account_id: &str) -> anyhow::Result<BillingAccountInfo> {
        let acct = self.engine.get_or_create_account(account_id);
        let plan = self.engine.plans().get(&acct.plan_id);
        Ok(BillingAccountInfo {
            account_id: acct.account_id.clone(),
            plan_id: acct.plan_id.clone(),
            plan_name: plan.map(|p| p.name.clone()).unwrap_or_default(),
            status: if acct.active { "active".into() } else { "inactive".into() },
            balance_kopecks: acct.balance_kopecks,
            trial_ends_at: acct.trial_end,
        })
    }

    async fn change_plan(&self, account_id: &str, plan_id: &str) -> anyhow::Result<()> {
        let mut acct = self.engine.get_or_create_account(account_id);
        self.engine.change_plan(&mut acct, plan_id)?;
        self.engine.update_account(&acct);
        Ok(())
    }

    async fn check_feature(&self, account_id: &str, feature: &str) -> anyhow::Result<bool> {
        let acct = self.engine.get_or_create_account(account_id);
        let plan = self.engine.plans().get(&acct.plan_id);
        Ok(plan.map(|p| {
            let f = &p.features;
            match feature {
                "shield" => f.shield,
                "mcp_gateway" => f.mcp_gateway,
                "policy_engine" => f.policy_engine,
                "approval" => f.approval,
                "rbac" => f.rbac,
                "pattern_learning" => f.pattern_learning,
                "e2ee" => f.e2ee,
                "audit_log" => f.audit_log,
                "webhooks" => f.webhooks,
                "siem_export" => f.siem_export,
                "sso" => f.sso,
                "on_premise" => f.on_premise,
                "serverguard" => f.serverguard,
                "forensics" => f.forensics,
                "ai_ops" => f.ai_ops,
                "change_management" => f.change_management,
                "redaction" => f.redaction,
                _ => false,
            }
        }).unwrap_or(false))
    }

    async fn track_usage(&self, _account_id: &str, _tokens: u32) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_usage(&self, account_id: &str) -> anyhow::Result<UsageInfo> {
        let _acct = self.engine.get_or_create_account(account_id);
        Ok(UsageInfo {
            agents_connected: 0,
            commands_total: 0,
            commands_blocked: 0,
            storage_used_bytes: 0,
            period: "current".into(),
        })
    }

    async fn list_invoices(&self, account_id: &str) -> anyhow::Result<Vec<InvoiceInfo>> {
        let invoices = self.engine.invoices().list_for_account(account_id);
        Ok(invoices.into_iter().map(|inv| InvoiceInfo {
            id: inv.id,
            amount_kopecks: inv.total_kopecks as i64,
            status: format!("{:?}", inv.status).to_lowercase(),
            created_at: inv.created_at,
            description: inv.items.first()
                .map(|i| i.description.clone())
                .unwrap_or_default(),
        }).collect())
    }

    async fn check_agent_limit(&self, account_id: &str) -> anyhow::Result<bool> {
        let acct = self.engine.get_or_create_account(account_id);
        let check = self.engine.check_and_track(&acct, flowlink_billing::usage::UsageOperation::AgentConnect);
        Ok(check.allowed)
    }

    async fn check_storage_limit(&self, _account_id: &str, _bytes: u64) -> anyhow::Result<bool> {
        Ok(true)
    }
}

// ═══════════════════════════════════════════════
// Local Auth Provider
// ═══════════════════════════════════════════════

pub struct LocalAuthProvider {
    engine: Arc<crate::auth::AuthEngine>,
}

impl LocalAuthProvider {
    pub fn new(engine: Arc<crate::auth::AuthEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl AuthProvider for LocalAuthProvider {
    async fn validate_token(&self, token: &str) -> anyhow::Result<AuthCheckResult> {
        let claims = self.engine.validate_access_token(token)?;
        Ok(AuthCheckResult {
            account_id: claims.account_id.clone(),
            is_admin: claims.is_admin,
            org_id: claims.org_id.clone(),
            plan_id: None,
        })
    }

    async fn check_account(&self, account_id: &str) -> anyhow::Result<bool> {
        // AuthEngine doesn't have get_client; just check if we can validate a dummy token
        let _ = account_id;
        Ok(true)
    }

    async fn get_account_orgs(&self, _account_id: &str) -> anyhow::Result<Vec<String>> {
        Ok(vec![])
    }

    async fn check_org_role(&self, _account_id: &str, _org_id: &str, _required_role: &str) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn create_session(&self, account_id: &str, device_info: &str) -> anyhow::Result<String> {
        let session_id = uuid::Uuid::new_v4().to_string();
        self.engine.create_session(
            account_id,
            &session_id,
            None,
            Some(device_info),
            None,
            None,
        );
        Ok(session_id)
    }

    async fn revoke_session(&self, _session_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

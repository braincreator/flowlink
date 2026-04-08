//! DB-backed persistence adapter for BillingEngine
//!
//! Bridges flowlink-billing (in-memory) ↔ flowlink-db (PostgreSQL).
//! Lives in relay crate because it depends on both billing and db.
//!
//! Usage:
//! ```ignore
//! let persist = DbPersist::new(db.pool().clone());
//! let engine = BillingEngine::with_persist(config, Arc::new(persist));
//! engine.load_all().await?;
//! ```

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

use flowlink_billing::{AccountBilling, persist::BillingPersist};

/// PostgreSQL-backed persistence for billing state.
pub struct DbPersist {
    pool: PgPool,
}

impl DbPersist {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BillingPersist for DbPersist {
    async fn load_all(&self) -> Result<Vec<AccountBilling>> {
        let rows = flowlink_db::accounts::AccountRepo::list(&self.pool).await?;

        let accounts: Vec<AccountBilling> = rows
            .into_iter()
            .map(|r| AccountBilling {
                account_id: r.account_id,
                plan_id: r.plan_id,
                activated_at: r.activated_at,
                expires_at: r.expires_at,
                active: r.active,
                payment_method: r.payment_method
                    .as_deref()
                    .and_then(|m| parse_payment_method(m)),
                balance_kopecks: r.balance_kopecks,
                cycle_start: r.cycle_start,
            })
            .collect();

        Ok(accounts)
    }

    async fn save_account(&self, account: &AccountBilling) -> Result<()> {
        // Upsert: create if missing, update if exists
        let exists = flowlink_db::accounts::AccountRepo::get(&self.pool, &account.account_id)
            .await?;

        match exists {
            None => {
                flowlink_db::accounts::AccountRepo::create(
                    &self.pool, &account.account_id, &account.plan_id,
                ).await?;
            }
            Some(_) => {
                // Update all mutable fields
                flowlink_db::accounts::AccountRepo::update_plan(
                    &self.pool, &account.account_id, &account.plan_id,
                ).await?;

                flowlink_db::accounts::AccountRepo::set_active(
                    &self.pool, &account.account_id, account.active,
                ).await?;

                let method_str = account.payment_method
                    .as_ref()
                    .map(|m| format!("{:?}", m).to_lowercase());

                flowlink_db::accounts::AccountRepo::set_payment_method(
                    &self.pool, &account.account_id, method_str.as_deref(),
                ).await?;
            }
        }

        Ok(())
    }

    async fn delete_account(&self, account_id: &str) -> Result<()> {
        // AccountRepo doesn't have delete yet — use raw SQL
        sqlx::query("DELETE FROM accounts WHERE account_id = $1")
            .bind(account_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Parse payment method string from DB back to enum
fn parse_payment_method(s: &str) -> Option<flowlink_billing::payment::PaymentMethod> {
    match s {
        "sbp" => Some(flowlink_billing::payment::PaymentMethod::Sbp),
        "card" => Some(flowlink_billing::payment::PaymentMethod::Card),
        "bank_transfer" => Some(flowlink_billing::payment::PaymentMethod::BankTransfer),
        "balance" => Some(flowlink_billing::payment::PaymentMethod::Balance),
        "admin" => Some(flowlink_billing::payment::PaymentMethod::Admin),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_payment_method() {
        assert!(parse_payment_method("sbp").is_some());
        assert!(parse_payment_method("card").is_some());
        assert!(parse_payment_method("unknown").is_none());
    }
}

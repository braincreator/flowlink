//! Billing persistence layer
//!
//! Decouples BillingEngine from storage backend.
//! Engine stays in-memory for speed; persistence syncs on mutations.
//!
//! Usage:
//! ```ignore
//! let engine = BillingEngine::new(config);
//! let persist = DbPersist::new(pool);
//! engine.load_all(&persist).await?;
//! // mutations auto-sync via persist callback
//! ```

use anyhow::Result;
use async_trait::async_trait;

use super::{AccountBilling, BillingEngine};

// ---------------------------------------------------------------------------
// Persist trait — storage backend abstraction
// ---------------------------------------------------------------------------

/// Persistence backend for billing state.
/// Implementations bridge BillingEngine ↔ DB/file/etc.
#[async_trait]
pub trait BillingPersist: Send + Sync {
    /// Load all accounts from storage into memory.
    /// Returns accounts to populate the in-memory DashMap.
    async fn load_all(&self) -> Result<Vec<AccountBilling>>;

    /// Persist a single account (upsert).
    /// Called after plan change, activation, payment, etc.
    async fn save_account(&self, account: &AccountBilling) -> Result<()>;

    /// Delete an account from storage.
    async fn delete_account(&self, account_id: &str) -> Result<()>;
}

// ---------------------------------------------------------------------------
// NullPersist — no-op for testing / dev mode
// ---------------------------------------------------------------------------

/// No-op persistence — data lives only in memory.
/// Perfect for tests and local development without DB.
pub struct NullPersist;

#[async_trait]
impl BillingPersist for NullPersist {
    async fn load_all(&self) -> Result<Vec<AccountBilling>> {
        Ok(vec![])
    }

    async fn save_account(&self, _account: &AccountBilling) -> Result<()> {
        Ok(())
    }

    async fn delete_account(&self, _account_id: &str) -> Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_null_persist_load_empty() {
        let p = NullPersist;
        let accounts = p.load_all().await.unwrap();
        assert!(accounts.is_empty());
    }

    #[tokio::test]
    async fn test_null_persist_save_noop() {
        let p = NullPersist;
        let billing = AccountBilling::new("test-acc");
        assert!(p.save_account(&billing).await.is_ok());
        assert!(p.delete_account("test-acc").await.is_ok());
    }
}

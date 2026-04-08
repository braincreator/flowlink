//! Usage tracking per account
//!
//! Tracks API requests, LLM tokens, active agents, storage.
//! Uses a sliding window for daily counters.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// Types of billable operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageOperation {
    /// Single API request
    ApiRequest,
    /// LLM tokens consumed
    Tokens(u64),
    /// Agent connection (counts toward concurrent limit)
    AgentConnect,
    /// Storage bytes used
    StorageBytes(u64),
}

/// Snapshot of current usage for an account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSnapshot {
    /// Account ID
    pub account_id: String,
    /// API requests today
    pub api_requests_today: u64,
    /// LLM tokens today
    pub tokens_today: u64,
    /// Currently connected agents
    pub active_agents: u64,
    /// Total storage bytes used
    pub storage_bytes: u64,
    /// Total API requests all time
    pub api_requests_total: u64,
    /// Total tokens all time
    pub tokens_total: u64,
    /// Snapshot timestamp
    pub measured_at: DateTime<Utc>,
}

/// Per-account usage counters
#[derive(Debug, Clone, Default)]
struct AccountUsage {
    /// Daily API request counter
    api_requests_today: u64,
    /// Daily token counter
    tokens_today: u64,
    /// Currently connected agents
    active_agents: u64,
    /// Storage bytes
    storage_bytes: u64,
    /// Lifetime counters
    api_requests_total: u64,
    tokens_total: u64,
    /// Last reset timestamp (for daily window)
    last_reset: DateTime<Utc>,
}

/// Usage tracker — tracks all billable operations
pub struct UsageTracker {
    /// Per-account usage data
    accounts: RwLock<HashMap<String, AccountUsage>>,
}

impl UsageTracker {
    /// Create a new usage tracker
    pub fn new() -> Self {
        Self {
            accounts: RwLock::new(HashMap::new()),
        }
    }

    /// Track a usage operation
    pub fn track(&self, account_id: &str, operation: &UsageOperation) {
        let mut accounts = self.accounts.write().unwrap();
        let usage = accounts.entry(account_id.to_string()).or_default();

        // Reset daily counters if needed
        let now = Utc::now();
        if now.date_naive() != usage.last_reset.date_naive() {
            usage.api_requests_today = 0;
            usage.tokens_today = 0;
            usage.last_reset = now;
        }

        match operation {
            UsageOperation::ApiRequest => {
                usage.api_requests_today += 1;
                usage.api_requests_total += 1;
            }
            UsageOperation::Tokens(n) => {
                usage.tokens_today += n;
                usage.tokens_total += n;
            }
            UsageOperation::AgentConnect => {
                usage.active_agents += 1;
            }
            UsageOperation::StorageBytes(n) => {
                usage.storage_bytes += n;
            }
        }
    }

    /// Release an agent connection (decrement counter)
    pub fn release_agent(&self, account_id: &str) {
        let mut accounts = self.accounts.write().unwrap();
        if let Some(usage) = accounts.get_mut(account_id) {
            usage.active_agents = usage.active_agents.saturating_sub(1);
        }
    }

    /// Subtract storage bytes (file deletion)
    pub fn release_storage(&self, account_id: &str, bytes: u64) {
        let mut accounts = self.accounts.write().unwrap();
        if let Some(usage) = accounts.get_mut(account_id) {
            usage.storage_bytes = usage.storage_bytes.saturating_sub(bytes);
        }
    }

    /// Get a snapshot of current usage for an account
    pub fn get_snapshot(&self, account_id: &str) -> UsageSnapshot {
        let accounts = self.accounts.read().unwrap();
        let usage = accounts.get(account_id);

        match usage {
            Some(u) => UsageSnapshot {
                account_id: account_id.to_string(),
                api_requests_today: u.api_requests_today,
                tokens_today: u.tokens_today,
                active_agents: u.active_agents,
                storage_bytes: u.storage_bytes,
                api_requests_total: u.api_requests_total,
                tokens_total: u.tokens_total,
                measured_at: Utc::now(),
            },
            None => UsageSnapshot {
                account_id: account_id.to_string(),
                api_requests_today: 0,
                tokens_today: 0,
                active_agents: 0,
                storage_bytes: 0,
                api_requests_total: 0,
                tokens_total: 0,
                measured_at: Utc::now(),
            },
        }
    }

    /// Reset daily counters for all accounts (call at midnight)
    pub fn reset_daily(&self) {
        let mut accounts = self.accounts.write().unwrap();
        let now = Utc::now();
        for usage in accounts.values_mut() {
            usage.api_requests_today = 0;
            usage.tokens_today = 0;
            usage.last_reset = now;
        }
    }

    /// Get usage for all accounts (admin endpoint)
    pub fn get_all_snapshots(&self) -> Vec<UsageSnapshot> {
        let accounts = self.accounts.read().unwrap();
        accounts.keys()
            .map(|id| self.get_snapshot(id))
            .collect()
    }

    /// Clear all data for an account
    pub fn clear_account(&self, account_id: &str) {
        let mut accounts = self.accounts.write().unwrap();
        accounts.remove(account_id);
    }
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_track_api_request() {
        let tracker = UsageTracker::new();
        tracker.track("acc-1", &UsageOperation::ApiRequest);
        tracker.track("acc-1", &UsageOperation::ApiRequest);
        tracker.track("acc-1", &UsageOperation::ApiRequest);

        let snap = tracker.get_snapshot("acc-1");
        assert_eq!(snap.api_requests_today, 3);
        assert_eq!(snap.api_requests_total, 3);
    }

    #[test]
    fn test_track_tokens() {
        let tracker = UsageTracker::new();
        tracker.track("acc-1", &UsageOperation::Tokens(1000));
        tracker.track("acc-1", &UsageOperation::Tokens(500));

        let snap = tracker.get_snapshot("acc-1");
        assert_eq!(snap.tokens_today, 1500);
        assert_eq!(snap.tokens_total, 1500);
    }

    #[test]
    fn test_track_agent_connect() {
        let tracker = UsageTracker::new();
        tracker.track("acc-1", &UsageOperation::AgentConnect);
        tracker.track("acc-1", &UsageOperation::AgentConnect);

        let snap = tracker.get_snapshot("acc-1");
        assert_eq!(snap.active_agents, 2);

        tracker.release_agent("acc-1");
        let snap = tracker.get_snapshot("acc-1");
        assert_eq!(snap.active_agents, 1);
    }

    #[test]
    fn test_release_agent_no_underflow() {
        let tracker = UsageTracker::new();
        tracker.release_agent("nonexistent"); // should not panic
        assert_eq!(tracker.get_snapshot("nonexistent").active_agents, 0);
    }

    #[test]
    fn test_track_storage() {
        let tracker = UsageTracker::new();
        tracker.track("acc-1", &UsageOperation::StorageBytes(1024));

        let snap = tracker.get_snapshot("acc-1");
        assert_eq!(snap.storage_bytes, 1024);

        tracker.release_storage("acc-1", 512);
        let snap = tracker.get_snapshot("acc-1");
        assert_eq!(snap.storage_bytes, 512);
    }

    #[test]
    fn test_empty_account_snapshot() {
        let tracker = UsageTracker::new();
        let snap = tracker.get_snapshot("nonexistent");
        assert_eq!(snap.api_requests_today, 0);
        assert_eq!(snap.tokens_today, 0);
    }

    #[test]
    fn test_separate_accounts() {
        let tracker = UsageTracker::new();
        tracker.track("acc-1", &UsageOperation::ApiRequest);
        tracker.track("acc-2", &UsageOperation::ApiRequest);
        tracker.track("acc-2", &UsageOperation::ApiRequest);

        assert_eq!(tracker.get_snapshot("acc-1").api_requests_today, 1);
        assert_eq!(tracker.get_snapshot("acc-2").api_requests_today, 2);
    }

    #[test]
    fn test_reset_daily() {
        let tracker = UsageTracker::new();
        tracker.track("acc-1", &UsageOperation::ApiRequest);
        tracker.track("acc-1", &UsageOperation::Tokens(500));

        tracker.reset_daily();

        let snap = tracker.get_snapshot("acc-1");
        assert_eq!(snap.api_requests_today, 0);
        assert_eq!(snap.tokens_today, 0);
        // Lifetime counters should be preserved
        assert_eq!(snap.api_requests_total, 1);
        assert_eq!(snap.tokens_total, 500);
    }

    #[test]
    fn test_clear_account() {
        let tracker = UsageTracker::new();
        tracker.track("acc-1", &UsageOperation::ApiRequest);
        tracker.clear_account("acc-1");
        assert_eq!(tracker.get_snapshot("acc-1").api_requests_today, 0);
    }
}

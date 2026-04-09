//! Billing usage tracking middleware for the relay.
//!
//! Tracks per-agent API requests, token consumption, and command executions.
//! Integrates with the WS handler and billing API endpoints.

use std::collections::HashMap;
use tokio::sync::RwLock;

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════
// Per-agent usage counters
// ═══════════════════════════════════════════════

/// Cumulative usage counters for a single agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentUsage {
    /// Total API requests (WS messages + HTTP calls).
    pub api_requests: u64,
    /// Tokens sent to LLM models (prompt tokens).
    pub tokens_in: u64,
    /// Tokens received from LLM models (completion tokens).
    pub tokens_out: u64,
    /// Shell commands executed via ExecRequest.
    pub commands_executed: u64,
}

impl AgentUsage {
    /// Merge another usage snapshot into this one (additive).
    pub fn merge(&mut self, other: &AgentUsage) {
        self.api_requests += other.api_requests;
        self.tokens_in += other.tokens_in;
        self.tokens_out += other.tokens_out;
        self.commands_executed += other.commands_executed;
    }

    /// Total tokens (in + out).
    pub fn total_tokens(&self) -> u64 {
        self.tokens_in + self.tokens_out
    }
}

// ═══════════════════════════════════════════════
// In-memory usage tracker with daily aggregation
// ═══════════════════════════════════════════════

/// In-memory usage tracker with daily aggregation.
///
/// Thread-safe: uses `RwLock` for both per-agent and daily maps.
pub struct UsageTracker {
    /// `agent_id` → current cumulative usage.
    usage: RwLock<HashMap<String, AgentUsage>>,
    /// Daily totals: `"YYYY-MM-DD"` → `(total_requests, total_tokens)`.
    daily: RwLock<HashMap<String, (u64, u64)>>,
}

impl UsageTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self {
            usage: RwLock::new(HashMap::new()),
            daily: RwLock::new(HashMap::new()),
        }
    }

    // ── Record methods ───────────────────────────

    /// Record an API request from an agent.
    ///
    /// Increments the per-agent request counter and today's daily total.
    pub async fn record_request(&self, agent_id: &str) {
        let mut usage = self.usage.write().await;
        let entry = usage.entry(agent_id.to_string()).or_default();
        entry.api_requests += 1;
        drop(usage);

        // Update daily counters
        let today = today_key();
        let mut daily = self.daily.write().await;
        let (reqs, _tokens) = daily.entry(today).or_insert((0, 0));
        *reqs += 1;
    }

    /// Record token usage from an LLM request.
    ///
    /// Increments the per-agent token counters and today's daily total.
    pub async fn record_tokens(&self, agent_id: &str, tokens_in: u64, tokens_out: u64) {
        if tokens_in == 0 && tokens_out == 0 {
            return;
        }

        let mut usage = self.usage.write().await;
        let entry = usage.entry(agent_id.to_string()).or_default();
        entry.tokens_in += tokens_in;
        entry.tokens_out += tokens_out;
        drop(usage);

        // Update daily counters
        let today = today_key();
        let mut daily = self.daily.write().await;
        let (_reqs, tokens) = daily.entry(today).or_insert((0, 0));
        *tokens += tokens_in + tokens_out;
    }

    /// Record a command execution for an agent.
    ///
    /// Also counts as an API request.
    pub async fn record_command(&self, agent_id: &str) {
        let mut usage = self.usage.write().await;
        let entry = usage.entry(agent_id.to_string()).or_default();
        entry.commands_executed += 1;
        entry.api_requests += 1;
        drop(usage);

        // Update daily counters
        let today = today_key();
        let mut daily = self.daily.write().await;
        let (reqs, _tokens) = daily.entry(today).or_insert((0, 0));
        *reqs += 1;
    }

    // ── Query methods ────────────────────────────

    /// Get usage snapshot for a specific agent.
    pub async fn get_usage(&self, agent_id: &str) -> AgentUsage {
        let usage = self.usage.read().await;
        usage.get(agent_id).cloned().unwrap_or_default()
    }

    /// Get all agent usage (for `/api/billing/usage` endpoint).
    pub async fn get_all_usage(&self) -> HashMap<String, AgentUsage> {
        let usage = self.usage.read().await;
        usage.clone()
    }

    /// Get today's aggregate stats: `(total_requests, total_tokens)`.
    pub async fn today_stats(&self) -> (u64, u64) {
        let daily = self.daily.read().await;
        let today = today_key();
        daily.get(&today).copied().unwrap_or((0, 0))
    }

    /// Get the number of agents with recorded usage.
    pub async fn active_agents(&self) -> usize {
        let usage = self.usage.read().await;
        usage.len()
    }

    /// Reset daily counters (called at midnight).
    pub async fn reset_daily(&self) {
        let mut daily = self.daily.write().await;
        daily.clear();
        log::info!("Usage tracker: daily counters reset");
    }
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract token counts from an LLM response payload.
///
/// Expects the payload to contain a `usage` object with `prompt_tokens` and
/// `completion_tokens` fields (matching the OpenAI-compatible response format).
pub fn extract_tokens_from_payload(payload: &serde_json::Value) -> (u64, u64) {
    let usage = payload.get("usage");
    let tokens_in = usage
        .and_then(|u| u.get("prompt_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let tokens_out = usage
        .and_then(|u| u.get("completion_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    (tokens_in, tokens_out)
}

/// Returns today's date key in `YYYY-MM-DD` format (UTC).
fn today_key() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

// ═══════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_request() {
        let tracker = UsageTracker::new();
        tracker.record_request("agent-1").await;
        tracker.record_request("agent-1").await;
        tracker.record_request("agent-2").await;

        let u1 = tracker.get_usage("agent-1").await;
        assert_eq!(u1.api_requests, 2);
        assert_eq!(u1.tokens_in, 0);
        assert_eq!(u1.commands_executed, 0);

        let u2 = tracker.get_usage("agent-2").await;
        assert_eq!(u2.api_requests, 1);

        // Nonexistent agent returns default
        let u3 = tracker.get_usage("ghost").await;
        assert_eq!(u3.api_requests, 0);
    }

    #[tokio::test]
    async fn test_record_tokens() {
        let tracker = UsageTracker::new();
        tracker.record_tokens("agent-1", 100, 50).await;
        tracker.record_tokens("agent-1", 200, 75).await;

        let usage = tracker.get_usage("agent-1").await;
        assert_eq!(usage.tokens_in, 300);
        assert_eq!(usage.tokens_out, 125);
        assert_eq!(usage.total_tokens(), 425);
    }

    #[tokio::test]
    async fn test_record_tokens_zero_noop() {
        let tracker = UsageTracker::new();
        tracker.record_tokens("agent-1", 0, 0).await;

        // Should not create an entry
        let usage = tracker.get_usage("agent-1").await;
        assert_eq!(usage.tokens_in, 0);
        assert_eq!(usage.tokens_out, 0);
    }

    #[tokio::test]
    async fn test_record_command() {
        let tracker = UsageTracker::new();
        tracker.record_command("agent-1").await;
        tracker.record_command("agent-1").await;

        let usage = tracker.get_usage("agent-1").await;
        assert_eq!(usage.commands_executed, 2);
        // record_command also increments api_requests
        assert_eq!(usage.api_requests, 2);
    }

    #[tokio::test]
    async fn test_get_all_usage() {
        let tracker = UsageTracker::new();
        tracker.record_request("a1").await;
        tracker.record_tokens("a2", 10, 5).await;

        let all = tracker.get_all_usage().await;
        assert_eq!(all.len(), 2);
        assert_eq!(all["a1"].api_requests, 1);
        assert_eq!(all["a2"].tokens_in, 10);
    }

    #[tokio::test]
    async fn test_today_stats() {
        let tracker = UsageTracker::new();
        tracker.record_request("a1").await;
        tracker.record_request("a2").await;
        tracker.record_tokens("a1", 50, 30).await;

        let (requests, tokens) = tracker.today_stats().await;
        assert_eq!(requests, 2);
        assert_eq!(tokens, 80);
    }

    #[tokio::test]
    async fn test_active_agents() {
        let tracker = UsageTracker::new();
        assert_eq!(tracker.active_agents().await, 0);

        tracker.record_request("a1").await;
        tracker.record_request("a2").await;
        assert_eq!(tracker.active_agents().await, 2);
    }

    #[tokio::test]
    async fn test_reset_daily() {
        let tracker = UsageTracker::new();
        tracker.record_request("a1").await;
        tracker.record_tokens("a1", 100, 50).await;

        let (reqs, tokens) = tracker.today_stats().await;
        assert_eq!(reqs, 1);
        assert_eq!(tokens, 150);

        tracker.reset_daily().await;

        let (reqs, tokens) = tracker.today_stats().await;
        assert_eq!(reqs, 0);
        assert_eq!(tokens, 0);

        // Per-agent usage should still be intact
        let usage = tracker.get_usage("a1").await;
        assert_eq!(usage.api_requests, 1);
        assert_eq!(usage.tokens_in, 100);
    }

    #[tokio::test]
    async fn test_merge() {
        let mut a = AgentUsage {
            api_requests: 5,
            tokens_in: 100,
            tokens_out: 50,
            commands_executed: 3,
        };
        let b = AgentUsage {
            api_requests: 10,
            tokens_in: 200,
            tokens_out: 100,
            commands_executed: 7,
        };
        a.merge(&b);
        assert_eq!(a.api_requests, 15);
        assert_eq!(a.tokens_in, 300);
        assert_eq!(a.tokens_out, 150);
        assert_eq!(a.commands_executed, 10);
    }

    #[test]
    fn test_extract_tokens_from_payload() {
        let payload = serde_json::json!({
            "usage": {
                "prompt_tokens": 150,
                "completion_tokens": 80
            }
        });
        let (in_tok, out_tok) = extract_tokens_from_payload(&payload);
        assert_eq!(in_tok, 150);
        assert_eq!(out_tok, 80);
    }

    #[test]
    fn test_extract_tokens_missing_usage() {
        let payload = serde_json::json!({"content": "hello"});
        let (in_tok, out_tok) = extract_tokens_from_payload(&payload);
        assert_eq!(in_tok, 0);
        assert_eq!(out_tok, 0);
    }

    #[test]
    fn test_extract_tokens_partial() {
        let payload = serde_json::json!({
            "usage": {
                "prompt_tokens": 100
            }
        });
        let (in_tok, out_tok) = extract_tokens_from_payload(&payload);
        assert_eq!(in_tok, 100);
        assert_eq!(out_tok, 0);
    }

    #[test]
    fn test_today_key_format() {
        let key = today_key();
        // Should be YYYY-MM-DD
        assert_eq!(key.len(), 10);
        assert_eq!(key.chars().filter(|&c| c == '-').count(), 2);
    }

    #[test]
    fn test_agent_usage_default() {
        let usage = AgentUsage::default();
        assert_eq!(usage.api_requests, 0);
        assert_eq!(usage.total_tokens(), 0);
    }

    #[test]
    fn test_agent_usage_serialization() {
        let usage = AgentUsage {
            api_requests: 42,
            tokens_in: 1000,
            tokens_out: 500,
            commands_executed: 10,
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"api_requests\":42"));
        assert!(json.contains("\"tokens_in\":1000"));
        assert!(json.contains("\"commands_executed\":10"));
    }
}

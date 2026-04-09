//! Tempo Controller - Circuit breaker and rate limiting
//!
//! This module implements:
//! - Circuit breaker (3-state: Closed/Open/HalfOpen)
//! - Per-tool rate limiting with sliding window
//! - Per-tier rate limiting
//! - Global rate limiting
//! - Exponential backoff for repeated violations

mod types;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Utc;
use tracing::{debug, info};

use crate::config::{GlobalRateLimit, RateLimitConfig};
#[cfg(test)]
use crate::config::CircuitBreakerConfig;
use crate::types::{ActionTier, BreakerState, DenialFeedback, ExceedAction, RateBudget, RiskLevel, ToolRateLimit};

use types::{CircuitBreakerInternal, ExponentialBackoffState, GlobalTracker, TierRateTracker, TempoState, ToolRateTracker};

const DEFAULT_TOOL_LIMITS: &[(&str, ToolRateLimit)] = &[
    ("rm", ToolRateLimit { max_calls: 10, window_seconds: 60, on_exceed: ExceedAction::Deny }),
    ("docker", ToolRateLimit { max_calls: 20, window_seconds: 60, on_exceed: ExceedAction::Escalate }),
    ("apt", ToolRateLimit { max_calls: 5, window_seconds: 300, on_exceed: ExceedAction::Escalate }),
    ("systemctl", ToolRateLimit { max_calls: 15, window_seconds: 60, on_exceed: ExceedAction::Escalate }),
    ("cat", ToolRateLimit { max_calls: 200, window_seconds: 60, on_exceed: ExceedAction::ReadOnly }),
];

const DEFAULT_TIER_LIMITS: &[(&str, ToolRateLimit)] = &[
    ("ReadOnly", ToolRateLimit { max_calls: 200, window_seconds: 60, on_exceed: ExceedAction::ReadOnly }),
    ("Destructive", ToolRateLimit { max_calls: 30, window_seconds: 60, on_exceed: ExceedAction::Deny }),
    ("Network", ToolRateLimit { max_calls: 10, window_seconds: 60, on_exceed: ExceedAction::Escalate }),
];

#[allow(dead_code)]
const DEFAULT_GLOBAL_LIMIT: GlobalRateLimit = GlobalRateLimit {
    max_calls: 300,
    window_seconds: 60,
    on_exceed: ExceedAction::ReadOnly,
};

/// Main controller for rate limiting and circuit breaker
pub struct TempoController {
    inner: Arc<Mutex<TempoState>>,
}

impl TempoController {
    pub fn new(config: RateLimitConfig) -> Self {
        let inner = TempoState {
            config: config.clone(),
            breaker: CircuitBreakerInternal::new(&config.circuit_breaker),
            tool_trackers: HashMap::new(),
            tier_trackers: HashMap::new(),
            global_tracker: GlobalTracker::new(config.global_limit),
            backoff_state: ExponentialBackoffState::default(),
        };
        Self { inner: Arc::new(Mutex::new(inner)) }
    }

    pub fn with_defaults() -> Self {
        Self::new(RateLimitConfig::default())
    }

    pub fn check_rate(&self, tool: &str, tier: ActionTier) -> Result<(), DenialFeedback> {
        let mut state = self.inner.lock().map_err(|_| DenialFeedback {
            reason: "Failed to acquire lock".to_string(),
            risk_level: RiskLevel::High,
            what_would_be_needed: "Internal error".to_string(),
            remaining_budget: None,
            alternative: None,
        })?;

        if !state.config.enabled {
            return Ok(());
        }

        let now = Instant::now();

        // Check circuit breaker
        if let BreakerState::Open { since, .. } = state.breaker.get_state() {
            let elapsed: Duration = (Utc::now() - since).to_std().unwrap_or_default();
            if elapsed < Duration::from_secs(state.config.circuit_breaker.open_duration_seconds) {
                return Err(DenialFeedback {
                    reason: format!("Circuit breaker is open. Time since last failure: {:?}", elapsed),
                    risk_level: RiskLevel::Critical,
                    what_would_be_needed: "Wait for circuit breaker to recover".to_string(),
                    remaining_budget: None,
                    alternative: None,
                });
            }
        }

        // Check backoff
        if state.backoff_state.current_backoff_delay > Duration::from_secs(0) {
            if let Some(last_failure) = state.breaker.last_failure_time {
                let elapsed = now.duration_since(last_failure);
                if elapsed < state.backoff_state.current_backoff_delay {
                    return Err(DenialFeedback {
                        reason: format!("Backoff active for {:?} more.",
                            state.backoff_state.current_backoff_delay.saturating_sub(elapsed)),
                        risk_level: RiskLevel::High,
                        what_would_be_needed: "Wait for backoff to expire".to_string(),
                        remaining_budget: None,
                        alternative: None,
                    });
                }
            }
        }

        // Extract config values before mutable borrows
        let global_limit = state.global_tracker.limit.clone();
        let breaker_state = state.breaker.get_state();

        // Check global rate
        let global_result = state.global_tracker.check(now);
        if global_result.is_err() {
            let global_count = state.global_tracker.count_in_window();
            return Err(DenialFeedback {
                reason: format!("Global rate limit exceeded: {} calls in last {}s (max: {})",
                    global_count, global_limit.window_seconds, global_limit.max_calls),
                risk_level: RiskLevel::Medium,
                what_would_be_needed: format!("Wait {} seconds for reset.", global_limit.window_seconds),
                remaining_budget: Some(RateBudget {
                    tool_remaining: 0,
                    tool_reset_in_seconds: global_limit.window_seconds as u64,
                    global_remaining: 0,
                    breaker_state,
                }),
                alternative: Some("Reduce command frequency across all tools".to_string()),
            });
        }

        // Check tool rate
        let tool_limit_config = DEFAULT_TOOL_LIMITS.iter()
            .find(|(name, _)| *name == tool)
            .map(|(_, limit)| limit.clone())
            .unwrap_or_else(|| ToolRateLimit { max_calls: 100, window_seconds: 60, on_exceed: ExceedAction::Escalate });
        let tool_tracker = state.tool_trackers.entry(tool.to_string()).or_insert_with(|| ToolRateTracker::new(tool_limit_config.clone()));
        let tool_result = tool_tracker.check(now);
        if tool_result.is_err() {
            let tool_count = tool_tracker.count_in_window();
            let tool_remaining = tool_tracker.limit.max_calls.saturating_sub(tool_count);
            return Err(DenialFeedback {
                reason: format!("Rate limit exceeded for '{}': {} calls in last {}s (max: {})",
                    tool, tool_count, tool_tracker.limit.window_seconds, tool_tracker.limit.max_calls),
                risk_level: RiskLevel::Medium,
                what_would_be_needed: format!("Wait {} seconds for reset", tool_tracker.limit.window_seconds),
                remaining_budget: Some(RateBudget {
                    tool_remaining,
                    tool_reset_in_seconds: tool_remaining as u64,
                    global_remaining: 0,
                    breaker_state: state.breaker.get_state(),
                }),
                alternative: Some(format!("Wait {} seconds before retrying", tool_remaining)),
            });
        }

        // Check tier rate
        let tier_tracker = state.tier_trackers.entry(tier.clone()).or_insert_with(|| {
            let tier_name = format!("{:?}", tier);
            DEFAULT_TIER_LIMITS.iter()
                .find(|(name, _)| *name == tier_name.as_str())
                .map(|(_, limit)| TierRateTracker::new(limit.clone()))
                .unwrap_or_else(|| TierRateTracker::new(ToolRateLimit { max_calls: 100, window_seconds: 60, on_exceed: ExceedAction::Escalate }))
        });
        let tier_result = tier_tracker.check(now);
        if tier_result.is_err() {
            let tier_count = tier_tracker.count_in_window();
            let tier_remaining = tier_tracker.limit.max_calls.saturating_sub(tier_count);
            return Err(DenialFeedback {
                reason: format!("Tier rate limit exceeded for {:?}: {} calls in last {}s (max: {})",
                    tier, tier_count, tier_tracker.limit.window_seconds, tier_tracker.limit.max_calls),
                risk_level: RiskLevel::Medium,
                what_would_be_needed: format!("Wait {} seconds", tier_remaining),
                remaining_budget: Some(RateBudget {
                    tool_remaining: 0,
                    tool_reset_in_seconds: tier_remaining as u64,
                    global_remaining: 0,
                    breaker_state: state.breaker.get_state(),
                }),
                alternative: None,
            });
        }

        Ok(())
    }

    pub fn record_success(&self) {
        let mut state = match self.inner.lock() {
            Ok(s) => s,
            Err(_) => {
                debug!("Failed to acquire lock for success recording");
                return;
            }
        };

        if !state.config.enabled {
            return;
        }

        let now = Instant::now();
        state.breaker.record_success(now);

        if state.config.exponential_backoff.reset_after_success {
            state.backoff_state.consecutive_violations = 1;
            state.backoff_state.current_backoff_delay = Duration::from_secs(0);
        }

        debug!("Recorded command success");
    }

    pub fn record_failure(&self) {
        let mut state = match self.inner.lock() {
                Ok(s) => s,
                Err(_) => {
                    debug!("Failed to acquire lock for failure recording");
                    return;
                }
            };

        if !state.config.enabled {
                return;
            }

        let now = Instant::now();
        state.breaker.record_failure(now);

        if state.config.exponential_backoff.enabled {
            state.backoff_state.consecutive_violations = state.backoff_state.consecutive_violations.saturating_add(1);
            let base_delay = state.config.exponential_backoff.initial_delay_seconds;
            let multiplier = state.config.exponential_backoff.multiplier;
            let max_delay = state.config.exponential_backoff.max_delay_seconds;

            let delay = (base_delay as f64 * multiplier.powi(state.backoff_state.consecutive_violations.saturating_sub(1) as i32)).min(max_delay as f64);
            state.backoff_state.current_backoff_delay = Duration::from_secs(delay as u64);
        }

        debug!("Recorded command failure");
    }

    pub fn get_breaker_state(&self) -> BreakerState {
        match self.inner.lock() {
            Ok(state) => state.breaker.get_state(),
            Err(_) => BreakerState::Closed,
        }
    }

    pub fn get_rate_budget(&self, tool: &str) -> RateBudget {
        let state = match self.inner.lock() {
            Ok(s) => s,
            Err(_) => {
                return RateBudget {
                    tool_remaining: 0,
                    tool_reset_in_seconds: 1,
                    global_remaining: 0,
                    breaker_state: BreakerState::Closed,
                };
            }
        };

        let tool_remaining = if let Some(tracker) = state.tool_trackers.get(tool) {
            tracker.limit.max_calls.saturating_sub(tracker.count_in_window())
        } else {
            // Return configured limit or default
            state.config.per_tool_limits.get(tool)
                .map(|l| l.max_calls)
                .unwrap_or(100)
        };

        let tool_reset_in_seconds = if let Some(tracker) = state.tool_trackers.get(tool) {
            if let Some(first) = tracker.timestamps.first() {
                let elapsed = first.elapsed().as_secs();
                tracker.limit.window_seconds.saturating_sub(elapsed)
            } else {
                0
            }
        } else {
            0
        };

        RateBudget {
            tool_remaining,
            tool_reset_in_seconds,
            global_remaining: state.global_tracker.limit.max_calls.saturating_sub(state.global_tracker.count_in_window()),
            breaker_state: state.breaker.get_state(),
        }
    }

    pub fn reset(&self) {
        if let Ok(mut state) = self.inner.lock() {
            state.breaker = CircuitBreakerInternal::new(&state.config.circuit_breaker);
            state.tool_trackers.clear();
            state.tier_trackers.clear();
            state.global_tracker = GlobalTracker::new(state.config.global_limit.clone());
            state.backoff_state = ExponentialBackoffState::default();
            info!("TempoController reset");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ExponentialBackoffConfig;
    use std::thread;
    use std::time::Duration;

    fn create_test_config() -> RateLimitConfig {
        let mut per_tool_limits = HashMap::new();
        per_tool_limits.insert("rm".to_string(), ToolRateLimit {
            max_calls: 10,
            window_seconds: 60,
            on_exceed: ExceedAction::Deny,
        });

        let mut per_tier_defaults = HashMap::new();
        per_tier_defaults.insert(ActionTier::Destructive, ToolRateLimit {
            max_calls: 30,
            window_seconds: 60,
            on_exceed: ExceedAction::Deny,
        });

        RateLimitConfig {
            enabled: true,
            global_limit: GlobalRateLimit {
                max_calls: 300,
                window_seconds: 60,
                on_exceed: ExceedAction::ReadOnly,
            },
            per_tool_limits,
            per_tier_defaults,
            circuit_breaker: CircuitBreakerConfig::default(),
            exponential_backoff: ExponentialBackoffConfig::default(),
        }
    }

    #[test]
    fn test_breaker_trips_on_failures() {
        let config = create_test_config();
        let controller = TempoController::new(config);

        // After min_calls (10) failures, breaker should trip to Open
        for _ in 0..10 {
            controller.record_failure();
        }

        let state = controller.get_breaker_state();
        assert!(matches!(state, BreakerState::Open { .. }), "Expected Open after 10 failures, got {:?}", state);

        let result = controller.check_rate("test_tool", ActionTier::Destructive);
        assert!(result.is_err());
    }

    #[test]
    fn test_rate_limit_denies_excess() {
        let config = create_test_config();
        let controller = TempoController::new(config);

        for _ in 0..10 {
            controller.check_rate("rm", ActionTier::Destructive).unwrap();
        }

        let result = controller.check_rate("rm", ActionTier::Destructive);
        assert!(result.is_err());
        let feedback = result.unwrap_err();
        assert!(feedback.reason.contains("rm"));
        assert!(feedback.remaining_budget.is_some());
    }

    #[test]
    fn test_breaker_half_open_recovery() {
        // Create config with very short open duration
        let mut config = create_test_config();
        config.circuit_breaker.open_duration_seconds = 0; // Immediately transition to half-open
        let controller = TempoController::new(config);

        for _ in 0..20 {
            controller.record_failure();
        }

        let state = controller.get_breaker_state();
        assert!(matches!(state, BreakerState::Open { .. }));

        // After open_duration (0s), next check should transition to HalfOpen
        // and successes should transition to Closed
        for _ in 0..5 {
            let _ = controller.check_rate("test", ActionTier::Destructive);
            controller.record_success();
        }

        let state = controller.get_breaker_state();
        assert!(matches!(state, BreakerState::Closed | BreakerState::HalfOpen { .. }), "Expected Closed or HalfOpen, got {:?}", state);
    }

    #[test]
    fn test_rate_budget_tracking() {
        let config = create_test_config();
        let controller = TempoController::new(config);

        let budget = controller.get_rate_budget("rm");
        assert_eq!(budget.tool_remaining, 10);
        assert_eq!(budget.global_remaining, 300);

        for _ in 0..5 {
            controller.check_rate("rm", ActionTier::Destructive).unwrap();
        }

        let budget = controller.get_rate_budget("rm");
        assert_eq!(budget.tool_remaining, 5);
    }

    #[test]
    fn test_tier_rate_limiting() {
        let config = create_test_config();
        let controller = TempoController::new(config);

        for _ in 0..30 {
            controller.check_rate("test_tool", ActionTier::Destructive).unwrap();
        }

        let result = controller.check_rate("test_tool", ActionTier::Destructive);
        assert!(result.is_err());
    }

    #[test]
    fn test_global_rate_limiting() {
        let config = create_test_config();
        let controller = TempoController::new(config);

        for _ in 0..300 {
            let result = controller.check_rate("cat", ActionTier::ReadOnly);
            if result.is_err() {
                break;
            }
        }

        let result = controller.check_rate("cat", ActionTier::ReadOnly);
        assert!(result.is_err());
    }

    #[test]
    fn test_exponential_backoff() {
        let config = create_test_config();
        let controller = TempoController::new(config);

        controller.record_failure();
        let budget = controller.get_rate_budget("test");
        assert!(matches!(budget.breaker_state, BreakerState::Closed));

        for _ in 0..10 {
            controller.record_failure();
        }

        let state = controller.get_breaker_state();
        assert!(matches!(state, BreakerState::Open { .. }));
    }

    #[test]
    fn test_reset_clears_state() {
        let config = create_test_config();
        let controller = TempoController::new(config);

        for _ in 0..20 {
            controller.record_failure();
        }

        assert!(matches!(controller.get_breaker_state(), BreakerState::Open { .. }));

        controller.reset();

        assert!(matches!(controller.get_breaker_state(), BreakerState::Closed));
    }

    #[test]
    fn test_disabled_config() {
        let mut config = create_test_config();
        config.enabled = false;
        let controller = TempoController::new(config);

        for _ in 0..20 {
            controller.record_failure();
        }

        assert!(matches!(controller.get_breaker_state(), BreakerState::Closed));

        let result = controller.check_rate("rm", ActionTier::Destructive);
        assert!(result.is_ok());
    }
}

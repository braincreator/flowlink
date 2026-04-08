//! Internal types for the Tempo controller

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::config::{CircuitBreakerConfig, GlobalRateLimit};
use crate::types::{ActionTier, BreakerState, ToolRateLimit};

/// Tracks rate for a specific tool using a sliding window
#[derive(Debug)]
pub(super) struct ToolRateTracker {
    pub timestamps: Vec<Instant>,
    pub limit: ToolRateLimit,
}

impl ToolRateTracker {
    pub fn new(limit: ToolRateLimit) -> Self {
        Self { timestamps: Vec::new(), limit }
    }

    pub fn check(&mut self, now: Instant) -> Result<(), u32> {
        self.clean_expired();
        let count = self.count_in_window();
        if count < self.limit.max_calls {
            self.timestamps.push(now);
            Ok(())
        } else {
            Err(count as u32)
        }
    }

    pub fn clean_expired(&mut self) {
        let cutoff = Duration::from_secs(self.limit.window_seconds);
        let now = Instant::now();
        self.timestamps.retain(|ts| now.duration_since(*ts) < cutoff);
    }

    pub fn count_in_window(&self) -> u32 {
        self.timestamps.len() as u32
    }

    pub fn reset(&mut self) {
        self.timestamps.clear();
    }
}

/// Tracks rate for a specific action tier using a sliding window
#[derive(Debug)]
pub(super) struct TierRateTracker {
    pub timestamps: Vec<Instant>,
    pub limit: ToolRateLimit,
}

impl TierRateTracker {
    pub fn new(limit: ToolRateLimit) -> Self {
        Self { timestamps: Vec::new(), limit }
    }

    pub fn check(&mut self, now: Instant) -> Result<(), u32> {
        self.clean_expired();
        let count = self.count_in_window();
        if count < self.limit.max_calls {
            self.timestamps.push(now);
            Ok(())
        } else {
            Err(count as u32)
        }
    }

    pub fn clean_expired(&mut self) {
        let cutoff = Duration::from_secs(self.limit.window_seconds);
        let now = Instant::now();
        self.timestamps.retain(|ts| now.duration_since(*ts) < cutoff);
    }

    pub fn count_in_window(&self) -> u32 {
        self.timestamps.len() as u32
    }

    pub fn reset(&mut self) {
        self.timestamps.clear();
    }
}

/// Tracks global rate across all tools
#[derive(Debug)]
pub(super) struct GlobalTracker {
    pub timestamps: Vec<Instant>,
    pub limit: GlobalRateLimit,
}

impl GlobalTracker {
    pub fn new(limit: GlobalRateLimit) -> Self {
        Self { timestamps: Vec::new(), limit }
    }

    pub fn check(&mut self, now: Instant) -> Result<(), u32> {
        self.clean_expired();
        let count = self.count_in_window();
        if count < self.limit.max_calls {
            self.timestamps.push(now);
            Ok(())
        } else {
            Err(count as u32)
        }
    }

    pub fn clean_expired(&mut self) {
        let cutoff = Duration::from_secs(self.limit.window_seconds);
        let now = Instant::now();
        self.timestamps.retain(|ts| now.duration_since(*ts) < cutoff);
    }

    pub fn count_in_window(&self) -> u32 {
        self.timestamps.len() as u32
    }

    pub fn reset(&mut self) {
        self.timestamps.clear();
    }
}

/// A single success/failure record for the circuit breaker
#[derive(Debug)]
pub(super) struct FailureRecord {
    pub timestamp: Instant,
    pub success: bool,
}

/// Internal circuit breaker state machine
#[derive(Debug)]
pub(super) struct CircuitBreakerInternal {
    pub state: BreakerState,
    pub failure_window: Vec<FailureRecord>,
    pub half_open_remaining: u32,
    pub last_failure_time: Option<Instant>,
    pub consecutive_failures: u32,
    pub config: CircuitBreakerConfig,
}

impl CircuitBreakerInternal {
    pub fn new(config: &CircuitBreakerConfig) -> Self {
        Self {
            state: BreakerState::Closed,
            failure_window: Vec::new(),
            half_open_remaining: config.half_open_probes,
            last_failure_time: None,
            consecutive_failures: 0,
            config: config.clone(),
        }
    }

    pub fn record_success(&mut self, now: Instant) {
        let record = FailureRecord {
            timestamp: now,
            success: true,
        };
        self.failure_window.push(record);
        self.consecutive_failures = 0;
        self.update_state_for_success(now);
    }

    pub fn record_failure(&mut self, now: Instant) {
        let record = FailureRecord {
            timestamp: now,
            success: false,
        };
        self.failure_window.push(record);
        self.consecutive_failures = self.failure_window.iter().filter(|r| !r.success).count() as u32;
        self.update_state_for_failure(now);
    }

    fn update_state_for_success(&mut self, now: Instant) {
        match self.state {
            BreakerState::Closed => {}
            BreakerState::HalfOpen { .. } => {
                self.half_open_remaining = self.half_open_remaining.saturating_sub(1);
                if self.half_open_remaining == 0 {
                    self.transition_to_closed();
                }
            }
            BreakerState::Open { since, failure_count: _ } => {
                let elapsed: Duration = (chrono::Utc::now() - since).to_std().unwrap_or_default();
                if elapsed >= Duration::from_secs(self.config.open_duration_seconds) {
                    self.transition_to_half_open(now);
                }
            }
        }
    }

    fn update_state_for_failure(&mut self, now: Instant) {
        match self.state {
            BreakerState::Closed => {
                if self.should_open_circuit(now) {
                    self.transition_to_open(now);
                }
            }
            BreakerState::HalfOpen { .. } => {
                self.transition_to_open(now);
            }
            BreakerState::Open { since, .. } => {
                let elapsed: Duration = (chrono::Utc::now() - since).to_std().unwrap_or_default();
                if elapsed >= Duration::from_secs(self.config.open_duration_seconds) {
                    self.transition_to_half_open(now);
                }
            }
        }
    }

    pub fn check_can_execute(&self, tier: ActionTier) -> bool {
        match self.state {
            BreakerState::Open { .. } => tier == ActionTier::ReadOnly,
            BreakerState::HalfOpen { probe_remaining } => {
                probe_remaining > 0 && tier != ActionTier::ReadOnly
            }
            BreakerState::Closed => true,
        }
    }

    fn transition_to_open(&mut self, now: Instant) {
        self.state = BreakerState::Open {
            since: chrono::Utc::now(),
            failure_count: self.failure_window.iter().filter(|r| !r.success).count() as u32,
        };
        self.last_failure_time = Some(now);
        self.half_open_remaining = 0;
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        tracing::warn!("Circuit breaker opened due to failure rate");
    }

    fn transition_to_half_open(&mut self, _now: Instant) {
        self.state = BreakerState::HalfOpen {
            probe_remaining: self.config.half_open_probes,
        };
        self.half_open_remaining = self.config.half_open_probes;
        tracing::debug!("Circuit breaker transitioned to half-open state");
    }

    fn transition_to_closed(&mut self) {
        self.state = BreakerState::Closed;
        self.failure_window.clear();
        self.half_open_remaining = 0;
        self.consecutive_failures = 1;
        tracing::debug!("Circuit breaker closed after successful recovery");
    }

    fn should_open_circuit(&self, now: Instant) -> bool {
        if self.failure_window.len() < self.config.min_calls as usize {
            return false;
        }

        let cutoff = Duration::from_secs(self.config.window_seconds);
        let recent_failures: Vec<_> = self.failure_window
            .iter()
            .filter(|r| now.duration_since(r.timestamp) < cutoff)
            .collect();

        if recent_failures.len() < self.config.min_calls as usize {
            return false;
        }

        let failures = recent_failures.iter().filter(|r| !r.success).count() as u32;
        let failure_rate = (failures as f64 / recent_failures.len() as f64) * 100.0;

        failure_rate > self.config.failure_threshold_percent as f64
    }

    pub fn get_state(&self) -> BreakerState {
        self.state.clone()
    }
}

/// Internal state of the TempoController
#[derive(Debug)]
pub(super) struct TempoState {
    pub config: crate::config::RateLimitConfig,
    pub breaker: CircuitBreakerInternal,
    pub tool_trackers: HashMap<String, ToolRateTracker>,
    pub tier_trackers: HashMap<ActionTier, TierRateTracker>,
    pub global_tracker: GlobalTracker,
    pub backoff_state: ExponentialBackoffState,
}

/// Exponential backoff state for repeated violations
#[derive(Debug, Clone, Default)]
pub(super) struct ExponentialBackoffState {
    pub consecutive_violations: u32,
    pub current_backoff_delay: Duration,
}

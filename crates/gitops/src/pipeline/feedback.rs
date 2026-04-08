//! Structured Denial Feedback - Human and machine-readable denial messages
//!
//! This module provides structured denial feedback generation for the shield pipeline.

use crate::types::{ActionTier, BreakerState, DenialFeedback, RateBudget, RiskLevel};
use chrono::Utc;
use std::time::Duration;

/// Builder for constructing structured denial messages
pub struct DenialFeedbackBuilder {
    reason: String,
    risk_level: RiskLevel,
    what_would_be_needed: String,
    remaining_budget: Option<RateBudget>,
    alternative: Option<String>,
}

impl DenialFeedbackBuilder {
    /// Create a new denial feedback builder
    pub fn new() -> Self {
        Self {
            reason: String::new(),
            risk_level: RiskLevel::Medium,
            what_would_be_needed: String::new(),
            remaining_budget: None,
            alternative: None,
        }
    }

    /// Set the denial reason
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = reason.into();
        self
    }

    /// Set the risk level
    pub fn risk_level(mut self, level: RiskLevel) -> Self {
        self.risk_level = level;
        self
    }

    /// Set what would be needed to proceed
    pub fn what_would_be_needed(mut self, needed: impl Into<String>) -> Self {
        self.what_would_be_needed = needed.into();
        self
    }

    /// Set the remaining rate budget
    pub fn remaining_budget(mut self, budget: RateBudget) -> Self {
        self.remaining_budget = Some(budget);
        self
    }

    /// Set an alternative command or action
    pub fn alternative(mut self, alt: impl Into<String>) -> Self {
        self.alternative = Some(alt.into());
        self
    }

    /// Build the final denial feedback
    pub fn build(self) -> DenialFeedback {
        DenialFeedback {
            reason: self.reason,
            risk_level: self.risk_level,
            what_would_be_needed: self.what_would_be_needed,
            remaining_budget: self.remaining_budget,
            alternative: self.alternative,
        }
    }
}

impl Default for DenialFeedbackBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate rate limit denial feedback
///
/// # Arguments
/// * `tool` - The tool/command name
/// * `window_secs` - Rate limit window in seconds
/// * `current_count` - Current call count in window
/// * `limit` - Maximum allowed calls
/// * `retry_after_secs` - Seconds until retry
pub fn rate_limit_denial(
    tool: &str,
    window_secs: u64,
    current_count: u32,
    limit: u32,
    retry_after_secs: u64,
) -> DenialFeedback {
    DenialFeedback {
        reason: format!(
            "ACTION DENIED: {} rate limit exceeded. Max {} calls per {}s.",
            tool, limit, window_secs
        ),
        risk_level: RiskLevel::Medium,
        what_would_be_needed: format!(
            "DETAILS: {} calls in last {}s (limit: {}). Rate limit will reset in {}s.",
            current_count, window_secs, limit, retry_after_secs
        ),
        remaining_budget: Some(RateBudget {
            tool_remaining: 0,
            tool_reset_in_seconds: retry_after_secs,
            global_remaining: 0,
            breaker_state: BreakerState::Closed,
        }),
        alternative: Some(format!(
            "TO PROCEED: Wait {}s for window to clear, or reduce operation frequency.",
            retry_after_secs
        )),
    }
}

/// Generate literal expansion denial feedback
///
/// # Arguments
/// * `unsafe_args` - List of (arg, reason) tuples for unsafe arguments
/// * `safe_alternative` - Suggested safe alternative approach
pub fn literal_denial(
    unsafe_args: Vec<(String, String)>,
    safe_alternative: String,
) -> DenialFeedback {
    let unsafe_list: Vec<String> = unsafe_args
        .iter()
        .map(|(arg, reason)| format!("'{}' ({})", arg, reason))
        .collect();
    
    let unsafe_str = unsafe_list.join(", ");
    
    DenialFeedback {
        reason: format!(
            "ACTION DENIED: Shell expansion in destructive command: {}",
            unsafe_str
        ),
        risk_level: RiskLevel::High,
        what_would_be_needed: format!(
            "Shell variables/globs detected in: {}",
            unsafe_str
        ),
        remaining_budget: None,
        alternative: Some(format!(
            "TO PROCEED: Use literal paths instead of shell variables/globs. ALTERNATIVE: {}",
            safe_alternative
        )),
    }
}

/// Generate blocked command denial feedback
///
/// # Arguments
/// * `command` - The blocked command
/// * `reason` - Why the command was blocked
/// * `alternative` - Suggested safe alternative
pub fn blocked_denial(
    command: &str,
    reason: &str,
    alternative: &str,
) -> DenialFeedback {
    DenialFeedback {
        reason: format!(
            "ACTION DENIED: {}",
            reason
        ),
        risk_level: RiskLevel::Critical,
        what_would_be_needed: format!(
            "Command '{}' is blocked by policy. Reason: {}",
            command, reason
        ),
        remaining_budget: None,
        alternative: if alternative.is_empty() {
            None
        } else {
            Some(format!("ALTERNATIVE: {}", alternative))
        },
    }
}

/// Generate circuit breaker denial feedback
///
/// # Arguments
/// * `state` - Current circuit breaker state
/// * `retry_after` - Duration until retry is allowed
pub fn circuit_breaker_denial(
    state: &BreakerState,
    retry_after: Duration,
) -> DenialFeedback {
    let (state_name, details) = match state {
        BreakerState::Closed => ("Closed", "Circuit breaker is closed".to_string()),
        BreakerState::Open { since, failure_count } => {
            let elapsed = (Utc::now() - since).num_seconds();
            (
                "Open",
                format!(
                    "Circuit breaker is open due to {} failures. Opened {}s ago.",
                    failure_count, elapsed
                ),
            )
        }
        BreakerState::HalfOpen { probe_remaining } => (
            "HalfOpen",
            format!(
                "Circuit breaker is in half-open state. {} probe attempts remaining.",
                probe_remaining
            ),
        ),
    };
    
    let retry_secs = retry_after.as_secs();
    
    DenialFeedback {
        reason: format!(
            "ACTION DENIED: Circuit breaker {} - too many recent failures",
            state_name
        ),
        risk_level: RiskLevel::High,
        what_would_be_needed: format!(
            "DETAILS: {}. Retry allowed in {}s.",
            details, retry_secs
        ),
        remaining_budget: Some(RateBudget {
            tool_remaining: 0,
            tool_reset_in_seconds: retry_secs,
            global_remaining: 0,
            breaker_state: state.clone(),
        }),
        alternative: Some(format!(
            "TO PROCEED: Wait {}s for circuit breaker to reset.",
            retry_secs
        )),
    }
}

/// Generate escalation denial feedback
///
/// # Arguments
/// * `original_tier` - Original action tier
/// * `escalated_to` - Tier escalated to
/// * `reason` - Why escalation occurred
pub fn escalation_denial(
    original_tier: ActionTier,
    escalated_to: ActionTier,
    reason: &str,
) -> DenialFeedback {
    DenialFeedback {
        reason: format!(
            "ACTION ESCALATED: Tier {} → {}",
            tier_to_string(&original_tier),
            tier_to_string(&escalated_to)
        ),
        risk_level: RiskLevel::Medium,
        what_would_be_needed: format!(
            "DETAILS: {}. Manual approval required for escalated tier.",
            reason
        ),
        remaining_budget: None,
        alternative: Some(
            "TO PROCEED: Request approval through configured channel (Telegram/Dashboard/CLI).".to_string()
        ),
    }
}

/// Convert ActionTier to human-readable string
fn tier_to_string(tier: &ActionTier) -> &'static str {
    match tier {
        ActionTier::ReadOnly => "ReadOnly",
        ActionTier::Destructive => "Destructive",
        ActionTier::Network => "Network",
        ActionTier::Modify => "Modify",
        ActionTier::Blocked => "Blocked",
        ActionTier::Unclassified => "Unclassified",
    }
}

/// Format a denial feedback message for display
pub fn format_denial_message(feedback: &DenialFeedback) -> String {
    let mut parts = vec![feedback.reason.clone()];
    
    parts.push(format!("RISK: {:?}", feedback.risk_level));
    
    if !feedback.what_would_be_needed.is_empty() {
        parts.push(feedback.what_would_be_needed.clone());
    }
    
    if let Some(ref budget) = feedback.remaining_budget {
        parts.push(format!(
            "RATE STATUS: tool_remaining={}, global_remaining={}, breaker={:?}",
            budget.tool_remaining,
            budget.global_remaining,
            budget.breaker_state
        ));
    }
    
    if let Some(ref alt) = feedback.alternative {
        parts.push(alt.clone());
    }
    
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_denial() {
        let feedback = rate_limit_denial("rm", 60, 15, 10, 30);
        
        assert!(feedback.reason.contains("rm rate limit exceeded"));
        assert!(feedback.reason.contains("Max 10 calls per 60s"));
        assert!(feedback.what_would_be_needed.contains("15 calls"));
        assert!(feedback.alternative.unwrap().contains("Wait 30s"));
        
        let budget = feedback.remaining_budget.unwrap();
        assert_eq!(budget.tool_remaining, 0);
        assert_eq!(budget.tool_reset_in_seconds, 30);
    }

    #[test]
    fn test_literal_denial() {
        let unsafe_args = vec![
            ("$VAR".to_string(), "variable expansion".to_string()),
            ("*.txt".to_string(), "glob pattern".to_string()),
        ];
        let feedback = literal_denial(unsafe_args, "ls files first, then rm each file".to_string());
        
        assert!(feedback.reason.contains("Shell expansion"));
        assert!(feedback.reason.contains("$VAR"));
        assert!(feedback.reason.contains("*.txt"));
        assert_eq!(feedback.risk_level, RiskLevel::High);
        assert!(feedback.alternative.unwrap().contains("Use literal paths"));
    }

    #[test]
    fn test_blocked_denial() {
        let feedback = blocked_denial(
            "rm -rf /",
            "Root filesystem deletion is blocked",
            "Use targeted deletion instead"
        );
        
        assert!(feedback.reason.contains("Root filesystem deletion"));
        assert_eq!(feedback.risk_level, RiskLevel::Critical);
        assert!(feedback.what_would_be_needed.contains("blocked by policy"));
        assert!(feedback.alternative.unwrap().contains("Use targeted deletion"));
    }

    #[test]
    fn test_blocked_denial_no_alternative() {
        let feedback = blocked_denial(
            "dangerous-cmd",
            "Command is blocked",
            ""
        );
        
        assert!(feedback.reason.contains("Command is blocked"));
        assert!(feedback.alternative.is_none());
    }

    #[test]
    fn test_circuit_breaker_denial_open() {
        let state = BreakerState::Open {
            since: Utc::now() - chrono::Duration::seconds(30),
            failure_count: 5,
        };
        let feedback = circuit_breaker_denial(&state, Duration::from_secs(90));
        
        assert!(feedback.reason.contains("Circuit breaker Open"));
        assert!(feedback.what_would_be_needed.contains("5 failures"));
        assert!(feedback.alternative.unwrap().contains("Wait 90s"));
    }

    #[test]
    fn test_circuit_breaker_denial_half_open() {
        let state = BreakerState::HalfOpen { probe_remaining: 2 };
        let feedback = circuit_breaker_denial(&state, Duration::from_secs(30));
        
        assert!(feedback.reason.contains("Circuit breaker HalfOpen"));
        assert!(feedback.what_would_be_needed.contains("2 probe attempts remaining"));
    }

    #[test]
    fn test_escalation_denial() {
        let feedback = escalation_denial(
            ActionTier::ReadOnly,
            ActionTier::Destructive,
            "Rate limit exceeded for ReadOnly tier"
        );
        
        assert!(feedback.reason.contains("ACTION ESCALATED"));
        assert!(feedback.reason.contains("ReadOnly"));
        assert!(feedback.reason.contains("Destructive"));
        assert!(feedback.what_would_be_needed.contains("Manual approval required"));
        assert!(feedback.alternative.unwrap().contains("Request approval"));
    }

    #[test]
    fn test_denial_feedback_builder() {
        let feedback = DenialFeedbackBuilder::new()
            .reason("Test denial")
            .risk_level(RiskLevel::High)
            .what_would_be_needed("Admin approval")
            .alternative("Use safer alternative")
            .build();
        
        assert_eq!(feedback.reason, "Test denial");
        assert_eq!(feedback.risk_level, RiskLevel::High);
        assert_eq!(feedback.what_would_be_needed, "Admin approval");
        assert_eq!(feedback.alternative, Some("Use safer alternative".to_string()));
    }

    #[test]
    fn test_denial_feedback_builder_with_budget() {
        let budget = RateBudget {
            tool_remaining: 5,
            tool_reset_in_seconds: 30,
            global_remaining: 100,
            breaker_state: BreakerState::Closed,
        };
        
        let feedback = DenialFeedbackBuilder::new()
            .reason("Rate limit")
            .remaining_budget(budget.clone())
            .build();
        
        assert!(feedback.remaining_budget.is_some());
        let fb = feedback.remaining_budget.unwrap();
        assert_eq!(fb.tool_remaining, 5);
        assert_eq!(fb.global_remaining, 100);
    }

    #[test]
    fn test_format_denial_message() {
        let feedback = rate_limit_denial("rm", 60, 15, 10, 30);
        let formatted = format_denial_message(&feedback);
        
        assert!(formatted.contains("rm rate limit exceeded"));
        assert!(formatted.contains("RISK:"));
        assert!(formatted.contains("RATE STATUS:"));
        assert!(formatted.contains("tool_remaining=0"));
    }

    #[test]
    fn test_tier_to_string() {
        assert_eq!(tier_to_string(&ActionTier::ReadOnly), "ReadOnly");
        assert_eq!(tier_to_string(&ActionTier::Destructive), "Destructive");
        assert_eq!(tier_to_string(&ActionTier::Network), "Network");
        assert_eq!(tier_to_string(&ActionTier::Modify), "Modify");
        assert_eq!(tier_to_string(&ActionTier::Blocked), "Blocked");
        assert_eq!(tier_to_string(&ActionTier::Unclassified), "Unclassified");
    }
}

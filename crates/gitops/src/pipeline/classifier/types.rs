//! Type definitions for the action classifier

use crate::types::{ActionTier, RiskLevel, ShieldVerdict};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Rule condition types for matching command arguments
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum RuleCondition {
    /// Check if a flag is present (e.g., "-rf", "--no-preserve-root")
    HasFlag(String),
    /// Check if any arg contains a substring
    ArgContains(String),
    /// Check if any arg matches a regex pattern
    ArgMatches(String),
    /// Check if a protected path is targeted
    PathProtected(String),
    /// Check if all args are literal (no shell expansion)
    AllArgsLiteral,
}

/// Action to take when a rule matches
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum RuleAction {
    /// Allow the command
    Allow,
    /// Block the command with a reason
    Block { reason: String },
    /// Modify the command with replacements
    Modify { rewrite: RewriteConfig },
}

/// Configuration for command rewriting
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RewriteConfig {
    /// Replacements to apply to args
    pub replacements: Vec<Replacement>,
    /// Message explaining the modification
    pub message: String,
}

/// Single replacement rule
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Replacement {
    /// Pattern to match
    pub match_pattern: String,
    /// Replacement string
    pub replace_with: String,
}

/// A classification rule for commands
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClassificationRule {
    /// Rule name for identification
    pub name: String,
    /// Command name pattern (regex or exact match)
    pub command_pattern: String,
    /// Conditions that must all match
    #[serde(default)]
    pub conditions: Vec<RuleCondition>,
    /// Tier to assign if rule matches
    pub tier: ActionTier,
    /// Action to take
    #[serde(default = "default_rule_action")]
    pub action: RuleAction,
    /// Human-readable message
    #[serde(default)]
    pub message: String,
}

pub(crate) fn default_rule_action() -> RuleAction {
    RuleAction::Allow
}

impl fmt::Display for ClassificationRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Result of command classification
#[derive(Debug)]
pub struct ClassificationResult {
    /// The action tier
    pub tier: ActionTier,
    /// Optional shield verdict (for blocked/modified commands)
    pub verdict: Option<ShieldVerdict>,
    /// Name of the matching rule
    pub rule_name: Option<String>,
}

impl ClassificationResult {
    pub fn risk_level(&self) -> RiskLevel {
        match &self.tier {
            ActionTier::Blocked => RiskLevel::Critical,
            ActionTier::Destructive => RiskLevel::Medium,
            ActionTier::Network => RiskLevel::Medium,
            ActionTier::Modify => RiskLevel::Low,
            ActionTier::ReadOnly => RiskLevel::Safe,
            ActionTier::Unclassified => RiskLevel::Medium,
        }
    }
}

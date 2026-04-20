// Pattern Learning — tracks command execution patterns and suggests policy changes.
//
// Heuristic-based: counts command occurrences, blocks, approvals, and suggests
// auto-approve for trusted commands or permanent deny for repeatedly blocked ones.

use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Tracked pattern for a command prefix (first 2 tokens).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandPattern {
    pub prefix: String,
    pub exec_count: u32,
    pub blocked_count: u32,
    pub approved_count: u32,
    pub last_risk: String,
    pub last_result: String,
}

/// A suggestion produced by pattern analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternSuggestion {
    /// Command executed many times without issues → suggest auto-approve (allow rule)
    AutoApprove { prefix: String, reason: String },
    /// Command always approved despite medium risk → suggest risk downgrade
    LowerRisk { prefix: String, from: String, reason: String },
    /// Command blocked many times → suggest permanent deny rule
    PermanentDeny { prefix: String, reason: String },
}

pub struct PatternLearner {
    patterns: HashMap<String, CommandPattern>,
    /// Minimum executions before suggesting auto-approve
    auto_approve_threshold: u32,
    /// Minimum blocks before suggesting permanent deny
    deny_threshold: u32,
}

impl PatternLearner {
    pub fn new() -> Self {
        Self {
            patterns: HashMap::new(),
            auto_approve_threshold: 20,
            deny_threshold: 5,
        }
    }

    /// Extract command prefix (first 2 tokens) for grouping.
    /// "docker ps -a" → "docker ps", "rm -rf /tmp" → "rm -rf"
    fn extract_prefix(command: &str) -> String {
        let parts: Vec<&str> = command.split_whitespace().take(2).collect();
        parts.join(" ")
    }

    /// Hash a command for grouping similar invocations.
    fn hash_command(command: &str) -> String {
        let prefix = Self::extract_prefix(command);
        let mut hasher = Sha256::new();
        hasher.update(prefix.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Record a command execution result.
    pub fn track(&mut self, command: &str, risk: &str, result: &str) {
        let hash = Self::hash_command(command);
        let prefix = Self::extract_prefix(command);

        let entry = self.patterns.entry(hash).or_insert_with(|| CommandPattern {
            prefix: prefix.clone(),
            exec_count: 0,
            blocked_count: 0,
            approved_count: 0,
            last_risk: risk.to_string(),
            last_result: result.to_string(),
        });

        entry.exec_count += 1;
        entry.last_risk = risk.to_string();
        entry.last_result = result.to_string();

        match result {
            "blocked" => entry.blocked_count += 1,
            "approved" => entry.approved_count += 1,
            _ => {}
        }
    }

    /// Analyze patterns and produce suggestions.
    pub fn analyze(&self) -> Vec<PatternSuggestion> {
        let mut suggestions = Vec::new();

        for pattern in self.patterns.values() {
            // Auto-approve: executed many times, never or rarely blocked
            if pattern.exec_count >= self.auto_approve_threshold
                && pattern.blocked_count == 0
                && pattern.last_result == "allowed"
            {
                suggestions.push(PatternSuggestion::AutoApprove {
                    prefix: pattern.prefix.clone(),
                    reason: format!(
                        "Executed {} times without issues (risk: {})",
                        pattern.exec_count, pattern.last_risk
                    ),
                });
                continue;
            }

            // Lower risk: medium risk but always approved
            if pattern.approved_count >= 10
                && pattern.blocked_count == 0
                && (pattern.last_risk == "medium" || pattern.last_risk == "high")
            {
                suggestions.push(PatternSuggestion::LowerRisk {
                    prefix: pattern.prefix.clone(),
                    from: pattern.last_risk.clone(),
                    reason: format!(
                        "Approved {} times, never blocked (current risk: {})",
                        pattern.approved_count, pattern.last_risk
                    ),
                });
                continue;
            }

            // Permanent deny: blocked many times
            if pattern.blocked_count >= self.deny_threshold {
                suggestions.push(PatternSuggestion::PermanentDeny {
                    prefix: pattern.prefix.clone(),
                    reason: format!(
                        "Blocked {} out of {} attempts",
                        pattern.blocked_count, pattern.exec_count
                    ),
                });
            }
        }

        suggestions
    }

    /// Save patterns to file for persistence across restarts.
    pub fn save_to_file(&self, path: &Path) {
        if let Ok(data) = serde_json::to_string_pretty(&self.patterns) {
            if let Err(e) = std::fs::write(path, data) {
                log::warn!("Failed to save pattern cache: {e}");
            }
        }
    }

    /// Load patterns from file.
    pub fn load_from_file(path: &Path) -> Self {
        let mut learner = Self::new();
        if let Ok(data) = std::fs::read_to_string(path) {
            if let Ok(patterns) = serde_json::from_str::<HashMap<String, CommandPattern>>(&data) {
                learner.patterns = patterns;
                log::info!("Loaded {} command patterns from cache", learner.patterns.len());
            }
        }
        learner
    }

    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_prefix() {
        assert_eq!(PatternLearner::extract_prefix("docker ps -a"), "docker ps");
        assert_eq!(PatternLearner::extract_prefix("rm -rf /tmp"), "rm -rf");
        assert_eq!(PatternLearner::extract_prefix("ls"), "ls");
    }

    #[test]
    fn test_track_and_analyze_auto_approve() {
        let mut learner = PatternLearner::new();
        learner.auto_approve_threshold = 5;

        for _ in 0..5 {
            learner.track("docker ps", "low", "allowed");
        }

        let suggestions = learner.analyze();
        assert_eq!(suggestions.len(), 1);
        match &suggestions[0] {
            PatternSuggestion::AutoApprove { prefix, .. } => assert_eq!(prefix, "docker ps"),
            _ => panic!("Expected AutoApprove"),
        }
    }

    #[test]
    fn test_track_and_analyze_deny() {
        let mut learner = PatternLearner::new();
        learner.deny_threshold = 3;

        for _ in 0..3 {
            learner.track("rm -rf /", "high", "blocked");
        }

        let suggestions = learner.analyze();
        assert_eq!(suggestions.len(), 1);
        match &suggestions[0] {
            PatternSuggestion::PermanentDeny { prefix, .. } => assert_eq!(prefix, "rm -rf"),
            _ => panic!("Expected PermanentDeny"),
        }
    }

    #[test]
    fn test_no_suggestion_for_few_execs() {
        let mut learner = PatternLearner::new();
        learner.track("ls", "none", "allowed");
        learner.track("ls", "none", "allowed");
        assert!(learner.analyze().is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("patterns.json");

        let mut learner = PatternLearner::new();
        learner.track("kubectl get pods", "low", "allowed");
        learner.save_to_file(&path);

        let loaded = PatternLearner::load_from_file(&path);
        assert_eq!(loaded.pattern_count(), 1);
    }
}

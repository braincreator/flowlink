//! Action Classifier - Maps commands to action tiers
//!
//! This module provides command classification based on patterns and conditions.

mod types;
pub use types::*;

use crate::types::{ActionTier, RiskLevel, ShieldVerdict, DenialFeedback};
use anyhow::Result;

/// Action Classifier - maps commands to action tiers
pub struct ActionClassifier {
    /// Classification rules in priority order
    rules: Vec<ClassificationRule>,
}

impl Default for ActionClassifier {
    fn default() -> Self {
        Self::with_default_rules()
    }
}

impl ActionClassifier {
    /// Create a new classifier with default embedded rules
    pub fn new() -> Self {
        Self::default()
    }

    /// Create classifier with default rules embedded in code
    pub fn with_default_rules() -> Self {
        let rules = Self::build_default_rules();
        Self { rules }
    }

    /// Create classifier with custom rules
    pub fn with_rules(rules: Vec<ClassificationRule>) -> Self {
        Self { rules }
    }

    /// Build default classification rules (embedded in binary)
    fn build_default_rules() -> Vec<ClassificationRule> {
        vec![
            // === BLOCKED TIER ===
            
            // ── Git history rewrite (irreversible) ──
            ClassificationRule {
                name: "block-git-filter-branch".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\b(filter-branch|filter-repo)\b".to_string()),
                ],
                tier: ActionTier::Blocked,
                action: RuleAction::Block {
                    reason: "Git history rewrite is blocked — use with explicit admin approval".to_string(),
                },
                message: "Attempting to rewrite git history".to_string(),
            },
            
            // rm -rf / or --no-preserve-root
            ClassificationRule {
                name: "block-rm-root".to_string(),
                command_pattern: "rm".to_string(),
                conditions: vec![
                    RuleCondition::HasFlag("-rf".to_string()),
                    RuleCondition::ArgContains("/".to_string()),
                ],
                tier: ActionTier::Blocked,
                action: RuleAction::Block {
                    reason: "Root filesystem deletion is blocked".to_string(),
                },
                message: "Attempting to delete root filesystem".to_string(),
            },
            ClassificationRule {
                name: "block-rm-no-preserve-root".to_string(),
                command_pattern: "rm".to_string(),
                conditions: vec![
                    RuleCondition::HasFlag("--no-preserve-root".to_string()),
                ],
                tier: ActionTier::Blocked,
                action: RuleAction::Block {
                    reason: "No-preserve-root flag is blocked".to_string(),
                },
                message: "Attempting to bypass root preservation".to_string(),
            },
            
            // mkfs, fdisk, parted, dd
            ClassificationRule {
                name: "block-filesystem-tools".to_string(),
                command_pattern: "mkfs|fdisk|parted".to_string(),
                conditions: vec![],
                tier: ActionTier::Blocked,
                action: RuleAction::Block {
                    reason: "Filesystem manipulation tools are blocked".to_string(),
                },
                message: "Filesystem manipulation blocked".to_string(),
            },
            ClassificationRule {
                name: "block-dd-disk".to_string(),
                command_pattern: "dd".to_string(),
                conditions: vec![
                    RuleCondition::ArgContains("of=/dev/".to_string()),
                ],
                tier: ActionTier::Blocked,
                action: RuleAction::Block {
                    reason: "Direct disk writes are blocked".to_string(),
                },
                message: "Direct disk write blocked".to_string(),
            },

            // === DESTRUCTIVE TIER ===
            
            // ── Git destructive operations ──
            ClassificationRule {
                name: "destructive-git-push-force".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\bpush\b".to_string()),
                    RuleCondition::ArgContains("--force".to_string()),
                ],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "Git force push — overwrites remote history".to_string(),
            },
            ClassificationRule {
                name: "destructive-git-push-force-lease".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\bpush\b".to_string()),
                    RuleCondition::ArgContains("--force-with-lease".to_string()),
                ],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "Git force push with lease".to_string(),
            },
            ClassificationRule {
                name: "destructive-git-reset-hard".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\breset\b".to_string()),
                    RuleCondition::ArgContains("--hard".to_string()),
                ],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "Git reset --hard — discards working changes".to_string(),
            },
            ClassificationRule {
                name: "destructive-git-clean-fd".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\bclean\b".to_string()),
                    RuleCondition::ArgContains("-f".to_string()),
                    RuleCondition::ArgContains("d".to_string()),
                ],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "Git clean -fd — removing untracked files".to_string(),
            },
            ClassificationRule {
                name: "destructive-git-branch-d".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\bbranch\b".to_string()),
                    RuleCondition::HasFlag("-D".to_string()),
                ],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "Git branch -D — force delete branch".to_string(),
            },
            ClassificationRule {
                name: "destructive-git-tag-delete".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\btag\b".to_string()),
                    RuleCondition::HasFlag("-d".to_string()),
                ],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "Git tag delete".to_string(),
            },
            ClassificationRule {
                name: "destructive-git-stash-drop".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\bstash\b".to_string()),
                    RuleCondition::ArgMatches(r"\b(drop|clear)\b".to_string()),
                ],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "Git stash drop/clear".to_string(),
            },
            ClassificationRule {
                name: "destructive-git-reflog-expire".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\breflog\b".to_string()),
                    RuleCondition::ArgMatches(r"\b(expire)\b".to_string()),
                ],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "Git reflog expire — losing recovery history".to_string(),
            },
            ClassificationRule {
                name: "destructive-git-gc-prune".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\bgc\b".to_string()),
                    RuleCondition::ArgContains("--prune=now".to_string()),
                ],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "Git gc with aggressive pruning".to_string(),
            },
            ClassificationRule {
                name: "destructive-git-worktree-remove".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\bworktree\b".to_string()),
                    RuleCondition::ArgMatches(r"\b(remove|prune)\b".to_string()),
                ],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "Git worktree remove/prune".to_string(),
            },
            ClassificationRule {
                name: "destructive-git-submodule-deinit".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\bsubmodule\b".to_string()),
                    RuleCondition::ArgMatches(r"\bdeinit\b".to_string()),
                ],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "Git submodule deinit".to_string(),
            },

            // rm, rmdir
            ClassificationRule {
                name: "destructive-rm".to_string(),
                command_pattern: "rm|rmdir".to_string(),
                conditions: vec![],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "File deletion command".to_string(),
            },
            
            // docker rm/stop/kill
            ClassificationRule {
                name: "destructive-docker-rm".to_string(),
                command_pattern: "docker".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\b(rm|rmi|stop|kill|down)\b".to_string()),
                ],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "Docker container/image manipulation".to_string(),
            },
            
            // systemctl stop/restart/disable
            ClassificationRule {
                name: "destructive-systemctl".to_string(),
                command_pattern: "systemctl".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\b(stop|restart|disable)\b".to_string()),
                ],
                tier: ActionTier::Destructive,
                action: RuleAction::Allow,
                message: "System service control".to_string(),
            },

            // === MODIFY TIER ===
            
            // chmod 777 → 755
            ClassificationRule {
                name: "modify-chmod-777".to_string(),
                command_pattern: "chmod".to_string(),
                conditions: vec![
                    RuleCondition::ArgContains("777".to_string()),
                ],
                tier: ActionTier::Modify,
                action: RuleAction::Modify {
                    rewrite: RewriteConfig {
                        replacements: vec![
                            Replacement {
                                match_pattern: "777".to_string(),
                                replace_with: "755".to_string(),
                            },
                        ],
                        message: "chmod 777 auto-corrected to 755".to_string(),
                    },
                },
                message: "Unsafe permission auto-corrected".to_string(),
            },
            
            // chmod 666 → 644
            ClassificationRule {
                name: "modify-chmod-666".to_string(),
                command_pattern: "chmod".to_string(),
                conditions: vec![
                    RuleCondition::ArgContains("666".to_string()),
                ],
                tier: ActionTier::Modify,
                action: RuleAction::Modify {
                    rewrite: RewriteConfig {
                        replacements: vec![
                            Replacement {
                                match_pattern: "666".to_string(),
                                replace_with: "644".to_string(),
                            },
                        ],
                        message: "chmod 666 auto-corrected to 644".to_string(),
                    },
                },
                message: "Unsafe permission auto-corrected".to_string(),
            },
            
            // chmod/chown general
            ClassificationRule {
                name: "modify-permissions".to_string(),
                command_pattern: "chmod|chown".to_string(),
                conditions: vec![],
                tier: ActionTier::Modify,
                action: RuleAction::Allow,
                message: "Permission modification".to_string(),
            },
            
            // apt/yum/dnf/pacman install/remove/update
            ClassificationRule {
                name: "modify-package-managers".to_string(),
                command_pattern: "apt|apt-get|yum|dnf|pacman".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\b(install|remove|update|upgrade|purge)\b".to_string()),
                ],
                tier: ActionTier::Modify,
                action: RuleAction::Allow,
                message: "Package manager operation".to_string(),
            },

            // === NETWORK TIER ===
            
            ClassificationRule {
                name: "network-tools".to_string(),
                command_pattern: "curl|wget|ping|ssh|scp|rsync|nc".to_string(),
                conditions: vec![],
                tier: ActionTier::Network,
                action: RuleAction::Allow,
                message: "Network operation".to_string(),
            },

            // === READ-ONLY TIER ===
            
            // ── Git read-only operations ──
            ClassificationRule {
                name: "readonly-git".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\b(status|log|diff|show|branch|tag|remote|shortlog|blame|rev-parse|describe|ls-files|ls-tree|reflog|count-objects|verify-pack|config)\b".to_string()),
                ],
                tier: ActionTier::ReadOnly,
                action: RuleAction::Allow,
                message: "Git read operation".to_string(),
            },
            ClassificationRule {
                name: "readonly-git-stash-list".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgContains("stash".to_string()),
                    RuleCondition::ArgContains("list".to_string()),
                ],
                tier: ActionTier::ReadOnly,
                action: RuleAction::Allow,
                message: "Git stash list".to_string(),
            },
            ClassificationRule {
                name: "readonly-git-submodule-status".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgContains("submodule".to_string()),
                    RuleCondition::ArgContains("status".to_string()),
                ],
                tier: ActionTier::ReadOnly,
                action: RuleAction::Allow,
                message: "Git submodule status".to_string(),
            },
            ClassificationRule {
                name: "readonly-git-worktree-list".to_string(),
                command_pattern: "git".to_string(),
                conditions: vec![
                    RuleCondition::ArgContains("worktree".to_string()),
                    RuleCondition::ArgContains("list".to_string()),
                ],
                tier: ActionTier::ReadOnly,
                action: RuleAction::Allow,
                message: "Git worktree list".to_string(),
            },

            // Basic read-only commands
            ClassificationRule {
                name: "readonly-basic".to_string(),
                command_pattern: "cat|ls|head|tail|grep|find|stat|file|which|echo|whoami|id|pwd|env|printenv".to_string(),
                conditions: vec![],
                tier: ActionTier::ReadOnly,
                action: RuleAction::Allow,
                message: "Read-only command".to_string(),
            },
            
            // docker ps/images/inspect/logs
            ClassificationRule {
                name: "readonly-docker".to_string(),
                command_pattern: "docker".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\b(ps|images|inspect|logs|top|stats|port|diff|history)\b".to_string()),
                ],
                tier: ActionTier::ReadOnly,
                action: RuleAction::Allow,
                message: "Docker read operation".to_string(),
            },
            
            // systemctl status/list-units
            ClassificationRule {
                name: "readonly-systemctl".to_string(),
                command_pattern: "systemctl".to_string(),
                conditions: vec![
                    RuleCondition::ArgMatches(r"\b(status|list-units|list-unit-files|show|is-active|is-enabled)\b".to_string()),
                ],
                tier: ActionTier::ReadOnly,
                action: RuleAction::Allow,
                message: "Systemd read operation".to_string(),
            },
        ]
    }

    /// Classify a command and its arguments
    pub fn classify(&self, command: &str, args: &[String]) -> anyhow::Result<ClassificationResult> {
        tracing::debug!(
            command = %command,
            args_count = args.len(),
            "Classifying command"
        );

        // Try each rule in order - first match wins
        for rule in &self.rules {
            if let Some(result) = self.match_rule(rule, command, args)? {
                tracing::info!(
                    rule = %rule.name,
                    tier = ?result.tier,
                    "Rule matched"
                );
                return Ok(result);
            }
        }

        // No rule matched - return Unclassified
        tracing::warn!(
            command = %command,
            "No classification rule matched"
        );
        
        Ok(ClassificationResult {
            tier: ActionTier::Unclassified,
            verdict: None,
            rule_name: None,
        })
    }

    /// Try to match a single rule
    fn match_rule(
        &self,
        rule: &ClassificationRule,
        command: &str,
        args: &[String],
    ) -> Result<Option<ClassificationResult>> {
        // Check if command matches the pattern
        if !self.command_matches_pattern(command, &rule.command_pattern)? {
            return Ok(None);
        }

        // Check all conditions
        if !self.check_conditions(&rule.conditions, args)? {
            return Ok(None);
        }

        // Rule matches - build result
        let verdict = self.build_verdict(rule, args)?;

        Ok(Some(ClassificationResult {
            tier: rule.tier.clone(),
            verdict,
            rule_name: Some(rule.name.clone()),
        }))
    }

    /// Check if command matches a pattern (regex or exact match)
    fn command_matches_pattern(&self, command: &str, pattern: &str) -> Result<bool> {
        // Simple check: if pattern contains |, treat as regex alternation
        if pattern.contains('|') {
            let regex_str = format!("^({})$", pattern);
            let regex = regex::Regex::new(&regex_str)
                .map_err(|e| anyhow::anyhow!("Invalid command pattern '{}': {}", pattern, e))?;
            Ok(regex.is_match(command))
        } else {
            // Exact match
            Ok(command == pattern)
        }
    }

    /// Check all conditions for a rule
    fn check_conditions(&self, conditions: &[RuleCondition], args: &[String]) -> Result<bool> {
        if conditions.is_empty() {
            return Ok(true);
        }

        // All conditions must match
        for condition in conditions {
            if !self.check_condition(condition, args)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check a single condition
    fn check_condition(&self, condition: &RuleCondition, args: &[String]) -> Result<bool> {
        match condition {
            RuleCondition::HasFlag(flag) => {
                // Check if flag is in args
                Ok(args.iter().any(|arg| arg == flag || arg.starts_with(flag)))
            }
            RuleCondition::ArgContains(substring) => {
                // Check if any arg contains the substring
                Ok(args.iter().any(|arg| arg.contains(substring)))
            }
            RuleCondition::ArgMatches(pattern) => {
                // Check if any arg matches the regex
                let regex = regex::Regex::new(pattern)
                    .map_err(|e| anyhow::anyhow!("Invalid arg pattern '{}': {}", pattern, e))?;
                Ok(args.iter().any(|arg| regex.is_match(arg)))
            }
            RuleCondition::PathProtected(path) => {
                // Check if any arg references a protected path
                Ok(args.iter().any(|arg| arg.contains(path) || arg == path))
            }
            RuleCondition::AllArgsLiteral => {
                // Check if all args are literal (no shell expansion characters)
                Ok(args.iter().all(|arg| self.is_literal(arg)))
            }
        }
    }

    /// Check if an argument is literal (no shell expansion)
    fn is_literal(&self, arg: &str) -> bool {
        // Check for shell expansion characters
        let dangerous_chars = ['$', '*', '?', '`', '|', ';', '&', '<', '>'];
        !arg.chars().any(|c| dangerous_chars.contains(&c))
    }

    /// Build shield verdict based on rule action
    fn build_verdict(&self, rule: &ClassificationRule, args: &[String]) -> Result<Option<ShieldVerdict>> {
        match &rule.action {
            RuleAction::Allow => Ok(Some(ShieldVerdict::Allow { audit: false })),
            RuleAction::Block { reason } => {
                Ok(Some(ShieldVerdict::Deny(DenialFeedback {
                    reason: reason.clone(),
                    risk_level: RiskLevel::Critical,
                    what_would_be_needed: "Administrator approval required".to_string(),
                    remaining_budget: None,
                    alternative: None,
                })))
            }
            RuleAction::Modify { rewrite } => {
                let rewritten = self.apply_rewrites(args, &rewrite.replacements)?;
                let original = self.build_command_string(args);
                
                Ok(Some(ShieldVerdict::Modify {
                    original,
                    rewritten,
                    reason: rewrite.message.clone(),
                }))
            }
        }
    }

    /// Apply rewrites to arguments
    fn apply_rewrites(&self, args: &[String], replacements: &[Replacement]) -> Result<String> {
        let mut modified_args = args.to_vec();
        
        for replacement in replacements {
            for arg in &mut modified_args {
                *arg = arg.replace(&replacement.match_pattern, &replacement.replace_with);
            }
        }
        
        Ok(self.build_command_string(&modified_args))
    }

    /// Build command string from args
    fn build_command_string(&self, args: &[String]) -> String {
        args.iter()
            .map(|arg| {
                if arg.contains(' ') || arg.contains('"') || arg.contains('\'') {
                    format!("'{}'", arg.escape_default())
                } else {
                    arg.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Add a custom rule to the classifier
    pub fn add_rule(&mut self, rule: ClassificationRule) {
        self.rules.push(rule);
    }

    /// Get all rules
    pub fn rules(&self) -> &[ClassificationRule] {
        &self.rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_readonly() {
        let classifier = ActionClassifier::new();
        
        // Test basic read-only commands
        let result = classifier.classify("cat", &["file.txt".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::ReadOnly);
        
        let result = classifier.classify("ls", &["-la".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::ReadOnly);
        
        let result = classifier.classify("grep", &["pattern".to_string(), "file.txt".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::ReadOnly);
    }

    #[test]
    fn test_classify_destructive() {
        let classifier = ActionClassifier::new();
        
        // Test rm
        let result = classifier.classify("rm", &["file.txt".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Destructive);
        
        // Test docker rm
        let result = classifier.classify("docker", &["rm".to_string(), "container".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Destructive);
        
        // Test systemctl stop
        let result = classifier.classify("systemctl", &["stop".to_string(), "nginx".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Destructive);
    }

    #[test]
    fn test_classify_modify_chmod() {
        let classifier = ActionClassifier::new();
        
        // Test chmod 777 - should be Modify tier with rewrite
        let result = classifier.classify("chmod", &["777".to_string(), "/path/file".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Modify);
        
        // Check that verdict is Modify with rewrite
        if let Some(ShieldVerdict::Modify { original, rewritten, reason }) = result.verdict {
            assert!(original.contains("777"));
            assert!(rewritten.contains("755"));
            assert!(reason.contains("auto-corrected"));
        } else {
            panic!("Expected Modify verdict");
        }
    }

    #[test]
    fn test_classify_blocked_rm_rf_root() {
        let classifier = ActionClassifier::new();
        
        // Test rm -rf / - should be blocked
        let result = classifier.classify("rm", &["-rf".to_string(), "/".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Blocked);
        
        // Check that verdict is Deny
        if let Some(ShieldVerdict::Deny(feedback)) = result.verdict {
            assert!(feedback.reason.contains("Root filesystem"));
        } else {
            panic!("Expected Deny verdict");
        }
    }

    #[test]
    fn test_classify_blocked_no_preserve_root() {
        let classifier = ActionClassifier::new();
        
        // Test rm --no-preserve-root - should be blocked
        let result = classifier.classify("rm", &["--no-preserve-root".to_string(), "-rf".to_string(), "/".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Blocked);
    }

    #[test]
    fn test_classify_network() {
        let classifier = ActionClassifier::new();
        
        // Test network commands
        let result = classifier.classify("curl", &["http://example.com".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Network);
        
        let result = classifier.classify("ssh", &["user@host".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Network);
    }

    #[test]
    fn test_classify_docker_readonly() {
        let classifier = ActionClassifier::new();
        
        // Test docker ps - should be ReadOnly
        let result = classifier.classify("docker", &["ps".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::ReadOnly);
        
        // Test docker logs - should be ReadOnly
        let result = classifier.classify("docker", &["logs".to_string(), "container".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::ReadOnly);
    }

    #[test]
    fn test_classify_systemctl_readonly() {
        let classifier = ActionClassifier::new();
        
        // Test systemctl status - should be ReadOnly
        let result = classifier.classify("systemctl", &["status".to_string(), "nginx".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::ReadOnly);
    }

    #[test]
    fn test_classify_unclassified() {
        let classifier = ActionClassifier::new();
        
        // Test unknown command - should be Unclassified
        let result = classifier.classify("unknowncmd", &["arg1".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Unclassified);
        assert!(result.verdict.is_none());
    }

    #[test]
    fn test_classify_blocked_dd_disk() {
        let classifier = ActionClassifier::new();
        
        // Test dd of=/dev/sda - should be blocked
        let result = classifier.classify("dd", &["if=/dev/zero".to_string(), "of=/dev/sda".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Blocked);
    }

    #[test]
    fn test_custom_rule() {
        let mut classifier = ActionClassifier::new();
        
        // Add custom rule
        classifier.add_rule(ClassificationRule {
            name: "custom-block".to_string(),
            command_pattern: "dangerous-cmd".to_string(),
            conditions: vec![],
            tier: ActionTier::Blocked,
            action: RuleAction::Block {
                reason: "Custom dangerous command".to_string(),
            },
            message: "Custom block rule".to_string(),
        });
        
        let result = classifier.classify("dangerous-cmd", &[]).unwrap();
        assert_eq!(result.tier, ActionTier::Blocked);
    }

    #[test]
    fn test_rule_priority() {
        let classifier = ActionClassifier::new();
        
        // More specific rules should match first
        // rm -rf / should be Blocked, not Destructive
        let result = classifier.classify("rm", &["-rf".to_string(), "/".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Blocked);
    }

    // ═══════════════════════════════════════════
    // Git operations
    // ═══════════════════════════════════════════

    #[test]
    fn test_git_readonly_operations() {
        let classifier = ActionClassifier::new();

        // Git read-only commands should be ReadOnly tier
        let readonly_cmds: Vec<(&str, Vec<String>)> = vec![
            ("status", vec!["status".to_string()]),
            ("log", vec!["log".to_string(), "--oneline".to_string()]),
            ("diff", vec!["diff".to_string(), "HEAD~1".to_string()]),
            ("show", vec!["show".to_string(), "abc123".to_string()]),
            ("branch_list", vec!["branch".to_string()]),
            ("branch_list_verbose", vec!["branch".to_string(), "-v".to_string()]),
            ("tag_list", vec!["tag".to_string()]),
            ("remote_list", vec!["remote".to_string(), "-v".to_string()]),
            ("stash_list", vec!["stash".to_string(), "list".to_string()]),
            ("blame", vec!["blame".to_string(), "file.rs".to_string()]),
            ("shortlog", vec!["shortlog".to_string()]),
        ];

        for (name, args) in readonly_cmds {
            let result = classifier.classify("git", &args).unwrap();
            assert_eq!(result.tier, ActionTier::ReadOnly, "git {} should be ReadOnly", name);
        }
    }

    #[test]
    fn test_git_safe_operations() {
        let classifier = ActionClassifier::new();

        // Git operations that are Modify tier (safe but state-changing)
        let safe_cmds = vec![
            ("add", vec!["add".to_string(), ".".to_string()]),
            ("commit", vec!["commit".to_string(), "-m".to_string(), "fix".to_string()]),
            ("checkout", vec!["checkout".to_string(), "-b".to_string(), "feature".to_string()]),
            ("merge", vec!["merge".to_string(), "feature-x".to_string()]),
            ("rebase", vec!["rebase".to_string(), "main".to_string()]),
            ("pull", vec!["pull".to_string(), "origin".to_string(), "main".to_string()]),
            ("push", vec!["push".to_string(), "origin".to_string(), "main".to_string()]),
            ("clone", vec!["clone".to_string(), "https://github.com/repo.git".to_string()]),
            ("fetch", vec!["fetch".to_string(), "--all".to_string()]),
            ("cherry-pick", vec!["cherry-pick".to_string(), "abc123".to_string()]),
            ("reset_soft", vec!["reset".to_string(), "--soft".to_string(), "HEAD~1".to_string()]),
            ("reset_mixed", vec!["reset".to_string(), "--mixed".to_string(), "HEAD~1".to_string()]),
            ("stash_push", vec!["stash".to_string(), "push".to_string(), "-m".to_string(), "wip".to_string()]),
            ("revert", vec!["revert".to_string(), "abc123".to_string()]),
            ("worktree_add", vec!["worktree".to_string(), "add".to_string(), "../wt".to_string(), "branch".to_string()]),
            ("branch_create", vec!["branch".to_string(), "feature-x".to_string()]),
            ("branch_d_safe", vec!["branch".to_string(), "-d".to_string(), "merged".to_string()]),
            ("submodule_add", vec!["submodule".to_string(), "add".to_string(), "url".to_string()]),
        ];

        for (name, args) in safe_cmds {
            let result = classifier.classify("git", &args).unwrap();
            assert_ne!(result.tier, ActionTier::Blocked, "git {} should NOT be blocked", name);
        }
    }

    #[test]
    fn test_git_destructive_operations() {
        let classifier = ActionClassifier::new();

        // Git destructive commands should be Destructive tier
        let destructive_cmds = vec![
            ("push_force", vec!["push".to_string(), "--force".to_string(), "origin".to_string()]),
            ("push_force_lease", vec!["push".to_string(), "--force-with-lease".to_string(), "origin".to_string()]),
            ("reset_hard", vec!["reset".to_string(), "--hard".to_string(), "HEAD~3".to_string()]),
            ("clean_fd", vec!["clean".to_string(), "-fd".to_string()]),
            ("branch_D", vec!["branch".to_string(), "-D".to_string(), "old-feature".to_string()]),
            ("tag_delete", vec!["tag".to_string(), "-d".to_string(), "v1.0".to_string()]),
            ("stash_drop", vec!["stash".to_string(), "drop".to_string()]),
            ("stash_clear", vec!["stash".to_string(), "clear".to_string()]),
            ("reflog_expire", vec!["reflog".to_string(), "expire".to_string(), "--expire=now".to_string()]),
            ("gc_prune", vec!["gc".to_string(), "--prune=now".to_string()]),
            ("worktree_remove", vec!["worktree".to_string(), "remove".to_string(), "../wt".to_string()]),
            ("submodule_deinit", vec!["submodule".to_string(), "deinit".to_string(), "libs/core".to_string()]),
        ];

        for (name, args) in destructive_cmds {
            let result = classifier.classify("git", &args).unwrap();
            assert_eq!(result.tier, ActionTier::Destructive, "git {} should be Destructive, got {:?}", name, result.tier);
        }
    }

    #[test]
    fn test_git_blocked_operations() {
        let classifier = ActionClassifier::new();

        // Git filter-branch / filter-repo should be Blocked
        let result = classifier.classify("git", &["filter-branch".to_string(), "--tree-filter".to_string(), "...".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Blocked);

        let result = classifier.classify("git", &["filter-repo".to_string(), "--invert-paths".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Blocked);

        // Check denial reason
        if let Some(ShieldVerdict::Deny(feedback)) = result.verdict {
            assert!(feedback.reason.contains("history rewrite"));
        } else {
            panic!("Expected Deny verdict for git filter-repo");
        }
    }

    #[test]
    fn test_git_push_normal_not_destructive() {
        let classifier = ActionClassifier::new();

        // Normal git push should NOT be Destructive
        let result = classifier.classify("git", &["push".to_string(), "origin".to_string(), "main".to_string()]).unwrap();
        assert_ne!(result.tier, ActionTier::Destructive, "normal git push should not be Destructive");
    }

    #[test]
    fn test_git_clean_f_only_not_destructive() {
        let classifier = ActionClassifier::new();

        // git clean -f without -d should not be Destructive
        let result = classifier.classify("git", &["clean".to_string(), "-f".to_string()]).unwrap();
        assert_ne!(result.tier, ActionTier::Destructive);
    }

    #[test]
    fn test_git_priority_blocked_over_destructive() {
        let classifier = ActionClassifier::new();

        // filter-branch should be Blocked (first match), not Destructive
        let result = classifier.classify("git", &["filter-branch".to_string(), "--all".to_string()]).unwrap();
        assert_eq!(result.tier, ActionTier::Blocked);
    }
}

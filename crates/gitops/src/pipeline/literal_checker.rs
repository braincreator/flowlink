//! Literal Checker - Rejects shell expansion patterns in destructive commands
//!
//! This module implements security validation for destructive commands by
//! detecting and rejecting shell expansion patterns like variables, globs,
//! command substitution, and shell operators.

use crate::types::{DenialFeedback, RiskLevel};

/// Commands that can cause irreversible data loss or system changes
const DESTRUCTIVE_COMMANDS: &[&str] = &[
    "rm", "rmdir", "mv", "cp", "chmod", "chown", "truncate", "dd", "shred", "tee", "sed",
];

/// Shell expansion patterns that are dangerous in destructive commands
/// Each pattern is a tuple of (pattern_name, detector_function_description)
/// LiteralChecker validates that destructive commands don't contain
/// shell expansion patterns that could lead to unintended consequences.
#[derive(Debug, Clone)]
pub struct LiteralChecker {
    /// Whether the checker is enabled
    enabled: bool,
}

impl Default for LiteralChecker {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl LiteralChecker {
    /// Create a new LiteralChecker
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a LiteralChecker with explicit enabled state
    pub fn with_enabled(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Check if a command is in the destructive commands list
    fn is_destructive_command(command: &str) -> bool {
        // Extract base command (handle paths like /bin/rm)
        let base_name = command.rsplit('/').next().unwrap_or(command);
        DESTRUCTIVE_COMMANDS.contains(&base_name)
    }

    /// Detect shell variable expansion patterns ($VAR, ${VAR})
    fn has_variable_expansion(arg: &str) -> bool {
        let chars: Vec<char> = arg.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '$' {
                // Check for ${...} pattern
                if i + 1 < chars.len() && chars[i + 1] == '{' {
                    return true;
                }
                // Check for $VAR pattern (letter or underscore followed by alphanumeric/underscore)
                if i + 1 < chars.len() {
                    let next = chars[i + 1];
                    if next.is_ascii_alphabetic() || next == '_' {
                        return true;
                    }
                }
            }
            i += 1;
        }
        false
    }

    /// Detect glob patterns (* and ?)
    /// Note: ? at the start with - (like -?) is allowed as it's likely a flag
    fn has_glob_pattern(arg: &str) -> bool {
        let chars: Vec<char> = arg.chars().collect();

        for (i, &c) in chars.iter().enumerate() {
            if c == '*' {
                // Glob star is always suspicious in destructive commands
                return true;
            }
            if c == '?' {
                // ? at the start with - is likely a flag (-?), allow it
                // ? elsewhere is suspicious
                if i > 0 || !arg.starts_with('-') {
                    return true;
                }
            }
        }
        false
    }

    /// Detect backtick command substitution
    fn has_backtick_substitution(arg: &str) -> bool {
        arg.contains('`')
    }

    /// Detect $(...) command substitution
    fn has_command_substitution(arg: &str) -> bool {
        let chars: Vec<char> = arg.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '(' {
                return true;
            }
            i += 1;
        }
        false
    }

    /// Detect shell operators (&&, ||, ;, |)
    fn has_shell_operators(arg: &str) -> bool {
        // Check for &&, ||, ;, |
        // Note: We check for these as standalone or as part of the argument
        let chars: Vec<char> = arg.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            match chars[i] {
                '&' => {
                    if i + 1 < chars.len() && chars[i + 1] == '&' {
                        return true;
                    }
                }
                '|' => {
                    // Check for || or single |
                    if i + 1 < chars.len() && chars[i + 1] == '|' {
                        return true;
                    }
                    // Single | is also a pipe operator
                    return true;
                }
                ';' => {
                    return true;
                }
                _ => {}
            }
            i += 1;
        }
        false
    }

    /// Analyze an argument for shell expansion patterns
    fn analyze_arg(arg: &str) -> Vec<(&'static str, &'static str)> {
        let mut patterns_found = Vec::new();

        if Self::has_variable_expansion(arg) {
            patterns_found.push(("variable", "Shell variable expansion ($VAR or ${VAR})"));
        }
        if Self::has_glob_pattern(arg) {
            patterns_found.push(("glob", "Glob pattern (* or ?)"));
        }
        if Self::has_backtick_substitution(arg) {
            patterns_found.push(("backtick", "Command substitution with backticks"));
        }
        if Self::has_command_substitution(arg) {
            patterns_found.push(("command_sub", "Command substitution $(...)"));
        }
        if Self::has_shell_operators(arg) {
            patterns_found.push(("shell_operator", "Shell operator (&&, ||, ;, |)"));
        }

        patterns_found
    }

    /// Check a command and its arguments for shell expansion patterns.
    ///
    /// Returns `None` if the command is safe (no expansion detected or not destructive).
    /// Returns `Some(DenialFeedback)` if unsafe patterns are found in destructive commands.
    pub fn check(&self, command: &str, args: &[String]) -> Option<DenialFeedback> {
        if !self.enabled {
            return None;
        }

        // Only check destructive commands
        if !Self::is_destructive_command(command) {
            tracing::debug!(
                command = %command,
                "Command is not destructive, passing through"
            );
            return None;
        }

        // Analyze each argument for shell expansion patterns
        let mut unsafe_args: Vec<(String, Vec<(&str, &str)>)> = Vec::new();

        for arg in args {
            let patterns = Self::analyze_arg(arg);
            if !patterns.is_empty() {
                unsafe_args.push((arg.clone(), patterns));
            }
        }

        if unsafe_args.is_empty() {
            tracing::debug!(
                command = %command,
                args = ?args,
                "Destructive command with safe literal arguments"
            );
            return None;
        }

        // Build denial feedback
        let mut reason_parts = Vec::new();
        let mut details_parts = Vec::new();

        reason_parts.push(format!(
            "Shell expansion detected in destructive command '{}'",
            command
        ));

        for (arg, patterns) in &unsafe_args {
            let pattern_names: Vec<&str> = patterns.iter().map(|(name, _)| *name).collect();
            details_parts.push(format!("Argument '{}': {}", arg, pattern_names.join(", ")));
        }

        let reason = format!("{}. {}", reason_parts.join(""), details_parts.join("; "));

        let what_would_be_needed = format!(
            "Use literal paths instead of shell variables/globs. \
             First resolve the paths (e.g., with 'ls {}'), then use the literal values.",
            unsafe_args
                .iter()
                .map(|(arg, _)| arg.as_str())
                .take(1)
                .collect::<Vec<_>>()
                .join(" ")
        );

        let alternative = Some(format!(
            "Resolve paths first: ls {} → then use literal file names in the command",
            unsafe_args
                .iter()
                .map(|(arg, _)| arg.as_str())
                .take(1)
                .collect::<Vec<_>>()
                .join(" ")
        ));

        tracing::warn!(
            command = %command,
            args = ?args,
            unsafe_args = ?unsafe_args,
            "Rejected destructive command with shell expansion"
        );

        Some(DenialFeedback {
            reason,
            risk_level: RiskLevel::High,
            what_would_be_needed,
            remaining_budget: None,
            alternative,
        })
    }

    /// Check if the LiteralChecker is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable the checker
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_rejects_var() {
        let checker = LiteralChecker::new();

        // Test $VAR pattern
        let result = checker.check("rm", &["$HOME/file.txt".to_string()]);
        assert!(result.is_some(), "Should reject $VAR in rm command");
        let feedback = result.unwrap();
        assert!(feedback.reason.contains("Shell expansion"));
        assert!(feedback.alternative.is_some());

        // Test ${VAR} pattern
        let result = checker.check("rm", &["${HOME}/file.txt".to_string()]);
        assert!(result.is_some(), "Should reject ${{HOME}} in rm command");

        // Test with mv command
        let result = checker.check("mv", &["$FILE".to_string(), "/dest/".to_string()]);
        assert!(result.is_some(), "Should reject $VAR in mv command");
    }

    #[test]
    fn test_literal_rejects_glob() {
        let checker = LiteralChecker::new();

        let result = checker.check("rm", &["*.log".to_string()]);
        assert!(result.is_some());

        let result = checker.check("rm", &["file?.txt".to_string()]);
        assert!(result.is_some());

        let result = checker.check("rm", &["/var/log/*.log".to_string()]);
        assert!(result.is_some());
    }

    #[test]
    fn test_literal_rejects_command_substitution() {
        let checker = LiteralChecker::new();

        let result = checker.check("rm", &["`find /tmp -name '*.log'`".to_string()]);
        assert!(result.is_some());

        let result = checker.check("rm", &["$(cat /tmp/files.txt)".to_string()]);
        assert!(result.is_some());
    }

    #[test]
    fn test_literal_rejects_shell_operators() {
        let checker = LiteralChecker::new();

        let result = checker.check("rm", &["file.txt".to_string(), "&&".to_string()]);
        assert!(result.is_some());

        let result = checker.check("rm", &["file.txt".to_string(), "||".to_string()]);
        assert!(result.is_some());

        let result = checker.check("rm", &["file.txt".to_string(), ";".to_string()]);
        assert!(result.is_some());

        let result = checker.check("rm", &["file.txt".to_string(), "|".to_string()]);
        assert!(result.is_some());
    }

    #[test]
    fn test_literal_allows_safe_args() {
        let checker = LiteralChecker::new();

        let result = checker.check("rm", &["/tmp/specific_file.txt".to_string()]);
        assert!(result.is_none());

        let result = checker.check("rm", &["-rf".to_string(), "/tmp/file.txt".to_string()]);
        assert!(result.is_none());

        let result = checker.check("mv", &["/tmp/a.txt".to_string(), "/tmp/b.txt".to_string()]);
        assert!(result.is_none());

        let result = checker.check(
            "cp",
            &["/etc/config".to_string(), "/etc/config.bak".to_string()],
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_literal_passes_readonly() {
        let checker = LiteralChecker::new();

        let result = checker.check("cat", &["$FILE".to_string()]);
        assert!(result.is_none());

        let result = checker.check("ls", &["*.log".to_string()]);
        assert!(result.is_none());

        let result = checker.check("grep", &["pattern".to_string(), "$FILE".to_string()]);
        assert!(result.is_none());
    }

    #[test]
    fn test_literal_allows_flag_with_question() {
        let checker = LiteralChecker::new();

        // -? contains glob pattern, so it IS rejected
        let result = checker.check("rm", &["-?".to_string()]);
        assert!(result.is_some(), "-? should be rejected as glob pattern");

        let result = checker.check("rm", &["--help".to_string()]);
        assert!(result.is_none());
    }

    #[test]
    fn test_is_destructive_command() {
        assert!(LiteralChecker::is_destructive_command("rm"));
        assert!(LiteralChecker::is_destructive_command("mv"));
        assert!(LiteralChecker::is_destructive_command("chmod"));
        assert!(LiteralChecker::is_destructive_command("/bin/rm"));
        assert!(LiteralChecker::is_destructive_command("/usr/bin/dd"));

        assert!(!LiteralChecker::is_destructive_command("cat"));
        assert!(!LiteralChecker::is_destructive_command("ls"));
        assert!(!LiteralChecker::is_destructive_command("echo"));
    }

    #[test]
    fn test_checker_disabled() {
        let checker = LiteralChecker::with_enabled(false);

        let result = checker.check("rm", &["$FILE".to_string()]);
        assert!(result.is_none());

        let result = checker.check("rm", &["*.log".to_string()]);
        assert!(result.is_none());
    }

    #[test]
    fn test_all_destructive_commands() {
        let checker = LiteralChecker::new();

        for cmd in DESTRUCTIVE_COMMANDS {
            let result = checker.check(cmd, &["$VAR".to_string()]);
            assert!(result.is_some());
        }
    }

    #[test]
    fn test_complex_patterns() {
        let checker = LiteralChecker::new();

        let result = checker.check("rm", &["$DIR/*.log".to_string()]);
        assert!(result.is_some());

        let result = checker.check("rm", &["$(echo ${FILE})".to_string()]);
        assert!(result.is_some());
    }

    #[test]
    fn test_feedback_content() {
        let checker = LiteralChecker::new();
        let result = checker
            .check("rm", &["$HOME/file.txt".to_string()])
            .unwrap();

        assert!(result.reason.contains("rm"));
        assert!(result.reason.contains("Shell expansion"));
        assert!(result.what_would_be_needed.contains("literal"));
        assert!(result.alternative.is_some());
        assert!(result.alternative.unwrap().contains("ls"));
        assert_eq!(result.risk_level, RiskLevel::High);
    }
}

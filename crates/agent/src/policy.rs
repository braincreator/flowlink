// Policy Layer — unified command checks before execution
// Port of internal/agent/policy.go
// Chain: KillSwitch → ReadOnly → Blacklist → Sandbox → Approval → Execute

use flowlink_core::*;
use flowlink_shield::{AnalysisEngine, Command as ShieldCommand};

#[derive(Debug, Clone)]
pub struct PolicyResult {
    pub allowed: bool,
    pub blocked: bool,
    pub reason: String,
    pub risk_level: RiskLevel,
    pub require_approval: bool,
    pub snapshot_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
}

pub struct PolicyEngine {
    shield: AnalysisEngine,
    read_only: bool,
    allow_sudo: bool,
    blocked_patterns: Vec<String>,
    allowed_dirs: Vec<String>,
    /// Dynamic allow rules added at runtime (regex patterns). Always allowed.
    runtime_allow: Vec<String>,
    /// Dynamic deny rules added at runtime (regex patterns). Always blocked.
    runtime_deny: Vec<String>,
    /// Path to cache file for persistence across restarts.
    cache_path: Option<String>,
}

impl PolicyEngine {
    pub fn new(read_only: bool, allow_sudo: bool) -> Self {
        let mut engine = Self {
            shield: AnalysisEngine {
                enable_ast: true,
                enable_interpreter: true,
            },
            read_only,
            allow_sudo,
            blocked_patterns: default_blocked_patterns(),
            allowed_dirs: vec![],
            runtime_allow: vec![],
            runtime_deny: vec![],
            cache_path: None,
        };
        // Auto-detect cache path from agent config dir
        if let Ok(config_dir) = std::env::var("FLOWLINK_AGENT_DIR") {
            engine.cache_path = Some(format!("{}/policy_cache.json", config_dir.trim_end_matches('/')));
            engine.load_cache_from_file();
        }
        engine
    }

    pub fn with_allowed_dirs(mut self, dirs: Vec<String>) -> Self {
        self.allowed_dirs = dirs;
        self
    }

    pub fn with_blocked_patterns(mut self, patterns: Vec<String>) -> Self {
        self.blocked_patterns = patterns;
        self
    }

    /// Update read_only flag at runtime (from ConfigUpdate).
    pub fn set_read_only(&mut self, value: bool) {
        self.read_only = value;
    }

    /// Check if read_only mode is active.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Add a runtime allow rule (regex or glob). Commands matching this are always allowed.
    pub fn add_allow_rule(&mut self, pattern: String) {
        self.runtime_allow.push(pattern);
        self.save_cache_to_file();
    }

    /// Add a runtime deny rule (regex or glob). Commands matching this are always blocked.
    pub fn add_deny_rule(&mut self, pattern: String) {
        self.runtime_deny.push(pattern);
        self.save_cache_to_file();
    }

    /// Remove a runtime rule (allow or deny) by exact pattern match.
    pub fn remove_rule(&mut self, pattern: &str) -> bool {
        if let Some(pos) = self.runtime_allow.iter().position(|r| r == pattern) {
            self.runtime_allow.remove(pos);
            self.save_cache_to_file();
            return true;
        }
        if let Some(pos) = self.runtime_deny.iter().position(|r| r == pattern) {
            self.runtime_deny.remove(pos);
            self.save_cache_to_file();
            return true;
        }
        false
    }

    /// Get current runtime rules.
    pub fn runtime_rules(&self) -> (Vec<String>, Vec<String>) {
        (self.runtime_allow.clone(), self.runtime_deny.clone())
    }

    /// Replace all runtime rules atomically (used for DB policy sync).
    pub fn replace_runtime_rules(&mut self, allows: Vec<String>, denies: Vec<String>) {
        self.runtime_allow = allows;
        self.runtime_deny = denies;
        self.save_cache_to_file();
    }

    /// Set cache file path explicitly.
    pub fn set_cache_path(&mut self, path: String) {
        self.cache_path = Some(path);
    }

    /// Save current runtime rules to cache file.
    fn save_cache_to_file(&self) {
        if let Some(ref path) = self.cache_path {
            let data = serde_json::json!({
                "allows": self.runtime_allow,
                "denies": self.runtime_deny,
                "cached_at": chrono::Utc::now().to_rfc3339(),
            });
            if let Ok(json) = serde_json::to_string_pretty(&data) {
                if std::fs::write(path, json).is_err() {
                    log::warn!("Failed to save policy cache to {}", path);
                } else {
                    log::info!("Policy cache saved: {} allows, {} denies", self.runtime_allow.len(), self.runtime_deny.len());
                }
            }
        }
    }

    /// Load runtime rules from cache file (fallback when relay is unreachable).
    fn load_cache_from_file(&mut self) {
        if let Some(ref path) = self.cache_path {
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
                    let allows: Vec<String> = parsed.get("allows")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    let denies: Vec<String> = parsed.get("denies")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    if !allows.is_empty() || !denies.is_empty() {
                        self.runtime_allow = allows;
                        self.runtime_deny = denies;
                        log::info!("Policy cache loaded: {} allows, {} denies", self.runtime_allow.len(), self.runtime_deny.len());
                    }
                }
            }
        }
    }

    /// Match a command against a simple glob pattern (* = wildcard).
    fn match_glob(pattern: &str, text: &str) -> bool {
        if pattern == text {
            return true;
        }
        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                return text.starts_with(prefix) && text.ends_with(suffix);
            }
        }
        false
    }

    /// Check command against full policy chain.
    pub fn check(&self, payload: &ExecRequestPayload) -> PolicyResult {
        let command = &payload.command;

        // 0. Runtime deny rules (highest priority — always blocked)
        for pattern in &self.runtime_deny {
            if Self::match_glob(pattern, command) {
                return PolicyResult {
                    allowed: false,
                    blocked: true,
                    reason: format!("POLICY_DENY: matched runtime deny rule '{}'", pattern),
                    risk_level: RiskLevel::High,
                    require_approval: false,
                    snapshot_id: None,
                };
            }
        }

        // 0b. Runtime allow rules (bypass shield + policy, always allowed)
        for pattern in &self.runtime_allow {
            if Self::match_glob(pattern, command) {
                return PolicyResult {
                    allowed: true,
                    blocked: false,
                    reason: format!("POLICY_ALLOW: matched runtime allow rule '{}'", pattern),
                    risk_level: RiskLevel::None,
                    require_approval: false,
                    snapshot_id: None,
                };
            }
        }

        // 1. Shield (L1+L2+L3 threat detection)
        let parts: Vec<&str> = command.split_whitespace().collect();
        let (binary, args) = match parts.split_first() {
            Some((b, a)) => (b.to_string(), a.iter().map(|s| s.to_string()).collect()),
            None => {
                return PolicyResult {
                    allowed: true,
                    blocked: false,
                    reason: String::new(),
                    risk_level: RiskLevel::None,
                    require_approval: false,
                    snapshot_id: None,
                }
            }
        };
        let shield_cmd = ShieldCommand {
            binary,
            args,
            raw: command.clone(),
        };
        let analysis = self.shield.analyze(&shield_cmd);
        if let Some(threat) = analysis.threat {
            return PolicyResult {
                allowed: false,
                blocked: true,
                reason: format!("SHIELD: {} — {}", threat.name, threat.description),
                risk_level: RiskLevel::High,
                require_approval: false,
                snapshot_id: None,
            };
        }

        // 2. Read-only mode
        if self.read_only && is_write_command(command) {
            return PolicyResult {
                allowed: false,
                blocked: true,
                reason: "EXEC_BLOCKED_READONLY: read-only mode active".into(),
                risk_level: RiskLevel::Medium,
                require_approval: false,
                snapshot_id: None,
            };
        }

        // 3. Blacklist patterns
        for pattern in &self.blocked_patterns {
            if command.contains(pattern) {
                return PolicyResult {
                    allowed: false,
                    blocked: true,
                    reason: format!("EXEC_BLOCKED: matched blocked pattern '{}'", pattern),
                    risk_level: RiskLevel::High,
                    require_approval: false,
                    snapshot_id: None,
                };
            }
        }

        // 4. Sudo check
        if !self.allow_sudo && (command.starts_with("sudo ") || command.contains(" sudo ")) {
            return PolicyResult {
                allowed: false,
                blocked: true,
                reason: "EXEC_BLOCKED_SUDO: sudo not allowed".into(),
                risk_level: RiskLevel::Medium,
                require_approval: false,
                snapshot_id: None,
            };
        }

        // 5. Sandbox path check
        if !self.allowed_dirs.is_empty() {
            if let Some(dir) = extract_target_dir(command) {
                let allowed = self.allowed_dirs.iter().any(|d| dir.starts_with(d));
                if !allowed {
                    return PolicyResult {
                        allowed: false,
                        blocked: true,
                        reason: format!(
                            "EXEC_BLOCKED_SANDBOX: path '{}' outside allowed dirs",
                            dir
                        ),
                        risk_level: RiskLevel::Medium,
                        require_approval: false,
                        snapshot_id: None,
                    };
                }
            }
        }

        PolicyResult {
            allowed: true,
            blocked: false,
            reason: String::new(),
            risk_level: RiskLevel::None,
            require_approval: false,
            snapshot_id: None,
        }
    }
}

fn is_write_command(cmd: &str) -> bool {
    let write_prefixes = [
        "rm ",
        "mv ",
        "cp ",
        "mkdir ",
        "rmdir ",
        "chmod ",
        "chown ",
        "dd ",
        "mkfs.",
        "shred ",
        "truncate ",
        "tee ",
        "docker rm",
        "docker rmi",
        "docker system prune",
    ];
    let lower = cmd.to_lowercase();
    write_prefixes.iter().any(|p| lower.starts_with(p))
        || cmd.contains(" > ")
        || cmd.contains(" >> ")
}

fn extract_target_dir(cmd: &str) -> Option<String> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    for part in &parts {
        if part.starts_with('/') || part.starts_with("~/") || part.starts_with("./") {
            return Some(part.to_string());
        }
    }
    None
}

fn default_blocked_patterns() -> Vec<String> {
    vec![
        "rm -rf /".into(),
        "rm -rf /*".into(),
        "mkfs.".into(),
        "dd if=/dev/zero".into(),
        ":(){ :|:& };:".into(),
        "> /dev/sda".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowlink_core::*;
    use std::collections::HashMap;

    fn test_payload(cmd: &str) -> ExecRequestPayload {
        ExecRequestPayload {
            command: cmd.into(),
            shell: None,
            env: None,
            dir: None,
            timeout_sec: 10,
            request_id: "test".into(),
        }
    }

    #[test]
    fn test_rm_rf_blocked() {
        let engine = PolicyEngine::new(false, false);
        let result = engine.check(&test_payload("rm -rf /"));
        assert!(result.blocked);
        assert!(result.reason.contains("rm -rf /") || result.reason.contains("SHIELD"));
    }

    #[test]
    fn test_mkfs_blocked() {
        let engine = PolicyEngine::new(false, false);
        let result = engine.check(&test_payload("mkfs.ext4 /dev/sda1"));
        assert!(result.blocked);
        // Blocked by either shield or blacklist
        assert!(result.reason.contains("mkfs") || result.reason.contains("SHIELD"));
    }

    #[test]
    fn test_safe_commands_pass() {
        let engine = PolicyEngine::new(false, false);
        for cmd in ["ls -la", "echo hello", "cat /tmp/file.txt", "pwd", "whoami"] {
            let result = engine.check(&test_payload(cmd));
            assert!(result.allowed, "expected '{}' to be allowed", cmd);
        }
    }

    #[test]
    fn test_read_only_mode() {
        let engine = PolicyEngine::new(true, false);
        let result = engine.check(&test_payload("rm -rf /tmp/test"));
        assert!(result.blocked);
        assert!(result.reason.contains("read-only"));
        // Read commands should still pass
        let r2 = engine.check(&test_payload("cat /tmp/file"));
        assert!(r2.allowed);
    }

    #[test]
    fn test_sudo_blocked() {
        let engine = PolicyEngine::new(false, false);
        let result = engine.check(&test_payload("sudo ls"));
        assert!(result.blocked);
        assert!(result.reason.contains("sudo"));
    }

    #[test]
    fn test_sudo_allowed() {
        let engine = PolicyEngine::new(false, true);
        let result = engine.check(&test_payload("sudo ls"));
        assert!(result.allowed);
    }

    #[test]
    fn test_sandbox_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = PolicyEngine::new(false, false).with_allowed_dirs(vec![tmp
            .path()
            .to_str()
            .unwrap()
            .into()]);
        let result = engine.check(&test_payload(&format!("cat {}/file", tmp.path().display())));
        assert!(result.allowed);
        let result2 = engine.check(&test_payload("cat /etc/passwd"));
        assert!(result2.blocked);
        assert!(result2.reason.contains("SANDBOX"));
    }

    #[test]
    fn test_custom_blacklist() {
        let engine = PolicyEngine::new(false, false).with_blocked_patterns(vec!["nmap".into()]);
        let result = engine.check(&test_payload("nmap -sP 192.168.1.0/24"));
        assert!(result.blocked);
    }

    #[test]
    fn test_empty_command_allowed() {
        let engine = PolicyEngine::new(false, false);
        let result = engine.check(&test_payload(""));
        assert!(result.allowed);
    }

    #[test]
    fn test_runtime_allow_bypasses_shield() {
        let mut engine = PolicyEngine::new(false, false);
        // rm -rf / is blocked by shield and blacklist
        let r = engine.check(&test_payload("rm -rf /"));
        assert!(r.blocked);
        // Add allow rule
        engine.add_allow_rule("rm -rf /".into());
        let r2 = engine.check(&test_payload("rm -rf /"));
        assert!(r2.allowed);
        assert!(r2.reason.contains("POLICY_ALLOW"));
    }

    #[test]
    fn test_runtime_allow_glob() {
        let mut engine = PolicyEngine::new(false, false);
        engine.add_allow_rule("docker *".into());
        let r = engine.check(&test_payload("docker rm -f container"));
        assert!(r.allowed);
        assert!(r.reason.contains("POLICY_ALLOW"));
    }

    #[test]
    fn test_runtime_deny_blocks() {
        let mut engine = PolicyEngine::new(false, false);
        // ls is normally allowed
        let r = engine.check(&test_payload("ls"));
        assert!(r.allowed);
        // Add deny rule
        engine.add_deny_rule("ls".into());
        let r2 = engine.check(&test_payload("ls"));
        assert!(r2.blocked);
        assert!(r2.reason.contains("POLICY_DENY"));
    }

    #[test]
    fn test_runtime_deny_has_priority_over_allow() {
        let mut engine = PolicyEngine::new(false, false);
        engine.add_allow_rule("docker *".into());
        engine.add_deny_rule("docker rm *".into());
        // docker ps → allowed
        let r = engine.check(&test_payload("docker ps"));
        assert!(r.allowed);
        // docker rm → denied (deny has priority)
        let r2 = engine.check(&test_payload("docker rm -f container"));
        assert!(r2.blocked);
        assert!(r2.reason.contains("POLICY_DENY"));
    }

    #[test]
    fn test_runtime_remove_rule() {
        let mut engine = PolicyEngine::new(false, false);
        engine.add_deny_rule("nmap".into());
        assert!(engine.check(&test_payload("nmap")).blocked);
        assert!(engine.remove_rule("nmap"));
        assert!(engine.check(&test_payload("nmap")).allowed);
        // Removing non-existent returns false
        assert!(!engine.remove_rule("nonexistent"));
    }

    #[test]
    fn test_runtime_rules_list() {
        let mut engine = PolicyEngine::new(false, false);
        engine.add_allow_rule("docker *".into());
        engine.add_deny_rule("rm *".into());
        let (allow, deny) = engine.runtime_rules();
        assert_eq!(allow, vec!["docker *"]);
        assert_eq!(deny, vec!["rm *"]);
    }
}

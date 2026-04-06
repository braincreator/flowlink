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
}

impl PolicyEngine {
    pub fn new(read_only: bool, allow_sudo: bool) -> Self {
        Self {
            shield: AnalysisEngine { enable_ast: true, enable_interpreter: true },
            read_only,
            allow_sudo,
            blocked_patterns: default_blocked_patterns(),
            allowed_dirs: vec![],
        }
    }

    pub fn with_allowed_dirs(mut self, dirs: Vec<String>) -> Self {
        self.allowed_dirs = dirs;
        self
    }

    pub fn with_blocked_patterns(mut self, patterns: Vec<String>) -> Self {
        self.blocked_patterns = patterns;
        self
    }

    /// Check command against full policy chain.
    pub fn check(&self, payload: &ExecRequestPayload) -> PolicyResult {
        let command = &payload.command;

        // 1. Shield (L1+L2+L3 threat detection)
        let parts: Vec<&str> = command.split_whitespace().collect();
        let (binary, args) = match parts.split_first() {
            Some((b, a)) => (b.to_string(), a.iter().map(|s| s.to_string()).collect()),
            None => return PolicyResult {
                allowed: true, blocked: false, reason: String::new(),
                risk_level: RiskLevel::None, require_approval: false, snapshot_id: None,
            },
        };
        let shield_cmd = ShieldCommand { binary, args, raw: command.clone() };
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
                        reason: format!("EXEC_BLOCKED_SANDBOX: path '{}' outside allowed dirs", dir),
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
        "rm ", "mv ", "cp ", "mkdir ", "rmdir ", "chmod ", "chown ",
        "dd ", "mkfs.", "shred ", "truncate ", "tee ",
        "docker rm", "docker rmi", "docker system prune",
    ];
    let lower = cmd.to_lowercase();
    write_prefixes.iter().any(|p| lower.starts_with(p))
        || cmd.contains(" > ") || cmd.contains(" >> ")
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
        assert!(result.reason.contains("rm -rf /"));
    }

    #[test]
    fn test_mkfs_blocked() {
        let engine = PolicyEngine::new(false, false);
        let result = engine.check(&test_payload("mkfs.ext4 /dev/sda1"));
        assert!(result.blocked);
        assert!(result.reason.contains("mkfs."));
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
        let result = engine.check(&test_payload("sudo rm -rf /"));
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
        let engine = PolicyEngine::new(false, false)
            .with_allowed_dirs(vec![tmp.path().to_str().unwrap().into()]);
        let result = engine.check(&test_payload(&format!("cat {}/file", tmp.path().display())));
        assert!(result.allowed);
        let result2 = engine.check(&test_payload("cat /etc/passwd"));
        assert!(result2.blocked);
        assert!(result2.reason.contains("SANDBOX"));
    }

    #[test]
    fn test_custom_blacklist() {
        let engine = PolicyEngine::new(false, false)
            .with_blocked_patterns(vec!["nmap".into()]);
        let result = engine.check(&test_payload("nmap -sP 192.168.1.0/24"));
        assert!(result.blocked);
    }

    #[test]
    fn test_empty_command_allowed() {
        let engine = PolicyEngine::new(false, false);
        let result = engine.check(&test_payload(""));
        assert!(result.allowed);
    }
}

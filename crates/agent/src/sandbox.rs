// Sandbox — execution isolation for commands
// Port of internal/agent/sandbox.go

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

/// Isolation level for sandboxed execution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IsolationLevel {
    None,
    Chroot,
    Container,
}

/// Sandbox configuration for validating commands and paths.
pub struct Sandbox {
    pub allowed_dirs: Vec<String>,
    pub blocked_patterns: Vec<String>,
    pub max_file_size: u64,
    pub max_exec_timeout: u32,
    pub allow_sudo: bool,
    pub isolation_level: IsolationLevel,
}

/// Environment prepared for sandboxed execution (placeholder for chroot/container).
pub struct SandboxEnv {
    /// Temporary directory or mount point created for the sandbox.
    pub temp_dir: Option<PathBuf>,
    /// Whether the environment was actually isolated.
    pub isolated: bool,
}

impl Sandbox {
    pub fn new(
        allowed_dirs: Vec<String>,
        blocked_patterns: Vec<String>,
        max_file_size: u64,
        max_exec_timeout: u32,
        allow_sudo: bool,
    ) -> Self {
        Self {
            allowed_dirs,
            blocked_patterns,
            max_file_size,
            max_exec_timeout,
            allow_sudo,
            isolation_level: IsolationLevel::None,
        }
    }

    /// Validate a file path: must be within allowed_dirs, resolve symlinks, reject traversal.
    pub fn validate_path(&self, path: &str) -> Result<PathBuf> {
        if path.is_empty() {
            bail!("path is empty");
        }

        let resolved = if path.starts_with('/') {
            // Canonicalize to resolve symlinks and traversal
            let p = Path::new(path);
            match p.canonicalize() {
                Ok(r) => r,
                Err(_) => {
                    // Path may not exist yet; clean it manually
                    let mut components = Vec::new();
                    for part in p.components() {
                        match part {
                            std::path::Component::ParentDir => { components.pop(); }
                            std::path::Component::CurDir => {}
                            std::path::Component::Normal(s) => { components.push(s); }
                            std::path::Component::RootDir => { components.clear(); }
                            std::path::Component::Prefix(p) => { components.clear(); components.push(p.as_os_str()); }
                        }
                    }
                    let mut cleaned = PathBuf::from("/");
                    for c in &components {
                        cleaned.push(c);
                    }
                    cleaned
                }
            }
        } else {
            bail!("only absolute paths are allowed");
        };

        // Check against allowed_dirs
        if !self.allowed_dirs.is_empty() {
            let allowed = self.allowed_dirs.iter().any(|dir| {
                let dir_path = Path::new(dir);
                resolved.starts_with(dir_path) || resolved == dir_path
            });
            if !allowed {
                bail!("path '{}' is outside allowed directories", resolved.display());
            }
        }

        Ok(resolved)
    }

    /// Validate a command: check sudo and blocked patterns.
    pub fn validate_command(&self, cmd: &str) -> Result<()> {
        if cmd.is_empty() {
            bail!("command is empty");
        }

        let trimmed = cmd.trim();

        // Check sudo
        if !self.allow_sudo && contains_sudo(trimmed) {
            bail!("sudo is not allowed");
        }

        // Check blocked patterns
        for pattern in &self.blocked_patterns {
            if match_glob(trimmed, pattern) {
                bail!("command blocked by pattern: {pattern}");
            }
        }

        Ok(())
    }

    /// Check if a file size is within limits.
    pub fn check_file_size(&self, size: u64) -> bool {
        self.max_file_size == 0 || size <= self.max_file_size
    }

    /// Clamp a timeout to the configured maximum.
    pub fn check_timeout(&self, requested: u32) -> u32 {
        if requested == 0 {
            return self.max_exec_timeout;
        }
        if self.max_exec_timeout > 0 && requested > self.max_exec_timeout {
            return self.max_exec_timeout;
        }
        requested
    }

    /// Prepare a sandboxed environment.
    pub fn prepare_env(&self) -> Result<SandboxEnv> {
        match self.isolation_level {
            IsolationLevel::None => Ok(SandboxEnv { temp_dir: None, isolated: false }),
            IsolationLevel::Chroot => {
                bail!("chroot isolation is not yet implemented");
            }
            IsolationLevel::Container => {
                bail!("container isolation is not yet implemented");
            }
        }
    }

    /// Cleanup after sandboxed execution.
    pub fn cleanup(&self, env: &SandboxEnv) {
        if let Some(ref dir) = env.temp_dir {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Check if a command starts with sudo.
fn contains_sudo(cmd: &str) -> bool {
    cmd == "sudo"
        || cmd.starts_with("sudo ")
        || cmd.starts_with("sudo\t")
}

/// Simple glob matching: supports `*` at start, end, or middle.
fn match_glob(cmd: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }

    if let Some(idx) = pattern.find('*') {
        if idx == 0 {
            // Prefix wildcard: *suffix
            return cmd.ends_with(&pattern[1..]);
        }
        if idx == pattern.len() - 1 {
            // Suffix wildcard: prefix*
            return cmd.starts_with(&pattern[..idx]);
        }
        // Middle wildcard: prefix*suffix
        return cmd.starts_with(&pattern[..idx]) && cmd.ends_with(&pattern[idx + 1..]);
    }

    cmd == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basic_sandbox() -> Sandbox {
        Sandbox::new(vec!["/home/user".into(), "/tmp".into()], vec![], 100, 60, false)
    }

    fn restricted_sandbox() -> Sandbox {
        Sandbox::new(
            vec!["/home/user".into()],
            vec!["rm -rf *".into(), "mkfs*".into()],
            100, 60, false,
        )
    }

    #[test]
    fn test_validate_path_allowed() {
        let sb = basic_sandbox();
        // Use existing paths to avoid canonicalize issues
        assert!(sb.validate_path("/tmp").is_ok());
    }

    #[test]
    fn test_validate_path_outside_allowed() {
        let sb = Sandbox::new(vec!["/home/user".into()], vec![], 0, 0, false);
        assert!(sb.validate_path("/etc/passwd").is_err());
    }

    #[test]
    fn test_validate_path_empty() {
        let sb = basic_sandbox();
        assert!(sb.validate_path("").is_err());
    }

    #[test]
    fn test_validate_path_relative() {
        let sb = basic_sandbox();
        assert!(sb.validate_path("relative/path").is_err());
    }

    #[test]
    fn test_validate_path_traversal() {
        let sb = Sandbox::new(vec!["/tmp".into()], vec![], 0, 0, false);
        // /tmp/../etc should resolve outside /tmp
        assert!(sb.validate_path("/tmp/../etc/passwd").is_err());
    }

    #[test]
    fn test_validate_command_safe() {
        let sb = basic_sandbox();
        assert!(sb.validate_command("ls -la").is_ok());
        assert!(sb.validate_command("echo hello").is_ok());
        assert!(sb.validate_command("git status").is_ok());
    }

    #[test]
    fn test_validate_command_empty() {
        let sb = basic_sandbox();
        assert!(sb.validate_command("").is_err());
    }

    #[test]
    fn test_validate_command_sudo_blocked() {
        let sb = basic_sandbox();
        assert!(sb.validate_command("sudo rm -rf /").is_err());
        assert!(sb.validate_command("sudo ls").is_err());
        assert!(sb.validate_command("sudo").is_err());
    }

    #[test]
    fn test_validate_command_sudo_allowed() {
        let mut sb = basic_sandbox();
        sb.allow_sudo = true;
        assert!(sb.validate_command("sudo ls -la").is_ok());
    }

    #[test]
    fn test_validate_command_blocked_patterns() {
        let sb = restricted_sandbox();
        assert!(sb.validate_command("rm -rf /home").is_err());
        assert!(sb.validate_command("mkfs.ext4 /dev/sda1").is_err());
        assert!(sb.validate_command("rm /tmp/file.txt").is_ok());
        assert!(sb.validate_command("ls -la").is_ok());
    }

    #[test]
    fn test_check_file_size() {
        let sb = Sandbox::new(vec![], vec![], 100, 0, false);
        assert!(sb.check_file_size(50));
        assert!(sb.check_file_size(100));
        assert!(!sb.check_file_size(150));

        let sb_unlimited = Sandbox::new(vec![], vec![], 0, 0, false);
        assert!(sb_unlimited.check_file_size(999999));
    }

    #[test]
    fn test_check_timeout() {
        let sb = Sandbox::new(vec![], vec![], 0, 300, false);
        assert_eq!(sb.check_timeout(0), 300);
        assert_eq!(sb.check_timeout(60), 60);
        assert_eq!(sb.check_timeout(600), 300);

        let sb_unlimited = Sandbox::new(vec![], vec![], 0, 0, false);
        assert_eq!(sb_unlimited.check_timeout(600), 600);
    }

    #[test]
    fn test_contains_sudo() {
        assert!(contains_sudo("sudo ls"));
        assert!(contains_sudo("sudo"));
        assert!(contains_sudo("sudo -u user ls"));
        assert!(!contains_sudo("ls && sudo"));
        assert!(!contains_sudo("ls -la"));
    }

    #[test]
    fn test_match_glob() {
        assert!(match_glob("ls -la", "ls*"));
        assert!(match_glob("systemctl status", "*status"));
        assert!(match_glob("systemctl status nginx", "systemctl*nginx"));
        assert!(!match_glob("cat file.txt", "ls*"));
        assert!(!match_glob("ls", ""));
        assert!(match_glob("  ls  ", "ls")); // trimmed
        assert!(!match_glob("LS", "ls")); // case-sensitive
    }

    #[test]
    fn test_prepare_env_none() {
        let sb = basic_sandbox();
        let env = sb.prepare_env().unwrap();
        assert!(!env.isolated);
        assert!(env.temp_dir.is_none());
    }

    #[test]
    fn test_prepare_env_chroot_not_implemented() {
        let mut sb = basic_sandbox();
        sb.isolation_level = IsolationLevel::Chroot;
        assert!(sb.prepare_env().is_err());
    }
}

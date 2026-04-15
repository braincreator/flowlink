//! CommandRunner — local command execution for ServerGuard
//!
//! Runs system commands (systemctl, nginx -t, docker, etc.) directly
//! on the host. Used by auto-fix and remediation actions.
//!
//! This is intentionally simple — no shell escaping games, no pipelines.
//! Just execute a binary with args and capture output.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{debug, warn};

/// Result of a local command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// Exit code (None if process didn't start)
    pub exit_code: Option<i32>,
    /// Stdout captured (truncated to 64KB)
    pub stdout: String,
    /// Stderr captured (truncated to 64KB)
    pub stderr: String,
    /// Whether the command succeeded (exit code 0)
    pub success: bool,
    /// Execution duration
    pub duration_ms: u64,
}

impl CommandResult {
    /// Create a failed result (command didn't start)
    pub fn failed(reason: &str) -> Self {
        Self {
            exit_code: None,
            stdout: String::new(),
            stderr: reason.to_string(),
            success: false,
            duration_ms: 0,
        }
    }
}

/// Trusted system commands that ServerGuard is allowed to run
pub struct CommandRunner {
    /// Maximum execution time
    timeout: Duration,
    /// Maximum output size per stream (bytes)
    max_output_bytes: usize,
}

impl CommandRunner {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_output_bytes: 64 * 1024, // 64KB
        }
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout,
            max_output_bytes: 64 * 1024,
        }
    }

    /// Run a command with the given binary and arguments
    ///
    /// No shell is involved — args are passed directly.
    /// Output is truncated to max_output_bytes per stream.
    pub async fn run(&self, binary: &str, args: &[&str]) -> CommandResult {
        self.run_with_env(binary, args, &[]).await
    }

    /// Run a command with additional environment variables
    pub async fn run_with_env(
        &self,
        binary: &str,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> CommandResult {
        let start = std::time::Instant::now();

        debug!("CommandRunner: executing {} {}", binary, args.join(" "));

        let mut cmd = Command::new(binary);
        cmd.args(args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        for (key, value) in env {
            cmd.env(key, value);
        }

        match tokio::time::timeout(self.timeout, cmd.output()).await {
            Ok(Ok(output)) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let stdout = truncate_output(&output.stdout, self.max_output_bytes);
                let stderr = truncate_output(&output.stderr, self.max_output_bytes);
                let exit_code = output.status.code();
                let success = exit_code == Some(0);

                if success {
                    debug!("CommandRunner: {} exited 0 in {}ms", binary, duration_ms);
                } else {
                    warn!(
                        "CommandRunner: {} exited {:?} in {}ms — {}",
                        binary, exit_code, duration_ms, stderr
                    );
                }

                CommandResult {
                    exit_code,
                    stdout,
                    stderr,
                    success,
                    duration_ms,
                }
            }
            Ok(Err(e)) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                warn!("CommandRunner: {} failed to start — {}", binary, e);
                CommandResult {
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!("Failed to start: {}", e),
                    success: false,
                    duration_ms,
                }
            }
            Err(_) => {
                warn!(
                    "CommandRunner: {} timed out after {}ms",
                    binary,
                    self.timeout.as_millis()
                );
                CommandResult {
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!("Timed out after {}ms", self.timeout.as_millis()),
                    success: false,
                    duration_ms: self.timeout.as_millis() as u64,
                }
            }
        }
    }

    /// Run a shell command string via /bin/sh -c
    ///
    /// ⚠️ Use sparingly — prefer run() for safety.
    /// Only use when you need shell features (pipes, redirects).
    pub async fn run_shell(&self, command: &str) -> CommandResult {
        self.run("sh", &["-c", command]).await
    }

    /// Convenience: systemctl restart
    pub async fn systemctl_restart(&self, service: &str) -> CommandResult {
        self.run("systemctl", &["restart", service]).await
    }

    /// Convenience: systemctl reload
    pub async fn systemctl_reload(&self, service: &str) -> CommandResult {
        self.run("systemctl", &["reload", service]).await
    }

    /// Convenience: systemctl status (check if running)
    pub async fn systemctl_is_active(&self, service: &str) -> bool {
        let result = self
            .run("systemctl", &["is-active", "--quiet", service])
            .await;
        result.success
    }

    /// Convenience: docker stop
    pub async fn docker_stop(&self, container: &str) -> CommandResult {
        self.run("docker", &["stop", container]).await
    }

    /// Convenience: docker rm
    pub async fn docker_rm(&self, container: &str) -> CommandResult {
        self.run("docker", &["rm", "-f", container]).await
    }

    /// Convenience: kill -STOP (freeze process)
    pub async fn freeze_process(&self, pid: u32) -> CommandResult {
        self.run("kill", &["-STOP", &pid.to_string()]).await
    }

    /// Convenience: kill -CONT (unfreeze process)
    pub async fn unfreeze_process(&self, pid: u32) -> CommandResult {
        self.run("kill", &["-CONT", &pid.to_string()]).await
    }

    /// Convenience: kill -9 (force kill)
    pub async fn kill_process(&self, pid: u32) -> CommandResult {
        self.run("kill", &["-9", &pid.to_string()]).await
    }
}

impl Default for CommandRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Truncate byte output to max_bytes, converting to UTF-8
fn truncate_output(bytes: &[u8], max_bytes: usize) -> String {
    let truncated = if bytes.len() > max_bytes {
        &bytes[..max_bytes]
    } else {
        bytes
    };
    String::from_utf8_lossy(truncated).to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_echo() {
        let runner = CommandRunner::new();
        let result = runner.run("echo", &["hello"]).await;
        assert!(result.success);
        assert!(result.stdout.contains("hello"));
        assert_eq!(result.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_run_nonexistent() {
        let runner = CommandRunner::new();
        let result = runner.run("nonexistent_binary_xyz", &[]).await;
        assert!(!result.success);
        assert!(result.exit_code.is_none());
    }

    #[tokio::test]
    async fn test_run_failing_command() {
        let runner = CommandRunner::new();
        let result = runner.run("false", &[]).await;
        assert!(!result.success);
        assert_eq!(result.exit_code, Some(1));
    }

    #[tokio::test]
    async fn test_run_shell() {
        let runner = CommandRunner::new();
        let result = runner.run_shell("echo shell-test").await;
        assert!(result.success);
        assert!(result.stdout.contains("shell-test"));
    }

    #[tokio::test]
    async fn test_timeout() {
        let runner = CommandRunner::with_timeout(Duration::from_millis(100));
        let result = runner.run("sleep", &["10"]).await;
        assert!(!result.success);
        assert!(result.stderr.contains("Timed out"));
    }

    #[test]
    fn test_truncate_output() {
        let data = "hello".repeat(10000); // 50KB
        let result = truncate_output(data.as_bytes(), 100);
        assert!(result.len() <= 100);
    }

    #[test]
    fn test_command_result_failed() {
        let result = CommandResult::failed("test error");
        assert!(!result.success);
        assert!(result.stderr.contains("test error"));
        assert_eq!(result.exit_code, None);
    }
}

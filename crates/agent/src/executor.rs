// Command executor — runs shell commands with timeout, captures output
// Supports concurrent execution with priority-based semaphores.
// Port of internal/agent/executor.go

use std::sync::Arc;
use tokio::sync::Semaphore;

use flowlink_core::*;

pub struct ExecResult {
    pub request_id: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timed_out: bool,
}

/// Executor with priority-based concurrency control.
///
/// Two independent semaphore pools prevent system commands from being
/// blocked by a queue of user commands (and vice versa):
///
/// - **User pool** (`max_user_concurrency`): regular commands, default 4
/// - **System pool** (`max_system_concurrency`): internal ops, default 2
///
/// Both pools can run in parallel. A system rollback will never wait for
/// a user's `sleep 300` to finish.
pub struct Executor {
    user_sem: Arc<Semaphore>,
    system_sem: Arc<Semaphore>,
}

impl Executor {
    /// Create a new executor with configurable concurrency limits.
    pub fn new(max_user_concurrency: usize, max_system_concurrency: usize) -> Self {
        Self {
            user_sem: Arc::new(Semaphore::new(max_user_concurrency.max(1))),
            system_sem: Arc::new(Semaphore::new(max_system_concurrency.max(1))),
        }
    }

    /// Create with defaults: 4 user, 2 system concurrent slots.
    pub fn default_executor() -> Self {
        Self::new(4, 2)
    }

    /// Execute a command with priority-based concurrency.
    ///
    /// System commands acquire a permit from the system semaphore,
    /// user commands from the user semaphore. Both pools run independently.
    pub async fn exec(
        &self,
        payload: &ExecRequestPayload,
        priority: Priority,
    ) -> anyhow::Result<ExecResult> {
        let sem = match priority {
            Priority::System => &self.system_sem,
            Priority::User => &self.user_sem,
        };

        let _permit = sem.acquire().await
            .map_err(|_| anyhow::anyhow!("Executor semaphore closed"))?;

        self.exec_inner(payload).await
    }

    /// Stateless execution (no semaphore) — used by tests and legacy paths.
    pub async fn exec_stateless(payload: &ExecRequestPayload) -> anyhow::Result<ExecResult> {
        Self::exec_inner_static(payload).await
    }

    /// Instance-based inner exec.
    async fn exec_inner(&self, payload: &ExecRequestPayload) -> anyhow::Result<ExecResult> {
        Self::exec_inner_static(payload).await
    }

    /// The actual command execution logic.
    async fn exec_inner_static(payload: &ExecRequestPayload) -> anyhow::Result<ExecResult> {
        let request_id = payload.request_id.clone();
        let timeout_secs = if payload.timeout_sec == 0 { 60 } else { payload.timeout_sec as u64 };
        let shell = payload.shell.as_deref().unwrap_or("/bin/sh");

        let start = std::time::Instant::now();

        let mut cmd = tokio::process::Command::new(shell);
        cmd.arg("-c")
            .arg(&payload.command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if let Some(dir) = &payload.dir {
            cmd.current_dir(dir);
        }
        if let Some(env) = &payload.env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            async {
                let output = cmd.output().await?;
                Ok::<_, anyhow::Error>((output.status, output.stdout, output.stderr))
            },
        ).await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok((status, out, err))) => Ok(ExecResult {
                request_id,
                exit_code: status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&out).into(),
                stderr: String::from_utf8_lossy(&err).into(),
                duration_ms,
                timed_out: false,
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(ExecResult {
                request_id,
                exit_code: -1,
                stdout: String::new(),
                stderr: "Command timed out".into(),
                duration_ms,
                timed_out: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowlink_core::*;

    fn test_payload(cmd: &str) -> ExecRequestPayload {
        ExecRequestPayload {
            command: cmd.into(),
            shell: None,
            env: None,
            dir: None,
            timeout_sec: 10,
            request_id: "test-1".into(),
        }
    }

    #[tokio::test]
    async fn test_echo_hello() {
        let payload = test_payload("echo hello");
        let result = Executor::exec_stateless(&payload).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.trim().contains("hello"));
        assert!(!result.timed_out);
    }

    #[tokio::test]
    async fn test_failing_command() {
        let payload = test_payload("false");
        let result = Executor::exec_stateless(&payload).await.unwrap();
        assert_ne!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn test_timeout() {
        let mut payload = test_payload("sleep 60");
        payload.timeout_sec = 1;
        let result = Executor::exec_stateless(&payload).await.unwrap();
        assert!(result.timed_out);
        assert_eq!(result.exit_code, -1);
        assert!(result.stderr.contains("timed out"));
    }

    #[tokio::test]
    async fn test_env_vars() {
        let mut env = std::collections::HashMap::new();
        env.insert("FOO_TEST_VAR".into(), "bar42".into());
        let mut payload = test_payload("echo $FOO_TEST_VAR");
        payload.env = Some(env);
        let result = Executor::exec_stateless(&payload).await.unwrap();
        assert!(result.stdout.trim().contains("bar42"));
    }

    #[tokio::test]
    async fn test_stderr_captured() {
        let payload = test_payload("echo errormsg >&2");
        let result = Executor::exec_stateless(&payload).await.unwrap();
        assert!(result.stderr.contains("errormsg"));
    }

    #[tokio::test]
    async fn test_working_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("marker.txt"), "here").unwrap();
        let mut payload = test_payload("cat marker.txt");
        payload.dir = Some(tmp.path().to_str().unwrap().into());
        let result = Executor::exec_stateless(&payload).await.unwrap();
        assert_eq!(result.stdout.trim(), "here");
    }

    #[tokio::test]
    async fn test_request_id_preserved() {
        let mut payload = test_payload("true");
        payload.request_id = "unique-123".into();
        let result = Executor::exec_stateless(&payload).await.unwrap();
        assert_eq!(result.request_id, "unique-123");
    }

    #[tokio::test]
    async fn test_duration_ms_nonzero() {
        let payload = test_payload("echo hi");
        let result = Executor::exec_stateless(&payload).await.unwrap();
        assert!(result.duration_ms > 0);
    }

    // ── Concurrency tests ──────────────────────────────────────────

    #[tokio::test]
    async fn test_concurrent_user_commands() {
        let executor = Executor::new(4, 2);
        let mut handles = vec![];

        for i in 0..4 {
            let exec = executor.user_sem.clone();
            let payload = test_payload("sleep 0.1 && echo done");
            handles.push(tokio::spawn(async move {
                let _permit = exec.acquire().await.unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                i
            }));
        }

        let results: Vec<_> = futures_util::future::join_all(handles).await;
        assert_eq!(results.len(), 4);
        for r in results {
            assert!(r.is_ok());
        }
    }

    #[tokio::test]
    async fn test_system_not_blocked_by_user_queue() {
        // User pool has only 1 slot, system pool has 1 slot.
        // Fill the user pool with a slow command.
        let executor = Executor::new(1, 1);

        // Block the user semaphore by acquiring its only permit
        let user_sem = executor.user_sem.clone();
        let _user_permit = user_sem.acquire().await.unwrap();

        // System command should still execute immediately
        let system_payload = test_payload("echo system-ok");
        let result = executor.exec(&system_payload, Priority::System).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("system-ok"));

        // User semaphore is still held — proves system didn't wait for it
        assert_eq!(executor.user_sem.available_permits(), 0);
    }

    #[tokio::test]
    async fn test_user_semaphore_limits_concurrency() {
        // Only 1 user slot — 2 commands must serialize through executor
        let executor = Executor::new(1, 2);

        let p1 = test_payload("sleep 0.2");
        let p2 = test_payload("sleep 0.2");

        let start = std::time::Instant::now();
        let h1 = executor.exec(&p1, Priority::User);
        let h2 = executor.exec(&p2, Priority::User);
        let (r1, r2) = tokio::join!(h1, h2);
        assert!(r1.is_ok());
        assert!(r2.is_ok());

        // With 1 slot, two 200ms sleeps must serialize → >= 300ms total
        assert!(start.elapsed() >= std::time::Duration::from_millis(300));
    }
}

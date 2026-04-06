// Command executor — runs shell commands with timeout, captures output
// Port of internal/agent/executor.go

use flowlink_core::*;

pub struct ExecResult {
    pub request_id: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timed_out: bool,
}

pub struct Executor;

impl Executor {
    pub async fn exec(payload: &ExecRequestPayload) -> anyhow::Result<ExecResult> {
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
        let result = Executor::exec(&payload).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.trim().contains("hello"));
        assert!(!result.timed_out);
    }

    #[tokio::test]
    async fn test_failing_command() {
        let payload = test_payload("false");
        let result = Executor::exec(&payload).await.unwrap();
        assert_ne!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn test_timeout() {
        let mut payload = test_payload("sleep 60");
        payload.timeout_sec = 1;
        let result = Executor::exec(&payload).await.unwrap();
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
        let result = Executor::exec(&payload).await.unwrap();
        assert!(result.stdout.trim().contains("bar42"));
    }

    #[tokio::test]
    async fn test_stderr_captured() {
        let payload = test_payload("echo errormsg >&2");
        let result = Executor::exec(&payload).await.unwrap();
        assert!(result.stderr.contains("errormsg"));
    }

    #[tokio::test]
    async fn test_working_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("marker.txt"), "here").unwrap();
        let mut payload = test_payload("cat marker.txt");
        payload.dir = Some(tmp.path().to_str().unwrap().into());
        let result = Executor::exec(&payload).await.unwrap();
        assert_eq!(result.stdout.trim(), "here");
    }

    #[tokio::test]
    async fn test_request_id_preserved() {
        let mut payload = test_payload("true");
        payload.request_id = "unique-123".into();
        let result = Executor::exec(&payload).await.unwrap();
        assert_eq!(result.request_id, "unique-123");
    }

    #[tokio::test]
    async fn test_duration_ms_nonzero() {
        let payload = test_payload("echo hi");
        let result = Executor::exec(&payload).await.unwrap();
        assert!(result.duration_ms > 0);
    }
}

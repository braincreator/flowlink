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

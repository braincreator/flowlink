// FlowLink Agent — Session Recorder
// Records terminal sessions in asciinema v2 format

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use anyhow::{Context, Result};

// ═══════════════════════════════════════════════
// Session Recorder
// ═══════════════════════════════════════════════

pub struct SessionRecorder {
    session_id: String,
    journal: File,
    start_time: Instant,
    commands_count: u32,
    elapsed_secs: f64,
    width: u16,
    height: u16,
}

impl SessionRecorder {
    pub fn new(session_id: &str, path: &Path) -> Result<Self> {
        let mut journal = File::create(path)
            .with_context(|| format!("Failed to create session journal at {}", path.display()))?;

        // Write asciinema v2 header
        let header = serde_json::json!({
            "version": 2,
            "width": 80,
            "height": 24,
            "timestamp": chrono::Utc::now().timestamp(),
            "env": {"TERM": "xterm-256color", "SHELL": "/bin/bash"}
        });
        writeln!(journal, "{}", serde_json::to_string(&header)?)?;

        Ok(Self {
            session_id: session_id.to_string(),
            journal,
            start_time: Instant::now(),
            commands_count: 0,
            elapsed_secs: 0.0,
            width: 80,
            height: 24,
        })
    }

    pub fn new_with_size(session_id: &str, path: &Path, width: u16, height: u16) -> Result<Self> {
        let mut rec = Self::new(session_id, path)?;
        rec.width = width;
        rec.height = height;
        // Rewrite header with correct size
        // (For simplicity, we keep the default header; production would seek back)
        Ok(rec)
    }

    /// Record output (stdout/stderr). Event type "o" for output.
    pub fn record_output(&mut self, data: &[u8]) -> Result<()> {
        self.elapsed_secs = self.start_time.elapsed().as_secs_f64();
        let output = String::from_utf8_lossy(data).replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t");
        writeln!(self.journal, "[{:.6}, \"o\", \"{}\"]", self.elapsed_secs, output)?;
        self.journal.flush()?;
        Ok(())
    }

    /// Record input (what user typed). Event type "i" for input.
    pub fn record_input(&mut self, data: &str) -> Result<()> {
        self.elapsed_secs = self.start_time.elapsed().as_secs_f64();
        let escaped = data.replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t");
        writeln!(self.journal, "[{:.6}, \"i\", \"{}\"]", self.elapsed_secs, escaped)?;
        self.journal.flush()?;
        Ok(())
    }

    /// Record a command being executed (increments counter, records as input)
    pub fn record_command(&mut self, command: &str) -> Result<()> {
        self.commands_count += 1;
        self.record_input(&format!("{}\n", command))
    }

    pub fn commands_count(&self) -> u32 {
        self.commands_count
    }

    pub fn duration_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Close the recorder and return summary
    pub fn close(self) -> Result<SessionSummary> {
        let duration_ms = self.start_time.elapsed().as_millis() as u64;
        let session_id = self.session_id;
        let commands_count = self.commands_count;
        let file_size = self.journal.metadata()?.len();
        // journal is closed on drop
        Ok(SessionSummary {
            session_id,
            duration_ms,
            commands_count,
            file_size,
        })
    }
}

// ═══════════════════════════════════════════════
// Session Summary
// ═══════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub session_id: String,
    pub duration_ms: u64,
    pub commands_count: u32,
    pub file_size: u64,
}

// ═══════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asciinema_v2_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.cast");
        let mut rec = SessionRecorder::new("test-session", &path).unwrap();

        rec.record_output(b"hello world\n").unwrap();
        rec.record_command("ls -la").unwrap();
        rec.record_output(b"total 42\n").unwrap();

        let summary = rec.close().unwrap();
        assert_eq!(summary.session_id, "test-session");
        assert_eq!(summary.commands_count, 1);
        assert!(summary.file_size > 0);

        // Verify format
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        // First line is header
        let header: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(header["version"], 2);
        assert_eq!(header["width"], 80);
        assert_eq!(header["height"], 24);
        // Remaining lines are output entries
        assert!(lines[1].starts_with("["));
        assert!(lines[1].contains("\"o\""));
        // Command is input
        assert!(lines[2].contains("\"i\""));
        assert!(lines[2].contains("ls -la"));
    }

    #[test]
    fn test_record_output_and_input() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.cast");
        let mut rec = SessionRecorder::new("s2", &path).unwrap();

        rec.record_input("echo hi\n").unwrap();
        rec.record_output(b"hi\n").unwrap();

        rec.close().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"i\""));
        assert!(content.contains("\"o\""));
        assert!(content.contains("echo hi"));
    }

    #[test]
    fn test_commands_count() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.cast");
        let mut rec = SessionRecorder::new("s3", &path).unwrap();

        rec.record_command("ls").unwrap();
        rec.record_command("pwd").unwrap();
        rec.record_command("whoami").unwrap();

        assert_eq!(rec.commands_count(), 3);
        rec.close().unwrap();
    }

    #[test]
    fn test_multiple_recorders() {
        let dir = tempfile::tempdir().unwrap();
        let mut rec1 = SessionRecorder::new("s1", &dir.path().join("s1.cast")).unwrap();
        let mut rec2 = SessionRecorder::new("s2", &dir.path().join("s2.cast")).unwrap();

        rec1.record_command("echo 1").unwrap();
        rec2.record_command("echo 2").unwrap();

        let sum1 = rec1.close().unwrap();
        let sum2 = rec2.close().unwrap();

        assert_eq!(sum1.session_id, "s1");
        assert_eq!(sum2.session_id, "s2");
    }
}

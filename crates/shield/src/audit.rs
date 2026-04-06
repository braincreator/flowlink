// FlowLink Shield — Audit logger
// Writes structured JSON audit trail

use chrono::Utc;
use serde::{Serialize, Deserialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditEntry {
    pub timestamp: String,
    pub pid: u32,
    pub ppid: u32,
    pub uid: u32,
    pub username: String,
    pub command: String,
    pub rule_name: String,
    pub action_taken: String, // "blocked", "warned", "allowed"
    pub snapshot: Option<String>,
    pub result: String, // "killed", "released", "timeout_killed"
}

pub struct AuditLog {
    file: File,
}

impl AuditLog {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self { file })
    }

    pub fn log(&mut self, entry: AuditEntry) -> anyhow::Result<()> {
        let json = serde_json::to_string(&entry)?;
        writeln!(self.file, "{}", json)?;
        self.file.flush()?;
        Ok(())
    }

    pub fn create_entry(
        &self,
        pid: u32,
        ppid: u32,
        uid: u32,
        username: String,
        command: String,
        rule_name: String,
        action_taken: String,
        result: String,
    ) -> AuditEntry {
        AuditEntry {
            timestamp: Utc::now().to_rfc3339(),
            pid,
            ppid,
            uid,
            username,
            command,
            rule_name,
            action_taken,
            snapshot: None,
            result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn create_entry_fields() {
        let log = AuditLog::open(Path::new("/dev/null")).unwrap();
        let entry = log.create_entry(
            1234, 100, 1000, "alice".into(), "rm -rf /".into(),
            "rm_rf".into(), "blocked".into(), "killed".into(),
        );
        assert_eq!(entry.pid, 1234);
        assert_eq!(entry.uid, 1000);
        assert_eq!(entry.command, "rm -rf /");
        assert_eq!(entry.rule_name, "rm_rf");
        assert_eq!(entry.action_taken, "blocked");
        assert_eq!(entry.result, "killed");
        assert!(entry.snapshot.is_none());
    }

    #[test]
    fn create_entry_timestamp_format() {
        let log = AuditLog::open(Path::new("/dev/null")).unwrap();
        let entry = log.create_entry(1, 0, 0, "root".into(), "ls".into(), "".into(), "allowed".into(), "allowed".into());
        // ISO 8601 format
        assert!(entry.timestamp.contains('T'));
        assert!(entry.timestamp.ends_with('Z') || entry.timestamp.contains('+'));
    }

    #[test]
    fn log_writes_valid_json() {
        let tmp = NamedTempFile::new().unwrap();
        let mut log = AuditLog::open(tmp.path()).unwrap();
        let entry = log.create_entry(
            42, 1, 1000, "bob".into(), "echo hello".into(),
            "".into(), "allowed".into(), "allowed".into(),
        );
        log.log(entry.clone()).unwrap();

        let contents = std::fs::read_to_string(tmp.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&contents.trim()).unwrap();
        assert_eq!(parsed["pid"], 42);
        assert_eq!(parsed["username"], "bob");
        assert_eq!(parsed["command"], "echo hello");
    }

    #[test]
    fn log_multiple_entries() {
        let tmp = NamedTempFile::new().unwrap();
        let mut log = AuditLog::open(tmp.path()).unwrap();
        for i in 0..5 {
            let entry = log.create_entry(i, 0, 0, "root".into(), format!("cmd {}", i), format!("rule {}", i), "allowed".into(), "allowed".into());
            log.log(entry).unwrap();
        }
        let contents = std::fs::read_to_string(tmp.path()).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn entry_serialization_roundtrip() {
        let entry = AuditEntry {
            timestamp: "2026-04-06T12:00:00Z".into(),
            pid: 999, ppid: 1, uid: 1000,
            username: "charlie".into(),
            command: "sudo rm -rf /".into(),
            rule_name: "rm_rf".into(),
            action_taken: "blocked".into(),
            snapshot: Some("tank@snap".into()),
            result: "killed".into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.pid, 999);
        assert_eq!(back.snapshot, Some("tank@snap".into()));
    }

    #[test]
    fn log_creates_file_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        assert!(!path.exists());
        let mut log = AuditLog::open(&path).unwrap();
        let entry = log.create_entry(1, 0, 0, "root".into(), "test".into(), "".into(), "allowed".into(), "allowed".into());
        log.log(entry).unwrap();
        assert!(path.exists());
    }
}

// FlowLink Shield — Audit logger
// Writes structured JSON audit trail

use chrono::Utc;
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Serialize, Clone)]
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

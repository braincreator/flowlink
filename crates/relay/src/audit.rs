// Audit Logger — structured JSON audit trail
// Port of internal/relay/audit.go

use chrono::Utc;
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;

#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub event: String,
    pub agent_id: Option<String>,
    pub client_id: Option<String>,
    pub command: Option<String>,
    pub action: String,
    pub details: Option<serde_json::Value>,
}

pub struct AuditLogger {
    path: String,
}

impl AuditLogger {
    pub fn new(path: &str) -> Self {
        Self { path: path.to_string() }
    }

    pub fn log(&self, entry: AuditEntry) {
        if let Ok(line) = serde_json::to_string(&entry) {
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.path) {
                let _ = writeln!(f, "{line}");
            }
        }
    }

    pub fn log_exec(&self, agent_id: &str, command: &str, action: &str) {
        self.log(AuditEntry {
            timestamp: Utc::now().to_rfc3339(),
            event: "exec".into(),
            agent_id: Some(agent_id.into()),
            client_id: None,
            command: Some(command.into()),
            action: action.into(),
            details: None,
        });
    }
}

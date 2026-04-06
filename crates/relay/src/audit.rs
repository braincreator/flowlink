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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        let logger = AuditLogger::new(path.to_str().unwrap());

        logger.log(AuditEntry {
            timestamp: "2024-01-01T00:00:00Z".into(),
            event: "exec".into(),
            agent_id: Some("agent-1".into()),
            client_id: None,
            command: Some("ls".into()),
            action: "run".into(),
            details: None,
        });

        let content = std::fs::read_to_string(&path).unwrap();
        let entry: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(entry["event"], "exec");
        assert_eq!(entry["agent_id"], "agent-1");
        assert_eq!(entry["command"], "ls");
    }

    #[test]
    fn test_log_exec() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        let logger = AuditLogger::new(path.to_str().unwrap());

        logger.log_exec("agent-2", "whoami", "run");

        let content = std::fs::read_to_string(&path).unwrap();
        let entry: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(entry["agent_id"], "agent-2");
        assert_eq!(entry["command"], "whoami");
    }

    #[test]
    fn test_multiple_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        let logger = AuditLogger::new(path.to_str().unwrap());

        for i in 0..5 {
            logger.log(AuditEntry {
                timestamp: format!("2024-01-01T00:0{i}:00Z"),
                event: "test".into(),
                agent_id: None, client_id: None, command: None,
                action: format!("action-{i}"),
                details: None,
            });
        }

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn test_structured_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        let logger = AuditLogger::new(path.to_str().unwrap());

        logger.log(AuditEntry {
            timestamp: "2024-01-01T00:00:00Z".into(),
            event: "shield".into(),
            agent_id: Some("a1".into()),
            client_id: Some("c1".into()),
            command: Some("sudo rm".into()),
            action: "blocked".into(),
            details: Some(serde_json::json!({"pid": 1234})),
        });

        let content = std::fs::read_to_string(&path).unwrap();
        let entry: serde_json::Value = serde_json::from_str(content.trim()).unwrap();
        assert_eq!(entry["details"]["pid"], 1234);
        assert_eq!(entry["client_id"], "c1");
    }
}

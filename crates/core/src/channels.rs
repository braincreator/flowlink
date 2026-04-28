// FlowLink Channels — Dual-channel architecture
// Channel 1 (E2EE): Agent ←encrypted→ Relay ←encrypted→ Client
// Channel 2 (Audit): Shield (on host) → plaintext audit events → Relay

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Audit event sent from Shield to Relay (plaintext, NOT encrypted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub agent_id: String,
    pub event_type: AuditEventType,
    pub timestamp_nanos: u64,
    pub timestamp_iso: String,
    /// Correlation ID linking related events (e.g., MCP request → tool calls → results)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forensic: Option<ForensicSummary>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    // Shield events
    CommandIntercepted {
        command: String,
        args: Vec<String>,
        action: String,
        threat_level: String,
        risk_score: u8,
    },
    CommandApproved {
        command: String,
        approved_by: String,
    },
    CommandRejected {
        command: String,
        rejected_by: String,
    },
    CommandExecuted {
        command: String,
        exit_code: i32,
        duration_ms: u64,
    },

    // Canary events
    CanaryTriggered {
        path: String,
        accessor: String,
        access_type: String,
    },

    // Session events
    SessionStarted {
        user: String,
        origin: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        terminal: Option<String>,
    },
    SessionEnded {
        user: String,
        duration_ms: u64,
        commands_count: u32,
    },

    // Agent events
    AgentRegistered {
        hostname: String,
        version: String,
    },
    AgentHeartbeat {
        status: String,
        uptime_secs: u64,
    },
    AgentDisconnected {
        reason: String,
    },

    // Policy events
    PolicyViolation {
        rule: String,
        command: String,
        user: String,
    },
    PolicyLoaded {
        rules_count: u32,
        version: String,
    },
    DiscoveryStarted {
        scan_id: String,
        agent_id: String,
    },
    DiscoveryApproved {
        scan_id: String,
        secret_count: usize,
    },
}

impl AuditEventType {
    /// Returns a string key for filtering, e.g. "command_intercepted"
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CommandIntercepted { .. } => "command_intercepted",
            Self::CommandApproved { .. } => "command_approved",
            Self::CommandRejected { .. } => "command_rejected",
            Self::CommandExecuted { .. } => "command_executed",
            Self::CanaryTriggered { .. } => "canary_triggered",
            Self::SessionStarted { .. } => "session_started",
            Self::SessionEnded { .. } => "session_ended",
            Self::AgentRegistered { .. } => "agent_registered",
            Self::AgentHeartbeat { .. } => "agent_heartbeat",
            Self::AgentDisconnected { .. } => "agent_disconnected",
            Self::PolicyViolation { .. } => "policy_violation",
            Self::PolicyLoaded { .. } => "policy_loaded",
            Self::DiscoveryStarted { .. } => "discovery_started",
            Self::DiscoveryApproved { .. } => "discovery_approved",
        }
    }

    /// Extract risk score if available
    pub fn risk_score(&self) -> Option<u8> {
        match self {
            Self::CommandIntercepted { risk_score, .. } => Some(*risk_score),
            _ => None,
        }
    }

    /// Extract the associated username if available
    pub fn username(&self) -> Option<&str> {
        match self {
            Self::CommandApproved { approved_by, .. } => Some(approved_by),
            Self::CommandRejected { rejected_by, .. } => Some(rejected_by),
            Self::SessionStarted { user, .. } => Some(user),
            Self::SessionEnded { user, .. } => Some(user),
            Self::PolicyViolation { user, .. } => Some(user),
            Self::CanaryTriggered { accessor, .. } => Some(accessor),
            _ => None,
        }
    }
}

/// Minimal forensic summary for audit (full forensic stays on host)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForensicSummary {
    pub uid: u32,
    pub username: String,
    pub origin: String,
    pub process_tree: Vec<String>,
    pub risk_score: u8,
}

/// Session recording chunk (stored on host, metadata sent to relay)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionChunk {
    pub session_id: String,
    pub agent_id: String,
    pub chunk_seq: u32,
    pub timestamp_ns: u64,
    pub data: Vec<u8>,
    pub is_encrypted: bool,
}

/// Canary token definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryToken {
    pub path: String,
    pub description: String,
    pub expected_readers: Vec<String>,
    pub alert_threshold: AlertThreshold,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlertThreshold {
    Any,
    UnknownUser,
    NonAdmin,
}

/// Helper to build an AuditEvent with current timestamps
impl AuditEvent {
    pub fn new(agent_id: &str, event_type: AuditEventType) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            event_type,
            timestamp_nanos: now.timestamp_nanos_opt().unwrap_or(0) as u64,
            timestamp_iso: now.to_rfc3339(),
            correlation_id: None,
            forensic: None,
            metadata: HashMap::new(),
        }
    }

    /// Set correlation ID for request tracing
    pub fn with_correlation(mut self, cid: impl Into<String>) -> Self {
        self.correlation_id = Some(cid.into());
        self
    }

    pub fn with_forensic(mut self, forensic: ForensicSummary) -> Self {
        self.forensic = Some(forensic);
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_serialization() {
        let event = AuditEvent::new(
            "agent-1",
            AuditEventType::CommandIntercepted {
                command: "rm -rf /".into(),
                args: vec!["-rf".into(), "/".into()],
                action: "blocked".into(),
                threat_level: "critical".into(),
                risk_score: 95,
            },
        )
        .with_forensic(ForensicSummary {
            uid: 1000,
            username: "alice".into(),
            origin: "ssh".into(),
            process_tree: vec!["sshd".into(), "bash".into(), "rm".into()],
            risk_score: 95,
        });

        let json = serde_json::to_string(&event).unwrap();
        let back: AuditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.agent_id, "agent-1");
        assert_eq!(back.forensic.as_ref().unwrap().username, "alice");
    }

    #[test]
    fn test_audit_event_type_as_str() {
        assert_eq!(
            AuditEventType::CanaryTriggered {
                path: "/x".into(),
                accessor: "bob".into(),
                access_type: "read".into()
            }
            .as_str(),
            "canary_triggered"
        );
        assert_eq!(
            AuditEventType::CommandApproved {
                command: "ls".into(),
                approved_by: "admin".into()
            }
            .as_str(),
            "command_approved"
        );
    }

    #[test]
    fn test_canary_token_serialization() {
        let token = CanaryToken {
            path: "/etc/shadow.bak".into(),
            description: "Fake shadow".into(),
            expected_readers: vec!["root".into()],
            alert_threshold: AlertThreshold::Any,
        };
        let json = serde_json::to_string(&token).unwrap();
        let back: CanaryToken = serde_json::from_str(&json).unwrap();
        assert_eq!(back.path, "/etc/shadow.bak");
    }

    #[test]
    fn test_session_chunk_roundtrip() {
        let chunk = SessionChunk {
            session_id: "s1".into(),
            agent_id: "a1".into(),
            chunk_seq: 0,
            timestamp_ns: 1234567890,
            data: vec![1, 2, 3],
            is_encrypted: false,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let back: SessionChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(back.chunk_seq, 0);
        assert_eq!(back.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_alert_threshold_variants() {
        let t1 = AlertThreshold::Any;
        let t2 = AlertThreshold::UnknownUser;
        let t3 = AlertThreshold::NonAdmin;
        let j1 = serde_json::to_string(&t1).unwrap();
        let j2 = serde_json::to_string(&t2).unwrap();
        let j3 = serde_json::to_string(&t3).unwrap();
        assert!(j1.contains("Any"));
        assert!(j2.contains("UnknownUser"));
        assert!(j3.contains("NonAdmin"));
    }

    #[test]
    fn test_event_type_username_extraction() {
        let et = AuditEventType::CommandApproved {
            command: "ls".into(),
            approved_by: "alice".into(),
        };
        assert_eq!(et.username(), Some("alice"));
        let et2 = AuditEventType::CommandIntercepted {
            command: "rm".into(),
            args: vec![],
            action: "block".into(),
            threat_level: "high".into(),
            risk_score: 80,
        };
        assert_eq!(et2.username(), None);
    }

    #[test]
    fn test_event_type_risk_score() {
        let et = AuditEventType::CommandIntercepted {
            command: "rm".into(),
            args: vec![],
            action: "block".into(),
            threat_level: "high".into(),
            risk_score: 42,
        };
        assert_eq!(et.risk_score(), Some(42));
        let et2 = AuditEventType::CommandApproved {
            command: "ls".into(),
            approved_by: "a".into(),
        };
        assert_eq!(et2.risk_score(), None);
    }

    #[test]
    fn test_all_audit_event_types_roundtrip() {
        let types = vec![
            AuditEventType::CommandIntercepted {
                command: "rm".into(),
                args: vec!["-rf".into()],
                action: "blocked".into(),
                threat_level: "high".into(),
                risk_score: 90,
            },
            AuditEventType::CommandApproved {
                command: "ls".into(),
                approved_by: "admin".into(),
            },
            AuditEventType::CommandRejected {
                command: "rm".into(),
                rejected_by: "admin".into(),
            },
            AuditEventType::CommandExecuted {
                command: "ls".into(),
                exit_code: 0,
                duration_ms: 100,
            },
            AuditEventType::CanaryTriggered {
                path: "/etc/shadow".into(),
                accessor: "bob".into(),
                access_type: "read".into(),
            },
            AuditEventType::SessionStarted {
                user: "alice".into(),
                origin: "ssh".into(),
                terminal: Some("xterm".into()),
            },
            AuditEventType::SessionEnded {
                user: "alice".into(),
                duration_ms: 60000,
                commands_count: 42,
            },
            AuditEventType::AgentRegistered {
                hostname: "srv1".into(),
                version: "1.0".into(),
            },
            AuditEventType::AgentHeartbeat {
                status: "ok".into(),
                uptime_secs: 86400,
            },
            AuditEventType::AgentDisconnected {
                reason: "timeout".into(),
            },
            AuditEventType::PolicyViolation {
                rule: "no_sudo".into(),
                command: "sudo rm".into(),
                user: "bob".into(),
            },
            AuditEventType::PolicyLoaded {
                rules_count: 15,
                version: "v2".into(),
            },
            AuditEventType::DiscoveryStarted {
                scan_id: "test-scan-123".into(),
                agent_id: "agent-1".into(),
            },
            AuditEventType::DiscoveryApproved {
                scan_id: "test-scan-123".into(),
                secret_count: 5,
            },
        ];
        for et in types {
            let json = serde_json::to_string(&et).unwrap();
            let back: AuditEventType = serde_json::from_str(&json).unwrap();
            assert_eq!(et.as_str(), back.as_str());
        }
    }

    #[test]
    fn test_forensic_summary_roundtrip() {
        let f = ForensicSummary {
            uid: 1000,
            username: "alice".into(),
            origin: "ssh".into(),
            process_tree: vec!["sshd".into(), "bash".into(), "vim".into()],
            risk_score: 75,
        };
        let json = serde_json::to_string(&f).unwrap();
        let back: ForensicSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(back.process_tree.len(), 3);
        assert_eq!(back.risk_score, 75);
    }

    #[test]
    fn test_audit_event_with_metadata() {
        let event = AuditEvent::new(
            "a1",
            AuditEventType::AgentHeartbeat {
                status: "ok".into(),
                uptime_secs: 3600,
            },
        )
        .with_metadata("source", "host")
        .with_metadata("version", "2.0");
        let json = serde_json::to_string(&event).unwrap();
        let back: AuditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.metadata["source"], "host");
        assert_eq!(back.metadata["version"], "2.0");
    }

    #[test]
    fn test_audit_event_without_forensic() {
        let event = AuditEvent::new(
            "a1",
            AuditEventType::CommandExecuted {
                command: "ls".into(),
                exit_code: 0,
                duration_ms: 10,
            },
        );
        let json = serde_json::to_string(&event).unwrap();
        let back: AuditEvent = serde_json::from_str(&json).unwrap();
        assert!(back.forensic.is_none());
    }

    #[test]
    fn test_canary_token_all_thresholds() {
        for thresh in [
            AlertThreshold::Any,
            AlertThreshold::UnknownUser,
            AlertThreshold::NonAdmin,
        ] {
            let token = CanaryToken {
                path: "/f".into(),
                description: "d".into(),
                expected_readers: vec!["root".into()],
                alert_threshold: thresh.clone(),
            };
            let json = serde_json::to_string(&token).unwrap();
            let back: CanaryToken = serde_json::from_str(&json).unwrap();
            assert_eq!(back.alert_threshold, thresh);
        }
    }

    #[test]
    fn test_session_chunk_encrypted() {
        let chunk = SessionChunk {
            session_id: "s1".into(),
            agent_id: "a1".into(),
            chunk_seq: 5,
            timestamp_ns: 999,
            data: vec![0xDE, 0xAD],
            is_encrypted: true,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let back: SessionChunk = serde_json::from_str(&json).unwrap();
        assert!(back.is_encrypted);
        assert_eq!(back.data, vec![0xDE, 0xAD]);
    }

    #[test]
    fn test_audit_event_type_username_all_variants() {
        assert!(AuditEventType::CommandApproved {
            command: "x".into(),
            approved_by: "a".into()
        }
        .username()
        .is_some());
        assert!(AuditEventType::CommandRejected {
            command: "x".into(),
            rejected_by: "a".into()
        }
        .username()
        .is_some());
        assert!(AuditEventType::SessionStarted {
            user: "a".into(),
            origin: "x".into(),
            terminal: None
        }
        .username()
        .is_some());
        assert!(AuditEventType::SessionEnded {
            user: "a".into(),
            duration_ms: 0,
            commands_count: 0
        }
        .username()
        .is_some());
        assert!(AuditEventType::PolicyViolation {
            rule: "x".into(),
            command: "x".into(),
            user: "a".into()
        }
        .username()
        .is_some());
        assert!(AuditEventType::CanaryTriggered {
            path: "x".into(),
            accessor: "a".into(),
            access_type: "r".into()
        }
        .username()
        .is_some());
        assert!(AuditEventType::CommandIntercepted {
            command: "x".into(),
            args: vec![],
            action: "x".into(),
            threat_level: "x".into(),
            risk_score: 0
        }
        .username()
        .is_none());
        assert!(AuditEventType::AgentRegistered {
            hostname: "x".into(),
            version: "x".into()
        }
        .username()
        .is_none());
    }

    #[test]
    fn test_audit_event_new_generates_uuid() {
        let event = AuditEvent::new(
            "a1",
            AuditEventType::AgentDisconnected {
                reason: "test".into(),
            },
        );
        assert!(uuid::Uuid::parse_str(&event.id).is_ok());
        assert!(!event.timestamp_iso.is_empty());
    }
}

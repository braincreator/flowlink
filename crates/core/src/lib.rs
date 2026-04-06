// FlowLink Core — Protocol types, error codes, config
// Port of internal/protocol/*.go

pub mod codes;
pub mod config;

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════
// Protocol Version
// ═══════════════════════════════════════════════

pub const PROTOCOL_VERSION: i32 = 1;

// ═══════════════════════════════════════════════
// Message Types
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    // Connection
    Connect,
    Connected,
    Disconnect,
    Heartbeat,
    HeartbeatAck,

    // Command Execution
    ExecRequest,
    ExecOutput,
    ExecDone,
    ExecApprove,
    ExecReject,
    NeedsApproval,
    ApprovalRequest,
    ApprovalResponse,

    // File Operations
    FileRead,
    FileWrite,
    FileList,
    FileResponse,

    // System Info
    SysInfo,
    SysInfoResp,

    // Configuration
    ConfigUpdate,
    ConfigAck,

    // Autonomous Tasks (L2)
    Task,
    TaskProgress,
    TaskDone,
    TaskCancel,

    // Skills
    SkillPush,
    SkillList,
    SkillDelete,

    // LLM Proxy
    LlmRequest,
    LlmResponse,

    // Backup
    BackupRequest,
    BackupResponse,
    BackupList,
    BackupListResp,
    BackupRestore,
    BackupRestoreOk,
    BackupDelete,
    BackupDeleteOk,
    BackupProgress,

    // Shield (NEW in v2)
    ShieldAlert,
    ShieldAlertResponse,

    // Error
    Error,
}

// ═══════════════════════════════════════════════
// Envelope
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: MessageType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub e2ee: Option<EncryptedData>,
}

impl Message {
    pub fn new(msg_type: MessageType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            msg_type,
            version: Some(PROTOCOL_VERSION),
            agent_id: None,
            session_id: None,
            payload: None,
            timestamp: chrono::Utc::now().timestamp(),
            error: None,
            encrypted: None,
            e2ee: None,
        }
    }

    pub fn with_agent_id(mut self, id: impl Into<String>) -> Self {
        self.agent_id = Some(id.into());
        self
    }

    pub fn with_payload(mut self, payload: impl Serialize) -> Self {
        self.payload = Some(serde_json::to_value(payload).unwrap_or_default());
        self
    }
}

// ═══════════════════════════════════════════════
// E2EE
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub key_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_key_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    pub ciphertext: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ephemeral_pub_key: Option<String>,
}

// ═══════════════════════════════════════════════
// Connection Payloads
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectPayload {
    pub agent_id: String,
    pub token: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedPayload {
    pub agent_id: String,
    pub relay_id: String,
    pub heartbeat_interval_sec: i32,
    pub server_time: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay_public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay_key_id: Option<String>,
}

// ═══════════════════════════════════════════════
// Execution Payloads
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecRequestPayload {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    pub timeout_sec: i32,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecOutputPayload {
    pub request_id: String,
    pub data: String,
    pub stream: String, // "stdout" | "stderr"
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecDonePayload {
    pub request_id: String,
    pub exit_code: i32,
    pub duration_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequestPayload {
    pub request_id: String,
    pub command: String,
    pub risk: String, // "low" | "medium" | "high"
    pub mode: String, // "auto" | "soft_ask" | "hard_ask"
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalResponsePayload {
    pub request_id: String,
    pub approved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

// ═══════════════════════════════════════════════
// File Payloads
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadPayload {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWritePayload {
    pub path: String,
    pub content: String,
    pub encoding: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub size: i64,
    pub is_dir: bool,
    pub mode: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResponsePayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_dir: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<FileEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ═══════════════════════════════════════════════
// System Info
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfoPayload {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub cpu_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_model: Option<String>,
    pub mem_total_bytes: u64,
    pub mem_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub uptime_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_avg: Option<Vec<f64>>,
}

// ═══════════════════════════════════════════════
// Backup Payloads
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRequestPayload {
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResponsePayload {
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub timestamp: i64,
    pub size: i64,
    pub paths: Vec<String>,
    pub filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRestorePayload {
    pub request_id: String,
    pub snapshot_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupProgressPayload {
    pub request_id: String,
    pub progress: u8, // 0-100
    pub message: String,
}

// ═══════════════════════════════════════════════
// Shield Payloads (NEW in v2)
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldAlertPayload {
    pub alert_id: String,
    pub pid: u32,
    pub uid: u32,
    pub username: String,
    pub command: String,
    pub rule_name: String,
    pub action: String, // "blocked" | "warned"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldAlertResponsePayload {
    pub alert_id: String,
    pub approved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

// ═══════════════════════════════════════════════
// Error
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
}

// ═══════════════════════════════════════════════
// Config
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigUpdatePayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_dir: Option<String>,
}

// ═══════════════════════════════════════════════
// Helper
// ═══════════════════════════════════════════════

pub fn request_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_new() {
        let msg = Message::new(MessageType::Connect);
        assert_eq!(msg.msg_type, MessageType::Connect);
        assert!(msg.agent_id.is_none());
        assert!(msg.payload.is_none());
        assert_eq!(msg.version, Some(PROTOCOL_VERSION));
    }

    #[test]
    fn test_message_with_agent_id() {
        let msg = Message::new(MessageType::Heartbeat).with_agent_id("agent-42");
        assert_eq!(msg.agent_id.as_deref(), Some("agent-42"));
    }

    #[test]
    fn test_message_with_payload() {
        let msg = Message::new(MessageType::ExecRequest)
            .with_payload(ExecRequestPayload {
                command: "ls".into(),
                shell: None,
                env: None,
                dir: None,
                timeout_sec: 30,
                request_id: "r1".into(),
            });
        assert!(msg.payload.is_some());
        let p: ExecRequestPayload = serde_json::from_value(msg.payload.unwrap()).unwrap();
        assert_eq!(p.command, "ls");
    }

    #[test]
    fn test_message_type_variants_exist() {
        let _ = MessageType::Connect;
        let _ = MessageType::Connected;
        let _ = MessageType::Disconnect;
        let _ = MessageType::Heartbeat;
        let _ = MessageType::HeartbeatAck;
        let _ = MessageType::ExecRequest;
        let _ = MessageType::ExecOutput;
        let _ = MessageType::ExecDone;
        let _ = MessageType::ExecApprove;
        let _ = MessageType::ExecReject;
        let _ = MessageType::NeedsApproval;
        let _ = MessageType::ApprovalRequest;
        let _ = MessageType::ApprovalResponse;
        let _ = MessageType::FileRead;
        let _ = MessageType::FileWrite;
        let _ = MessageType::FileList;
        let _ = MessageType::FileResponse;
        let _ = MessageType::SysInfo;
        let _ = MessageType::SysInfoResp;
        let _ = MessageType::ConfigUpdate;
        let _ = MessageType::ConfigAck;
        let _ = MessageType::Task;
        let _ = MessageType::TaskProgress;
        let _ = MessageType::TaskDone;
        let _ = MessageType::TaskCancel;
        let _ = MessageType::SkillPush;
        let _ = MessageType::SkillList;
        let _ = MessageType::SkillDelete;
        let _ = MessageType::LlmRequest;
        let _ = MessageType::LlmResponse;
        let _ = MessageType::BackupRequest;
        let _ = MessageType::BackupResponse;
        let _ = MessageType::BackupList;
        let _ = MessageType::BackupListResp;
        let _ = MessageType::BackupRestore;
        let _ = MessageType::BackupRestoreOk;
        let _ = MessageType::BackupDelete;
        let _ = MessageType::BackupDeleteOk;
        let _ = MessageType::BackupProgress;
        let _ = MessageType::ShieldAlert;
        let _ = MessageType::ShieldAlertResponse;
        let _ = MessageType::Error;
    }

    #[test]
    fn test_message_serialize_deserialize() {
        let msg = Message::new(MessageType::Disconnect).with_agent_id("a1");
        let json = serde_json::to_string(&msg).unwrap();
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(back.msg_type, MessageType::Disconnect);
        assert_eq!(back.agent_id.as_deref(), Some("a1"));
    }

    #[test]
    fn test_connect_payload_roundtrip() {
        let p = ConnectPayload {
            agent_id: "a1".into(), token: "t1".into(),
            hostname: "host".into(), os: "linux".into(), arch: "x86_64".into(),
            client_version: Some("1.0".into()), public_key: None, protocol_version: Some(1),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: ConnectPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.agent_id, "a1");
        assert_eq!(back.client_version.as_deref(), Some("1.0"));
    }

    #[test]
    fn test_exec_request_payload_roundtrip() {
        let p = ExecRequestPayload {
            command: "echo hi".into(), shell: Some("/bin/bash".into()),
            env: Some([("X".into(), "1".into())].into_iter().collect()),
            dir: Some("/tmp".into()), timeout_sec: 10, request_id: "r1".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: ExecRequestPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.command, "echo hi");
        assert_eq!(back.env.as_ref().unwrap().get("X").unwrap(), "1");
    }

    #[test]
    fn test_exec_done_payload_roundtrip() {
        let p = ExecDonePayload { request_id: "r1".into(), exit_code: 0, duration_ms: 100, error: None };
        let json = serde_json::to_string(&p).unwrap();
        let back: ExecDonePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.exit_code, 0);
    }

    #[test]
    fn test_approval_request_payload_roundtrip() {
        let p = ApprovalRequestPayload {
            request_id: "r1".into(), command: "rm -rf /".into(),
            risk: "high".into(), mode: "hard_ask".into(), timestamp: 1234,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: ApprovalRequestPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.risk, "high");
    }

    #[test]
    fn test_error_payload_roundtrip() {
        let p = ErrorPayload { code: "EXEC_FAILED".into(), message: "boom".into() };
        let json = serde_json::to_string(&p).unwrap();
        let back: ErrorPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.code, "EXEC_FAILED");
    }

    #[test]
    fn test_payload_with_complex_nested_json() {
        let payload = serde_json::json!({
            "nested": {"a": [1, 2, 3], "b": true},
            "map": {"key": "value"}
        });
        let msg = Message::new(MessageType::ConfigUpdate).with_payload(&payload);
        let json = serde_json::to_string(&msg).unwrap();
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(back.payload.as_ref().unwrap()["nested"]["a"][0], 1);
    }

    #[test]
    fn test_message_roundtrip_through_json() {
        let msg = Message::new(MessageType::LlmRequest)
            .with_agent_id("ai-1")
            .with_payload(serde_json::json!({"model": "gpt-4", "messages": []}));
        let json = serde_json::to_string(&msg).unwrap();
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(back.msg_type, MessageType::LlmRequest);
        assert_eq!(back.agent_id.as_deref(), Some("ai-1"));
        assert_eq!(back.payload.as_ref().unwrap()["model"], "gpt-4");
    }

    #[test]
    fn test_message_serializes_type_as_snake_case() {
        let msg = Message::new(MessageType::ExecDone);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"exec_done\""));
    }

    #[test]
    fn test_message_optional_fields_skip() {
        let msg = Message::new(MessageType::Connect);
        let json = serde_json::to_string(&msg).unwrap();
        assert!(!json.contains("agent_id"));
        assert!(!json.contains("payload"));
        assert!(!json.contains("error"));
    }

    #[test]
    fn test_request_id_generates_uuid() {
        let id = request_id();
        assert!(uuid::Uuid::parse_str(&id).is_ok());
    }

    #[test]
    fn test_file_write_payload_roundtrip() {
        let p = FileWritePayload { path: "/a/b".into(), content: "data".into(), encoding: "utf-8".into(), mode: Some(0o644) };
        let json = serde_json::to_string(&p).unwrap();
        let back: FileWritePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.mode, Some(0o644));
    }

    #[test]
    fn test_connected_payload_roundtrip() {
        let p = ConnectedPayload {
            agent_id: "a1".into(), relay_id: "relay1".into(),
            heartbeat_interval_sec: 30, server_time: 1000,
            relay_public_key: Some("pk".into()), relay_key_id: Some("kid".into()),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: ConnectedPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back.relay_id, "relay1");
    }

    #[test]
    fn test_encrypted_data_serialization() {
        let ed = EncryptedData {
            key_id: "k1".into(), sender_key_id: Some("k2".into()),
            nonce: Some("n".into()), ciphertext: "ct".into(),
            ephemeral_pub_key: None,
        };
        let json = serde_json::to_string(&ed).unwrap();
        let back: EncryptedData = serde_json::from_str(&json).unwrap();
        assert_eq!(back.key_id, "k1");
        assert!(back.ephemeral_pub_key.is_none());
    }

    #[test]
    fn test_protocol_version() {
        assert_eq!(PROTOCOL_VERSION, 1);
    }
}

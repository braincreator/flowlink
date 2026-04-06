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

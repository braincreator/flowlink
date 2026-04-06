// FlowLink Core — Configuration types
// Port of internal/config/config.go

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

// ═══════════════════════════════════════════════
// Agent Configuration
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub agent_id: String,
    pub token: String,
    pub relay_url: String,
    #[serde(default = "default_heartbeat")]
    pub heartbeat_sec: u32,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub work_dir: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub use_relay_llm: bool,

    #[serde(default)]
    pub sandbox: SandboxConfig,
    #[serde(default)]
    pub approval: ApprovalConfig,
    #[serde(default)]
    pub backup: BackupConfig,
    #[serde(default)]
    pub shield: ShieldConfig,
    #[serde(default)]
    pub tls: TlsConfig,
}

fn default_heartbeat() -> u32 { 30 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    #[serde(default)]
    pub allowed_dirs: Vec<String>,
    #[serde(default)]
    pub blocked_patterns: Vec<String>,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    #[serde(default = "default_max_exec_timeout")]
    pub max_exec_timeout: u32,
    #[serde(default)]
    pub allow_sudo: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            allowed_dirs: vec![],
            blocked_patterns: vec![],
            max_file_size: default_max_file_size(),
            max_exec_timeout: default_max_exec_timeout(),
            allow_sudo: false,
        }
    }
}

fn default_max_file_size() -> u64 { 100 * 1024 * 1024 } // 100MB
fn default_max_exec_timeout() -> u32 { 300 } // 5 min

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalConfig {
    /// "auto" | "soft_ask" | "hard_ask"
    #[serde(default = "default_approval_mode")]
    pub mode: String,
    #[serde(default = "default_true")]
    pub soft_ask_notify: bool,
    #[serde(default = "default_hard_ask_timeout")]
    pub hard_ask_timeout_sec: u32,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

impl Default for ApprovalConfig {
    fn default() -> Self {
        Self {
            mode: default_approval_mode(),
            soft_ask_notify: true,
            hard_ask_timeout_sec: default_hard_ask_timeout(),
            max_retries: default_max_retries(),
        }
    }
}

fn default_approval_mode() -> String { "auto".into() }
fn default_hard_ask_timeout() -> u32 { 3600 }
fn default_max_retries() -> u32 { 3 }
fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_snapshots")]
    pub max_snapshots: u32,
    #[serde(default = "default_max_total_size")]
    pub max_total_size: u64,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_backup_dir")]
    pub backup_dir: String,
    #[serde(default)]
    pub compression: String, // "gzip" | "zstd" | "none"
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_snapshots: default_max_snapshots(),
            max_total_size: default_max_total_size(),
            retention_days: default_retention_days(),
            backup_dir: default_backup_dir(),
            compression: "gzip".into(),
        }
    }
}

fn default_max_snapshots() -> u32 { 50 }
fn default_max_total_size() -> u64 { 5 * 1024 * 1024 * 1024 } // 5GB
fn default_retention_days() -> u32 { 7 }
fn default_backup_dir() -> String { "~/.flowlink/backups".into() }

// ═══════════════════════════════════════════════
// Shield Configuration (NEW in v2)
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub enable_ast: bool,
    #[serde(default = "default_true")]
    pub enable_interpreter: bool,
    #[serde(default)]
    pub rules_path: Option<String>,
    #[serde(default)]
    pub snapshot_dataset: Option<String>,
    #[serde(default = "default_audit_log")]
    pub audit_log: String,
    #[serde(default)]
    pub webhook_url: Option<String>,
    /// Auto-deny after timeout (seconds)
    #[serde(default = "default_shield_timeout")]
    pub auto_deny_timeout: u32,
}

impl Default for ShieldConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            enable_ast: true,
            enable_interpreter: true,
            rules_path: None,
            snapshot_dataset: None,
            audit_log: default_audit_log(),
            webhook_url: None,
            auto_deny_timeout: default_shield_timeout(),
        }
    }
}

fn default_audit_log() -> String { "/var/log/flowlink-shield.jsonl".into() }
fn default_shield_timeout() -> u32 { 60 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct TlsConfig {
    #[serde(default)]
    pub insecure: bool,
    #[serde(default)]
    pub cert_pinning: bool,
    #[serde(default)]
    pub ca_cert: Option<String>,
}


// ═══════════════════════════════════════════════
// Relay Configuration
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    #[serde(default = "default_relay_name")]
    pub client_name: String,
    #[serde(default)]
    pub client_email: String,
    pub api_token: String,
    #[serde(default = "default_wss_addr")]
    pub wss_addr: SocketAddr,
    #[serde(default = "default_http_addr")]
    pub http_addr: SocketAddr,
    #[serde(default)]
    pub tls: TlsConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub billing: BillingConfig,
    #[serde(default)]
    pub registry: RegistryConfig,
}

fn default_relay_name() -> String { "FlowLink Relay".into() }
fn default_wss_addr() -> SocketAddr { "0.0.0.0:8443".parse().unwrap() }
fn default_http_addr() -> SocketAddr { "0.0.0.0:8080".parse().unwrap() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub backends: Vec<LlmBackend>,
    #[serde(default = "default_llm_timeout")]
    pub timeout_sec: u32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self { enabled: false, backends: vec![], timeout_sec: default_llm_timeout() }
    }
}

fn default_llm_timeout() -> u32 { 30 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmBackend {
    pub name: String,
    pub provider: String, // "openai" | "anthropic" | "ollama"
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub plans: Vec<PlanConfig>,
}

impl Default for BillingConfig {
    fn default() -> Self {
        Self { enabled: false, currency: "RUB".into(), plans: vec![] }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanConfig {
    pub id: String,
    pub name: String,
    pub price: u64, // in cents/kopecks
    pub period: String, // "monthly" | "yearly"
    #[serde(default)]
    pub features: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    #[serde(default = "default_registry_path")]
    pub data_path: String,
    #[serde(default)]
    pub max_agents: u32,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self { data_path: default_registry_path(), max_agents: 100 }
    }
}

fn default_registry_path() -> String { "~/.flowlink/relay".into() }

// ═══════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════

impl AgentConfig {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &str) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl RelayConfig {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }
}

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
    #[serde(default)]
    pub database: DatabaseConfig,
}

/// Database connection configuration.
/// Supports primary/replica topology for read scalability.
/// - `primary`: required for writes and migrations (direct connection)
/// - `replicas`: optional, one or more read replicas (load-balanced round-robin)
/// - On dev: single local PostgreSQL
/// - On VPS: single container PostgreSQL
/// - In prod: managed service (Neon, Supabase Cloud, RDS) with read replicas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Primary connection string (write + read fallback + migrations)
    #[serde(default)]
    pub primary: Option<String>,
    /// Read replica connection strings (optional, for read scaling)
    #[serde(default)]
    pub replicas: Vec<String>,
    /// Max connections in the pool
    #[serde(default = "default_db_pool_size")]
    pub pool_size: u32,
    /// Run migrations on startup
    #[serde(default = "default_true")]
    pub migrate_on_start: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            primary: None,
            replicas: vec![],
            pool_size: default_db_pool_size(),
            migrate_on_start: true,
        }
    }
}

fn default_db_pool_size() -> u32 { 10 }

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
    /// JWT-токен для авторизации в API Точка Банка
    #[serde(default)]
    pub tochka_jwt_token: Option<String>,
}

impl Default for BillingConfig {
    fn default() -> Self {
        Self { enabled: false, currency: "RUB".into(), plans: vec![], tochka_jwt_token: None }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_agent_config() -> AgentConfig {
        AgentConfig {
            agent_id: "test-agent".into(),
            token: "tok123".into(),
            relay_url: "wss://relay.example.com".into(),
            heartbeat_sec: 30,
            label: "".into(),
            work_dir: "/tmp".into(),
            read_only: false,
            use_relay_llm: false,
            sandbox: SandboxConfig::default(),
            approval: ApprovalConfig::default(),
            backup: BackupConfig::default(),
            shield: ShieldConfig::default(),
            tls: TlsConfig::default(),
        }
    }

    #[test]
    fn test_sandbox_defaults() {
        let sb = SandboxConfig::default();
        assert_eq!(sb.max_file_size, 100 * 1024 * 1024);
        assert_eq!(sb.max_exec_timeout, 300);
        assert!(!sb.allow_sudo);
        assert!(sb.allowed_dirs.is_empty());
    }

    #[test]
    fn test_approval_defaults() {
        let ap = ApprovalConfig::default();
        assert_eq!(ap.mode, "auto");
        assert!(ap.soft_ask_notify);
        assert_eq!(ap.hard_ask_timeout_sec, 3600);
        assert_eq!(ap.max_retries, 3);
    }

    #[test]
    fn test_tls_defaults() {
        let tls = TlsConfig::default();
        assert!(!tls.insecure);
        assert!(!tls.cert_pinning);
        assert!(tls.ca_cert.is_none());
    }

    #[test]
    fn test_agent_config_serialize_deserialize_roundtrip() {
        let cfg = sample_agent_config();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.agent_id, "test-agent");
        assert_eq!(back.token, "tok123");
        assert_eq!(back.heartbeat_sec, 30);
    }

    #[test]
    fn test_agent_config_load_from_json() {
        let json = r#"{"agent_id":"a1","token":"t1","relay_url":"wss://r.com"}"#;
        let cfg: AgentConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.agent_id, "a1");
        assert_eq!(cfg.heartbeat_sec, 30); // default
    }

    #[test]
    fn test_agent_config_save_load_roundtrip() {
        let cfg = sample_agent_config();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        cfg.save(path.to_str().unwrap()).unwrap();
        let loaded = AgentConfig::load(path.to_str().unwrap()).unwrap();
        assert_eq!(loaded.agent_id, cfg.agent_id);
        assert_eq!(loaded.token, cfg.token);
    }

    #[test]
    fn test_relay_config_defaults() {
        let json = r#"{"api_token":"tok"}"#;
        let cfg: RelayConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.http_addr.to_string(), "0.0.0.0:8080");
        assert_eq!(cfg.wss_addr.to_string(), "0.0.0.0:8443");
        assert_eq!(cfg.client_name, "FlowLink Relay");
    }

    #[test]
    fn test_relay_config_serialize_deserialize() {
        let cfg = RelayConfig {
            client_name: "Test".into(),
            client_email: "".into(),
            api_token: "tok".into(),
            wss_addr: "0.0.0.0:9443".parse().unwrap(),
            http_addr: "0.0.0.0:9080".parse().unwrap(),
            tls: TlsConfig::default(),
            llm: LlmConfig::default(),
            billing: BillingConfig::default(),
            registry: RegistryConfig::default(),
            database: DatabaseConfig::default(),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: RelayConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.api_token, "tok");
        assert_eq!(back.wss_addr.to_string(), "0.0.0.0:9443");
        assert!(back.database.primary.is_none());
        assert!(back.database.replicas.is_empty());
    }

    #[test]
    fn test_tls_serialization() {
        let tls = TlsConfig { insecure: true, cert_pinning: true, ca_cert: Some("/path".into()) };
        let json = serde_json::to_string(&tls).unwrap();
        let back: TlsConfig = serde_json::from_str(&json).unwrap();
        assert!(back.insecure);
        assert!(back.cert_pinning);
        assert_eq!(back.ca_cert.as_deref(), Some("/path"));
    }

    #[test]
    fn test_llm_config_with_backends() {
        let llm = LlmConfig {
            enabled: true,
            backends: vec![
                LlmBackend { name: "gpt4".into(), provider: "openai".into(), model: "gpt-4".into(), api_key: Some("k".into()), base_url: None },
            ],
            timeout_sec: 60,
        };
        let json = serde_json::to_string(&llm).unwrap();
        let back: LlmConfig = serde_json::from_str(&json).unwrap();
        assert!(back.enabled);
        assert_eq!(back.backends.len(), 1);
        assert_eq!(back.backends[0].provider, "openai");
    }

    #[test]
    fn test_billing_config_with_plans() {
        let billing = BillingConfig {
            enabled: true,
            currency: "USD".into(),
            plans: vec![PlanConfig {
                id: "pro".into(), name: "Pro".into(), price: 999,
                period: "monthly".into(), features: Default::default(),
            }],
        };
        let json = serde_json::to_string(&billing).unwrap();
        let back: BillingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.plans[0].price, 999);
    }

    #[test]
    fn test_registry_defaults() {
        let reg = RegistryConfig::default();
        assert_eq!(reg.data_path, "~/.flowlink/relay");
        assert_eq!(reg.max_agents, 100);
    }

    #[test]
    fn test_backup_defaults() {
        let b = BackupConfig::default();
        assert!(b.enabled);
        assert_eq!(b.compression, "gzip");
        assert_eq!(b.max_snapshots, 50);
    }

    #[test]
    fn test_shield_defaults() {
        let s = ShieldConfig::default();
        assert!(!s.enabled);
        assert!(s.enable_ast);
        assert_eq!(s.auto_deny_timeout, 60);
    }

    #[test]
    fn test_invalid_json_errors() {
        let result: Result<AgentConfig, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }


    #[test]
    fn test_agent_config_defaults() {
        let json = r#"{"agent_id":"a","token":"t","relay_url":"wss://r"}"#;
        let cfg: AgentConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.heartbeat_sec, 30);
        assert!(cfg.label.is_empty());
        assert!(cfg.work_dir.is_empty());
        assert!(!cfg.read_only);
        assert!(!cfg.use_relay_llm);
        assert!(!cfg.sandbox.allow_sudo);
        assert!(cfg.sandbox.allowed_dirs.is_empty());
        assert_eq!(cfg.approval.mode, "auto");
        assert!(cfg.backup.enabled);
        assert!(!cfg.shield.enabled);
        assert!(!cfg.tls.insecure);
    }

    #[test]
    fn test_agent_config_from_yaml() {
        let yaml = "agent_id: a1\ntoken: t1\nrelay_url: wss://r\"";
        let cfg: AgentConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.agent_id, "a1");
        assert_eq!(cfg.heartbeat_sec, 30);
    }

    #[test]
    fn test_agent_config_partial_overrides() {
        let json = r#"{"agent_id":"a","token":"t","relay_url":"wss://r","heartbeat_sec":60,"read_only":true,"use_relay_llm":true}"#;
        let cfg: AgentConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.heartbeat_sec, 60);
        assert!(cfg.read_only);
        assert!(cfg.use_relay_llm);
        assert_eq!(cfg.approval.mode, "auto");
    }

    #[test]
    fn test_relay_config_all_defaults() {
        let json = r#"{"api_token":"t"}"#;
        let cfg: RelayConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.client_name, "FlowLink Relay");
        assert!(cfg.client_email.is_empty());
        assert!(!cfg.llm.enabled);
        assert!(cfg.llm.backends.is_empty());
        assert!(!cfg.billing.enabled);
        assert_eq!(cfg.billing.currency, "RUB");
        assert_eq!(cfg.registry.data_path, "~/.flowlink/relay");
        assert_eq!(cfg.registry.max_agents, 100);
    }

    #[test]
    fn test_relay_config_from_yaml() {
        let yaml = "api_token: secret\nclient_name: MyRelay\nllm:\n  enabled: true\n  backends:\n    - name: gpt4\n      provider: openai\n      model: gpt-4\n      api_key: k\n      base_url: null";
        let cfg: RelayConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.client_name, "MyRelay");
        assert!(cfg.llm.enabled);
        assert_eq!(cfg.llm.backends.len(), 1);
    }

    #[test]
    fn test_config_all_llm_backends() {
        let llm = LlmConfig {
            enabled: true,
            backends: vec![
                LlmBackend { name: "gpt4".into(), provider: "openai".into(), model: "gpt-4".into(), api_key: Some("k1".into()), base_url: None },
                LlmBackend { name: "claude".into(), provider: "anthropic".into(), model: "claude-3".into(), api_key: Some("k2".into()), base_url: Some("https://api.anthropic.com".into()) },
                LlmBackend { name: "local".into(), provider: "ollama".into(), model: "llama3".into(), api_key: None, base_url: Some("http://localhost:11434".into()) },
            ],
            timeout_sec: 120,
        };
        let json = serde_json::to_string(&llm).unwrap();
        let back: LlmConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.backends.len(), 3);
        assert_eq!(back.backends[2].provider, "ollama");
    }

    #[test]
    fn test_llm_config_zero_backends_default() {
        let json = r#"{}"#;
        let cfg: LlmConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.enabled);
        assert!(cfg.backends.is_empty());
        assert_eq!(cfg.timeout_sec, 30);
    }

    #[test]
    fn test_invalid_relay_config_missing_token() {
        let json = r#"{"client_name":"x"}"#;
        let result: Result<RelayConfig, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_sandbox_config_custom() {
        let json = r#"{"allowed_dirs":["/home"],"blocked_patterns":["/etc/*"],"max_file_size":1024,"max_exec_timeout":60,"allow_sudo":true}"#;
        let sb: SandboxConfig = serde_json::from_str(json).unwrap();
        assert_eq!(sb.allowed_dirs, vec!["/home"]);
        assert_eq!(sb.max_file_size, 1024);
        assert!(sb.allow_sudo);
    }

    #[test]
    fn test_shield_config_full() {
        let s = ShieldConfig {
            enabled: true, enable_ast: false, enable_interpreter: false,
            rules_path: Some("/rules.yaml".into()),
            snapshot_dataset: Some("dataset".into()),
            audit_log: "/var/log/shield.log".into(),
            webhook_url: Some("https://hooks.example.com".into()),
            auto_deny_timeout: 120,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: ShieldConfig = serde_json::from_str(&json).unwrap();
        assert!(back.enabled);
        assert!(!back.enable_ast);
        assert_eq!(back.auto_deny_timeout, 120);
    }

    #[test]
    fn test_plan_config_features() {
        let plan = PlanConfig {
            id: "pro".into(), name: "Pro".into(), price: 999, period: "monthly".into(),
            features: [("max_agents".into(), serde_json::json!(10))].into_iter().collect(),
        };
        let json = serde_json::to_string(&plan).unwrap();
        let back: PlanConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.features["max_agents"], 10);
    }

    #[test]
    fn test_backup_config_full() {
        let b = BackupConfig {
            enabled: true, max_snapshots: 10, max_total_size: 1024,
            retention_days: 30, backup_dir: "/backup".into(), compression: "zstd".into(),
        };
        let json = serde_json::to_string(&b).unwrap();
        let back: BackupConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.compression, "zstd");
        assert_eq!(back.retention_days, 30);
    }
}
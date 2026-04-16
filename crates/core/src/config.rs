// FlowLink Core — Configuration types
// Port of internal/config/config.go

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;

// OAuth configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub vk: VkConfig,
    pub yandex: YandexConfig,
    pub github: GithubConfig,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            vk: VkConfig::default(),
            yandex: YandexConfig::default(),
            github: GithubConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VkConfig {
    pub app_id: String,
    pub app_secret: String,
    pub service_token: String,
    pub oauth_endpoint: String,
}

impl Default for VkConfig {
    fn default() -> Self {
        Self {
            app_id: "mock_vk_app_id".to_string(),
            app_secret: "mock_vk_app_secret".to_string(),
            service_token: "mock_vk_service_token".to_string(),
            oauth_endpoint: "https://id.vk.ru/oauth2/auth".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YandexConfig {
    pub client_id: String,
    pub client_secret: String,
    pub oauth_endpoint: String,
}

impl Default for YandexConfig {
    fn default() -> Self {
        Self {
            client_id: "mock_yandex_client_id".to_string(),
            client_secret: "mock_yandex_client_secret".to_string(),
            oauth_endpoint: "https://oauth.yandex.ru/authorize".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubConfig {
    pub client_id: String,
    pub client_secret: String,
    pub oauth_endpoint: String,
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self {
            client_id: "mock_github_client_id".to_string(),
            client_secret: "mock_github_client_secret".to_string(),
            oauth_endpoint: "https://github.com/login/oauth/authorize".to_string(),
        }
    }
}

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

impl AgentConfig {
    pub fn vk_oauth_endpoint(&self) -> String {
        "https://id.vk.ru/oauth2/auth".to_string()
    }
    
    pub fn vk_service_token(&self) -> String {
        "mock_service_token".to_string()
    }
    
    pub fn vk_app_id(&self) -> String {
        "mock_app_id".to_string()
    }
    
    pub fn vk_app_secret(&self) -> String {
        "mock_app_secret".to_string()
    }
}

fn default_heartbeat() -> u32 {
    30
}

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

fn default_max_file_size() -> u64 {
    100 * 1024 * 1024
} // 100MB
fn default_max_exec_timeout() -> u32 {
    300
} // 5 min

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

fn default_approval_mode() -> String {
    "auto".into()
}
fn default_hard_ask_timeout() -> u32 {
    3600
}
fn default_max_retries() -> u32 {
    3
}
fn default_true() -> bool {
    true
}

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

fn default_max_snapshots() -> u32 {
    50
}
fn default_max_total_size() -> u64 {
    5 * 1024 * 1024 * 1024
} // 5GB
fn default_retention_days() -> u32 {
    7
}
fn default_backup_dir() -> String {
    "~/.flowlink/backups".into()
}

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

fn default_audit_log() -> String {
    "/var/log/flowlink-shield.jsonl".into()
}
fn default_shield_timeout() -> u32 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    /// WSS address — always started at relay boot.
    /// TLS cert+key must be configured via wss_tls (required).
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
    /// WSS TLS — when cert_path and key_path are set, starts a separate
    /// TLS listener on wss_addr for direct agent WebSocket connections.
    #[serde(default)]
    pub wss_tls: WssTlsConfig,
    /// Telegram bot token — when set, starts the TG bot.
    #[serde(default)]
    pub tg_bot_token: Option<String>,
    /// Telegram webhook URL (e.g. https://example.com/api/tg/webhook).
    /// When set, bot runs in webhook mode; otherwise polling.
    #[serde(default)]
    pub tg_webhook_url: Option<String>,
    /// SMTP configuration for transactional emails
    #[serde(default)]
    pub smtp: SmtpConfig,
    /// Auth configuration (JWT + OAuth)
    #[serde(default)]
    pub auth: AuthConfig,
    /// OAuth providers configuration
    #[serde(default)]
    pub oauth: OAuthConfig,
    /// Public URL for dashboard (used in OAuth redirects). Defaults to http://{http_addr}
    #[serde(default)]
    pub dashboard_url: Option<String>,
}

/// SMTP configuration for transactional emails
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    #[serde(default = "default_smtp_host")]
    pub host: String,
    #[serde(default = "default_smtp_port")]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_smtp_from")]
    pub from: String,
}

impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            host: default_smtp_host(),
            port: default_smtp_port(),
            username: String::new(),
            password: String::new(),
            from: default_smtp_from(),
        }
    }
}

fn default_smtp_host() -> String {
    "mail.flow-masters.ru".into()
}
fn default_smtp_port() -> u16 {
    587
}
fn default_smtp_from() -> String {
    "noreply@flowlink.flow-masters.ru".into()
}

/// Auth configuration for JWT and OAuth providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// JWT secret key (HS256)
    #[serde(default)]
    pub jwt_secret: String,
    /// Access token TTL in minutes
    #[serde(default = "default_access_ttl")]
    pub access_token_ttl_min: i64,
    /// Refresh token TTL in days
    #[serde(default = "default_refresh_ttl")]
    pub refresh_token_ttl_days: i64,
    /// VK OAuth config
    #[serde(default)]
    pub vk: Option<OAuthProviderConfig>,
    /// Yandex OAuth config
    #[serde(default)]
    pub yandex: Option<OAuthProviderConfig>,
    /// GitHub OAuth config
    #[serde(default)]
    pub github: Option<OAuthProviderConfig>,
}

/// OAuth provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProviderConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

fn default_access_ttl() -> i64 {
    15
}
fn default_refresh_ttl() -> i64 {
    30
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: String::new(),
            access_token_ttl_min: default_access_ttl(),
            refresh_token_ttl_days: default_refresh_ttl(),
            vk: None,
            yandex: None,
            github: None,
        }
    }
}

/// WSS TLS configuration for the relay's WebSocket listener.
/// When set, the relay starts a separate TLS listener on `wss_addr`.
/// Agents connect via `wss://` directly (bypassing nginx).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WssTlsConfig {
    /// Path to the TLS certificate (PEM).
    #[serde(default)]
    pub cert_path: Option<String>,
    /// Path to the TLS private key (PEM).
    #[serde(default)]
    pub key_path: Option<String>,
}

impl Default for WssTlsConfig {
    fn default() -> Self {
        Self {
            cert_path: None,
            key_path: None,
        }
    }
}

impl WssTlsConfig {
    /// Returns true if both cert and key paths are set (WSS is enabled).
    pub fn is_enabled(&self) -> bool {
        self.cert_path.is_some() && self.key_path.is_some()
    }
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

fn default_db_pool_size() -> u32 {
    10
}

fn default_relay_name() -> String {
    "FlowLink Relay".into()
}
fn default_wss_addr() -> SocketAddr {
    "0.0.0.0:8443".parse().unwrap()
}
fn default_http_addr() -> SocketAddr {
    "0.0.0.0:8080".parse().unwrap()
}

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
        Self {
            enabled: false,
            backends: vec![],
            timeout_sec: default_llm_timeout(),
        }
    }
}

fn default_llm_timeout() -> u32 {
    30
}

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
    /// Client ID (customer_code) в Точка Банка
    #[serde(default)]
    pub tochka_client_id: Option<String>,
    /// Secret key для HMAC-верификации вебхуков
    #[serde(default)]
    pub tochka_webhook_secret: Option<String>,
}

impl Default for BillingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            currency: "RUB".into(),
            plans: vec![],
            tochka_jwt_token: None,
            tochka_client_id: None,
            tochka_webhook_secret: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanConfig {
    pub id: String,
    pub name: String,
    pub price: u64,     // in cents/kopecks
    pub period: String, // "monthly" | "yearly"
    #[serde(default)]
    pub features: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    #[serde(default = "default_registry_path")]
    pub data_path: String,
    #[serde(default)]
    pub max_hosts: u32,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            data_path: default_registry_path(),
            max_hosts: 100,
        }
    }
}

fn default_registry_path() -> String {
    "~/.flowlink/relay".into()
}

// ═══════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════

impl AgentConfig {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        // Try flat format first
        if let Ok(config) = serde_json::from_str::<Self>(&content) {
            return Ok(config);
        }
        // Try nested format (config.json with "agent_config" key)
        let value: serde_json::Value = serde_json::from_str(&content)?;
        if let Some(agent) = value.get("agent_config") {
            let config: Self = serde_json::from_value(agent.clone())?;
            return Ok(config);
        }
        anyhow::bail!("Cannot parse AgentConfig: file has neither flat AgentConfig fields nor an 'agent_config' key")
    }

    pub fn save(&self, path: &str) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl RelayConfig {
    /// Apply Vault secrets to config fields.
    ///
    /// Reads secrets from Vault KV v2 mount at `flowlink/`.
    /// If Vault is unavailable, silently skips (non-blocking).
    /// Requires `vault` feature and these env vars:
    /// - `VAULT_ADDR` (default: https://127.0.0.1:8200)
    /// - `VAULT_TOKEN` or `VAULT_ROLE_ID`+`VAULT_SECRET_ID`
    /// - `VAULT_SKIP_VERIFY=true` for self-signed certs
    #[cfg(feature = "vault")]
    pub async fn apply_vault_overrides(&mut self) {
        let mut client = super::vault::VaultClient::from_env();
        let mut loaded = 0usize;

        macro_rules! vault_set {
            ($path:expr, $field:expr) => {
                if let Ok(val) = client.read_secret($path).await {
                    if !val.is_empty() {
                        $field = val;
                        loaded += 1;
                    }
                }
            };
            ($path:expr, $field:expr, Some) => {
                if let Ok(val) = client.read_secret($path).await {
                    if !val.is_empty() {
                        $field = Some(val);
                        loaded += 1;
                    }
                }
            };
        }

        vault_set!("api_token", self.api_token);
        vault_set!("tg_bot_token", self.tg_bot_token, Some);
        vault_set!("tg_webhook_url", self.tg_webhook_url, Some);
        vault_set!("auth/jwt_secret", self.auth.jwt_secret);
        vault_set!("auth/vk/client_secret", self.oauth.vk.app_secret);
        vault_set!("auth/yandex/client_secret", self.oauth.yandex.client_secret);
        vault_set!("auth/github/client_secret", self.oauth.github.client_secret);
        vault_set!("billing/tochka_jwt_token", self.billing.tochka_jwt_token, Some);
        vault_set!("billing/tochka_webhook_secret", self.billing.tochka_webhook_secret, Some);
        vault_set!("smtp/password", self.smtp.password);

        // LLM backends — match by provider name
        for backend in &mut self.llm.backends {
            if let Ok(key) = client.read_secret(&format!("llm/{}/api_key", backend.name)).await {
                if !key.is_empty() {
                    backend.api_key = Some(key);
                    loaded += 1;
                }
            }
        }

        if loaded > 0 {
            println!("[vault] Loaded {loaded} secrets from Vault");
        } else {
            println!("[vault] Connected but no secrets loaded (config file values used)");
        }
    }

    #[cfg(not(feature = "vault"))]
    pub async fn apply_vault_overrides(&mut self) {
        // No-op when vault feature is disabled
    }

    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        // Try flat format first (relay.json)
        if let Ok(config) = serde_json::from_str::<Self>(&content) {
            return Ok(config);
        }
        // Try nested format (config.json with "relay_config" key)
        let value: serde_json::Value = serde_json::from_str(&content)?;
        if let Some(relay) = value.get("relay_config") {
            let config: Self = serde_json::from_value(relay.clone())?;
            return Ok(config);
        }
        anyhow::bail!("Cannot parse RelayConfig: file has neither flat RelayConfig fields nor a 'relay_config' key")
    }

    /// Apply environment variable overrides.
    ///
    /// Environment variables take precedence over config file values.
    /// Prefix: `FLOWLINK_` — e.g. `FLOWLINK_API_TOKEN`, `FLOWLINK_JWT_SECRET`.
    /// Nested fields use `__` as separator: `FLOWLINK_SMTP__HOST`, `FLOWLINK_AUTH__JWT_SECRET`.
    ///
    /// Call this after `load()` to allow Docker/K8s/env-based secrets.
    pub fn apply_env_overrides(&mut self) {
        // ── Top-level fields ──
        if let Ok(v) = std::env::var("FLOWLINK_API_TOKEN") { self.api_token = v; }
        if let Ok(v) = std::env::var("FLOWLINK_CLIENT_NAME") { self.client_name = v; }
        if let Ok(v) = std::env::var("FLOWLINK_CLIENT_EMAIL") { self.client_email = v; }
        if let Ok(v) = std::env::var("FLOWLINK_HTTP_ADDR") {
            if let Ok(addr) = v.parse() { self.http_addr = addr; }
        }
        if let Ok(v) = std::env::var("FLOWLINK_WSS_ADDR") {
            if let Ok(addr) = v.parse() { self.wss_addr = addr; }
        }
        if let Ok(v) = std::env::var("FLOWLINK_TG_BOT_TOKEN") { self.tg_bot_token = Some(v); }
        if let Ok(v) = std::env::var("FLOWLINK_TG_WEBHOOK_URL") { self.tg_webhook_url = Some(v); }

        // ── SMTP ──
        if let Ok(v) = std::env::var("FLOWLINK_SMTP__HOST") { self.smtp.host = v; }
        if let Ok(v) = std::env::var("FLOWLINK_SMTP__PORT") {
            if let Ok(p) = v.parse() { self.smtp.port = p; }
        }
        if let Ok(v) = std::env::var("FLOWLINK_SMTP__USERNAME") { self.smtp.username = v; }
        if let Ok(v) = std::env::var("FLOWLINK_SMTP__PASSWORD") { self.smtp.password = v; }
        if let Ok(v) = std::env::var("FLOWLINK_SMTP__FROM") { self.smtp.from = v; }

        // ── Auth ──
        if let Ok(v) = std::env::var("FLOWLINK_AUTH__JWT_SECRET") { self.auth.jwt_secret = v; }
        if let Ok(v) = std::env::var("FLOWLINK_JWT_SECRET") { self.auth.jwt_secret = v; }

        // ── Database ──
        if let Ok(v) = std::env::var("FLOWLINK_DATABASE__URL") { self.database.primary = Some(v); }
        if let Ok(v) = std::env::var("FLOWLINK_DATABASE_URL") { self.database.primary = Some(v); }
        if let Ok(v) = std::env::var("FLOWLINK_DATABASE__POOL_SIZE") {
            if let Ok(n) = v.parse() { self.database.pool_size = n; }
        }

        // ── OAuth providers ──
        if let Ok(v) = std::env::var("FLOWLINK_OAUTH__VK__APP_ID") { self.oauth.vk.app_id = v; }
        if let Ok(v) = std::env::var("FLOWLINK_OAUTH__VK__APP_SECRET") { self.oauth.vk.app_secret = v; }
        if let Ok(v) = std::env::var("FLOWLINK_OAUTH__VK__SERVICE_TOKEN") { self.oauth.vk.service_token = v; }
        if let Ok(v) = std::env::var("FLOWLINK_OAUTH__YANDEX__CLIENT_ID") { self.oauth.yandex.client_id = v; }
        if let Ok(v) = std::env::var("FLOWLINK_OAUTH__YANDEX__CLIENT_SECRET") { self.oauth.yandex.client_secret = v; }
        if let Ok(v) = std::env::var("FLOWLINK_OAUTH__GITHUB__CLIENT_ID") { self.oauth.github.client_id = v; }
        if let Ok(v) = std::env::var("FLOWLINK_OAUTH__GITHUB__CLIENT_SECRET") { self.oauth.github.client_secret = v; }

        // ── Billing ──
        if let Ok(v) = std::env::var("FLOWLINK_BILLING__TOCHKA_CLIENT_ID") {
            self.billing.tochka_client_id = Some(v);
        }
        if let Ok(v) = std::env::var("FLOWLINK_BILLING__TOCHKA_WEBHOOK_SECRET") {
            self.billing.tochka_webhook_secret = Some(v);
        }

        // ── TLS ──
        if let Ok(v) = std::env::var("FLOWLINK_WSS_TLS__CERT_PATH") { self.wss_tls.cert_path = Some(v); }
        if let Ok(v) = std::env::var("FLOWLINK_WSS_TLS__KEY_PATH") { self.wss_tls.key_path = Some(v); }
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
            wss_tls: WssTlsConfig::default(),
            tg_bot_token: None,
            tg_webhook_url: None,
            smtp: SmtpConfig::default(),
            auth: AuthConfig::default(),
            oauth: OAuthConfig::default(),
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
        let tls = TlsConfig {
            insecure: true,
            cert_pinning: true,
            ca_cert: Some("/path".into()),
        };
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
            backends: vec![LlmBackend {
                name: "gpt4".into(),
                provider: "openai".into(),
                model: "gpt-4".into(),
                api_key: Some("k".into()),
                base_url: None,
            }],
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
                id: "starter".into(),
                name: "Starter".into(),
                price: 2990,
                period: "monthly".into(),
                features: Default::default(),
            }],
            tochka_jwt_token: None,
            tochka_client_id: None,
            tochka_webhook_secret: None,
        };
        let json = serde_json::to_string(&billing).unwrap();
        let back: BillingConfig = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_registry_defaults() {
        let reg = RegistryConfig::default();
        assert_eq!(reg.data_path, "~/.flowlink/relay");
        assert_eq!(reg.max_hosts, 100);
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
        assert_eq!(cfg.registry.max_hosts, 100);
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
                LlmBackend {
                    name: "gpt4".into(),
                    provider: "openai".into(),
                    model: "gpt-4".into(),
                    api_key: Some("k1".into()),
                    base_url: None,
                },
                LlmBackend {
                    name: "claude".into(),
                    provider: "anthropic".into(),
                    model: "claude-3".into(),
                    api_key: Some("k2".into()),
                    base_url: Some("https://api.anthropic.com".into()),
                },
                LlmBackend {
                    name: "local".into(),
                    provider: "ollama".into(),
                    model: "llama3".into(),
                    api_key: None,
                    base_url: Some("http://localhost:11434".into()),
                },
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
            enabled: true,
            enable_ast: false,
            enable_interpreter: false,
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
            id: "starter".into(),
            name: "Starter".into(),
            price: 2990,
            period: "monthly".into(),
            features: [("max_hosts".into(), serde_json::json!(10))]
                .into_iter()
                .collect(),
        };
        let json = serde_json::to_string(&plan).unwrap();
        let back: PlanConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.features["max_hosts"], 10);
    }

    #[test]
    fn test_backup_config_full() {
        let b = BackupConfig {
            enabled: true,
            max_snapshots: 10,
            max_total_size: 1024,
            retention_days: 30,
            backup_dir: "/backup".into(),
            compression: "zstd".into(),
        };
        let json = serde_json::to_string(&b).unwrap();
        let back: BackupConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.compression, "zstd");
        assert_eq!(back.retention_days, 30);
    }
}

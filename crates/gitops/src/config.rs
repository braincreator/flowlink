use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GitOpsConfig {
    pub enabled: bool,
    
    pub git: GitConfig,
    pub state: StateConfig,
    pub backup: BackupConfig,
    pub vault: VaultConfig,
    pub drift: DriftConfig,
    pub tempo: RateLimitConfig,
    pub health: HealthConfig,
    pub approval: ApprovalConfig,
    pub audit: AuditConfig,
}

impl Default for GitOpsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            git: GitConfig::default(),
            state: StateConfig::default(),
            backup: BackupConfig::default(),
            vault: VaultConfig::default(),
            drift: DriftConfig::default(),
            tempo: RateLimitConfig::default(),
            health: HealthConfig::default(),
            approval: ApprovalConfig::default(),
            audit: AuditConfig::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GitConfig {
    pub repo_path: String,
    pub remote_url: Option<String>,
    pub branch: String,
    pub sync_strategy: SyncStrategy,
    pub signing_key: Option<String>,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            repo_path: "~/.flowlink/gitops/repo/".to_string(),
            remote_url: None,
            branch: "main".to_string(),
            sync_strategy: SyncStrategy::default(),
            signing_key: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum SyncStrategy {
    Realtime,
    Batched { interval_secs: u64, max_batch_size: usize },
    Scheduled { cron: String },
    Manual,
}

impl Default for SyncStrategy {
    fn default() -> Self {
        SyncStrategy::Batched {
            interval_secs: 30,
            max_batch_size: 50,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StateConfig {
    pub collect_interval_seconds: u64,
    pub tracked_paths: Vec<String>,
}

impl Default for StateConfig {
    fn default() -> Self {
        Self {
            collect_interval_seconds: 300,
            tracked_paths: vec![
                "/etc/nginx".to_string(),
                "/etc/docker".to_string(),
                "/etc/systemd".to_string(),
            ],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupConfig {
    pub auto_backup_destructive: bool,
    pub max_backup_size_mb: u64,
    pub retention: RetentionPolicy,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            auto_backup_destructive: true,
            max_backup_size_mb: 500,
            retention: RetentionPolicy::Days(30),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VaultConfig {
    pub path: String,
    pub permissions: u32,
    pub encryption: VaultEncryption,
    pub max_size_mb: u64,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            path: if cfg!(target_os = "macos") {
                "/usr/local/flowlink-vault/".to_string()
            } else {
                "/opt/flowlink-vault/".to_string()
            },
            permissions: 0o700,
            encryption: VaultEncryption::MachineKey,
            max_size_mb: 5000,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum VaultEncryption {
    MachineKey,
    ProvidedKey { key_id: String },
    None,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DriftConfig {
    pub enabled: bool,
    pub event_driven: bool,
    pub auto_fix: bool,
    pub rules_path: String,
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            event_driven: true,
            auto_fix: false,
            rules_path: "policies/drift_auto_fix.yaml".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub global_limit: GlobalRateLimit,
    pub per_tool_limits: std::collections::HashMap<String, ToolRateLimit>,
    pub per_tier_defaults: std::collections::HashMap<ActionTier, ToolRateLimit>,
    pub circuit_breaker: CircuitBreakerConfig,
    pub exponential_backoff: ExponentialBackoffConfig,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        let mut per_tool_limits = std::collections::HashMap::new();
        per_tool_limits.insert("rm".to_string(), ToolRateLimit {
            max_calls: 10,
            window_seconds: 60,
            on_exceed: ExceedAction::Deny,
        });
        per_tool_limits.insert("docker".to_string(), ToolRateLimit {
            max_calls: 20,
            window_seconds: 60,
            on_exceed: ExceedAction::Escalate,
        });
        per_tool_limits.insert("apt".to_string(), ToolRateLimit {
            max_calls: 5,
            window_seconds: 300,
            on_exceed: ExceedAction::Escalate,
        });
        per_tool_limits.insert("systemctl".to_string(), ToolRateLimit {
            max_calls: 15,
            window_seconds: 60,
            on_exceed: ExceedAction::Escalate,
        });
        per_tool_limits.insert("cat".to_string(), ToolRateLimit {
            max_calls: 200,
            window_seconds: 60,
            on_exceed: ExceedAction::ReadOnly,
        });

        let mut per_tier_defaults = std::collections::HashMap::new();
        per_tier_defaults.insert(ActionTier::ReadOnly, ToolRateLimit {
            max_calls: 200,
            window_seconds: 60,
            on_exceed: ExceedAction::ReadOnly,
        });
        per_tier_defaults.insert(ActionTier::Destructive, ToolRateLimit {
            max_calls: 30,
            window_seconds: 60,
            on_exceed: ExceedAction::Deny,
        });
        per_tier_defaults.insert(ActionTier::Network, ToolRateLimit {
            max_calls: 10,
            window_seconds: 60,
            on_exceed: ExceedAction::Escalate,
        });

        Self {
            enabled: true,
            global_limit: GlobalRateLimit::default(),
            per_tool_limits,
            per_tier_defaults,
            circuit_breaker: CircuitBreakerConfig::default(),
            exponential_backoff: ExponentialBackoffConfig::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GlobalRateLimit {
    pub max_calls: u32,
    pub window_seconds: u64,
    pub on_exceed: ExceedAction,
}

impl Default for GlobalRateLimit {
    fn default() -> Self {
        Self {
            max_calls: 300,
            window_seconds: 60,
            on_exceed: ExceedAction::ReadOnly,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CircuitBreakerConfig {
    pub enabled: bool,
    pub failure_threshold_percent: u32,
    pub window_seconds: u64,
    pub min_calls: u32,
    pub open_duration_seconds: u64,
    pub half_open_probes: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            failure_threshold_percent: 50,
            window_seconds: 60,
            min_calls: 10,
            open_duration_seconds: 120,
            half_open_probes: 3,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExponentialBackoffConfig {
    pub enabled: bool,
    pub initial_delay_seconds: u64,
    pub max_delay_seconds: u64,
    pub multiplier: f64,
    pub reset_after_success: bool,
}

impl Default for ExponentialBackoffConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            initial_delay_seconds: 5,
            max_delay_seconds: 300,
            multiplier: 2.0,
            reset_after_success: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HealthConfig {
    pub enabled: bool,
    pub check_delay_seconds: u64,
    pub auto_restore: bool,
    pub max_auto_restores_per_hour: u32,
    pub checks: Vec<HealthCheck>,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_delay_seconds: 10,
            auto_restore: true,
            max_auto_restores_per_hour: 3,
            checks: vec![
                HealthCheck::DiskUsage { path: "/".to_string(), max_percent: 90 },
                HealthCheck::MemoryUsage { max_percent: 95 },
            ],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApprovalConfig {
    pub enabled: bool,
    pub channels: Vec<ApprovalChannel>,
    pub default_timeout_minutes: u32,
    pub auto_reject_after_hours: u32,
}

impl Default for ApprovalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            channels: vec![ApprovalChannel::Cli, ApprovalChannel::Dashboard],
            default_timeout_minutes: 30,
            auto_reject_after_hours: 1,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuditConfig {
    pub enabled: bool,
    pub storage_path: String,
    pub hmac_key_source: HmacKeySource,
    pub retention_days: u32,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            storage_path: "~/.flowlink/gitops/audit/".to_string(),
            hmac_key_source: HmacKeySource::MachineId,
            retention_days: 365,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum HmacKeySource {
    MachineId,
    ConfigKey { key: String },
}

impl GitOpsConfig {
    pub fn load_from_path(path: &str) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let config: GitOpsConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn load_default() -> Result<Self, anyhow::Error> {
        let config_path = shellexpand::tilde("~/.flowlink/gitops.yaml");
        if std::path::Path::new(&*config_path).exists() {
            Self::load_from_path(&config_path)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save_to_path(&self, path: &str) -> Result<(), anyhow::Error> {
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn expand_paths(&mut self) -> Result<(), anyhow::Error> {
        self.git.repo_path = shellexpand::tilde(&self.git.repo_path).to_string();
        self.vault.path = shellexpand::tilde(&self.vault.path).to_string();
        self.audit.storage_path = shellexpand::tilde(&self.audit.storage_path).to_string();
        self.drift.rules_path = shellexpand::tilde(&self.drift.rules_path).to_string();
        
        for path in &mut self.state.tracked_paths {
            *path = shellexpand::tilde(path).to_string();
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GitOpsConfig::default();
        assert!(config.enabled);
        assert_eq!(config.git.branch, "main");
        assert!(config.backup.auto_backup_destructive);
        assert!(config.health.enabled);
    }

    #[test]
    fn test_config_serialization() {
        let config = GitOpsConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: GitOpsConfig = serde_yaml::from_str(&yaml).unwrap();
        
        assert_eq!(config.enabled, deserialized.enabled);
        assert_eq!(config.git.branch, deserialized.git.branch);
        assert_eq!(config.tempo.enabled, deserialized.tempo.enabled);
    }

    #[test]
    fn test_path_expansion() {
        let mut config = GitOpsConfig::default();
        config.git.repo_path = "~/test/repo".to_string();
        config.expand_paths().unwrap();
        
        assert!(!config.git.repo_path.starts_with('~'));
    }

    #[test]
    fn test_rate_limit_defaults() {
        let config = RateLimitConfig::default();
        assert!(config.enabled);
        assert_eq!(config.global_limit.max_calls, 300);
        assert!(config.per_tool_limits.contains_key("rm"));
        assert!(config.per_tier_defaults.contains_key(&ActionTier::ReadOnly));
    }

    #[test]
    fn test_vault_path_platform_specific() {
        let config = VaultConfig::default();
        if cfg!(target_os = "macos") {
            assert!(config.path.starts_with("/usr/local"));
        } else {
            assert!(config.path.starts_with("/opt"));
        }
    }
}
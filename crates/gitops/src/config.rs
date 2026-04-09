use crate::types::*;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Error type for configuration validation failures.
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// A file path is empty or cannot be created.
    InvalidPath { field: String, path: String, reason: String },
    /// A numeric or enum value is out of the acceptable range.
    InvalidValue { field: String, value: String, reason: String },
    /// A required field is missing or empty.
    MissingField { field: String },
    /// Two or more settings conflict with each other.
    ConflictingSettings { message: String },
    /// Multiple validation errors collected together.
    Multiple(Vec<String>),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::InvalidPath { field, path, reason } => {
                write!(f, "invalid path for '{}': '{}' ({})", field, path, reason)
            }
            ConfigError::InvalidValue { field, value, reason } => {
                write!(f, "invalid value for '{}': '{}' ({})", field, value, reason)
            }
            ConfigError::MissingField { field } => {
                write!(f, "missing required field: '{}'", field)
            }
            ConfigError::ConflictingSettings { message } => {
                write!(f, "conflicting settings: {}", message)
            }
            ConfigError::Multiple(errors) => {
                write!(f, "multiple validation errors ({}):\n  - {}", errors.len(), errors.join("\n  - "))
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

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
    pub pipeline: Option<GitOpsPipelineConfig>,
    pub server_guard: Option<GitOpsServerGuardConfig>,
}

/// Optional pipeline configuration for the GitOps engine.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GitOpsPipelineConfig {
    pub max_concurrent_commands: Option<u32>,
    pub command_timeout_secs: Option<u64>,
}

impl Default for GitOpsPipelineConfig {
    fn default() -> Self {
        Self {
            max_concurrent_commands: None,
            command_timeout_secs: None,
        }
    }
}

/// Optional server-guard configuration for the GitOps engine.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GitOpsServerGuardConfig {
    pub state_collect_interval_secs: Option<u64>,
}

impl Default for GitOpsServerGuardConfig {
    fn default() -> Self {
        Self {
            state_collect_interval_secs: None,
        }
    }
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
            pipeline: None,
            server_guard: None,
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

    /// Validate the configuration, collecting all errors into a list.
    ///
    /// This is lenient: it only checks fields that ARE set and doesn't
    /// require optional fields. Returns `Ok(())` if there are no issues.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // --- git.repo_path ---
        let repo_path = std::path::Path::new(&self.git.repo_path);
        if self.git.repo_path.trim().is_empty() {
            errors.push("git.repo_path must not be empty".into());
        } else {
            // Check if the directory exists or its parent is writable
            if !repo_path.exists() {
                if let Some(parent) = repo_path.parent() {
                    if !parent.exists() {
                        // Try to see if we can create the parent by probing writability
                        // of the closest existing ancestor
                        let mut probe = parent;
                        while !probe.exists() {
                            probe = match probe.parent() {
                                Some(p) if !p.as_os_str().is_empty() => p,
                                _ => break,
                            };
                        }
                        if probe.as_os_str().is_empty() || !probe.is_dir() {
                            errors.push(format!(
                                "git.repo_path '{}' has no valid parent directory",
                                self.git.repo_path
                            ));
                        }
                    }
                } else {
                    errors.push(format!(
                        "git.repo_path '{}' has no valid parent directory",
                        self.git.repo_path
                    ));
                }
            }
        }

        // --- git.branch ---
        if self.git.branch.trim().is_empty() {
            errors.push("git.branch must not be empty".into());
        }

        // --- vault.path ---
        if self.vault.path.trim().is_empty() {
            errors.push("vault.path must not be empty".into());
        }

        // --- pipeline.max_concurrent_commands (if set) ---
        if let Some(ref pipeline) = self.pipeline {
            if let Some(max_concurrent) = pipeline.max_concurrent_commands {
                if max_concurrent == 0 {
                    errors.push(
                        "pipeline.max_concurrent_commands must be > 0".into()
                    );
                }
            }
            if let Some(timeout) = pipeline.command_timeout_secs {
                if timeout == 0 {
                    errors.push(
                        "pipeline.command_timeout_secs must be > 0".into()
                    );
                }
            }
        }

        // --- server_guard.state_collect_interval_secs (if set) ---
        if let Some(ref sg) = self.server_guard {
            if let Some(interval) = sg.state_collect_interval_secs {
                if interval < 60 {
                    errors.push(format!(
                        "server_guard.state_collect_interval_secs must be >= 60, got {}",
                        interval
                    ));
                }
            }
        }

        // --- Conflicting settings ---
        match &self.git.sync_strategy {
            SyncStrategy::Realtime => {
                if self.git.remote_url.as_ref().map_or(true, |u| u.trim().is_empty()) {
                    errors.push(
                        "sync_strategy is Realtime but git.remote_url is not set; \
                         realtime sync requires a remote to push/pull from".into()
                    );
                }
            }
            SyncStrategy::Scheduled { cron } => {
                if cron.trim().is_empty() {
                    errors.push(
                        "sync_strategy is Scheduled but cron expression is empty".into()
                    );
                }
            }
            SyncStrategy::Batched { interval_secs, max_batch_size } => {
                if *interval_secs == 0 {
                    errors.push(
                        "sync_strategy Batched interval_secs must be > 0".into()
                    );
                }
                if *max_batch_size == 0 {
                    errors.push(
                        "sync_strategy Batched max_batch_size must be > 0".into()
                    );
                }
            }
            SyncStrategy::Manual => {}
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate the config and return ownership of self on success, or a ConfigError.
    ///
    /// This is a convenience wrapper around [`validate()`] that produces a single
    /// [`ConfigError`] (using the `Multiple` variant when there are several issues).
    pub fn validated(self) -> Result<Self, ConfigError> {
        match self.validate() {
            Ok(()) => Ok(self),
            Err(errors) => Err(ConfigError::Multiple(errors)),
        }
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

    // ── Validation tests ──────────────────────────────────────────────

    #[test]
    fn test_validate_default_config() {
        let config = GitOpsConfig::default();
        // Default config uses Realtime... no, it uses Batched. Let's check.
        assert!(config.validate().is_ok(), "default config should validate");
    }

    #[test]
    fn test_validated_default_config() {
        let config = GitOpsConfig::default();
        assert!(config.validated().is_ok());
    }

    #[test]
    fn test_validate_empty_repo_path() {
        let mut config = GitOpsConfig::default();
        config.git.repo_path = String::new();
        let errs = config.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("repo_path") && e.contains("empty")));
    }

    #[test]
    fn test_validate_empty_branch() {
        let mut config = GitOpsConfig::default();
        config.git.branch = String::new();
        let errs = config.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("branch") && e.contains("empty")));
    }

    #[test]
    fn test_validate_empty_vault_path() {
        let mut config = GitOpsConfig::default();
        config.vault.path = String::new();
        let errs = config.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("vault.path") && e.contains("empty")));
    }

    #[test]
    fn test_validate_realtime_without_remote_url() {
        let mut config = GitOpsConfig::default();
        config.git.sync_strategy = SyncStrategy::Realtime;
        config.git.remote_url = None;
        let errs = config.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Realtime") && e.contains("remote_url")));
    }

    #[test]
    fn test_validate_realtime_with_empty_remote_url() {
        let mut config = GitOpsConfig::default();
        config.git.sync_strategy = SyncStrategy::Realtime;
        config.git.remote_url = Some(String::new());
        let errs = config.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Realtime") && e.contains("remote_url")));
    }

    #[test]
    fn test_validate_realtime_with_remote_url() {
        let mut config = GitOpsConfig::default();
        config.git.sync_strategy = SyncStrategy::Realtime;
        config.git.remote_url = Some("git@github.com:example/repo.git".into());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_scheduled_with_empty_cron() {
        let mut config = GitOpsConfig::default();
        config.git.sync_strategy = SyncStrategy::Scheduled { cron: String::new() };
        let errs = config.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Scheduled") && e.contains("cron")));
    }

    #[test]
    fn test_validate_scheduled_with_valid_cron() {
        let mut config = GitOpsConfig::default();
        config.git.sync_strategy = SyncStrategy::Scheduled {
            cron: "0 */5 * * * *".into(),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_batched_zero_interval() {
        let mut config = GitOpsConfig::default();
        config.git.sync_strategy = SyncStrategy::Batched {
            interval_secs: 0,
            max_batch_size: 50,
        };
        let errs = config.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("interval_secs")));
    }

    #[test]
    fn test_validate_batched_zero_batch_size() {
        let mut config = GitOpsConfig::default();
        config.git.sync_strategy = SyncStrategy::Batched {
            interval_secs: 30,
            max_batch_size: 0,
        };
        let errs = config.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("max_batch_size")));
    }

    #[test]
    fn test_validate_manual_strategy_always_ok() {
        let mut config = GitOpsConfig::default();
        config.git.sync_strategy = SyncStrategy::Manual;
        config.git.remote_url = None;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_pipeline_max_concurrent_zero() {
        let mut config = GitOpsConfig::default();
        config.pipeline = Some(GitOpsPipelineConfig {
            max_concurrent_commands: Some(0),
            command_timeout_secs: None,
        });
        let errs = config.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("max_concurrent_commands") && e.contains("> 0")));
    }

    #[test]
    fn test_validate_pipeline_timeout_zero() {
        let mut config = GitOpsConfig::default();
        config.pipeline = Some(GitOpsPipelineConfig {
            max_concurrent_commands: None,
            command_timeout_secs: Some(0),
        });
        let errs = config.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("command_timeout_secs") && e.contains("> 0")));
    }

    #[test]
    fn test_validate_pipeline_valid_values() {
        let mut config = GitOpsConfig::default();
        config.pipeline = Some(GitOpsPipelineConfig {
            max_concurrent_commands: Some(10),
            command_timeout_secs: Some(300),
        });
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_server_guard_interval_too_low() {
        let mut config = GitOpsConfig::default();
        config.server_guard = Some(GitOpsServerGuardConfig {
            state_collect_interval_secs: Some(30),
        });
        let errs = config.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("state_collect_interval_secs") && e.contains(">= 60")));
    }

    #[test]
    fn test_validate_server_guard_interval_at_minimum() {
        let mut config = GitOpsConfig::default();
        config.server_guard = Some(GitOpsServerGuardConfig {
            state_collect_interval_secs: Some(60),
        });
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_server_guard_none_is_ok() {
        let config = GitOpsConfig::default(); // server_guard is None
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_multiple_errors() {
        let mut config = GitOpsConfig::default();
        config.git.repo_path = String::new();
        config.git.branch = String::new();
        config.vault.path = String::new();
        let errs = config.validate().unwrap_err();
        assert!(errs.len() >= 3, "expected at least 3 errors, got {}", errs.len());
    }

    #[test]
    fn test_validated_returns_config_on_success() {
        let config = GitOpsConfig::default();
        let result = config.validated().unwrap();
        assert_eq!(result.git.branch, "main");
    }

    #[test]
    fn test_validated_returns_error_on_failure() {
        let mut config = GitOpsConfig::default();
        config.git.repo_path = String::new();
        let err = config.validated().unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("validation errors"));
    }

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::InvalidPath {
            field: "git.repo_path".into(),
            path: "/no/such/dir".into(),
            reason: "parent not creatable".into(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("git.repo_path"));
        assert!(msg.contains("/no/such/dir"));
    }

    #[test]
    fn test_config_error_invalid_value() {
        let err = ConfigError::InvalidValue {
            field: "pipeline.max_concurrent_commands".into(),
            value: "0".into(),
            reason: "must be > 0".into(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("0") && msg.contains("must be > 0"));
    }

    #[test]
    fn test_config_error_missing_field() {
        let err = ConfigError::MissingField {
            field: "git.branch".into(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("git.branch"));
    }

    #[test]
    fn test_config_error_conflicting_settings() {
        let err = ConfigError::ConflictingSettings {
            message: "Realtime sync requires remote_url".into(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Realtime sync requires remote_url"));
    }

    #[test]
    fn test_config_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ConfigError>();
    }

    #[test]
    fn test_config_serialization_with_optional_fields() {
        let config = GitOpsConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: GitOpsConfig = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(config.pipeline, deserialized.pipeline);
        assert_eq!(config.server_guard, deserialized.server_guard);
    }

    #[test]
    fn test_validate_repo_path_with_temp_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = GitOpsConfig::default();
        config.git.repo_path = temp_dir.path().join("subdir").to_string_lossy().to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_repo_path_whitespace_only() {
        let mut config = GitOpsConfig::default();
        config.git.repo_path = "   ".to_string();
        let errs = config.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.contains("repo_path") && e.contains("empty")));
    }
}
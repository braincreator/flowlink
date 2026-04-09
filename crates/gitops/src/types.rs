use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd, Ord, Eq)]
pub enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
}

impl Default for RiskLevel {
    fn default() -> Self {
        RiskLevel::Safe
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum DriftSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum DriftCategory {
    ManualChange,
    PackageUpdate,
    ApplicationBehavior,
    ServiceFailure,
    SecurityIncident,
    NetworkChange,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash, Eq)]
pub enum ActionTier {
    ReadOnly,
    Destructive,
    Network,
    Modify,
    Blocked,
    Unclassified,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ShieldVerdict {
    Allow {
        audit: bool,
    },
    Deny(DenialFeedback),
    AutoBackup {
        impact: ImpactReport,
        backup_type: BackupType,
        message: String,
    },
    Escalate {
        reason: String,
        backup_first: bool,
        channel: ApprovalChannel,
    },
    Modify {
        original: String,
        rewritten: String,
        reason: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ShieldVerdictType {
    Allow,
    Deny,
    AutoBackup,
    Escalate,
    Modify,
}

impl From<&ShieldVerdict> for ShieldVerdictType {
    fn from(v: &ShieldVerdict) -> Self {
        match v {
            ShieldVerdict::Allow { .. } => ShieldVerdictType::Allow,
            ShieldVerdict::Deny(_) => ShieldVerdictType::Deny,
            ShieldVerdict::AutoBackup { .. } => ShieldVerdictType::AutoBackup,
            ShieldVerdict::Escalate { .. } => ShieldVerdictType::Escalate,
            ShieldVerdict::Modify { .. } => ShieldVerdictType::Modify,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DenialFeedback {
    pub reason: String,
    pub risk_level: RiskLevel,
    pub what_would_be_needed: String,
    pub remaining_budget: Option<RateBudget>,
    pub alternative: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RateBudget {
    pub tool_remaining: u32,
    pub tool_reset_in_seconds: u64,
    pub global_remaining: u32,
    pub breaker_state: BreakerState,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum BreakerState {
    Closed,
    Open { since: DateTime<Utc>, failure_count: u32 },
    HalfOpen { probe_remaining: u32 },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ExceedAction {
    Deny,
    Escalate,
    ReadOnly,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolRateLimit {
    pub max_calls: u32,
    pub window_seconds: u64,
    pub on_exceed: ExceedAction,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ApprovalChannel {
    Telegram,
    Dashboard,
    Cli,
    Api,
    GitPR,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ApprovalStatus {
    PendingBackup,
    PendingApproval,
    Approved { by: ApprovalIdentity },
    Rejected { by: ApprovalIdentity, reason: String },
    Expired,
    Executing,
    Completed { exit_code: i32 },
    Failed { error: String },
    AutoRestored { backup_id: String },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApprovalIdentity {
    pub user_id: String,
    pub channel: ApprovalChannel,
    pub timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerState {
    pub hostname: String,
    pub timestamp: DateTime<Utc>,
    pub version: String,
    pub os: OsInfo,
    pub hardware: HardwareInfo,
    pub components: HashMap<String, ComponentState>,
    pub checksum: String,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            hostname: String::new(),
            timestamp: Utc::now(),
            version: "1".to_string(),
            os: OsInfo::default(),
            hardware: HardwareInfo::default(),
            components: HashMap::new(),
            checksum: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GitSnapshot {
    pub timestamp: DateTime<Utc>,
    pub tag: String,
    pub message: String,
    pub files_changed: usize,
    pub head_commit: Option<String>,
    pub integrity_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OsInfo {
    pub name: String,
    pub version: String,
    pub arch: String,
    pub kernel: String,
}

impl Default for OsInfo {
    fn default() -> Self {
        Self { name: String::new(), version: String::new(), arch: String::new(), kernel: String::new() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct HardwareInfo {
    pub cpu_cores: u32,
    pub memory_total_bytes: u64,
    pub disk_total_bytes: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ComponentState {
    pub component: String,
    pub version: u32,
    pub collected_at: DateTime<Utc>,
    pub data: serde_json::Value,
    pub checksum: String,
}

impl Default for ComponentState {
    fn default() -> Self {
        Self {
            component: String::new(),
            version: 0,
            collected_at: Utc::now(),
            data: serde_json::Value::Null,
            checksum: String::new(),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct ImpactReport {
    pub risk_level: RiskLevel,
    pub shield_verdict: ShieldVerdictType,
    pub shield_rules: Vec<String>,
    pub files_at_risk: Vec<String>,
    pub databases_at_risk: Vec<String>,
    pub containers_at_risk: Vec<String>,
    pub services_at_risk: Vec<String>,
    pub security_impact: bool,
    pub backup_plan: BackupType,
    pub estimated_backup_size: u64,
    pub estimated_backup_time_ms: u64,
    pub rollback_possible: bool,
}

impl Default for ShieldVerdictType {
    fn default() -> Self {
        ShieldVerdictType::Allow
    }
}

impl Default for BackupType {
    fn default() -> Self {
        BackupType::FileSnapshot {
            paths: vec![],
            include_hashes: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BackupType {
    StateSnapshot,
    FileSnapshot {
        paths: Vec<String>,
        include_hashes: bool,
    },
    DatabaseDump {
        db_type: DbType,
        databases: Vec<String>,
        tables: Option<Vec<String>>,
        format: DumpFormat,
    },
    DockerState {
        containers: Vec<String>,
        include_volumes: bool,
        include_env: bool,
    },
    SystemConfig {
        components: Vec<String>,
    },
    FullSnapshot {
        include_databases: bool,
        include_docker: bool,
        include_configs: bool,
    },
    Incremental {
        since_backup_id: String,
    },
}

impl std::fmt::Display for BackupType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            BackupType::StateSnapshot => "StateSnapshot",
            BackupType::FileSnapshot { .. } => "FileSnapshot",
            BackupType::DatabaseDump { .. } => "DatabaseDump",
            BackupType::DockerState { .. } => "DockerState",
            BackupType::SystemConfig { .. } => "SystemConfig",
            BackupType::FullSnapshot { .. } => "FullSnapshot",
            BackupType::Incremental { .. } => "Incremental",
        };
        write!(f, "{}", name)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DbType {
    PostgreSQL,
    MySQL,
    MongoDB,
    SQLite,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DumpFormat {
    Sql,
    Custom,
    Tar,
    Json,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BackupTrigger {
    PreExecAuto,
    PreDestructive,
    PreConfigChange,
    ScheduledDaily,
    ScheduledWeekly,
    Manual { tag: Option<String> },
    PreDeploy,
    PreUpdate,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RetentionPolicy {
    Days(u32),
    LastN(u32),
    Tiered {
        hours: u32,
        daily_days: u32,
        weekly_days: u32,
        monthly_keep: u32,
    },
    Forever,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupManifest {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub hostname: String,
    pub trigger: BackupTrigger,
    pub trigger_command: Option<String>,
    pub risk_level: RiskLevel,
    pub backup_type: BackupType,
    pub size_bytes: u64,
    pub checksum: String,
    pub files_count: u32,
    pub databases: Vec<String>,
    pub containers: Vec<String>,
    pub configs: Vec<String>,
    pub local_path: String,
    pub cloud_path: Option<String>,
    pub git_committed: bool,
    pub verified: bool,
    pub verified_at: Option<DateTime<Utc>>,
    pub restore_tested: bool,
    pub encrypted: bool,
    pub encryption_key_id: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub retention_policy: RetentionPolicy,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Drift {
    pub path: String,
    pub expected: serde_json::Value,
    pub actual: serde_json::Value,
    pub action: DriftAction,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum DriftAction {
    Added,
    Removed,
    Changed,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DriftReport {
    pub component: String,
    pub drifts: Vec<Drift>,
    pub severity: DriftSeverity,
    pub auto_fixable: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClassifiedDrift {
    pub drift: Drift,
    pub severity: DriftSeverity,
    pub category: DriftCategory,
    pub suggested_fix: Option<String>,
    pub auto_fix_command: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DriftEvent {
    pub timestamp: DateTime<Utc>,
    pub source: DriftSource,
    pub component: String,
    pub detail: String,
    pub severity: DriftSeverity,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DriftSource {
    FileChange { path: String, kind: FileChangeKind },
    DockerEvent { container: String, action: String },
    SystemdEvent { service: String, from: String, to: String },
    PeriodicCheck { component: String },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
    Moved,
    MetadataChanged,
    PermissionChanged,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum HealthCheck {
    HttpGet { url: String, expected_status: u16 },
    TcpPort { port: u16 },
    ProcessRunning { name: String },
    DockerContainer { name: String },
    SystemdService { name: String },
    CustomCommand { command: String },
    DatabasePing { db_type: DbType, host: String, port: u16 },
    DiskUsage { path: String, max_percent: u8 },
    MemoryUsage { max_percent: u8 },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CheckResult {
    Pass,
    Fail,
    Error(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IndividualCheck {
    pub check: HealthCheck,
    pub result: CheckResult,
    pub detail: String,
    pub latency_ms: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HealthCheckResult {
    pub checks: Vec<IndividualCheck>,
    pub overall: HealthStatus,
    pub checked_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ChangeType {
    StateUpdate,
    ConfigChange,
    PolicyUpdate,
    Backup,
    Rollback,
    DriftCorrection,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuditEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub agent_id: String,
    pub session_id: String,
    
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env_var_names: Vec<String>,
    
    pub risk_level: RiskLevel,
    pub shield_verdict: ShieldVerdictType,
    pub shield_rules_matched: Vec<String>,
    pub tier: ActionTier,
    
    pub original_command: Option<String>,
    pub rewritten_command: Option<String>,
    
    pub rate_remaining: Option<RateBudget>,
    pub breaker_state: Option<BreakerState>,
    
    pub exit_code: Option<i32>,
    pub stdout_hash: String,
    pub stderr_hash: String,
    pub duration_ms: u64,
    
    pub files_modified: Vec<String>,
    pub services_affected: Vec<String>,
    pub containers_affected: Vec<String>,
    pub databases_affected: Vec<String>,
    
    pub git_commit: String,
    pub backup_id: Option<String>,
    pub rollback_available: bool,
    
    pub health_check: Option<HealthCheckResult>,
    pub auto_restored: bool,
    pub auto_restore_backup_id: Option<String>,
    
    pub policy_hash: String,
    pub classification_rule: Option<String>,
    pub hmac: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CommandResult {
    Blocked(DenialFeedback),
    Executed(ExecOutput),
    Modified {
        original: String,
        rewritten: String,
        result: ExecOutput,
        reason: String,
    },
    ExecutedWithBackup {
        result: ExecOutput,
        backup_id: String,
        health: HealthCheckResult,
    },
    ExecutedWithApproval {
        result: ExecOutput,
        approved_by: ApprovalIdentity,
        health: HealthCheckResult,
    },
    AutoRestored {
        command: String,
        backup_id: String,
        health: HealthCheckResult,
        restore: RestoreResult,
    },
    Rejected {
        by: ApprovalIdentity,
        reason: String,
    },
    Expired,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExecOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RestoreResult {
    pub backup_id: String,
    pub pre_restore_backup_id: String,
    pub verification: HealthCheckResult,
    pub duration_ms: u64,
    pub files_restored: u32,
    pub databases_restored: u32,
    pub containers_restarted: u32,
}

pub type SemanticDrift = Drift;

#[derive(Clone, Debug)]
pub struct RepoStatus {
    pub is_clean: bool,
    pub head_commit: Option<String>,
    pub modified_files: Vec<PathBuf>,
    pub untracked_files: Vec<PathBuf>,
    pub deleted_files: Vec<PathBuf>,
    pub branch: String,
}

#[derive(Clone, Debug)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub timestamp: i64,
    pub change_type: Option<ChangeType>,
    pub parent_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IntegrityStatus {
    pub is_healthy: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
    pub last_checked: chrono::DateTime<chrono::Utc>,
}
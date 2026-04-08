# FlowLink GitOps — Coding PRD

**Для:** Coding Agent (Codex/Claude Code)
**Крейт:** `crates/gitops` (новый, 8-й в workspace)
**Порядок:** Файлы создавать сверху вниз (типы → traits → реализация → интеграция)
**Тесты:** Каждый модуль с unit tests

---

## 1. Workspace Setup

### 1.1 Добавить в корневой `Cargo.toml`:

```toml
[workspace]
members = [
    "crates/core",
    "crates/crypto",
    "crates/shield",
    "crates/agent",
    "crates/relay",
    "crates/cli",
    "crates/k8s",
    "crates/gitops",    # NEW
]
```

### 1.2 Создать `crates/gitops/Cargo.toml`:

```toml
[package]
name = "flowlink-gitops"
version = "0.1.0"
edition = "2021"

[dependencies]
flowlink-core = { path = "../core" }
flowlink-shield = { path = "../shield" }

tokio = { version = "1", features = ["full", "sync"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }

# Git operations
git2 = "0.19"

# File watching (inotify/FSEvents)
notify = { version = "7", features = ["macos_kqueue"] }

# Docker API
bollard = "0.18"

# Backup archives
tar = "0.4"
flate2 = "1"

# Crypto (integrity chain, vault encryption)
sha2 = "0.10"
hmac = "0.12"
aes-gcm = "0.10"

# HTTP client (S3 upload, relay API)
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json"] }

# Async traits
async-trait = "0.1"

# Utilities
thiserror = "2"
anyhow = "1"
glob-match = "0.2"

[dev-dependencies]
tempfile = "3"
tokio = { version = "1", features = ["full", "test-util", "macros"] }
```

---

## 2. Module Structure

```
crates/gitops/
├── Cargo.toml
├── src/
│   ├── lib.rs                    ← re-exports
│   │
│   ├── types.rs                  ← ALL shared types (enums, structs)
│   │
│   ├── pipeline/
│   │   ├── mod.rs                ← Command pipeline orchestrator
│   │   ├── literal_checker.rs    ← Reject shell expansion in destructive
│   │   ├── tempo.rs              ← Circuit breaker + rate limiting
│   │   ├── classifier.rs         ← Tiered action classification
│   │   └── feedback.rs           ← Structured denial messages
│   │
│   ├── state/
│   │   ├── mod.rs                ← StateManager (orchestrates collectors)
│   │   ├── collector.rs          ← StateCollector trait
│   │   ├── packages.rs           ← PackageCollector
│   │   ├── services.rs           ← ServiceCollector
│   │   ├── docker_state.rs       ← DockerCollector
│   │   └── files.rs              ← FileCollector
│   │
│   ├── audit/
│   │   ├── mod.rs                ← AuditTrail (write + integrity)
│   │   ├── entry.rs              ← AuditEntry type
│   │   └── integrity.rs          ← HMAC chain verification
│   │
│   ├── backup/
│   │   ├── mod.rs                ← BackupEngine (orchestrator)
│   │   ├── impact.rs             ← ImpactAnalyzer
│   │   ├── vault.rs              ← Vault storage (agent-unreachable)
│   │   ├── file_backup.rs        ← FileSnapshot backup
│   │   ├── db_backup.rs          ← DatabaseDump backup
│   │   ├── docker_backup.rs      ← DockerState backup
│   │   ├── manifest.rs           ← BackupManifest type
│   │   └── restore.rs            ← RestoreEngine
│   │
│   ├── drift/
│   │   ├── mod.rs                ← DriftDetector (orchestrator)
│   │   ├── event_driven.rs       ← inotify/docker event watcher
│   │   ├── semantic_diff.rs      ← Semantic state diffing
│   │   └── auto_fix.rs           ← Auto-fix rule engine
│   │
│   ├── plan/
│   │   ├── mod.rs                ← PlanEngine (dry-run preview)
│   │   └── types.rs              ← ExecutionPlan type
│   │
│   ├── git/
│   │   ├── mod.rs                ← GitOpsEngine (core git operations)
│   │   ├── repo.rs               ← Repository init/open
│   │   ├── commit.rs             ← Commit operations
│   │   ├── sync.rs               ← Push/pull/remote
│   │   └── rollback.rs           ← Rollback operations
│   │
│   ├── approval/
│   │   ├── mod.rs                ← ApprovalManager
│   │   ├── queue.rs              ← In-memory + JSONL approval queue
│   │   └── types.rs              ← ApprovalRequest, ApprovalStatus
│   │
│   ├── health/
│   │   ├── mod.rs                ← HealthChecker
│   │   └── auto_restore.rs       ← AutoRestoreEngine
│   │
│   └── config.rs                 ← GitOpsConfig (loaded from YAML)
│
├── policies/                     ← Default policy files
│   ├── classification.yaml
│   ├── drift_auto_fix.yaml
│   ├── approval_policies.yaml
│   └── backup_rules.yaml
│
└── tests/
    ├── integration_test.rs
    └── fixtures/
        ├── test_state.json
        ├── test_audit.jsonl
        └── test_classification.yaml
```

---

## 3. Types (`src/types.rs`)

ALL types in ONE file. Other modules re-export from here.

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// === Risk & Severity ===

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd, Ord)]
pub enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum DriftSeverity {
    Info,       // cosmetic
    Low,        // non-critical (temp file, extra log)
    Medium,     // service degradation (config changed, service stopped)
    High,       // security risk (port opened, user added)
    Critical,   // firewall changed, SSH key added, encryption disabled
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

// === Action Tier (classification) ===

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash, Eq)]
pub enum ActionTier {
    ReadOnly,       // cat, ls, grep, find, stat, head, tail, wc, file, which, echo, pwd
    Destructive,    // rm, mv, truncate, dd, shred, sed -i, tee (overwrite)
    Network,        // curl, wget, ssh, scp, rsync, nc
    Modify,         // chmod 777→755 (auto-rewrite unsafe params)
    Blocked,        // rm -rf /, mkfs, curl | bash
    Unclassified,   // anything not in policy
}

// === Shield Verdicts ===

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

// === Denial Feedback (structured, from Agent Gate) ===

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DenialFeedback {
    pub reason: String,
    pub risk_level: RiskLevel,
    pub what_would_be_needed: String,
    pub remaining_budget: Option<RateBudget>,
    pub alternative: Option<String>,
}

// === Rate Limiting & Circuit Breaker ===

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

// === Approval ===

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

// === Server State ===

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OsInfo {
    pub name: String,
    pub version: String,
    pub arch: String,
    pub kernel: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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

// === Impact Analysis ===

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

// === Backup ===

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BackupType {
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

// === Drift ===

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Drift {
    pub path: String,
    pub expected: serde_json::Value,
    pub actual: serde_json::Value,
    pub action: DriftAction,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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

// === Drift Events (event-driven) ===

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

// === Health Checks ===

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

// === Audit Entry ===

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuditEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub agent_id: String,
    pub session_id: String,
    
    // Command
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env_var_names: Vec<String>,
    
    // Shield
    pub risk_level: RiskLevel,
    pub shield_verdict: ShieldVerdictType,
    pub shield_rules_matched: Vec<String>,
    pub tier: ActionTier,
    
    // MODIFY details
    pub original_command: Option<String>,
    pub rewritten_command: Option<String>,
    
    // Tempo
    pub rate_remaining: Option<RateBudget>,
    pub breaker_state: Option<BreakerState>,
    
    // Execution
    pub exit_code: Option<i32>,
    pub stdout_hash: String,
    pub stderr_hash: String,
    pub duration_ms: u64,
    
    // Impact
    pub files_modified: Vec<String>,
    pub services_affected: Vec<String>,
    pub containers_affected: Vec<String>,
    pub databases_affected: Vec<String>,
    
    // GitOps
    pub git_commit: String,
    pub backup_id: Option<String>,
    pub rollback_available: bool,
    
    // Health
    pub health_check: Option<HealthCheckResult>,
    pub auto_restored: bool,
    pub auto_restore_backup_id: Option<String>,
    
    // Integrity
    pub policy_hash: String,
    pub classification_rule: Option<String>,
    pub hmac: String,
}

// === Command Result ===

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
```

---

## 4. Traits (`src/state/collector.rs`, etc.)

### StateCollector trait:

```rust
use async_trait::async_trait;
use crate::types::*;

#[async_trait]
pub trait StateCollector: Send + Sync {
    fn component(&self) -> &str;
    async fn collect(&self) -> Result<ComponentState, anyhow::Error>;
    async fn apply(&self, desired: &ComponentState) -> Result<(), anyhow::Error>;
    async fn diff(&self, current: &ComponentState, desired: &ComponentState) -> Result<Vec<SemanticDrift>, anyhow::Error>;
}
```

**4 collectors to implement:**

1. **PackageCollector** — `dpkg -l` / `rpm -qa` / `apk info`
2. **ServiceCollector** — `systemctl list-units --type=service --all --output=json`
3. **DockerCollector** — bollard: `docker.inspect_container()`, `docker.list_containers()`
4. **FileCollector** — tracked paths with sha256 content hashes

---

## 5. Key Implementation Details

### 5.1 LiteralChecker (`src/pipeline/literal_checker.rs`)

**Purpose:** Reject shell expansion ($VAR, *, ?, backticks, &&, ||, ;, |) in destructive commands.

**Destructive commands list:** `rm`, `rmdir`, `mv`, `cp`, `chmod`, `chown`, `truncate`, `dd`, `shred`, `tee`, `sed`

**Logic:**
- Only check destructive commands
- For each arg, check for: `$`, `*`, `?` (not at arg start with `-`), backtick, `$(`, `&&`, `||`, `;`, `|`
- On match → return `Some(DenialFeedback)` with reason + alternative
- On no match → return `None` (pass)

**Test:** `test_literal_rejects_var()`, `test_literal_allows_safe_args()`, `test_literal_passes_readonly()`

### 5.2 TempoController (`src/pipeline/tempo.rs`)

**Purpose:** Circuit breaker + per-tool rate limiting.

**Circuit Breaker (3-state):**
- **Closed:** Normal. Track success/failure of commands (exit_code != 0 = failure)
- **Open:** Failure rate > threshold (default 50%) over window (default 60s) with min_calls (default 10). ALL non-read commands denied. Duration: 120s.
- **HalfOpen:** After open_duration, allow `half_open_probes` (default 3) non-read calls. Success → Closed. Failure → Open.

**Rate Limiting:**
- Per-tool: sliding window counter. Default: rm=10/60s, docker=20/60s, apt=5/300s, systemctl=15/60s, cat=200/60s
- Per-tier defaults: ReadOnly=200/60s, Destructive=30/60s, Network=10/60s
- Global limit: 300/60s. On exceed → read-only mode
- ExceedAction per tool: Deny, Escalate, or ReadOnly

**Exponential backoff for repeated violations:** 5s → 10s → 20s → 40s → ... → max 300s. Reset on success.

**Test:** `test_breaker_trips_on_failures()`, `test_rate_limit_denies_excess()`, `test_breaker_half_open_recovery()`

### 5.3 ActionClassifier (`src/pipeline/classifier.rs`)

**Purpose:** Map command+args → ActionTier. Handle MODIFY tier auto-rewrite.

**Config:** Load from `policies/classification.yaml` (include default embedded in binary).

**Classification logic:**
1. Check command against rules in order
2. For each rule, check conditions (HasFlag, ArgContains, ArgMatches regex, PathProtected, AllArgsLiteral)
3. First matching rule wins → return tier
4. No match → Unclassified

**MODIFY rewrite:**
- When tier=Modify, apply `rewrite.replacements` to args
- Each replacement: match_pattern → replace_with (simple string replace in each arg)
- Return rewritten args

**Example rules (YAML):**
```yaml
rules:
  - command: "chmod"
    conditions:
      - type: arg_contains
        value: "777"
    tier: modify
    rewrite:
      replacements:
        - match: "777"
          replace: "755"
      message: "chmod 777 auto-corrected to 755"
  
  - command: "rm"
    conditions:
      - type: arg_contains
        value: "-rf /"
    tier: blocked
    message: "Root deletion blocked"
  
  - command: "rm"
    tier: destructive
  
  - command: "cat"
    tier: read_only
```

**Test:** `test_classify_readonly()`, `test_classify_destructive()`, `test_classify_modify_chmod()`, `test_classify_blocked_rm_rf_root()`

### 5.4 Structured Denial (`src/pipeline/feedback.rs`)

**Purpose:** Generate human+machine readable denial messages.

**Format for rate limit:**
```
ACTION DENIED: {tool} rate limit exceeded. Max {max} calls per {window}s.
DETAILS: {count} calls in last {window}s (limit: {max}).
RATE STATUS: tool_remaining={tool_rem}, global_remaining={global_rem}, breaker={state}
TO PROCEED: Wait {wait}s for window to clear, or reduce operation frequency.
```

**Format for literal:**
```
ACTION DENIED: Shell expansion in destructive command: {unsafe_args}
TO PROCEED: Use literal paths instead of shell variables/globs.
ALTERNATIVE: Resolve paths first: ls {pattern} → then rm file1 file2
```

**Format for blocked:**
```
ACTION DENIED: {message}
RISK: {risk_level}
ALTERNATIVE: {safe_alternative}
```

### 5.5 GitOpsEngine (`src/git/mod.rs`)

**Purpose:** Core git operations using `git2` crate.

**Key operations:**
- `init_repo(path)` — init bare repo + worktree at `~/.flowlink/gitops/repo/`
- `commit_state(state: &ServerState)` — serialize to JSON, git add, commit with message `"state({hostname}): {version}"`
- `commit_audit(entry: &AuditEntry)` — append to `audit/{date}.jsonl`, git add + commit `"audit({tier}): {command_truncated}"`
- `get_state_at(timestamp)` — find commit by timestamp, deserialize state.json
- `diff_states(from, to)` — get two commits, diff state JSONs
- `push()` — batched push to remote (every 30s or 50 commits)
- `pull_and_apply()` — pull + apply if remote has changes
- `create_change_branch(name, changes)` — branch for PR approval flow
- `merge_change_branch(name)` — merge after approval
- `rollback_to(commit_sha)` — revert to specific commit
- `rollback_component(component, commit_sha)` — revert only one component
- `verify_integrity()` — check HMAC chain from beginning

**Commit signing:** GPG sign every commit if configured.

**Commit message format:**
```
audit(shield=allow): apt install nginx
audit(shield=block): DROP TABLE users CASCADE
audit(shield=autobackup): rm -rf /var/log/*.log
audit(shield=modify): chmod 777 /var/www → chmod 755
audit(shield=escalate): docker compose down
state(prod-web-01): v42
backup(create): #47 (2.1MB, pre-exec)
backup(restore): #47 → auto-restore (health check failed)
drift(auto-fix): nginx config restored from git
```

**Test:** `test_init_repo()`, `test_commit_state()`, `test_commit_audit()`, `test_integrity_chain()`, `test_rollback()`

### 5.6 AuditTrail (`src/audit/mod.rs`)

**Purpose:** Append-only audit log with HMAC integrity chain.

**Storage:** `audit/{YYYY-MM-DD}.jsonl` — one JSON line per AuditEntry.

**HMAC chain:**
```
Entry_1.hmac = HMAC-SHA256(key, "0" || Entry_1.serialized_fields)
Entry_2.hmac = HMAC-SHA256(key, Entry_1.hmac || Entry_2.serialized_fields)
Entry_N.hmac = HMAC-SHA256(key, Entry_{N-1}.hmac || Entry_N.serialized_fields)
```

**Key derivation:** HMAC key = SHA256(machine_id || agent_token). Machine ID from `/etc/machine-id` on Linux, `ioreg -rd1 -c IOPlatformExpertDevice` on macOS.

**Policy hash:** SHA256 of all policy YAML files concatenated. Stored in each AuditEntry so you can verify which policy version made each decision.

**Operations:**
- `log(entry: AuditEntry)` — compute HMAC, append to JSONL
- `get_log(from, to)` — read entries from JSONL files in date range
- `find_entry(id)` — search by UUID
- `verify_integrity()` — read all entries, verify HMAC chain
- `search(query)` — text search across all entries

**Test:** `test_hmac_chain_valid()`, `test_hmac_chain_tampered_detected()`, `test_log_and_retrieve()`

### 5.7 BackupEngine (`src/backup/mod.rs`)

**Purpose:** Smart backup with vault storage.

**Vault architecture:**
- Vault path: `/opt/flowlink-vault/` (Linux), `/usr/local/flowlink-vault/` (macOS)
- Permissions: `chmod 700`, owned by root
- Agent process (non-root) CANNOT access vault
- Backup operations go through a privileged helper or sudo

**Backup pipeline (for AutoBackup verdict):**
1. ImpactAnalyzer determines what's at risk
2. Select BackupType based on impact (FileSnapshot, DatabaseDump, DockerState)
3. Create backup in temp dir
4. Compute SHA256 checksum
5. Move to vault (atomic rename)
6. Create BackupManifest (JSON)
7. Commit manifest to git
8. Return backup_id

**FileSnapshot:** tar.gz of specified paths + SHA256 manifest of each file
**DatabaseDump:** `pg_dump` / `mysqldump` / `sqlite3 .dump` → SQL file → gzip
**DockerState:** `docker inspect` JSON + `docker export` for containers + volume tar

**Restore pipeline:**
1. Verify backup integrity (checksum)
2. Create pre-restore emergency backup
3. Stop affected services
4. Extract files / restore DB / restore docker
5. Restart services
6. Run health checks
7. Git commit
8. Return RestoreResult

**Auto-restore trigger:** After executing a destructive command, wait `check_delay_seconds` (default 10s), run health checks. If ALL fail → auto-restore from pre-exec backup. Rate limited: max 3/hour.

**Test:** `test_create_file_backup()`, `test_restore_file_backup()`, `test_vault_permissions()`, `test_auto_restore_on_health_fail()`

### 5.8 ImpactAnalyzer (`src/backup/impact.rs`)

**Purpose:** Analyze command + args → determine what's at risk.

**Pattern matching:**

| Command pattern | What's at risk | Backup type |
|---|---|---|
| `rm`, `rmdir` | Resolved file paths | FileSnapshot |
| `docker rm/rmi/down` | Named containers + volumes | DockerState |
| `psql/mysql/sqlite3` with SQL | Extract SQL → find tables/DBs | DatabaseDump |
| `systemctl stop/disable` | Named services | SystemConfig |
| `apt/yum/dnf/apk install/remove` | Package list | SystemConfig |
| `git reset/rebase` | `.git/` directory | FileSnapshot |
| `chmod/chown` on /etc/ssl, /etc/ssh | Security files | FileSnapshot + flag security_impact=true |

**SQL analysis:** Parse SQL commands to find affected tables/databases:
- `DROP TABLE (name)` → table (name)
- `TRUNCATE (name)` → table (name)
- `DELETE FROM (name)` → table (name)
- `DROP DATABASE (name)` → database (name)
- `ALTER TABLE (name)` → table (name)

**Integration with Shield:** ImpactAnalyzer receives Shield analysis (risk_level, rules matched) and enriches with concrete file/DB/container paths.

### 5.9 DriftDetector (`src/drift/mod.rs`)

**Purpose:** Detect configuration drift via events + periodic checks.

**Event sources:**
1. **inotify (notify crate):** Watch tracked paths (/etc/nginx, /etc/docker, docker-compose.yml, etc.)
2. **Docker events (bollard):** `docker.events()` stream — container start/stop/die/destroy
3. **Periodic collectors:** Every N minutes (configurable per component)

**On drift detected:**
1. Classify severity (Info/Low/Medium/High/Critical)
2. Classify category (ManualChange/PackageUpdate/etc.)
3. Check auto-fix rules (from `policies/drift_auto_fix.yaml`)
4. If auto_fixable AND auto_fix=true → execute fix → commit → notify
5. If NOT auto_fixable → SSE event → dashboard + TG notification

**Auto-fix examples:**
- Container stopped but expected running → `docker start {name}` (max 3 retries)
- Config file changed → restore from git
- Crashed service → `systemctl restart {name}` (max 3 retries)

**ALERT ONLY (never auto-fix):**
- SSH authorized_keys changed
- Firewall rules changed
- New user added
- New listening port
- Security package removed

### 5.10 PlanEngine (`src/plan/mod.rs`)

**Purpose:** Dry-run preview. Like `terraform plan` for server commands.

**`plan(command, args, ctx)` returns ExecutionPlan:**
- Classification (tier)
- Risk level
- Verdict (WillAllow / WillBackupAndExecute / WillEscalate / WillRewrite / WillBlock)
- Impact (files/DBs/containers at risk)
- Predicted state changes (diff current vs predicted)
- Backup plan (what will be backed up, estimated size)
- Post-exec health checks
- Estimated duration

**Usage:**
```bash
flowlink plan "apt install nginx"
flowlink plan "docker compose down"
flowlink plan "rm -rf /var/log/app.log"
```

### 5.11 HealthChecker (`src/health/mod.rs`)

**Purpose:** Post-exec health verification.

**Health checks defined per-server in config:**
```yaml
health_checks:
  - type: http_get
    url: "http://localhost:80"
    expected_status: 200
  - type: docker_container
    name: "postgres"
  - type: systemd_service
    name: "nginx"
  - type: disk_usage
    path: "/"
    max_percent: 90
  - type: memory_usage
    max_percent: 95
```

**Execution:** Run all checks in parallel. Timeout per check: 5s.

**Auto-restore logic (in auto_restore.rs):**
1. After destructive command, wait `check_delay_seconds` (default 10s)
2. Run all health checks
3. If `overall == Unhealthy` → find pre-exec backup → restore → re-check
4. Rate limit: max 3 auto-restores per hour
5. If restore fails or still unhealthy → TG escalation
6. Log to audit trail

---

## 6. Config (`src/config.rs`)

```rust
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GitConfig {
    pub repo_path: String,           // "~/.flowlink/gitops/repo/"
    pub remote_url: Option<String>,  // "git@github.com:org/flowlink-state.git"
    pub branch: String,              // "main"
    pub sync_strategy: SyncStrategy,
    pub signing_key: Option<String>, // GPG key ID
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum SyncStrategy {
    Realtime,
    Batched { interval_secs: u64, max_batch_size: usize },
    Scheduled { cron: String },
    Manual,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StateConfig {
    pub collect_interval_seconds: u64,  // default: 300 (5 min)
    pub tracked_paths: Vec<String>,     // ["/etc/nginx", "/etc/docker", "/etc/systemd"]
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupConfig {
    pub auto_backup_destructive: bool,  // default: true
    pub max_backup_size_mb: u64,        // default: 500
    pub retention: RetentionPolicy,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VaultConfig {
    pub path: String,                   // "/opt/flowlink-vault/"
    pub permissions: u32,               // 0o700
    pub encryption: VaultEncryption,
    pub max_size_mb: u64,               // default: 5000
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
    pub event_driven: bool,             // inotify + docker events
    pub auto_fix: bool,
    pub rules_path: String,             // "policies/drift_auto_fix.yaml"
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HealthConfig {
    pub enabled: bool,
    pub check_delay_seconds: u64,       // 10
    pub auto_restore: bool,
    pub max_auto_restores_per_hour: u32, // 3
    pub checks: Vec<HealthCheck>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApprovalConfig {
    pub enabled: bool,
    pub channels: Vec<ApprovalChannel>,
    pub default_timeout_minutes: u32,    // 30
    pub auto_reject_after_hours: u32,    // 1
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuditConfig {
    pub enabled: bool,
    pub storage_path: String,           // "~/.flowlink/gitops/audit/"
    pub hmac_key_source: HmacKeySource,
    pub retention_days: u32,            // 365
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum HmacKeySource {
    MachineId,
    ConfigKey { key: String },
}
```

Default config loaded from embedded YAML, overridden by `~/.flowlink/gitops.yaml`.

---

## 7. Integration with Existing Crates

### 7.1 Shield Integration (`crates/shield`)

The existing `ShieldEngine` in `flowlink-shield` has L1 (pattern match) and L2 (context analysis). Add L3 that delegates to GitOps:

```rust
// In crates/shield/src/lib.rs — add L3 delegate
use flowlink_gitops::GitOpsL3;

impl ShieldEngine {
    pub fn with_gitops(gitops: GitOpsL3) -> Self {
        Self {
            l1: PatternMatcher::new(),
            l2: ContextAnalyzer::new(),
            l3: Some(gitops),  // NEW: Option<GitOpsL3>
        }
    }
}
```

**Important:** Shield L3 is optional. If `flowlink-gitops` feature is not enabled, L3 is None and shield works as before (L1+L2 only).

### 7.2 Agent Integration (`crates/agent`)

Agent exec loop calls Shield for every command. With GitOps:

```rust
// In crates/agent/src/executor.rs
async fn execute(command: &str, args: &[String]) -> CommandResult {
    // Shield analysis (now includes L3 GitOps)
    let decision = self.shield.analyze_command(command, args, &ctx).await;
    
    // Execute based on verdict (backup, approve, modify, etc.)
    let result = flowlink_gitops::execute_command(command, args, &ctx, &self.engines).await;
    
    result
}
```

### 7.3 Relay Integration (`crates/relay`)

Add 20+ API endpoints for GitOps operations (see GITOPS_PLAN.md "API Surface" section).

### 7.4 CLI Integration (`crates/cli`)

Add subcommands:
```bash
flowlink gitops status          # Current state summary
flowlink gitops drift           # Show drifts
flowlink gitops backup list     # List backups
flowlink gitops backup create   # Manual backup
flowlink gitops backup restore  # Restore from backup
flowlink gitops audit           # Show audit log
flowlink gitops audit search    # Search audit
flowlink gitops plan "cmd"      # Preview execution
flowlink gitops rollback last   # Undo last command
flowlink gitops undo <id>       # Undo specific command
flowlink gitops approvals       # List pending approvals
flowlink gitops approve <id>    # Approve request
flowlink gitops reject <id>     # Reject request
```

---

## 8. Implementation Order (MVP Path — 19h)

Build in this exact order (each step builds on previous):

| Step | Files | Time | Depends on |
|------|-------|------|-----------|
| **1. Types** | `src/types.rs`, `src/lib.rs` | 1h | — |
| **2. Config** | `src/config.rs` | 1h | Types |
| **3. Git Engine** | `src/git/mod.rs`, `repo.rs`, `commit.rs` | 3h | Types, Config |
| **4. Audit Trail** | `src/audit/mod.rs`, `entry.rs`, `integrity.rs` | 2h | Types, Git |
| **5. LiteralChecker** | `src/pipeline/literal_checker.rs` | 1h | Types |
| **6. TempoController** | `src/pipeline/tempo.rs` | 2h | Types |
| **7. ActionClassifier** | `src/pipeline/classifier.rs` | 1h | Types, Config |
| **8. Structured Feedback** | `src/pipeline/feedback.rs` | 0.5h | Types |
| **9. State Collectors** | `src/state/mod.rs`, 4 collectors | 3h | Types, Git |
| **10. Backup Engine** | `src/backup/mod.rs`, impact, vault, file_backup | 3h | Types, Git, State |
| **11. Pipeline Orchestrator** | `src/pipeline/mod.rs` — wire L3 together | 1h | Steps 5-10 |
| **12. Shield Integration** | Modify `crates/shield` to use L3 | 0.5h | Pipeline |
| **13. Unit Tests** | All modules | 1h | All |

**Total: ~19h**

---

## 9. Testing Requirements

### Every module MUST have:

1. **Unit tests** in same file (`#[cfg(test)] mod tests`)
2. **At least 3 test cases per function**
3. **Error path testing** (what happens when git fails, docker not running, etc.)

### Critical test scenarios:

| # | Test | What it verifies |
|---|------|-----------------|
| 1 | Shield blocks `rm -rf /` | Blocked tier works |
| 2 | Auto-backup before `rm file.log` | Destructive → backup → execute |
| 3 | MODIFY rewrites `chmod 777` → `chmod 755` | Auto-rewrite works |
| 4 | Literal rejects `$VAR` in rm | Shell expansion blocked |
| 5 | Circuit breaker trips on 50% failures | Open → deny non-read |
| 6 | Rate limit denies 11th rm in 60s | Per-tool rate limit |
| 7 | HMAC chain detects tampering | Integrity verification |
| 8 | State diff detects package change | Semantic diff works |
| 9 | Backup + restore roundtrip | Backup created → restored → verified |
| 10 | Plan preview shows impact | Dry-run without execution |
| 11 | Full E2E pipeline | Agent → Shield → Backup → Execute → Verify → Commit |
| 12 | Vault agent-unreachable | Agent process cannot read vault dir |
| 13 | Auto-restore on health fail | Health check → auto-rollback |
| 14 | Policy hash in audit entry | SHA256 of policy stored correctly |
| 15 | Git commit + push roundtrip | State committed and pushed |

---

## 10. Default Policy Files

### `policies/classification.yaml`

Full rules for classifying commands into tiers. See Section 5.3 above for structure.

Must cover at minimum:
- **ReadOnly:** cat, ls, grep, find, stat, head, tail, wc, file, which, echo, pwd, ps, df, free, docker ps/logs/inspect/images
- **Modify:** chmod 777→755, chmod 666→644
- **Destructive:** rm, mv, truncate, dd, shred, sed -i, docker rm/rmi/down, systemctl stop/disable, apt remove/purge, psql with DROP/TRUNCATE/DELETE
- **Network:** curl, wget, ssh, scp, rsync
- **Blocked:** rm -rf /, mkfs, dd of=/dev/, chmod 777 on /etc/shadow|/etc/passwd|/etc/ssh

### `policies/drift_auto_fix.yaml`

Auto-fix rules for drifts. See Section 5.9 above.

Must cover at minimum:
- Restart stopped Docker containers (max 3 retries)
- Restart crashed systemd services (max 3 retries)
- Alert on SSH key changes (never auto-fix)
- Alert on firewall changes (never auto-fix)
- Alert on new users (never auto-fix)
- Alert on new listening ports (never auto-fix)

### `policies/approval_policies.yaml`

When to require approval. See GITOPS_PLAN.md Module 6.

### `policies/backup_rules.yaml`

When to trigger auto-backup. See Section 5.7 above.

---

## 11. Constraints & Guidelines

1. **All I/O must be async** — use tokio throughout
2. **Errors:** Use `thiserror` for library errors, `anyhow` for application errors
3. **Logging:** Use `tracing` macros (debug!, info!, warn!, error!)
4. **No unwrap()** in production code — use `?` or explicit error handling
5. **No panics** — every fallible operation returns Result
6. **Feature flag:** GitOps is behind `gitops` feature in shield crate
7. **Zero-copy where possible** — avoid cloning large JSON structures
8. **Backwards compatible** — existing shield L1+L2 must work without gitops crate
9. **Cross-platform:** Linux (primary), macOS (secondary). No Windows support needed
10. **Embed default policies** — use `include_str!` for default YAML files
11. **No external dependencies at runtime** — git2 is statically linked, no git binary needed
12. **Idempotent operations** — collectors and apply must be safe to run multiple times

---

## 12. File Count Estimate

| Category | Files | Lines (est.) |
|----------|-------|-------------|
| Types | 1 | ~500 |
| Config | 1 | ~150 |
| Pipeline (literal, tempo, classifier, feedback) | 5 | ~800 |
| State (manager + 4 collectors) | 5 | ~600 |
| Audit (trail + entry + integrity) | 3 | ~400 |
| Backup (engine + impact + vault + types + restore) | 7 | ~1000 |
| Drift (detector + events + diff + auto_fix) | 4 | ~500 |
| Git (engine + repo + commit + sync + rollback) | 5 | ~700 |
| Health (checker + auto_restore) | 2 | ~300 |
| Plan engine | 2 | ~200 |
| Approval (manager + queue + types) | 3 | ~300 |
| Integration (shield, agent, relay, cli) | 4 | ~300 |
| Policy YAML files | 4 | ~200 |
| Tests | 3+ | ~500 |
| **TOTAL** | **~49 files** | **~5,450 lines** |

---

## 13. References

- **Full plan:** `docs/GITOPS_PLAN.md` (67KB) — detailed architecture, data flows, diagrams
- **Competitor analysis:** `docs/GITOPS_COMPETITORS.md` (11KB) — Agent Gate, ArgoCD, Cohesity patterns
- **Existing crate structure:** `crates/shield/` (L1+L2 shield engine)
- **Existing types:** `crates/core/src/` (protocol types, config, error codes)

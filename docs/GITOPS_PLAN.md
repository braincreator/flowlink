# FlowLink GitOps — Master Plan v2.0

**Дата:** 2026-04-07
**Версия:** v2.0 (с конкурентным анализом)
**Статус:** Draft
**Крейт:** `flowlink-gitops` (8-й в workspace)
**Общий бюджет:** 86h

---

## 🎯 Vision

**FlowLink GitOps = Shield + State + Backup + Audit + Approval в одном бинарнике.**

Каждая команда AI-агента проходит через pipeline:
```
Analyze → Classify → Backup (if needed) → Execute → Collect State → Verify → Commit → Push
```

Если что-то пошло не так — откат в один клик. Если кто-то поменял конфиг руками — узнаем за секунды.

---

## 🏗️ Архитектура (обновлённая)

```
Agent получает команду
         │
         ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     FLOWLINK AGENT (на сервере)                      │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    COMMAND PIPELINE                           │   │
│  │                                                              │   │
│  │  Input ──▶ L1 Pattern ──▶ L2 Context ──▶ L3 GitOps ──▶ ... │   │
│  │                 │              │               │              │   │
│  │             Block/Pass     Block/Pass     See below          │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│                              ▼                                       │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    L3 GITOPS ENGINE                           │   │
│  │                                                              │   │
│  │  1. Literal Check ──▶ reject $VAR/globs in destructive      │   │
│  │  2. Tempo Check ────▶ circuit breaker + rate limit           │   │
│  │  3. Classify ───────▶ tiered response (see below)            │   │
│  │  4. Impact Analyze ─▶ what files/DBs/containers affected     │   │
│  │  5. Plan Preview ───▶ show what will change (dry-run)        │   │
│  │  6. Backup ─────────▶ vault-backup (agent-unreachable)       │   │
│  │  7. Execute ────────▶ run the command                        │   │
│  │  8. Health Check ───▶ verify service still alive              │   │
│  │  9. Collect State ──▶ update state.json                       │   │
│  │ 10. Diff ────────────▶ what actually changed                  │   │
│  │ 11. Commit ──────────▶ git commit (audit + state)             │   │
│  │ 12. Push ────────────▶ async batched push                    │   │
│  │ 13. Notify ──────────▶ SSE event to relay                    │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                              │                                       │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐        │
│  │  Shield   │  │  GitOps   │  │  Backup   │  │  State    │        │
│  │  Engine   │  │  Engine   │  │  Engine   │  │  Manager  │        │
│  │  (L1-L3)  │  │  (git)    │  │  (vault)  │  │  (collect)│        │
│  └───────────┘  └───────────┘  └───────────┘  └───────────┘        │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    DRIFT DETECTOR                             │   │
│  │                                                              │   │
│  │  inotify/fsnotify ──▶ immediate file change detection        │   │
│  │  Periodic collectors ──▶ service/package/docker drift        │   │
│  │  Auto-classify severity ──▶ auto-fix or alert                │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
         │ WSS
         ▼
┌──────────────────────────────────────────────────────────────────────┐
│                     FLOWLINK RELAY                                    │
│                                                                      │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐     │
│  │  GitOps    │  │  Approval  │  │  Dashboard │  │  Remote    │     │
│  │  API       │  │  Manager   │  │  SSE       │  │  Git Sync  │     │
│  │  (14 ep)   │  │  (TG+Web)  │  │  Events    │  │  (push)    │     │
│  └────────────┘  └────────────┘  └────────────┘  └────────────┘     │
│                                                                      │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐                     │
│  │  Backup    │  │  Rollback  │  │  Policy    │                     │
│  │  Registry  │  │  API       │  │  Engine    │                     │
│  └────────────┘  └────────────┘  └────────────┘                     │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────────────────────────┐
│                     GIT REMOTE (GitHub/Gitea/S3)                      │
│                                                                      │
│  servers/{hostname}/                                                  │
│  ├── state.json          ← declarative state (desired)               │
│  ├── packages.json       ← installed packages                        │
│  ├── services.json       ← running services                          │
│  ├── docker.json         ← containers + volumes                      │
│  ├── files/              ← tracked config files                      │
│  ├── cron.json           ← crontab entries                           │
│  ├── users.json          ← system users + SSH keys                   │
│  ├── firewall.json       ← iptables/nftables rules                   │
│  ├── network.json        ← listening ports                           │
│  ├── metadata.json       ← hardware, OS, version                    │
│  ├── tls.json            ← SSL cert expiry tracking                  │
│  └── mounts.json         ← disks, mount points                       │
│                                                                      │
│  backups/                                                             │
│  ├── {id}/                                                            │
│  │   ├── manifest.json                                                │
│  │   ├── database/    ← DB dumps (encrypted)                         │
│  │   ├── files/       ← file archives (encrypted)                    │
│  │   └── config/      ← config snapshots                             │
│  └── index.json        ← backup catalog                              │
│                                                                      │
│  audit/                                                               │
│  └── {date}.jsonl       ← all commands (HMAC-chained)                │
│                                                                      │
│  policies/                                                            │
│  ├── shield.yaml        ← shield rules                               │
│  ├── drift_auto_fix.yaml ← auto-fix rules                           │
│  ├── approval_policies.yaml ← approval rules                         │
│  └── backup_rules.yaml  ← backup triggers                            │
│                                                                      │
│  vault/ (agent-unreachable)                                           │
│  └── backups/         ← vault backups (outside agent envelope)       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 📦 Модуль 1: `command_pipeline` — Command Processing Pipeline

### 1.1 L1: Pattern Match (существующий, без изменений)

Быстрая проверка по regex паттернам. Block/Pass.

### 1.2 L2: Context Analysis (существующий, без изменений)

Контекстная проверка (scope, env, user). Block/Pass.

### 1.3 L3: GitOps Engine (НОВОЕ — полная замена L3)

L3 теперь проходит через расширенный pipeline:

```rust
pub struct GitOpsL3 {
    literal_checker: LiteralChecker,
    tempo_controller: TempoController,
    classifier: ActionClassifier,
    impact_analyzer: ImpactAnalyzer,
    planner: PlanEngine,
    backup_engine: BackupEngine,
    health_checker: HealthChecker,
    state_manager: StateManager,
    git_engine: GitOpsEngine,
}

impl ShieldLayer for GitOpsL3 {
    async fn analyze(&self, command: &str, args: &[String], ctx: &CommandContext) -> ShieldVerdict {
        // Step 1: Literal check
        if let Some(denial) = self.literal_checker.check(command, args) {
            return ShieldVerdict::Deny(denial);
        }
        
        // Step 2: Tempo check (circuit breaker + rate limit)
        if let Some(denial) = self.tempo_controller.check(ctx).await {
            return ShieldVerdict::Deny(denial);
        }
        
        // Step 3: Classify action tier
        let tier = self.classifier.classify(command, args);
        
        // Step 4: Impact analysis
        let impact = self.impact_analyzer.analyze(command, args, &ctx).await;
        
        // Step 5: Tiered response
        match tier {
            ActionTier::ReadOnly => {
                // Auto-allow: cat, ls, grep, find, stat, etc.
                ShieldVerdict::Allow { audit: true }
            }
            ActionTier::Destructive => {
                // Backup to vault, then allow
                ShieldVerdict::AutoBackup {
                    impact: impact.clone(),
                    backup_type: impact.suggested_backup(),
                    message: format!(
                        "Destructive: {}. Auto-backup before execution.",
                        impact.summary()
                    ),
                }
            }
            ActionTier::Network => {
                // Escalate for human approval
                ShieldVerdict::Escalate {
                    reason: format!("Network operation: {}", impact.summary()),
                    backup_first: impact.risk_level >= RiskLevel::Medium,
                    channel: ApprovalChannel::Telegram,
                }
            }
            ActionTier::Modify => {
                // Auto-rewrite unsafe params
                let rewritten = self.classifier.rewrite_safe(command, args);
                ShieldVerdict::Modify {
                    original: format!("{} {}", command, args.join(" ")),
                    rewritten: rewritten.join(" "),
                    reason: "Unsafe parameters auto-corrected".into(),
                }
            }
            ActionTier::Blocked => {
                // Hard deny with structured feedback
                ShieldVerdict::Deny(DenialFeedback {
                    reason: format!("Blocked: {}", command),
                    risk_level: RiskLevel::Critical,
                    what_would_be_needed: "Manual override via --force flag + TG approval".into(),
                    remaining_budget: self.tempo_controller.get_budget(ctx).await,
                    alternative: self.suggest_alternative(command, args),
                })
            }
            ActionTier::Unclassified => {
                // Default deny, request human review
                ShieldVerdict::Escalate {
                    reason: "Unclassified command — human review required".into(),
                    backup_first: false,
                    channel: ApprovalChannel::Dashboard,
                }
            }
        }
    }
}
```

### 1.4 LiteralChecker — Защита от shell expansion

**Источник:** Agent Gate

**Проблема:** Агент отправляет `rm $LOG_DIR/*.log` — мы не знаем какие файлы будут удалены.

**Решение:** Reject любую destructive команду с shell expansion, потребовать literal paths.

```rust
pub struct LiteralChecker;

impl LiteralChecker {
    pub fn check(&self, command: &str, args: &[String]) -> Option<DenialFeedback> {
        let destructive_commands = ["rm", "rmdir", "mv", "cp", "chmod", "chown", 
                                     "truncate", "dd", "shred", "tee", "sed"];
        
        if !destructive_commands.contains(&command) {
            return None; // Only check destructive commands
        }
        
        let mut unsafe_args = Vec::new();
        
        for arg in args {
            // Detect shell expansion patterns
            if arg.contains('$')           // $VAR, ${VAR}
                || arg.contains("*")        // *.log, file*.txt
                || arg.contains("?") && !arg.starts_with('-')  // single char wildcard
                || arg.contains('`')        // backtick substitution
                || arg.starts_with("$(")    // command substitution
                || arg.contains("&&")       // command chaining
                || arg.contains("||")       
                || arg.contains(";")        // command separator
                || arg.contains("|")        // pipe
            {
                unsafe_args.push(arg.clone());
            }
        }
        
        if !unsafe_args.is_empty() {
            return Some(DenialFeedback {
                reason: format!(
                    "Shell expansion in destructive command: {}",
                    unsafe_args.iter().map(|a| format!("'{}'", a)).collect::<Vec<_>>().join(", ")
                ),
                risk_level: RiskLevel::High,
                what_would_be_needed: "Use literal paths instead of shell variables/globs".into(),
                remaining_budget: None,
                alternative: Some(format!(
                    "Resolve paths first, then use literal values:\n  \
                     Example: ls {} → then rm file1.log file2.log",
                    unsafe_args[0]
                )),
            });
        }
        
        None
    }
}
```

**Как интегрируется:**
- Вызывается перед L3 классификацией
- Не влияет на read-only команды (cat, ls, grep)
- Агент получает structured denial с подсказкой как исправить
- OpenClaw/Claude автоматически исправят и переотправят с literal paths

### 1.5 TempoController — Circuit Breaker + Rate Limiting

**Источник:** Agent Gate

**Проблема:** Агент в рамках authority может выполнять команды слишком быстро и уронить прод.

**Решение:** Трёх-state circuit breaker + per-tool rate limiting.

```rust
pub struct TempoController {
    rate_limits: RateLimitConfig,
    breaker: CircuitBreaker,
    counters: Arc<Mutex<HashMap<String, SlidingWindowCounter>>>,
}

/// Circuit breaker states
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum BreakerState {
    /// Normal operation
    Closed,
    /// Failure rate exceeded — restrict to read-only
    Open { since: DateTime<Utc>, failure_count: u32 },
    /// Probing — allow limited non-read calls to test recovery
    HalfOpen { probe_remaining: u32 },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RateLimitConfig {
    /// Per-tool rate limits
    /// Example: "rm" → 10 calls per 60s
    pub tools: HashMap<String, ToolRateLimit>,
    
    /// Per-tier defaults (when tool not specified)
    pub tier_defaults: HashMap<ActionTier, ToolRateLimit>,
    
    /// Global limit (all tool calls combined)
    pub global: ToolRateLimit,
    
    /// Circuit breaker config
    pub breaker: BreakerConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolRateLimit {
    pub max_calls: u32,
    pub window_seconds: u64,
    /// What to do on exceed: "deny" | "escalate" | "read_only"
    pub on_exceed: ExceedAction,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BreakerConfig {
    /// Failure rate threshold to trip breaker (0.0 - 1.0)
    pub failure_threshold: f64,
    /// Window for calculating failure rate (seconds)
    pub window_seconds: u64,
    /// Minimum calls before evaluating threshold
    pub min_calls: u32,
    /// How long to stay OPEN before HALF_OPEN (seconds)
    pub open_duration_seconds: u64,
    /// Number of probe calls allowed in HALF_OPEN
    pub half_open_probes: u32,
    /// Exponential backoff for repeated violations
    pub backoff: BackoffConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackoffConfig {
    pub initial_seconds: u64,   // 5s
    pub multiplier: f64,         // 2x
    pub max_seconds: u64,        // 5 min (300s)
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        let mut tools = HashMap::new();
        tools.insert("rm".into(), ToolRateLimit { max_calls: 10, window_seconds: 60, on_exceed: ExceedAction::Deny });
        tools.insert("docker".into(), ToolRateLimit { max_calls: 20, window_seconds: 60, on_exceed: ExceedAction::Escalate });
        tools.insert("apt".into(), ToolRateLimit { max_calls: 5, window_seconds: 300, on_exceed: ExceedAction::Deny });
        tools.insert("systemctl".into(), ToolRateLimit { max_calls: 15, window_seconds: 60, on_exceed: ExceedAction::Escalate });
        tools.insert("cat".into(), ToolRateLimit { max_calls: 200, window_seconds: 60, on_exceed: ExceedAction::Deny });
        
        let mut tier_defaults = HashMap::new();
        tier_defaults.insert(ActionTier::ReadOnly, ToolRateLimit { max_calls: 200, window_seconds: 60, on_exceed: ExceedAction::Deny });
        tier_defaults.insert(ActionTier::Destructive, ToolRateLimit { max_calls: 30, window_seconds: 60, on_exceed: ExceedAction::Escalate });
        tier_defaults.insert(ActionTier::Network, ToolRateLimit { max_calls: 10, window_seconds: 60, on_exceed: ExceedAction::Escalate });
        
        Self {
            tools,
            tier_defaults,
            global: ToolRateLimit { max_calls: 300, window_seconds: 60, on_exceed: ExceedAction::ReadOnly },
            breaker: BreakerConfig {
                failure_threshold: 0.5,
                window_seconds: 60,
                min_calls: 10,
                open_duration_seconds: 120,
                half_open_probes: 3,
                backoff: BackoffConfig {
                    initial_seconds: 5,
                    multiplier: 2.0,
                    max_seconds: 300,
                },
            },
        }
    }
}
```

**Интеграция:**
- Вызывается после LiteralChecker, перед классификацией
- Circuit breaker отслеживает failure rate всех команд (exit code != 0)
- При OPEN — все non-read команды denied с structured feedback
- При HALF_OPEN — разрешается N probe calls
- Rate limits — per-tool sliding window
- При превышении — exponential backoff (5s → 10s → 20s → ... → 5min)
- Successful call within limits сбрасывает backoff

**Structured denial при rate limit:**
```
ACTION DENIED: rm rate limit exceeded. Max 10 calls per 60s.
DETAILS: 11 calls in last 60s (limit: 10).
RATE STATUS: tool_remaining=0, global_remaining=189, breaker=closed
TO PROCEED: Wait 23 seconds for window to clear, or reduce operation frequency.
```

### 1.6 ActionClassifier — Tiered Response

**Источник:** Agent Gate

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Hash)]
pub enum ActionTier {
    /// cat, ls, grep, find, stat, head, tail, wc, file, which, echo, pwd, env
    ReadOnly,
    /// rm, mv, truncate, dd, shred, sed -i, tee (overwrite)
    Destructive,
    /// curl, wget, ssh, scp, rsync, nc
    Network,
    /// chmod 777→755, chown с небезопасными params
    Modify,
    /// rm -rf /, mkfs, curl | bash, :(){:|:&};:
    Blocked,
    /// Всё что не в policy
    Unclassified,
}

pub struct ActionClassifier {
    /// Карта: команда → (pattern → tier)
    /// Загружается из YAML policy
    rules: Vec<ClassificationRule>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClassificationRule {
    pub command: String,           // "rm", "chmod", "docker"
    pub tier: ActionTier,
    pub conditions: Vec<RuleCondition>,
    pub rewrite: Option<RewriteRule>,
    pub message: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RuleCondition {
    /// Flag присутствует в args
    HasFlag(String),
    /// Arg matches regex
    ArgMatches { index: usize, pattern: String },
    /// Arg contains value
    ArgContains { value: String },
    /// Path is in protected list
    PathProtected { patterns: Vec<String> },
    /// All args match pattern
    AllArgsLiteral,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RewriteRule {
    /// Replace specific args
    pub replacements: Vec<RewriteReplacement>,
    pub message: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RewriteReplacement {
    pub match_pattern: String,    // "777"
    pub replace_with: String,     // "755"
}
```

**Classification rules (YAML):**

```yaml
# policies/classification.yaml
rules:
  # === READ-ONLY ===
  - command: "cat"
    tier: read_only
  - command: "ls"
    tier: read_only
  - command: "grep"
    tier: read_only
  - command: "find"
    tier: read_only
  - command: "stat"
    tier: read_only
  - command: "ps"
    tier: read_only
  - command: "df"
    tier: read_only
  - command: "free"
    tier: read_only
  - command: "docker"
    conditions:
      - type: arg_contains
        value: "ps"
      - type: arg_contains
        value: "logs"
      - type: arg_contains
        value: "inspect"
      - type: arg_contains
        value: "images"
    tier: read_only

  # === MODIFY (auto-rewrite) ===
  - command: "chmod"
    conditions:
      - type: arg_contains
        value: "777"
    tier: modify
    rewrite:
      replacements:
        - match: "777"
          replace: "755"
      message: "chmod 777 auto-corrected to 755 for security"

  - command: "chmod"
    conditions:
      - type: arg_contains
        value: "666"
    tier: modify
    rewrite:
      replacements:
        - match: "666"
          replace: "644"
      message: "chmod 666 auto-corrected to 644 for security"

  # === DESTRUCTIVE (backup + allow) ===
  - command: "rm"
    tier: destructive
  - command: "mv"
    tier: destructive
  - command: "truncate"
    tier: destructive
  - command: "dd"
    tier: destructive
  - command: "sed"
    conditions:
      - type: has_flag
        flag: "-i"
    tier: destructive
  - command: "docker"
    conditions:
      - type: arg_contains
        value: "rm"
      - type: arg_contains
        value: "rmi"
      - type: arg_contains
        value: "down"
    tier: destructive
  - command: "systemctl"
    conditions:
      - type: arg_contains
        value: "stop"
      - type: arg_contains
        value: "disable"
    tier: destructive
  - command: "apt"
    conditions:
      - type: arg_contains
        value: "remove"
      - type: arg_contains
        value: "purge"
    tier: destructive

  # === NETWORK (escalate) ===
  - command: "curl"
    tier: network
  - command: "wget"
    tier: network
  - command: "ssh"
    tier: network
  - command: "scp"
    tier: network
  - command: "rsync"
    tier: network

  # === BLOCKED (hard deny) ===
  - command: "rm"
    conditions:
      - type: arg_contains
        value: "-rf /"
    tier: blocked
    message: "Root deletion blocked. This would destroy the entire filesystem."

  - command: "mkfs"
    tier: blocked
    message: "Filesystem formatting blocked."

  - command: "dd"
    conditions:
      - type: arg_contains
        value: "of=/dev/"
    tier: blocked
    message: "Direct device write blocked."

  - command: "chmod"
    conditions:
      - type: path_protected
        patterns: ["/etc/shadow", "/etc/passwd", "/etc/ssh", "/root/.ssh"]
      - type: arg_contains
        value: "777"
    tier: blocked
    message: "Cannot open critical system files to world access."

  # === SQL patterns (via psql/mysql/sqlite3 commands) ===
  - command: "psql"
    conditions:
      - type: arg_matches
        index: 0
        pattern: "(?i).*DROP\\s+(TABLE|DATABASE|SCHEMA).*"
    tier: destructive
  - command: "psql"
    conditions:
      - type: arg_matches
        index: 0
        pattern: "(?i).*TRUNCATE.*"
    tier: destructive
  - command: "psql"
    conditions:
      - type: arg_matches
        index: 0
        pattern: "(?i).*DELETE\\s+FROM.*WHERE\\s+1\\s*=\\s*1.*"
    tier: destructive
```

**Интеграция:**
- Rules загружаются из YAML при старте агента
- Каждое правило компилируется в Rust matcher
- Classification result передаётся в tempo_controller и impact_analyzer
- MODIFY tier автоматически переписывает unsafe params
- Все правила versioned в git (policies/classification.yaml)

### 1.7 MODIFY Verdict — Auto-Rewrite

**Источник:** Agent Gate

**Концепция:** Не блокировать unsafe params, а автоматически исправлять.

```rust
impl ActionClassifier {
    pub fn rewrite_safe(&self, command: &str, args: &[String]) -> Vec<String> {
        let rule = self.find_matching_rule(command, args);
        
        if let Some(rule) = rule {
            if let Some(rewrite) = &rule.rewrite {
                let mut new_args = args.to_vec();
                
                for replacement in &rewrite.replacements {
                    for arg in &mut new_args {
                        *arg = arg.replace(&replacement.match_pattern, &replacement.replace_with);
                    }
                }
                
                return new_args;
            }
        }
        
        args.to_vec()
    }
}
```

**Примеры:**

| Оригинал | Auto-rewritten | Почему |
|----------|---------------|--------|
| `chmod 777 /var/www` | `chmod 755 /var/www` | 777 = world writable |
| `chmod 666 /etc/config` | `chmod 644 /etc/config` | 666 = world writable |
| `rm -rf /var/log/*.log` | **DENIED** | Glob in destructive |
| `rm -rf /var/log/app.log /var/log/error.log` | ✅ (backup first) | Literal paths OK |

**Интеграция:**
- После classification, если tier=Modify → auto-rewrite args
- Оригинальная команда сохраняется в audit entry
- Rewritten команда выполняется
- Agent получает feedback: "chmod 777 auto-corrected to 755"

### 1.8 Structured Denial Feedback

**Источник:** Agent Gate

При любом deny/escalate агент получает actionable информацию:

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DenialFeedback {
    /// Почему заблокировано
    pub reason: String,
    
    /// Уровень риска
    pub risk_level: RiskLevel,
    
    /// Что нужно чтобы команда прошла
    pub what_would_be_needed: String,
    
    /// Оставшийся rate limit budget
    pub remaining_budget: Option<RateBudget>,
    
    /// Альтернативная безопасная команда
    pub alternative: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RateBudget {
    pub tool_remaining: u32,
    pub tool_reset_in_seconds: u64,
    pub global_remaining: u32,
    pub breaker_state: BreakerState,
}
```

**Примеры feedback:**

```
# Rate limit exceeded
ACTION DENIED: rm rate limit exceeded. Max 10 calls per 60s.
DETAILS: 11 calls in last 60s (limit: 10).
RATE STATUS: tool_remaining=0, global_remaining=189, breaker=closed
TO PROCEED: Wait 23 seconds for window to clear.

# Shell expansion
ACTION DENIED: Shell expansion in destructive command: '$LOG_DIR/*.log'
TO PROCEED: Resolve paths first, then use literal values.
ALTERNATIVE: ls $LOG_DIR/*.log → then rm file1.log file2.log

# Blocked
ACTION DENIED: Root deletion blocked. rm -rf / would destroy the filesystem.
RISK: Critical
ALTERNATIVE: Use targeted deletion: rm -rf /path/to/specific/directory

# Circuit breaker open
ACTION DENIED: Circuit breaker OPEN — failure rate 60% exceeds threshold (50%).
RATE STATUS: breaker=open, read_only until 14:35:00 UTC
TO PROCEED: Wait 3 minutes for breaker recovery.
```

---

## 📦 Модуль 2: `state_collector` — State Collection & Tracking

### 2.1 Collectors

12 collectors (из v1 плана, без изменений). Каждый реализует `StateCollector` trait.

### 2.2 Event-Driven Drift (НОВОЕ — из ArgoCD)

**Проблема:** Polling каждые N минут — задержка обнаружения drift.

**Решение:** inotify/fsnotify для файлов + event-driven для Docker/systemd.

```rust
pub struct EventDrivenCollector {
    /// File watcher for tracked config files
    file_watcher: RecommendedWatcher,  // notify crate
    
    /// Docker event stream
    docker_events: DockerEventStream,
    
    /// systemd journal
    journal_watcher: JournalWatcher,
    
    /// Callback on change
    on_change: Box<dyn Fn(DriftEvent) + Send + Sync>,
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
    /// File changed (inotify)
    FileChange { path: String, kind: FileChangeKind },
    /// Docker container state changed
    DockerEvent { container: String, action: String },
    /// systemd service state changed
    SystemdEvent { service: String, from: String, to: String },
    /// Manual check found drift
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
```

**Интеграция:**
- Запускается как background task при старте агента
- inotify watches на `/etc/`, tracked config paths, docker compose files
- Docker event stream через `docker events --filter`
- systemd через `journalctl -f` или D-Bus
- При событии → немедленный DriftEvent → classify → auto-fix или alert
- События логируются в audit trail

### 2.3 State Diff with Semantic Understanding

**Источник:** ArgoCD semantic diff

```rust
pub struct SemanticDiffer;

impl SemanticDiffer {
    /// Diff two states with semantic understanding
    pub fn diff(&self, current: &ServerState, desired: &ServerState) -> Vec<SemanticDrift> {
        let mut drifts = Vec::new();
        
        // Per-component semantic diff
        for (component, desired_data) in &desired.components {
            let current_data = current.components.get(component);
            
            match component.as_str() {
                "packages" => {
                    // Package diff: version changes, additions, removals
                    drifts.extend(self.diff_packages(current_data, desired_data));
                }
                "services" => {
                    // Service diff: status changes, enabled/disabled
                    drifts.extend(self.diff_services(current_data, desired_data));
                }
                "docker" => {
                    // Docker diff: container state, image versions, env changes
                    drifts.extend(self.diff_docker(current_data, desired_data));
                }
                "files" => {
                    // File diff: content hash changes, permission changes
                    drifts.extend(self.diff_files(current_data, desired_data));
                }
                _ => {
                    // Generic JSON diff
                    drifts.extend(self.diff_generic(component, current_data, desired_data));
                }
            }
        }
        
        drifts
    }
    
    fn diff_packages(&self, current: Option<&ComponentState>, desired: &ComponentState) -> Vec<SemanticDrift> {
        // Parse package lists
        // Report: added packages, removed packages, version changes
        // Severity: Low (add/remove) | Medium (version downgrade) | High (security package removed)
        // ...
    }
    
    fn diff_docker(&self, current: Option<&ComponentState>, desired: &ComponentState) -> Vec<SemanticDrift> {
        // Parse docker state
        // Report: container stopped, image changed, port mapped, volume removed
        // Severity: Low (container stopped) | Medium (image tag changed) | High (port exposed to 0.0.0.0)
        // ...
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SemanticDrift {
    pub component: String,
    pub path: String,            // JSON path: "containers[nginx].status"
    pub change_type: ChangeType, // Added, Removed, Modified
    pub old_value: serde_json::Value,
    pub new_value: serde_json::Value,
    pub severity: DriftSeverity,
    pub category: DriftCategory,
    pub auto_fixable: bool,
    pub auto_fix_command: Option<String>,
    pub explanation: String,     // Human-readable explanation
}
```

---

## 📦 Модуль 3: `backup_engine` — Smart Backup с Vault

### 3.1 Vault Architecture (НОВОЕ — из Agent Gate)

**Ключевой принцип:** Бэкапы хранятся в vault, недоступном агенту.

```
Server filesystem:
├── /home/user/app/          ← Agent has access
├── /var/lib/docker/         ← Agent has access
├── /etc/                    ← Agent has access
└── /opt/flowlink-vault/     ← Agent CANNOT access (separate mount, chmod 700 root:root)
    └── backups/
        ├── pre-exec-{id}/
        │   ├── manifest.json
        │   ├── files.tar.gz
        │   ├── database/
        │   └── docker-state/
        └── index.json
```

```rust
pub struct VaultConfig {
    /// Vault directory (outside agent's permitted envelope)
    /// Default: /opt/flowlink-vault (Linux), /usr/local/flowlink-vault (macOS)
    pub vault_path: PathBuf,
    
    /// Vault permissions: 0o700 (drwx------)
    /// Owned by root, agent process cannot read/write
    pub permissions: u32,
    
    /// Encryption at rest
    pub encryption: VaultEncryption,
    
    /// Maximum vault size (rotate old backups)
    pub max_size_mb: u64,
    
    /// Retention policy
    pub retention: RetentionPolicy,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum VaultEncryption {
    /// AES-256-GCM with key derived from machine ID + agent token
    MachineKey,
    /// AES-256-GCM with provided key (stored in relay config)
    ProvidedKey { key_id: String },
    /// No encryption (local-only, trusted environment)
    None,
}
```

**Интеграция:**
- При установке агента создаётся vault directory с правильными permissions
- Vault path добавляется в agent config
- Agent process (non-root) не может читать vault
- Backup process выполняется через sudo/helper с root privileges
- Agent отправляет "backup request" → helper процесс выполняет backup → возвращает manifest

### 3.2 Impact-Aware Backup (из v1, без изменений)

ImpactAnalyzer определяет что бэкапить на основе команды.

### 3.3 Auto-Restore on Anomaly (НОВОЕ — из Cohesity)

**Концепция:** Если health check падает после команды → автоматически откатить.

```rust
pub struct AutoRestoreEngine {
    backup_engine: Arc<BackupEngine>,
    health_checker: Arc<HealthChecker>,
    policy: AutoRestorePolicy,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AutoRestorePolicy {
    /// Enable auto-restore on health failure
    pub enabled: bool,
    
    /// How long to wait after command before health check
    pub check_delay_seconds: u64,    // default: 10
    
    /// Health checks to perform
    pub checks: Vec<HealthCheck>,
    
    /// Auto-restore if ANY check fails
    pub restore_on_failure: bool,
    
    /// Maximum auto-restores per hour (prevent restore loops)
    pub max_restores_per_hour: u32,  // default: 3
    
    /// Notify via TG on auto-restore
    pub notify: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum HealthCheck {
    /// HTTP GET expects 200
    HttpGet { url: String, expected_status: u16 },
    /// TCP port is listening
    TcpPort { port: u16 },
    /// Process is running
    ProcessRunning { name: String },
    /// Docker container is running
    DockerContainer { name: String },
    /// Systemd service is active
    SystemdService { name: String },
    /// Custom command (exit 0 = healthy)
    CustomCommand { command: String },
    /// Database responds to ping
    DatabasePing { db_type: DbType, host: String, port: u16 },
    /// Disk usage below threshold
    DiskUsage { path: String, max_percent: u8 },
    /// Memory usage below threshold  
    MemoryUsage { max_percent: u8 },
}
```

**Поток автоматического восстановления:**

```
1. Command executes (e.g., apt upgrade nginx)
2. Wait 10 seconds (check_delay_seconds)
3. Run health checks:
   - HTTP GET http://localhost → 502 Bad Gateway ❌
   - Docker container nginx → stopped ❌
   - Systemd nginx → failed ❌
4. ALL checks failing → trigger auto-restore
5. Find most recent pre-exec backup for this command
6. Restore from backup:
   - Restore nginx config
   - Restart container
   - Restart systemd service
7. Re-run health checks:
   - HTTP GET → 200 OK ✅
   - Docker → running ✅
8. Notify: "⚠️ Auto-restore triggered for 'apt upgrade nginx'.
   Service was down for 12 seconds. Restored from backup #47.
   Review: flowlink audit show <id>"
```

**Интеграция:**
- AutoRestoreEngine вызывается после каждой destructive команды
- Health checks определяются в YAML config (per-server)
- Если auto-restore не помогает → escalate через TG
- Rate limited: max 3 auto-restores/hour чтобы избежать restore loops
- Логируется в audit trail как отдельная запись

---

## 📦 Модуль 4: `audit_trail` — Immutable Audit с Policy Hash

### 4.1 AuditEntry (из v1 + дополнения)

```rust
#[derive(Serialize, Deserialize, Clone)]
pub struct AuditEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub agent_id: String,
    pub session_id: String,
    
    // Command context
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env_var_names: Vec<String>,   // ONLY names, never values
    
    // Shield analysis
    pub risk_level: RiskLevel,
    pub shield_verdict: ShieldVerdictType, // Allow, Block, AutoBackup, RequireApproval, Modify
    pub shield_rules_matched: Vec<String>,
    pub tier: ActionTier,
    
    // MODIFY verdict details
    pub original_command: Option<String>,  // Before rewrite
    pub rewritten_command: Option<String>, // After rewrite
    
    // Tempo at time of execution
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
    
    // Health check result (post-exec)
    pub health_check: Option<HealthCheckResult>,
    pub auto_restored: bool,
    pub auto_restore_backup_id: Option<String>,
    
    // Integrity chain (НОВОЕ — policy hash)
    pub policy_hash: String,          // SHA256 of active policy at decision time
    pub classification_rule: Option<String>, // Which rule matched
    pub hmac: String,                 // HMAC chain
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HealthCheckResult {
    pub checks: Vec<IndividualCheck>,
    pub overall: HealthStatus,
    pub checked_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IndividualCheck {
    pub check: HealthCheck,
    pub result: CheckResult,  // Pass, Fail, Error
    pub detail: String,
    pub latency_ms: Option<u64>,
}
```

### 4.2 Policy Hash в Audit

**Источник:** Agent Gate

**Проблема:** Audit entry говорит "blocked by rule X", но что если правило потом изменили? Как доказать что правило было именно таким?

**Решение:** SHA256 активной политики на момент решения — в каждую audit entry.

```rust
impl AuditTrail {
    async fn create_entry(&self, command: &str, verdict: &ShieldVerdict) -> AuditEntry {
        // Snapshot current policy state
        let policy_hash = self.policy_engine.compute_hash().await;
        // = SHA256(shield.yaml + classification.yaml + approval_policies.yaml + backup_rules.yaml)
        
        AuditEntry {
            // ... all fields ...
            policy_hash,
            classification_rule: verdict.matched_rule().map(|r| r.name.clone()),
            hmac: self.compute_hmac(/* previous entry */),
        }
    }
}

impl PolicyEngine {
    pub async fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&self.shield_rules_raw);
        hasher.update(&self.classification_rules_raw);
        hasher.update(&self.approval_policies_raw);
        hasher.update(&self.backup_rules_raw);
        format!("{:x}", hasher.finalize())
    }
}
```

**Интеграция:**
- Policy hash вычисляется один раз при загрузке policies
- Кэшируется, обновляется при reload
- Записывается в каждую AuditEntry
- При integrity check можно verify: загрузить policy из git history → сравнить hash

---

## 📦 Модуль 5: `drift_detector` — Event-Driven Drift Detection

### 5.1 Drift Pipeline (обновлённый)

```
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│  Event Sources   │  │  Periodic       │  │  Manual          │
│  (inotify,      │  │  Collectors     │  │  Trigger         │
│   docker events, │  │  (every N min)  │  │  (API/CLI)       │
│   systemd)       │  │                 │  │                  │
└────────┬────────┘  └────────┬────────┘  └────────┬────────┘
         │                    │                     │
         └────────────────────┼─────────────────────┘
                              ▼
                   ┌─────────────────────┐
                   │  Drift Classifier   │
                   │  - Severity         │
                   │  - Category         │
                   │  - Auto-fixable?    │
                   └──────────┬──────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
     ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
     │  Auto-Fix    │ │  Alert       │ │  Ignore      │
     │  (safe)      │ │  (needs      │ │  (cosmetic)  │
     │              │ │  human)      │ │              │
     └──────┬───────┘ └──────┬───────┘ └──────────────┘
            │                │
            ▼                ▼
     ┌──────────────┐ ┌──────────────┐
     │  Commit Fix  │ │  SSE Notify  │
     │  + Audit     │ │  → Dashboard │
     └──────────────┘ │  → Telegram  │
                      └──────────────┘
```

### 5.2 Auto-Fix Rules (YAML)

```yaml
# policies/drift_auto_fix.yaml
rules:
  # === Docker ===
  - name: "Restart stopped Docker containers"
    component: "docker"
    condition: "container.expected_running == true && container.is_running == false"
    action: "docker start {container.name}"
    auto_fix: true
    notify: true
    max_retries: 3
    retry_delay_seconds: 30
    severity: medium

  - name: "Alert on new Docker images"
    component: "docker"
    condition: "image.added && !image.in_desired_state"
    action: "alert"
    auto_fix: false
    notify: true
    severity: low

  # === Services ===
  - name: "Restart crashed services"
    component: "services"
    condition: "service.status == 'failed' && service.restart_count < 3"
    action: "systemctl restart {service.name}"
    auto_fix: true
    notify: true
    max_retries: 3
    severity: medium

  - name: "Alert on disabled services"
    component: "services"
    condition: "service.was_enabled && !service.is_enabled"
    action: "alert"
    auto_fix: false
    notify: true
    severity: high

  # === Files ===
  - name: "Auto-fix config file changes"
    component: "files"
    condition: "file.in_tracked_paths && file.content_hash_changed && file.has_known_format"
    action: "restore_from_git {file.path}"
    auto_fix: true
    notify: true
    severity: high
    excluded_paths: ["/etc/machine-id", "/etc/hostname"]

  - name: "Alert on SSH authorized_keys changes"
    component: "files"
    condition: "file.path.matches('*/.ssh/authorized_keys') && file.content_changed"
    action: "alert"
    auto_fix: false
    notify: true
    severity: critical

  # === Firewall ===
  - name: "Alert on firewall rule changes"
    component: "firewall"
    condition: "rules.changed"
    action: "alert"
    auto_fix: false
    notify: true
    severity: critical

  - name: "Alert on new listening ports"
    component: "network"
    condition: "port.added && !port.in_desired_state"
    action: "alert"
    auto_fix: false
    notify: true
    severity: high

  # === Users ===
  - name: "Alert on new users"
    component: "users"
    condition: "user.added && !user.in_desired_state"
    action: "alert"
    auto_fix: false
    notify: true
    severity: critical

  - name: "Alert on new SSH keys"
    component: "users"
    condition: "ssh_key.added"
    action: "alert"
    auto_fix: false
    notify: true
    severity: critical

  # === Packages ===
  - name: "Alert on removed security packages"
    component: "packages"
    condition: "package.removed && package.is_security_related"
    action: "alert"
    auto_fix: false
    notify: true
    severity: critical
    security_packages: ["fail2ban", "ufw", "openssh-server", "openssl"]
```

---

## 📦 Модуль 6: `plan_engine` — Preview Mode (НОВОЕ — из Spacelift)

### 6.1 `flowlink plan` — Dry-Run с Impact Report

**Концепция:** Как `terraform plan` — показать что изменится без выполнения.

```rust
pub struct PlanEngine {
    classifier: Arc<ActionClassifier>,
    impact_analyzer: Arc<ImpactAnalyzer>,
    state_manager: Arc<StateManager>,
    backup_engine: Arc<BackupEngine>,
}

impl PlanEngine {
    /// Generate execution plan without running anything
    pub async fn plan(&self, command: &str, args: &[String], ctx: &CommandContext) -> ExecutionPlan {
        // 1. Classify
        let tier = self.classifier.classify(command, args);
        
        // 2. Check tempo
        // ... (rate limits, circuit breaker status)
        
        // 3. Analyze impact
        let impact = self.impact_analyzer.analyze(command, args, ctx).await;
        
        // 4. Determine backup needs
        let backup_plan = if tier == ActionTier::Destructive || tier == ActionTier::Network {
            Some(self.backup_engine.plan_backup(&impact).await)
        } else {
            None
        };
        
        // 5. Predict state change
        let current_state = self.state_manager.get_current_state().await;
        let predicted_state = self.predict_state_change(command, args, &current_state);
        
        // 6. Health checks that will run after
        let post_checks = self.get_post_exec_health_checks(command, &impact);
        
        ExecutionPlan {
            command: format!("{} {}", command, args.join(" ")),
            classification: tier,
            risk_level: impact.risk_level,
            verdict: match tier {
                ActionTier::ReadOnly => PlanVerdict::WillAllow,
                ActionTier::Destructive => PlanVerdict::WillBackupAndExecute(backup_plan.unwrap()),
                ActionTier::Network => PlanVerdict::WillEscalate,
                ActionTier::Modify => PlanVerdict::WillRewrite(/* ... */),
                ActionTier::Blocked => PlanVerdict::WillBlock,
                ActionTier::Unclassified => PlanVerdict::WillEscalate,
            },
            impact: impact.clone(),
            files_at_risk: impact.files_at_risk,
            databases_at_risk: impact.databases_at_risk,
            containers_at_risk: impact.containers_at_risk,
            services_at_risk: impact.services_at_risk,
            predicted_state_changes: predicted_state,
            backup_plan,
            post_exec_health_checks: post_checks,
            estimated_duration_ms: impact.estimated_backup_time_ms + 1000, // backup + exec
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExecutionPlan {
    pub command: String,
    pub classification: ActionTier,
    pub risk_level: RiskLevel,
    pub verdict: PlanVerdict,
    pub impact: ImpactReport,
    pub files_at_risk: Vec<String>,
    pub databases_at_risk: Vec<String>,
    pub containers_at_risk: Vec<String>,
    pub services_at_risk: Vec<String>,
    pub predicted_state_changes: HashMap<String, serde_json::Value>,
    pub backup_plan: Option<BackupPlan>,
    pub post_exec_health_checks: Vec<HealthCheck>,
    pub estimated_duration_ms: u64,
}
```

**CLI Usage:**

```bash
# Preview what will happen
$ flowlink plan "apt install nginx"

📋 Execution Plan
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Command:    apt install nginx
Tier:       Destructive (package install)
Risk:       Low
Verdict:    ✅ Will execute (auto-backup first)

📦 Backup Plan:
  Type:     SystemConfig (packages)
  Size:     ~12KB (package list snapshot)
  Duration: <1s

🎯 Predicted Impact:
  + package: nginx (new)
  + package: nginx-common (new, dependency)
  + package: libnginx-mod-http-geoip2 (new, dependency)
  + service: nginx (new, will be enabled and started)
  + file: /etc/nginx/nginx.conf (new)
  + file: /etc/nginx/sites-available/default (new)

🩺 Post-exec Health Checks:
  ✓ systemctl is-active nginx
  ✓ curl -s http://localhost > /dev/null

⏱️  Estimated duration: 15-30s + 1s backup

Run without plan: flowlink exec "apt install nginx"
```

**Интеграция:**
- `flowlink plan` — CLI команда
- `POST /api/v1/gitops/plan` — API endpoint
- Dashboard: "Plan" button перед выполнением
- Relay proxy: relay может запросить plan перед approval

---

## 📦 Модуль 7: `git_ops_engine` — Core Git Operations

(Без изменений из v1 — commit, push, pull, branches, rollback)

---

## 📦 Модуль 8: `approval_flow` — Multi-Channel Approval

### 8.1 Approval Channels (расширенные)

| Channel | Trigger | UI | Response time |
|---------|---------|-----|---------------|
| **Telegram** | Tier=Network, RequireApproval | Inline buttons (✅/❌) + impact details | Instant |
| **Dashboard** | All escalations | Card with full context + plan | On-demand |
| **CLI** | Manual | `flowlink approve {id}` | On-demand |
| **API** | Automation | `POST /api/v1/gitops/approvals/{id}/approve` | On-demand |
| **Git PR comment** | GitOps flow | Comment `/approve` or `/reject` on PR | On-demand |

### 8.2 Approval Request Format

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub agent_id: String,
    pub server_hostname: String,
    
    // What triggered
    pub command: String,
    pub args: Vec<String>,
    pub tier: ActionTier,
    pub risk_level: RiskLevel,
    
    // Context
    pub execution_plan: ExecutionPlan,    // Full plan with impact
    pub backup_status: BackupStatus,      // Pre-created backup info
    pub matched_rules: Vec<String>,       // Which policies triggered
    
    // Timing
    pub expires_at: DateTime<Utc>,
    pub auto_reject_at: DateTime<Utc>,
    
    // State
    pub status: ApprovalStatus,
    pub approved_by: Option<ApprovalIdentity>,
    pub rejected_by: Option<ApprovalIdentity>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ApprovalStatus {
    PendingBackup,
    PendingApproval,
    Approved,
    Rejected { reason: String },
    Expired,
    Executing,
    Completed { exit_code: i32 },
    Failed { error: String },
    AutoRestored { backup_id: String },
}
```

### 8.3 Telegram Approval Message

```
🤖 FlowLink: Approval Required
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🖥️ Server: prod-web-01
⚡ Command: docker compose down
📊 Tier: Destructive | Risk: High

📋 Impact:
  • 5 containers will be stopped
  • 12 services dependent
  • 3 volumes may be affected

💾 Backup: ✅ Ready (#47, 2.1MB)
  • docker-state snapshot created
  • Config files preserved
  • Database dump included

⏱️  Expires: 30 minutes
Policy: "Docker compose operations require approval"

[✅ Approve] [❌ Reject] [🔍 View Plan]
```

---

## 📦 Модуль 9: `remote_sync` — Remote Git Sync

(Без изменений из v1 — Git/S3/Relay backends, sync strategies)

---

## 📦 Модуль 10: Integration Glue

### 10.1 Shield + GitOps Integration Point

```rust
impl ShieldEngine {
    /// Main entry point — replaces current analyze()
    pub async fn analyze_command(
        &self,
        command: &str,
        args: &[String],
        ctx: &CommandContext,
    ) -> CommandDecision {
        // L1: Pattern match (fast, no I/O)
        let l1 = self.l1.analyze(command, args);
        if let Some(verdict) = l1.as_verdict() {
            return CommandDecision::from_verdict(verdict, None, None);
        }
        
        // L2: Context analysis (some I/O)
        let l2 = self.l2.analyze(command, args, ctx).await;
        if let Some(verdict) = l2.as_verdict() {
            return CommandDecision::from_verdict(verdict, None, None);
        }
        
        // L3: Full GitOps pipeline (significant I/O)
        let l3 = self.l3.analyze(command, args, ctx).await;
        
        // Enrich with GitOps context
        let gitops_context = self.build_gitops_context(ctx).await;
        
        // State-aware + drift-aware + frequency-aware analysis
        let enriched = self.enrich_with_context(l3, &gitops_context).await;
        
        enriched
    }
    
    async fn enrich_with_context(
        &self,
        base: ShieldVerdict,
        ctx: &GitOpsContext,
    ) -> CommandDecision {
        // Check: command frequency in last 30min
        if let Some(freq) = &ctx.recent_command_frequency {
            if freq.same_command_count > 5 {
                return CommandDecision::deny(
                    "Command executed >5 times in 30min — possible loop or attack",
                    ctx.remaining_budget.clone(),
                );
            }
        }
        
        // Check: unresolved high-severity drift exists
        if let Some(drifts) = &ctx.unresolved_drifts {
            if drifts.iter().any(|d| d.severity >= DriftSeverity::High) {
                // Downgrade verdict: Allow → RequireApproval
                match base {
                    ShieldVerdict::Allow { .. } => {
                        return CommandDecision::escalate(
                            "High-severity drift detected — additional caution required",
                            true,
                        );
                    }
                    _ => {}
                }
            }
        }
        
        // Check: state-aware risk
        if let Some(state) = &ctx.current_state {
            let impact = self.analyze_state_impact(/* ... */);
            if impact.severity >= DriftSeverity::Critical {
                return CommandDecision::deny(
                    &format!("Critical state impact: {}", impact.description),
                    ctx.remaining_budget.clone(),
                );
            }
        }
        
        CommandDecision::from_verdict(base, ctx.remaining_budget.clone(), ctx.policy_hash.clone())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GitOpsContext {
    pub current_state: Option<ServerState>,
    pub recent_command_frequency: Option<CommandFrequency>,
    pub unresolved_drifts: Option<Vec<ClassifiedDrift>>,
    pub remaining_budget: Option<RateBudget>,
    pub policy_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CommandFrequency {
    pub same_command_count: u32,
    pub similar_tier_count: u32,
    pub total_last_30min: u32,
    pub commands: Vec<String>,
}
```

### 10.2 Full Command Execution Flow

```rust
pub async fn execute_command(
    command: &str,
    args: &[String],
    ctx: &CommandContext,
    engines: &Engines,
) -> CommandResult {
    // 1. Analyze through Shield pipeline
    let decision = engines.shield.analyze_command(command, args, ctx).await;
    
    // 2. Handle verdict
    match decision.verdict {
        ShieldVerdict::Deny(feedback) => {
            // Log to audit as blocked
            engines.audit.log_blocked(command, args, &feedback).await;
            return CommandResult::Blocked(feedback);
        }
        
        ShieldVerdict::Allow { audit } => {
            // Execute directly
            let result = engines.executor.exec(command, args, ctx).await;
            if audit {
                engines.audit.log_executed(command, args, &result, &decision).await;
            }
            engines.state.collect_and_commit().await;
            return CommandResult::Executed(result);
        }
        
        ShieldVerdict::Modify { original, rewritten, reason } => {
            // Execute rewritten command
            let rewritten_parts = shell_words::split(&rewritten)?;
            let result = engines.executor.exec(&rewritten_parts[0], &rewritten_parts[1..], ctx).await;
            engines.audit.log_modified(&original, &rewritten, &reason, &result, &decision).await;
            engines.state.collect_and_commit().await;
            return CommandResult::Modified {
                original,
                rewritten,
                result,
                reason,
            };
        }
        
        ShieldVerdict::AutoBackup { impact, backup_type, message } => {
            // Create vault backup
            let backup = engines.backup.create_vault_backup(&impact, &backup_type).await?;
            
            // Execute command
            let result = engines.executor.exec(command, args, ctx).await;
            
            // Post-exec health check
            let health = engines.health.check_post_exec(&impact).await;
            
            // Auto-restore if unhealthy
            if !health.is_healthy() && engines.auto_restore.should_restore() {
                let restore_result = engines.backup.restore(&backup.id).await?;
                engines.audit.log_auto_restored(command, &backup, &restore_result, &decision).await;
                engines.state.collect_and_commit().await;
                return CommandResult::AutoRestored {
                    command: command.into(),
                    backup_id: backup.id,
                    health,
                    restore: restore_result,
                };
            }
            
            engines.audit.log_executed_with_backup(command, args, &result, &backup, &health, &decision).await;
            engines.state.collect_and_commit().await;
            return CommandResult::ExecutedWithBackup {
                result,
                backup_id: backup.id,
                health,
            };
        }
        
        ShieldVerdict::Escalate { reason, backup_first, channel } => {
            // Create backup if needed
            let backup = if backup_first {
                let impact = engines.impact_analyzer.analyze(command, args, ctx).await;
                Some(engines.backup.create_vault_backup(&impact, &impact.suggested_backup()).await?)
            } else {
                None
            };
            
            // Create approval request
            let plan = engines.planner.plan(command, args, ctx).await;
            let approval = engines.approval.create_request(
                command, args, &plan, backup.as_ref(), &reason, channel
            ).await?;
            
            // Wait for approval (with timeout)
            match engines.approval.wait_for_decision(&approval.id).await {
                ApprovalDecision::Approved { by } => {
                    // Execute
                    let result = engines.executor.exec(command, args, ctx).await;
                    let health = engines.health.check_post_exec(/* ... */).await;
                    engines.audit.log_approved_exec(command, args, &result, &approval, &by, &health, &decision).await;
                    engines.state.collect_and_commit().await;
                    CommandResult::ExecutedWithApproval { result, approved_by: by, health }
                }
                ApprovalDecision::Rejected { by, reason } => {
                    engines.audit.log_rejected(command, args, &approval, &by, &reason, &decision).await;
                    CommandResult::Rejected { by, reason }
                }
                ApprovalDecision::Expired => {
                    engines.audit.log_expired(command, args, &approval, &decision).await;
                    CommandResult::Expired
                }
            }
        }
    }
}
```

---

## 📊 Полный API Surface (Relay)

```
# GitOps
GET    /api/v1/gitops/state                    ← текущее состояние
GET    /api/v1/gitops/state/history             ← история состояний (paginated)
GET    /api/v1/gitops/state/diff?from=X&to=Y    ← diff между состояниями
POST   /api/v1/gitops/state/apply               ← применить desired state
POST   /api/v1/gitops/plan                      ← preview (dry-run)
GET    /api/v1/gitops/policy                    ← текущая политика + hash
PUT    /api/v1/gitops/policy                    ← обновить политику
GET    /api/v1/gitops/policy/history            ← история изменений политики

# Drift
GET    /api/v1/gitops/drift                     ← текущие drifts
POST   /api/v1/gitops/drift/fix/{id}            ← auto-fix drift
POST   /api/v1/gitops/drift/ignore/{id}         ← игнорировать drift
GET    /api/v1/gitops/drift/history             ← история drifts

# Backup
GET    /api/v1/gitops/backups                   ← список бэкапов (paginated)
POST   /api/v1/gitops/backups/create            ← создать бэкап (manual)
POST   /api/v1/gitops/backups/{id}/restore      ← восстановить
DELETE /api/v1/gitops/backups/{id}              ← удалить
GET    /api/v1/gitops/backups/{id}/verify       ← проверить целостность
GET    /api/v1/gitops/backups/{id}/contents     ← что внутри бэкапа

# Audit
GET    /api/v1/gitops/audit                     ← audit log (paginated, filterable)
GET    /api/v1/gitops/audit/{id}                ← конкретная запись
GET    /api/v1/gitops/audit/integrity           ← integrity check
GET    /api/v1/gitops/audit/search?q=X          ← поиск по audit
GET    /api/v1/gitops/audit/stats               ← статистика (frequency, tiers, etc.)

# Rollback
POST   /api/v1/gitops/rollback/commit/{sha}    ← откатить к коммиту
POST   /api/v1/gitops/rollback/backup/{id}     ← откатить к бэкапу
POST   /api/v1/gitops/rollback/undo/{audit_id} ← отменить конкретную команду
GET    /api/v1/gitops/rollback/history          ← история откатов

# Approval
GET    /api/v1/gitops/approvals                 ← список pending
GET    /api/v1/gitops/approvals/{id}            ← детали запроса
POST   /api/v1/gitops/approvals/{id}/approve    ← одобрить
POST   /api/v1/gitops/approvals/{id}/reject     ← отклонить
GET    /api/v1/gitops/approvals/history         ← история approval

# Tempo
GET    /api/v1/gitops/tempo/status              ← circuit breaker + rate status
POST   /api/v1/gitops/tempo/breaker/reset       ← manually reset circuit breaker
GET    /api/v1/gitops/tempo/rules               ← current rate limit rules
```

---

## 🗓️ Roadmap (обновлённый)

### Wave G1: Core (27h) — без изменений
### Wave G1.5: Competitor-Inspired (8h)

| # | Задача | Файлы | Время |
|---|--------|-------|-------|
| G1.5a | LiteralChecker — reject $VAR/globs | `src/shield/literal_checker.rs` | 2h |
| G1.5b | TempoController — circuit breaker + rate limit | `src/shield/tempo.rs`, `src/shield/circuit_breaker.rs` | 2h |
| G1.5c | ActionClassifier — tiered response + MODIFY rewrite | `src/shelf/classifier.rs`, `src/shield/rewriter.rs` | 1h |
| G1.5d | Structured denial feedback | `src/shield/feedback.rs` | 1h |
| G1.5e | PlanEngine — `flowlink plan` preview | `src/gitops/plan_engine.rs` | 2h |

### Wave G2: Drift + Approval (20h) — без изменений
### Wave G2.5: Advanced Features (6h)

| # | Задача | Файлы | Время |
|---|--------|-------|-------|
| G2.5a | EventDrivenCollector — inotify/docker events | `src/gitops/event_driven.rs` | 2h |
| G2.5b | Post-exec health checks | `src/gitops/health_checker.rs` | 2h |
| G2.5c | AutoRestoreEngine — auto-rollback on anomaly | `src/gitops/auto_restore.rs` | 2h |

### Wave G3: Sync + Dashboard (15h) — без изменений
### Wave G4: Production (10h) — без изменений

---

## 🧪 Test Environment (Docker)

```yaml
# docker-compose.test.yml
version: '3.8'

services:
  # FlowLink agent test target
  test-server:
    image: ubuntu:24.04
    privileged: true
    volumes:
      - ./test-state:/state
      - ./test-vault:/opt/flowlink-vault  # Vault outside agent envelope
    command: |
      bash -c "
        apt-get update && 
        apt-get install -y systemctl nginx postgresql-client docker.io &&
        sleep infinity
      "
    
  # PostgreSQL for DB backup tests
  test-postgres:
    image: postgres:16
    environment:
      POSTGRES_DB: testdb
      POSTGRES_USER: test
      POSTGRES_PASSWORD: test
    volumes:
      - test-pgdata:/var/lib/postgresql/data
      - ./test-sql:/docker-entrypoint-initdb.d
    
  # nginx for config tracking tests
  test-nginx:
    image: nginx:latest
    volumes:
      - ./test-nginx-conf:/etc/nginx/conf.d
    ports:
      - "8888:80"
    
  # Redis for testing
  test-redis:
    image: redis:7-alpine
    ports:
      - "6399:6379"
    
  # FlowLink relay (local)
  test-relay:
    build:
      context: ../
      dockerfile: Dockerfile.test
    ports:
      - "8080:8080"
      - "8443:8443"
    environment:
      - FLOWLINK_MODE=test
    depends_on:
      - test-server
      - test-postgres

volumes:
  test-pgdata:
```

### Test Scenarios

```rust
#[cfg(test)]
mod integration_tests {
    // 1. Shield blocks rm -rf /
    #[tokio::test]
    async fn test_shield_blocks_root_deletion() { /* ... */ }
    
    // 2. Auto-backup before destructive command
    #[tokio::test]
    async fn test_auto_backup_before_rm() { /* ... */ }
    
    // 3. Vault is agent-unreachable
    #[tokio::test]
    async fn test_vault_agent_cannot_access() { /* ... */ }
    
    // 4. Circuit breaker trips on failures
    #[tokio::test]
    async fn test_circuit_breaker_trips() { /* ... */ }
    
    // 5. MODIFY verdict auto-rewrites chmod 777
    #[tokio::test]
    async fn test_modify_verdict_chmod() { /* ... */ }
    
    // 6. Literal-only rejects $VAR in destructive
    #[tokio::test]
    async fn test_literal_rejects_var() { /* ... */ }
    
    // 7. Event-driven drift detects file change
    #[tokio::test]
    async fn test_event_drift_file_change() { /* ... */ }
    
    // 8. Auto-restore on health check failure
    #[tokio::test]
    async fn test_auto_restore_on_failure() { /* ... */ }
    
    // 9. Undo specific command
    #[tokio::test]
    async fn test_undo_command() { /* ... */ }
    
    // 10. Plan preview shows impact
    #[tokio::test]
    async fn test_plan_preview() { /* ... */ }
    
    // 11. Policy hash in audit entry
    #[tokio::test]
    async fn test_policy_hash_in_audit() { /* ... */ }
    
    // 12. HMAC integrity chain verification
    #[tokio::test]
    async fn test_hmac_integrity_chain() { /* ... */ }
    
    // 13. Full E2E: agent → shield → backup → execute → verify → commit
    #[tokio::test]
    async fn test_full_e2e_pipeline() { /* ... */ }
}
```

---

## 📊 Сводка

| Wave | Задач | Время | Что |
|------|-------|-------|-----|
| G1 Core | 7 | 27h | GitOps engine + collectors + audit + backup + Shield L3 |
| G1.5 Enhancements | 5 | 8h | Literal check + circuit breaker + MODIFY + plan preview |
| G2 Drift+Approval | 6 | 20h | Drift detection + auto-fix + PR approval + restore |
| G2.5 Advanced | 3 | 6h | Event-driven drift + health checks + auto-restore |
| G3 Sync+Dashboard | 4 | 15h | Remote sync + web UI + 20 API endpoints |
| G4 Production | 5 | 10h | Encryption + docs + load testing + release |
| **ИТОГО** | **30** | **86h** | **Full GitOps with competitive advantages** |

### MVP Path: 19h (G1 без G1.5)
### Competitive Path: 27h (G1 + G1.5)
### Full Product: 86h (All waves)

---

**Last Updated:** 2026-04-07 | **Version:** 2.0 | **Sources:** Agent Gate, ArgoCD, Cohesity, Spacelift

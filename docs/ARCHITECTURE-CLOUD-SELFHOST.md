# FlowLink Architecture: Cloud + Self-Hosted

## Принцип

**Один код. Два режима запуска.** Cloud (multi-tenant SaaS) и Self-hosted (single-tenant on-premise) используют одни и те же crate'ы, но разную конфигурацию и топологию развёртывания.

---

## Режимы развёртывания

### Cloud (SaaS)

```
┌─────────────────────────────────────────────────────────────────┐
│  flowlink.io (Kubernetes)                                       │
│                                                                  │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐               │
│  │ relay × N  │  │ relay × N  │  │ relay × N  │  (auto-scale) │
│  │ (gateway)  │  │ (gateway)  │  │ (gateway)  │               │
│  └─────┬──────┘  └─────┬──────┘  └─────┬──────┘               │
│        │               │               │                        │
│        └───────────────┼───────────────┘                        │
│                        │                                         │
│  ┌─────────────────────▼──────────────────────┐                │
│  │              Shared Services               │                │
│  │                                             │                │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐   │                │
│  │  │ Auth     │ │ Billing  │ │ Notifi-  │   │                │
│  │  │ Service  │ │ Service  │ │ cations  │   │                │
│  │  └──────────┘ └──────────┘ └──────────┘   │                │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐   │                │
│  │  │ Agent    │ │ Shield   │ │ MCP      │   │                │
│  │  │ Manager  │ │ Engine   │ │ Gateway  │   │                │
│  │  └──────────┘ └──────────┘ └──────────┘   │                │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐   │                │
│  │  │ Integ.   │ │ Audit    │ │ LLM      │   │                │
│  │  │ Manager  │ │ Service  │ │ Proxy    │   │                │
│  │  └──────────┘ └──────────┘ └──────────┘   │                │
│  └─────────────────────────────────────────────┘                │
│                        │                                         │
│  ┌─────────────────────▼──────────────────────┐                │
│  │  PostgreSQL (managed)     Redis (sessions)  │                │
│  │  S3 (audit logs)          NATS (events)     │                │
│  └─────────────────────────────────────────────┘                │
│                                                                  │
│  ┌────────────┐                                                 │
│  │ Dashboard  │  ← React SPA, CDN-distributed                  │
│  │ (static)   │                                                 │
│  └────────────┘                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Масштабирование:** Каждый сервис — отдельный deployment в K8s.
- relay (gateway) — масштабируется по количеству WebSocket-соединений
- billing — масштабируется по нагрузке оплаты
- agent-manager — масштабируется по количеству агентов
- shield — масштабируется по интенсивности команд

### Self-hosted (on-premise)

```
┌─────────────────────────────────────────────────────────────────┐
│  customer.internal (Docker / systemd)                           │
│                                                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  flowlink-relay (single binary)                           │  │
│  │                                                           │  │
│  │  All services in one process:                             │  │
│  │  ├── Auth (local JWT + optional LDAP/AD)                  │  │
│  │  ├── Billing (local plans or Enterprise license)          │  │
│  │  ├── Agents, Shield, Terminal, MCP, Audit, E2EE           │  │
│  │  ├── Integration Manager (bots, webhooks)                 │  │
│  │  └── Dashboard (embedded static files)                    │  │
│  │                                                           │  │
│  │  ┌─────────┐     ┌─────────────────┐                     │  │
│  │  │ SQLite  │ or  │ PostgreSQL       │                     │  │
│  │  │ (embed) │     │ (customer DB)    │                     │  │
│  │  └─────────┘     └─────────────────┘                     │  │
│  └───────────────────────────────────────────────────────────┘  │
│                           │                                      │
│                     ┌─────▼─────┐                                │
│                     │ Licence   │  ← HTTPS to api.flowlink.io   │
│                     │ Check     │     раз в 24ч, кэш 30 дней    │
│                     └───────────┘                                │
└─────────────────────────────────────────────────────────────────┘
```

**Масштабирование:** Вертикальное (ресурсы сервера). Для Enterprise — кластер с load balancer.

---

## Crate Dependency Graph

```
                    ┌─────────────────┐
                    │ flowlink-core   │  ← config, types, messages, protocol
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────────┐
              │              │                   │
     ┌────────▼───────┐ ┌───▼──────────┐ ┌─────▼──────────┐
     │ flowlink-crypto │ │ flowlink-db  │ │ flowlink-auth  │
     └────────────────┘ └──────────────┘ └────────────────┘
              │              │                   │
     ┌────────▼───────┐ ┌───▼──────────┐       │
     │ flowlink-shield│ │ flowlink-api │◄──────┘
     └────────────────┘ └──────────────┘
              │              │           ↑
     ┌────────▼───────┐     │    ┌──────┴──────────────┐
     │ flowlink-agent │     │    │ flowlink-bot-client  │ ← NEW
     └────────────────┘     │    └─────────────────────┘
              │              │              │
     ┌────────▼───────┐ ┌───▼──────────────▼──┐
     │ flowlink-mcp   │ │ flowlink-integrations │
     └────────────────┘ │  ├── core             │
     ┌────────────────┐ │  ├── telegram         │
     │ flowlink-billing│ │  ├── slack           │
     └───────┬────────┘ │  └── marketplace      │
             │          └───────────────────────┘
     ┌───────▼────────┐
     │ flowlink-sentinel│
     └────────────────┘

     ┌─────────────────────────────────────────────────────┐
     │  flowlink-relay (assembles everything)              │
     │  ├── cloud mode: connects to shared service mesh   │
     │  └── standalone mode: runs everything in-process   │
     └─────────────────────────────────────────────────────┘

     ┌────────────────┐
     │ flowlink-cli   │  ← uses flowlink-bot-client internally
     └────────────────┘
```

---

## API Layer: flowlink-bot-client

**Ключевой crate** — делает ботов, CLI и интеграции независимыми от AppState.

```rust
// crates/bot-client/src/lib.rs

pub struct FlowLinkClient {
    base_url: String,
    auth: AuthMethod,
    client: reqwest::Client,
}

pub enum AuthMethod {
    Jwt(String),                          // user/admin token
    ApiKey { key: String, secret: String }, // API key pair
    ServiceToken(String),                 // inter-service (cloud mode)
}

// ── All API methods mirror /api/* endpoints ──

impl FlowLinkClient {
    // Agents
    pub async fn list_agents(&self) -> Result<Vec<Agent>>;
    pub async fn get_agent(&self, id: &str) -> Result<Agent>;
    
    // Shield
    pub async fn get_alerts(&self) -> Result<Vec<ShieldAlert>>;
    pub async fn approve(&self, id: &str) -> Result<()>;
    pub async fn reject(&self, id: &str) -> Result<()>;
    pub async fn get_approvals(&self) -> Result<Vec<Approval>>;
    
    // Billing
    pub async fn get_plans(&self) -> Result<Vec<Plan>>;
    pub async fn get_billing(&self) -> Result<BillingInfo>;
    pub async fn subscribe(&self, plan_id: &str) -> Result<()>;
    pub async fn get_invoices(&self) -> Result<Vec<Invoice>>;
    pub async fn get_usage(&self) -> Result<UsageStats>;
    pub async fn change_plan(&self, plan_id: &str) -> Result<()>;
    
    // Audit
    pub async fn get_audit(&self, filter: AuditFilter) -> Result<Vec<AuditEvent>>;
    
    // Config
    pub async fn get_config(&self) -> Result<Config>;
    pub async fn reload_config(&self) -> Result<()>;
    
    // System
    pub async fn get_health(&self) -> Result<HealthStatus>;
    pub async fn get_status(&self) -> Result<SystemStatus>;
    
    // Integrations
    pub async fn list_integrations(&self) -> Result<Vec<Integration>>;
    pub async fn install_integration(&self, req: InstallRequest) -> Result<Integration>;
    pub async fn uninstall_integration(&self, id: &str) -> Result<()>;
    
    // MCP
    pub async fn mcp_request(&self, req: McpRequest) -> Result<McpResponse>;
}
```

**Используется:**
- Telegram integration → `FlowLinkClient` для команд
- Slack integration → `FlowLinkClient` для команд
- CLI → `FlowLinkClient` для `flowlink status`, `flowlink agents` и т.д.
- Self-hosted → `FlowLinkClient` → `http://localhost:3000`
- Cloud → `FlowLinkClient` → `https://api.flowlink.io`

---

## Service Communication

### Cloud mode (inter-service)

```rust
// В cloud режиме сервисы общаются через internal API:

// relay (gateway) → billing service
pub struct BillingServiceClient {
    inner: FlowLinkClient,  // base_url = "http://billing-svc:8080"
}

// relay (gateway) → auth service  
pub struct AuthServiceClient {
    inner: FlowLinkClient,  // base_url = "http://auth-svc:8080"
}
```

### Standalone mode (in-process)

Relay's `AppState` holds `Arc<dyn XxxProvider>` trait objects instead of concrete types:

```rust
pub struct AppState {
    // Trait objects — work in both modes
    pub billing_provider: Option<Arc<dyn flowlink_service_traits::BillingProvider>>,
    pub auth_provider: Option<Arc<dyn flowlink_service_traits::AuthProvider>>,
    // Mode indicator
    pub service_mode: flowlink_service_traits::ServiceMode,
    // ...
}
```

**Standalone mode** — local implementations wrap in-process engines:
```rust
// crates/relay/src/service_local.rs
pub struct LocalBillingProvider { engine: Arc<BillingEngine> }
pub struct LocalAuthProvider { engine: Arc<AuthEngine> }
```

**Cloud mode** — remote implementations call microservices via HTTP:
```rust
pub struct RemoteBillingProvider { client: FlowLinkClient }
pub struct RemoteAuthProvider { client: FlowLinkClient }
```

**Defined in `crates/service-traits`:**
- `BillingProvider` — 11 methods (plans, accounts, usage, limits)
- `AuthProvider` — 6 methods (validate, check, orgs, sessions)
- `AgentProvider` — 6 methods (list, register, heartbeat)
- `ShieldProvider` — 6 methods (alerts, approvals, resolve)
- `LicenceProvider` — 6 methods (verify, features, limits)
- `NotificationProvider` — 4 methods (send, preferences)
- `ServiceMode` — Standalone / Cloud enum
- `ServiceEndpoints` — URLs for cloud microservices

---

## Licence System (Self-hosted)

```rust
// crates/licence/src/lib.rs

pub struct Licence {
    pub key: String,
    pub customer: String,
    pub tier: LicenceTier,         // Free / Team / Enterprise
    pub max_agents: u32,
    pub max_users: u32,
    pub expires_at: DateTime<Utc>,
    pub features: Vec<String>,     // ["billing", "orgs", "saml", ...]
    pub offline_days: u32,         // 7 / 30 / 90
}

pub enum LicenceTier {
    Free,          // до 3 агентов, 1 пользователь
    Team,          // до 20 агентов, 10 пользователей
    Enterprise,    // неограниченно, LDAP/AD, кластер
}

pub struct LicenceManager {
    licence: RwLock<Option<Licence>>,
    last_check: RwLock<DateTime<Utc>>,
    cache_path: PathBuf,
    check_url: String,  // https://api.flowlink.io/api/licence/verify
}

impl LicenceManager {
    /// Check licence on startup
    pub async fn verify(&self) -> Result<Licence> {
        // 1. Try cloud check
        // 2. On failure → use cached licence if within offline_days
        // 3. On no cache → deny startup
    }
    
    /// Background periodic check (every 24h)
    pub async fn start_periodic_check(&self) {
        // tokio::spawn loop
    }
    
    /// Check if a feature is available
    pub fn has_feature(&self, feature: &str) -> bool {
        self.licence.read().unwrap()
            .as_ref()
            .map(|l| l.features.contains(&feature.to_string()))
            .unwrap_or(false)
    }
}
```

---

## Configuration

```toml
# flowlink.toml — Cloud mode
[relay]
mode = "cloud"
base_url = "https://api.flowlink.io"

[relay.cloud]
database_url = "postgres://..."        # managed DB
redis_url = "redis://..."              # sessions
nats_url = "nats://..."                # events

[relay.services]
billing = "http://billing-svc:8080"    # internal K8s service
auth = "http://auth-svc:8080"
notifications = "http://notifications-svc:8080"
integrations = "http://integrations-svc:8080"

[relay.scaling]
websocket_workers = 4
event_buffer_size = 4096
```

```toml
# flowlink.toml — Self-hosted mode
[relay]
mode = "standalone"
base_url = "https://relay.customer.internal"

[relay.standalone]
database_url = "sqlite:///var/lib/flowlink/flowlink.db"
# or: database_url = "postgres://localhost/flowlink"

[relay.licence]
key = "FL-XXXX-XXXX-XXXX"
check_url = "https://api.flowlink.io/api/licence/verify"
offline_days = 30

[relay.auth]
provider = "local"          # "local" | "ldap" | "oidc"
# ldap_url = "ldap://dc.company.local"
# oidc_issuer = "https://idp.company.com"

[relay.billing]
provider = "none"           # "none" | "local" | "cloud"
# local_plans = "/etc/flowlink/plans.json"

[relay.telegram]
bot_token = "..."           # admin's bot
admin_chat_id = 123456789
```

---

## Scaling Strategy (Cloud)

### Горизонтальное масштабирование

| Компонент | Триггер | Стратегия |
|---|---|---|
| **relay (gateway)** | WebSocket connections | K8s HPA по кол-ву соединений |
| **billing** | Payment events | Stateless, scale by RPS |
| **auth** | Login rate | Stateless, scale by RPS |
| **shield** | Command volume | Per-agent sharding |
| **agent-manager** | Agent count | Hash-ring по agent_id |
| **integrations** | Event volume | Per-account workers |
| **MCP** | MCP sessions | Sticky sessions |
| **audit** | Write volume | Queue + batch writer |

### State management

```
WebSocket connections → relay (sticky via IP hash)
Session state        → Redis
Events               → NATS (pub/sub)
DB state             → PostgreSQL (managed)
File storage         → S3 (audit logs, backups)
Config               → etcd / Consul
```

---

## Feature Matrix by Tier

| Feature | Free (Self-hosted) | Team (Cloud) | Enterprise |
|---|---|---|---|
| Agents | 3 | 20 | ∞ |
| Users | 1 | 10 | ∞ |
| Auth | Local JWT | OAuth + SAML | + LDAP/AD/OIDC |
| Shield | ✅ Basic | ✅ + ML | ✅ + Custom rules |
| Terminal | ✅ | ✅ | ✅ + Recording |
| Audit | 7 days | 90 days | ∞ + SIEM export |
| Billing | — | ✅ Tochka | Custom |
| Orgs | — | ✅ | ✅ + Custom roles |
| Integrations | 2 | ∞ | ∞ + Custom |
| MCP | ✅ | ✅ | ✅ |
| E2EE | ✅ | ✅ | ✅ + HSM |
| Dashboard | ✅ Local | ✅ Cloud | ✅ Custom domain |
| Support | Community | Email | Dedicated |
| SLA | — | 99.9% | 99.99% |
| Offline | 7 days | — | 90 days |

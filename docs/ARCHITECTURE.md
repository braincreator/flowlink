# Architecture

## Component Diagram

```
┌──────────────────────────────────────────────────────────────────────┐
│                        Client Machine                                │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │                      flowlink (agent)                          │  │
│  │                                                                │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐  │  │
│  │  │ Executor │  │ Sandbox  │  │ Backup   │  │ KillSwitch   │  │  │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────────┘  │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐  │  │
│  │  │ Approver │  │ Skill    │  │ Task     │  │ RemoteLLM    │  │  │
│  │  │ V2       │  │ Store    │  │ Manager  │  │              │  │  │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────────┘  │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐                   │  │
│  │  │ Device   │  │ Crypto   │  │ Health   │                   │  │
│  │  │ Pairing  │  │ (E2EE)   │  │ Monitor  │                   │  │
│  │  └──────────┘  └──────────┘  └──────────┘                   │  │
│  └──────────────────────────┬─────────────────────────────────────┘  │
└─────────────────────────────┼────────────────────────────────────────┘
                              │ WSS (outbound)
                              ▼
┌──────────────────────────────────────────────────────────────────────┐
│                         Relay Server (VPS)                           │
│                                                                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐│
│  │ Agent    │  │ Auth     │  │ Rate     │  │ Audit                ││
│  │ Pool     │  │ Manager  │  │ Limiter  │  │ Logger (HMAC)        ││
│  └──────────┘  └──────────┘  └──────────┘  └──────────────────────┘│
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐│
│  │ LLM      │  │ Event    │  │ Registry │  │ Billing              ││
│  │ Proxy    │  │ Bus      │  │ (Multi-  │  │ (Plans, Usage,       ││
│  │          │  │ (SSE)    │  │  tenancy)│  │  Invoices)           ││
│  └──────────┘  └──────────┘  └──────────┘  └──────────────────────┘│
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐│
│  │ Device   │  │ Health   │  │ Integration│ │ Nginx               ││
│  │ Trust    │  │ Monitor  │  │ (Autoscale│ │ Proxy               ││
│  │          │  │          │  │  Webhook) │ │                     ││
│  └──────────┘  └──────────┘  └──────────┘  └──────────────────────┘│
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    HTTP API + MCP Server                     │   │
│  │                    + Web Dashboard (i18n)                    │   │
│  └──────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────┘
```

## Internal Packages

```
internal/
├── agent/          # Agent daemon: executor, sandbox, approval, backup, kill switch
├── audit/          # HMAC audit verification
├── billing/        # Plans, usage tracking, invoices, payments (SBP, T-Банк, Точка)
├── config/         # Configuration loading (agent + relay)
├── crypto/         # E2EE: X25519 key exchange, AES-256-GCM encryption
├── dashboard/      # Web Dashboard SPA (embedded, i18n: RU/EN)
├── devices/        # Device trust, pairing protocol, device approval flow
├── health/         # Health monitoring, readiness/liveness checks
├── integration/    # Autoscale, webhooks, notifier, provisioner
├── nginx/          # Nginx reverse proxy integration
├── protocol/       # WebSocket message types, serialization, i18n (RU/EN)
├── relay/          # Relay server: WSS, HTTP API, MCP, auth, audit, registry, LLM proxy
├── tgbot/          # Telegram Bot (long polling, payment handlers)
└── transport/      # WebSocket transport layer
```

## Core Modules

### Devices Module (`internal/devices/`)

Device trust and pairing system for secure agent authentication.

**Components:**
- `devices.go` — Device registry, trust management
- `pairing.go` — Device pairing protocol (QR code, manual approval)

**Flow:**
```
1. Agent registers → generates device key pair
2. Dashboard shows QR code → User scans
3. User approves device → Device marked as trusted
4. Future connections: Device authenticated by key
```

### Health Module (`internal/health/`)

Health monitoring and readiness checks.

**Endpoints:**
- `GET /health` — Liveness probe (service alive)
- `GET /ready` — Readiness probe (service ready to accept traffic)

**Metrics:**
- Relay uptime
- Connected agents count
- Active sessions
- Memory usage

### Integration Module (`internal/integration/`)

Autoscale, webhooks, and notification system.

**Components:**
- `manager.go` — Integration manager (webhook registration)
- `notifier.go` — Event notifications (email, Telegram, webhook)
- `provisioner.go` — Auto-provisioning agents
- `billing_autoscale.go` — Auto-scale based on billing plan
- `webhook_handler.go` — Webhook endpoint handler

**Autoscale Flow:**
```
1. Client exceeds plan limits
2. Billing system triggers autoscale
3. Provisioner spins up new relay instance
4. Load balancer adds to pool
5. Client notified
```

### Crypto Module (`internal/crypto/`)

End-to-end encryption for sensitive data.

**Algorithms:**
- **Key Exchange:** X25519 (Curve25519 ECDH)
- **Encryption:** AES-256-GCM (AEAD)
- **Key Derivation:** HKDF-SHA256

**Use Cases:**
- Encrypt credentials in config
- Secure backup snapshots
- Protect sensitive audit logs

### Audit Module (`internal/audit/`)

HMAC-based audit log verification.

**Purpose:**
- Detect tampering of audit logs
- Verify log integrity
- Compliance (SOX, PCI-DSS)

**Implementation:**
- Each log entry signed with HMAC-SHA256
- Chain hash (previous entry hash included)
- Verification endpoint

## WebSocket Protocol

All messages are JSON-encoded. Agents connect outbound (WSS) to the relay, which pierces NAT.

### Message Format

```json
{
  "id": "uuid-v4",
  "type": "exec_request",
  "agent_id": "abc-123",
  "session_id": "optional-session",
  "payload": { ... },
  "timestamp": 1712345678,
  "error": ""
}
```

### Message Types

#### Connection

| Type | Direction | Description |
|------|-----------|-------------|
| `connect` | Agent → Relay | Registration with hostname, OS, arch, version |
| `connected` | Relay → Agent | Confirmation with assigned agent_id |
| `disconnect` | Either | Graceful disconnect |
| `heartbeat` | Agent → Relay | Keep-alive ping (every 30s) |
| `heartbeat_ack` | Relay → Agent | Keep-alive pong |

#### Command Execution

| Type | Direction | Description |
|------|-----------|-------------|
| `exec_request` | Relay → Agent | Execute shell command |
| `exec_output` | Agent → Relay | stdout/stderr chunk (streaming) |
| `exec_done` | Agent → Relay | Command completed (exit code, duration) |
| `exec_approve` | Agent → Relay | Client approved execution |
| `exec_reject` | Agent → Relay | Client rejected execution |
| `needs_approval` | Agent → Relay | Execution requires approval |
| `approval_request` | Agent → Relay | V2 approval request |
| `approval_response` | Relay → Agent | V2 approval response |

#### File Operations

| Type | Direction | Description |
|------|-----------|-------------|
| `file_read` | Relay → Agent | Read file content |
| `file_write` | Relay → Agent | Write file content |
| `file_list` | Relay → Agent | List directory |
| `file_response` | Agent → Relay | File operation result |

#### System

| Type | Direction | Description |
|------|-----------|-------------|
| `sys_info` | Relay → Agent | Request system information |
| `sys_info_resp` | Agent → Relay | CPU, RAM, disk, uptime |
| `config_update` | Relay → Agent | Update agent configuration |
| `config_ack` | Agent → Relay | Configuration updated |

#### Autonomous Tasks (L2)

| Type | Direction | Description |
|------|-----------|-------------|
| `task` | Relay → Agent | Submit autonomous task |
| `task_progress` | Agent → Relay | Task progress update |
| `task_done` | Agent → Relay | Task completed |
| `task_cancel` | Relay → Agent | Cancel task |

#### Skills

| Type | Direction | Description |
|------|-----------|-------------|
| `skill_push` | Relay → Agent | Deploy skill to agent |
| `skill_list` | Agent → Relay | List installed skills |
| `skill_delete` | Relay → Agent | Remove skill |

#### Device Pairing

| Type | Direction | Description |
|------|-----------|-------------|
| `device_register` | Agent → Relay | Register device with public key |
| `device_qr` | Relay → Agent | QR code for pairing |
| `device_approve` | Agent → Relay | User approved device |
| `device_trust` | Relay → Agent | Device marked as trusted |

#### Health Checks

| Type | Direction | Description |
|------|-----------|-------------|
| `health_ping` | Relay → Agent | Health check request |
| `health_pong` | Agent → Relay | Health check response |

#### LLM Proxy

| Type | Direction | Description |
|------|-----------|-------------|
| `llm_request` | Agent → Relay | LLM request (proxied) |
| `llm_response` | Relay → Agent | LLM response |

#### Error

| Type | Direction | Description |
|------|-----------|-------------|
| `error` | Either | Error with message |

### Example: Command Execution Flow

```
1. OpenClaw → POST /api/v1/agents/exec → Relay
2. Relay → WSS exec_request → Agent
3. Agent → Sandbox check → Approver check
   ├─ auto: execute immediately
   ├─ soft_ask: execute + notify client
   └─ hard_ask: wait for approval
4. Agent → WSS exec_output (chunks) → Relay → SSE → OpenClaw
5. Agent → WSS exec_done → Relay → HTTP response → OpenClaw
```

## Configuration Format

### Agent Config (`~/.flowlink/config.yaml`)

```yaml
agent_id: "auto-generated-uuid"
token: "pairwise-auth-token"
relay_url: "wss://relay.example.com/ws"
heartbeat_sec: 30
label: "my-server"
work_dir: "/home/deploy"

sandbox:
  allowed_dirs: ["/home/deploy", "/var/www"]
  blocked_patterns: ["rm -rf /*", "mkfs.*", ":(){ :|:& };:"]
  max_file_size: 104857600    # 100 MB
  max_exec_timeout: 300        # 5 min
  allow_sudo: false

approval:
  mode: "soft_ask"             # auto | soft_ask | hard_ask
  soft_ask_notify: true
  hard_ask_timeout_sec: 3600
  max_retries: 3

backup:
  enabled: true
  max_snapshots: 50
  max_total_size: 5368709120   # 5 GB
  retention_days: 7
  backup_dir: "~/.flowlink/backups"

devices:
  pairing_timeout_sec: 300      # 5 minutes
  max_devices: 10
  trust_duration_days: 365      # 1 year

health:
  enabled: true
  check_interval_sec: 30
  unhealthy_threshold: 3

crypto:
  enabled: true
  key_rotation_days: 90
  backup_encryption: true
```

### Relay Config (`relay.yaml`)

```yaml
wss_addr: ":8443"
api_addr: ":8080"

tls_mode: "letsencrypt"        # self-signed | letsencrypt | manual
tls_domain: "relay.example.com"
tls_cache: "/var/lib/flowlink/tls-cache"
tls_cert: ""                   # for manual mode
tls_key: ""                    # for manual mode

api_token: "your-secret-token"

data_dir: "/var/lib/flowlink"
rate_limit_rpm: 60

devices:
  enabled: true
  pairing_url: "https://relay.example.com/pair"

health:
  enabled: true
  metrics_port: 9090

integration:
  autoscale:
    enabled: false
    max_instances: 5
    scale_threshold: 80      # CPU %
  webhooks:
    enabled: false
    endpoint: "https://api.example.com/webhook"
```

## Data Storage

FlowLink uses file-based storage (no external database required).

```
/var/lib/flowlink/
├── clients/                   # Client registry
│   ├── {client_id}.json       # Client info + plan
│   └── ...
├── agents/                    # Agent registry
│   ├── {agent_id}.json        # Agent metadata + token
│   └── ...
├── devices/                   # Device registry
│   ├── {device_id}.json       # Device keys + trust status
│   └── ...
├── audit/                     # Audit logs (JSONL)
│   └── audit-2026-03-27.jsonl
├── billing/                   # Billing data
│   ├── usage-{client_id}.jsonl
│   └── invoices/
├── health/                    # Health check history
│   └── health-{date}.jsonl
├── integration/               # Integration configs
│   ├── webhooks.json
│   └── autoscale.json
└── tls-cache/                 # Let's Encrypt certificates
```

## Authorization (JWT Flow)

```
1. Agent starts → connects to WSS with pairwise token
2. Relay validates token → registers in AgentPool
3. API request → Authorization: Bearer <JWT>
4. Middleware validates JWT → extracts client_id
5. Rate limiter checks request count
6. Handler executes → AuditLogger records action
```

### Token Types

| Token | Scope | Lifetime |
|-------|-------|----------|
| Pairwise token | Agent ↔ Relay (WSS) | Permanent (rotated on re-register) |
| API token | HTTP API (admin) | Permanent (config) |
| JWT | API requests | 24h (refreshable) |

## MCP Protocol

FlowLink exposes an MCP server at `POST /mcp` using Streamable HTTP transport.

### Connection

```json
{
  "mcpServers": {
    "flowlink": {
      "url": "https://relay.example.com/mcp",
      "headers": {
        "Authorization": "Bearer your-api-token"
      }
    }
  }
}
```

### Available Tools

| Tool | Parameters | Description |
|------|-----------|-------------|
| `flowlink_agents` | `status` (all/online) | List connected agents |
| `flowlink_exec` | `agent_id`, `command`, `timeout_sec` | Execute command |
| `flowlink_read` | `agent_id`, `path` | Read file |
| `flowlink_write` | `agent_id`, `path`, `content` | Write file |
| `flowlink_list` | `agent_id`, `dir` | List directory |
| `flowlink_sysinfo` | `agent_id` | System information |
| `flowlink_task` | `agent_id`, `task`, `description` | Submit autonomous task |
| `flowlink_task_status` | `agent_id`, `task_id` | Task status |

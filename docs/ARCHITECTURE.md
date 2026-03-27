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
│  └──────────────────────────┬─────────────────────────────────────┘  │
└─────────────────────────────┼────────────────────────────────────────┘
                              │ WSS (outbound)
                              ▼
┌──────────────────────────────────────────────────────────────────────┐
│                         Relay Server (VPS)                           │
│                                                                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐│
│  │ Agent    │  │ Auth     │  │ Rate     │  │ Audit                ││
│  │ Pool     │  │ Manager  │  │ Limiter  │  │ Logger               ││
│  └──────────┘  └──────────┘  └──────────┘  └──────────────────────┘│
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────────┐│
│  │ LLM      │  │ Event    │  │ Registry │  │ Billing              ││
│  │ Proxy    │  │ Bus      │  │ (Multi-  │  │ (Plans, Usage,       ││
│  │          │  │ (SSE)    │  │  tenancy)│  │  Invoices)           ││
│  └──────────┘  └──────────┘  └──────────┘  └──────────────────────┘│
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    HTTP API + MCP Server                     │   │
│  │                    + Web Dashboard                           │   │
│  └──────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────┘
```

## Internal Packages

```
internal/
├── agent/          # Agent daemon: executor, sandbox, approval, backup, kill switch
├── billing/        # Plans, usage tracking, invoices, payments
├── config/         # Configuration loading (agent + relay)
├── dashboard/      # Web Dashboard SPA (embedded)
├── protocol/       # WebSocket message types and serialization
├── relay/          # Relay server: WSS, HTTP API, MCP, auth, audit, registry
├── tgbot/          # Telegram Bot (long polling)
└── transport/      # WebSocket transport layer
```

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
├── audit/                     # Audit logs (JSONL)
│   └── audit-2026-03-27.jsonl
├── billing/                   # Billing data
│   ├── usage-{client_id}.jsonl
│   └── invoices/
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

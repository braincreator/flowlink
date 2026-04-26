# 🏗️ FlowLink Architecture

**MCP Gateway + AI-Native SecOps — zero-trust control plane for AI agents.**

---

## System Overview

```
    AI Agents                     FlowLink Platform                    Infrastructure
  ┌───────────┐             ┌──────────────────────────┐          ┌──────────────────┐
  │ Claude    │             │  Cloudflare CDN           │          │  Servers (Linux) │
  │ Cursor    │──MCP/WS────→│  flowlink.flow-masters.ru │          │  ┌────────────┐  │
  │ Copilot   │             └────────────┬─────────────┘          │  │ Agent      │  │
  │ Windsurf  │                          │                        │  │ (exec/IO)  │  │
  │ Codex     │             ┌────────────▼─────────────┐          │  └────────────┘  │
  │ Cline     │             │  VPS (93.93.207.44)      │          │  ┌────────────┐  │
  │ Aider     │             │  ┌───────┐  ┌─────────┐  │          │  │ ServerGuard│  │
  └───────────┘             │  │ nginx │─→│ Relay   │  │          │  │ (GitOps)   │  │
                            │  │ (SSL) │  │ :8080   │  │          │  └────────────┘  │
                            │  └───────┘  └────┬────┘  │          └──────────────────┘
                            │                  │        │
                            │  ┌───────────────▼─────┐  │                 │
                            │  │  PostgreSQL :5432    │◄─┼── audit/billing │
                            │  │  (accounts, usage,   │  │                 │
                            │  │   audit, invoices)   │  │          ┌──────▼───────┐
                            │  └──────────────────────┘  │          │ K8s Cluster  │
                            └────────────────────────────┘          │ (CRD+Webhook)│
                                                                   └──────────────┘
```

---

## Crate Dependency Graph

```
                            ┌──────┐
                            │ cli  │  (binary: flowlink)
                            └──┬───┘
                   ┌───────────┼───────────┐
                   ▼           ▼           ▼
              ┌─────────┐ ┌─────────┐ ┌──────────┐
              │  relay  │ │  agent  │ │  shield  │
              └──┬──┬───┘ └──┬──────┘ └────┬─────┘
                 │  │        │              │
        ┌────────┘  │    ┌───┘         ┌────┘
        ▼           ▼    ▼             ▼
   ┌────────┐  ┌──────┐ ┌──────┐ ┌──────────┐
   │  db    │  │ bill │ │ core │ │  gitops  │
   └────────┘  └──────┘ └──────┘ └──────────┘
   ┌──────┐  ┌──────┐ ┌──────┐
   │  k8s │  │ crypto│ │  mcp │
   └──────┘  └──────┘ └──────┘
   ┌───────────┐
   │ sentinel  │
   └───────────┘
```

| Crate    | Lines  | Tests | Purpose                                    |
|----------|--------|-------|--------------------------------------------|
| core     | ~15K   | 105   | Message types, config, channels            |
| crypto   | ~3K    | 62    | X25519 + AES-256-GCM encryption            |
| db       | ~12K   | 65    | PostgreSQL repos (sqlx)                    |
| billing  | ~8K    | 72    | Plans, invoices, usage, Tochka Bank        |
| agent    | ~25K   | 130   | Dispatch, policy, sandbox, killswitch, exec|
| relay    | ~35K   | 222   | WS server, REST API, RBAC, E2EE, MCP      |
| shield   | ~20K   | 253   | eBPF/macOS ES, threat analysis, L1-L7      |
| gitops   | ~19K   | 218   | Drift detection, ServerGuard, backup engine|
| k8s      | ~5K    | 76    | Operator, CRD, admission webhooks          |
| mcp      | ~3K    | —     | MCP protocol types and server              |
| sentinel | ~5K    | —     | AI Ops assistant, pattern learning         |
| cli      | ~8K    | —     | Binary entrypoint, MCP server              |
| **Total**| **~158K**| **~1187**|                                          |

---

## Shield Security Pipeline (7 Levels)

```
┌─────────────┐    ┌───────────┐    ┌─────────────┐    ┌───────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│  KillSwitch │───→│ ReadOnly  │───→│  Blacklist  │───→│  Policy   │───→│ Sandbox  │───→│ Approval │───→│  Backup  │───→│ Execute  │
│  (instant   │    │  (deny    │    │  (L1 regex  │    │  (allow/  │    │  (isolate │    │  (human   │    │  (auto    │    │          │
│   block)    │    │   writes) │    │   + L2 AST) │    │   deny)   │    │   exec)  │    │   review) │    │   save)  │    │          │
└─────────────┘    └───────────┘    └─────────────┘    └───────────┘    └──────────┘    └──────────┘    └──────────┘    └──────────┘
```

Each level can **BLOCK** (return error), **REWRITE** (modify command), or **PASS** (continue to next level).

| Level | Name | Method | Example |
|-------|------|--------|---------|
| 0 | KillSwitch | Emergency toggle | Agent paused, all blocked |
| 1 | ReadOnly | Write detection | `rm`, `chmod`, `dd` blocked |
| 2 | Blacklist (Pattern) | Regex matching | `rm -rf /`, `curl | bash` |
| 3 | Blacklist (AST) | Structural parse | `$(dangerous)`, globs |
| 4 | Policy Engine | Custom rules | Allow `apt update`, deny `apt remove` |
| 5 | Sandbox | Execution isolation | Run in namespace/container |
| 6 | Approval | Human review | Block → Telegram alert → Approve |
| 7 | Backup | Auto-save | Snapshot before destructive ops |

### Threat Vectors Covered

| Vector | Example | Shield Level |
|--------|---------|-------------|
| Shell injection | `; rm -rf /` | L2 Blacklist |
| Credential theft | `cat /etc/shadow` | L1 Pattern |
| Data exfiltration | `curl -d @secrets` | L1 Pattern |
| Privilege escalation | `sudo su -` | L3 AST |
| Supply chain | `curl | bash` | L2 Blacklist |
| Container escape | `nsenter --target 1` | L4 Policy |
| Destructive ops | `rm -rf /var/log` | L1-L7 Full |

---

## Message Protocol

Agents connect to the relay via WebSocket (`/ws`). All messages are JSON with a `type` field.

### Core Message Types

| Type | Direction | Description |
|------|-----------|-------------|
| `connect` | Agent→Relay | Authentication + public key exchange |
| `connect_ack` | Relay→Agent | Connection accepted, relay public key |
| `heartbeat` | Both | Keep-alive ping/pong |
| `command` | Relay→Agent | Execute a command on the agent |
| `command_result` | Agent→Relay | Command execution result (streaming) |
| `llm_request` | Agent→Relay | Forward LLM API request |
| `llm_response` | Relay→Agent | LLM API response |
| `config_update` | Relay→Agent | Hot-reload configuration |
| `config_ack` | Agent→Relay | Config update acknowledged |
| `sys_info_req` | Relay→Agent | Request system info |
| `sys_info_resp` | Agent→Relay | System info response |
| `disconnect` | Both | Graceful disconnect |

### MCP Tools (12)

| Tool | Description |
|------|-------------|
| `flowlink_agents` | List connected agents |
| `flowlink_exec` | Execute command on agent |
| `flowlink_read` | Read file from agent |
| `flowlink_write` | Write file to agent |
| `flowlink_list` | List directory on agent |
| `flowlink_sysinfo` | Get system info |
| `flowlink_kill` | Kill process on agent |
| `flowlink_deregister` | Disconnect agent |
| `flowlink_health` | Health check |
| `flowlink_config_update` | Update agent config |
| `flowlink_approve` | Approve pending command |
| `flowlink_policy` | Manage security policies |

---

## Security Model

### Zero-Trust Principles

1. **Never trust, always verify** — every command passes through Shield
2. **Least privilege** — agents only access allowed tools/paths
3. **Assume breach** — secrets never in agent context, E2EE optional
4. **Audit everything** — triple-write audit trail
5. **Human in the loop** — approval workflow for dangerous operations

### E2EE (Optional)

- **Algorithm:** X25519 key exchange + AES-256-GCM
- **Key format:** Base64-encoded public keys
- **Envelope:** `{key_id, sender_key_id, sender_public_key, nonce, ciphertext}`
- **Fallback:** Agents without keys work in plaintext

### RBAC

Roles: `admin`, `operator`, `viewer`
- **admin:** Full access (manage agents, policies, billing)
- **operator:** Manage agents, view billing
- **viewer:** Read-only access

### Device Trust

Score 0-100 calculated from:
- Base: 30 (new device)
- +10 per successful pairing (max +50)
- -15 per failed attempt
- +1 per day since pairing (max +20)
- -20 per suspicious flag

### Zero-Trust Secret Injection

Secrets are injected at runtime via HashiCorp Vault integration. The agent never sees the secret value — it receives a short-lived reference that is resolved server-side during execution.

---

## Data Model

### PostgreSQL Schema

```sql
-- accounts
CREATE TABLE accounts (
    id          UUID PRIMARY KEY,
    email       TEXT UNIQUE NOT NULL,
    name        TEXT,
    plan        TEXT DEFAULT 'starter',
    status      TEXT DEFAULT 'active',
    created_at  TIMESTAMPTZ DEFAULT NOW()
);

-- usage (daily aggregation)
CREATE TABLE usage (
    id          UUID PRIMARY KEY,
    account_id  UUID REFERENCES accounts(id),
    date        DATE NOT NULL,
    api_requests BIGINT DEFAULT 0,
    tokens_in   BIGINT DEFAULT 0,
    tokens_out  BIGINT DEFAULT 0,
    commands    BIGINT DEFAULT 0
);

-- invoices
CREATE TABLE invoices (
    id          UUID PRIMARY KEY,
    account_id  UUID REFERENCES accounts(id),
    amount      DECIMAL(10,2),
    currency    TEXT DEFAULT 'RUB',
    status      TEXT DEFAULT 'pending',
    created_at  TIMESTAMPTZ DEFAULT NOW()
);

-- audit_log
CREATE TABLE audit_log (
    id          UUID PRIMARY KEY,
    timestamp   TIMESTAMPTZ DEFAULT NOW(),
    level       TEXT,
    category    TEXT,
    agent_id    TEXT,
    action      TEXT,
    target      TEXT,
    result      TEXT,
    risk_score  DECIMAL(3,1),
    metadata    JSONB
);

-- api_keys
CREATE TABLE api_keys (
    id          UUID PRIMARY KEY,
    account_id  UUID REFERENCES accounts(id),
    key_hash    TEXT NOT NULL,
    name        TEXT,
    permissions TEXT[],
    created_at  TIMESTAMPTZ DEFAULT NOW()
);
```

### Audit Trail

Triple-write: DashMap (memory) + JSONL (disk) + PostgreSQL (optional, async)

SIEM export formats: CEF, LEEF, JSON + RuSIEM + MaxPatrol connectors.

---

## REST API

All endpoints under `http://relay:8080`

### Core

| Method | Path | Description |
|--------|------|-------------|
| GET | `/healthz` | Health check `{status, version}` |
| GET | `/api/v1/agents` | List connected agents |
| POST | `/api/v1/agents/:id/commands` | Send command to agent |
| POST | `/api/v1/auth/signup` | Register account |
| POST | `/api/v1/auth/login` | Login |
| POST | `/api/v1/auth/api-keys` | Create API key |

### Billing

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/billing/usage` | Per-agent usage stats |
| GET | `/api/v1/billing/invoices` | Invoice history |
| GET | `/api/v1/billing/plans` | Available plans |
| POST | `/api/v1/billing/subscribe` | Subscribe to plan |

### Security & Observability

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/audit` | Query audit events |
| GET | `/api/v1/audit/export?format=cef` | SIEM export (CEF/LEEF/JSON) |
| GET | `/api/v1/audit/stats` | Audit statistics |
| GET | `/api/v1/devices` | List paired devices |
| GET | `/api/v1/devices/:id/trust` | Device trust score |
| GET | `/api/v1/compliance/audit` | Compliance audit |
| GET | `/api/v1/forensics/timeline` | Forensic timeline |

### GitOps (feature-gated)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/gitops/drift/:id` | Drift status for agent |
| POST | `/api/v1/gitops/backup/:id` | Trigger backup |
| GET | `/api/v1/gitops/backups/:id` | List backups |
| POST | `/api/v1/gitops/restore/:id` | Restore from backup |
| GET | `/api/v1/gitops/guard/:id` | Server guard status |

Rate limited: 100 requests / 10 seconds per IP (whitelisted: `/healthz`, `/ws`)

---

## Deployment

### Docker Compose

```bash
docker compose up -d
```

### Standalone

```bash
# Build with GitOps support
cargo build --release --features gitops

# Without GitOps (default)
cargo build --release

./flowlink relay --config relay.json
./flowlink agent --config agent.json
```

### Kubernetes

```bash
# Apply CRD
kubectl apply -f config/crd.yaml

# Create policy
kubectl apply -f config/policy.yaml

# Deploy operator
cargo build --release -p flowlink-k8s
./flowlink-k8s --relay-url https://flowlink.flow-masters.ru/api/v1
```

### VPS (flowlink.flow-masters.ru)

```bash
./scripts/deploy.sh              # Full deploy (binary + website)
./scripts/deploy-website-only.sh # Frontend updates only
```

---

## Configuration Hot-Reload

```
1. Admin edits relay.json on disk
2. File watcher detects change
3. Relay parses new config
4. Relay sends ConfigUpdate to all connected agents via WS
5. Each agent updates: policy, sandbox, killswitch, approval
6. Each agent sends ConfigAck back
```

No restart needed. Zero downtime config changes.

---

## OWASP MCP Risk Mapping

| OWASP Risk | FlowLink Mitigation |
|------------|-------------------|
| Prompt Injection | Shield L1-L7 pipeline, policy engine |
| Tool Poisoning | Allowlisted tools only, MCP validation |
| Data Exfiltration | E2EE, redaction, audit trail |
| Supply Chain | `curl | bash` blocked, literal enforcement |
| Credential Theft | Zero-trust secret injection, never in context |
| Privilege Escalation | RBAC, sandbox, least-privilege |
| Unauthorized Access | Device trust scoring, approval workflow |

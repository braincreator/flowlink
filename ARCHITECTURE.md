# 🏗️ FlowLink Architecture

**AI-native webhook gateway for managing AI agents and LLM operations.**

---

## System Overview

```
                        ┌─────────────────────────────────────┐
                        │           Cloudflare CDN             │
                        │   flowlink.flow-masters.ru          │
                        └──────────────┬──────────────────────┘
                                       │
                        ┌──────────────▼──────────────────────┐
                        │        VPS (93.93.207.44)           │
                        │  ┌──────────┐    ┌──────────────┐   │
                        │  │  nginx   │───▶│  FlowLink    │   │
                        │  │  (SSL)   │    │   Relay      │   │
                        │  └────┬─────┘    │  :8080       │   │
                        │       │          └──────┬───────┘   │
                        │       │                 │ WS        │
                        │  ┌────▼─────────────────▼───────┐   │
                        │  │    PostgreSQL  :5432         │   │
                        │  │    (accounts, usage, audit)  │   │
                        │  └──────────────────────────────┘   │
                        └─────────────────────────────────────┘
                                       │
                    ┌──────────────────┼──────────────────┐
                    │                  │                   │
               ┌────▼────┐      ┌─────▼────┐      ┌─────▼────┐
               │ Agent A │      │ Agent B  │      │ Agent C  │
               │ (WS)    │      │ (WS+E2EE)│      │ (WS)     │
               └─────────┘      └──────────┘      └──────────┘
```

---

## Crate Dependency Graph

```
                        ┌──────┐
                        │ cli  │  (binary: flowlink)
                        └──┬───┘
                           │
              ┌────────────┼────────────────┐
              ▼            ▼                ▼
         ┌────────┐  ┌─────────┐     ┌──────────┐
         │ relay  │  │  agent  │     │  shield  │
         └──┬─────┘  └──┬──────┘     └──────────┘
            │           │
     ┌──────┼──────┐    │
     ▼      ▼      ▼    ▼
  ┌──────┐┌─────┐┌────┐┌──────┐
  │ db   ││bill ││e2ee││ core │
  └──────┘└─────┘└────┘└──────┘
  ┌──────┐┌─────┐
  │ k8s  ││gitop│
  └──────┘└─────┘
```

| Crate    | Lines | Tests | Purpose                              |
|----------|-------|-------|--------------------------------------|
| core     | 2,490 | 105   | Message types, config, channels      |
| crypto   | 868   | 62    | X25519 + AES-256-GCM encryption      |
| db       | 1,692 | 65    | PostgreSQL repos (sqlx)              |
| billing  | 2,752 | 55    | Plans, invoices, usage tracking      |
| agent    | 5,089 | 130   | Dispatch, policy, sandbox, killswitch|
| relay    | 8,043 | 222   | WS server, REST API, RBAC, E2EE      |
| shield   | 7,197 | 253   | eBPF/macOS ES, threat analysis       |
| k8s      | 2,849 | 76    | Operator, CRD, webhooks              |
| gitops   | 15,537| 218   | Drift detection, restore engine     |
| cli      | 671   | —     | Binary entrypoint                    |
| **Total**| **47K**| **1187**|                                     |

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
| `command_result` | Agent→Relay | Command execution result |
| `llm_request` | Agent→Relay | Forward LLM API request |
| `llm_response` | Relay→Agent | LLM API response |
| `config_update` | Relay→Agent | Hot-reload configuration |
| `config_ack` | Agent→Relay | Config update acknowledged |
| `sys_info_req` | Relay→Agent | Request system info |
| `sys_info_resp` | Agent→Relay | System info response |
| `disconnect` | Both | Graceful disconnect |

### Message Flow

```
Agent                    Relay                   LLM API
  │──connect─────────────▶│                        │
  │◀─connect_ack──────────│                        │
  │                       │                        │
  │──command_result──────▶│                        │
  │                       │                        │
  │──llm_request─────────▶│──forward──────────────▶│
  │◀─llm_response─────────│◀──response─────────────│
  │                       │                        │
  │◀─config_update────────│  (hot-reload)          │
  │──config_ack──────────▶│                        │
```

---

## Security Model

### E2EE (Optional)

```
Agent                          Relay
  │                             │
  │  connect(public_key=PK_A)   │
  │────────────────────────────▶│
  │                             │
  │  connect_ack(relay_key=PK_R)│
  │◀────────────────────────────│
  │                             │
  │  encrypt(msg, SK_A, PK_R)   │
  │  EncryptedEnvelope{...}     │
  │────────────────────────────▶│
  │                             │ decrypt(SK_R, envelope)
```

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

- `score >= 50`: Trusted
- `score < 20`: Auto-denied

### Killswitch

Per-agent emergency stop. When paused:
- All commands blocked immediately
- LLM requests rejected
- Agent receives `config_update` with `killswitch.paused = true`

---

## Data Model

### PostgreSQL Schema

```sql
-- accounts
CREATE TABLE accounts (
    id          UUID PRIMARY KEY,
    email       TEXT UNIQUE NOT NULL,
    name        TEXT,
    plan        TEXT DEFAULT 'free',
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

---

## REST API

All endpoints under `http://relay:8080`

| Method | Path | Description |
|--------|------|-------------|
| GET | `/healthz` | Health check `{status, version}` |
| GET | `/api/agents` | List connected agents |
| POST | `/api/agents/:id/commands` | Send command to agent |
| GET | `/api/billing/usage` | Per-agent usage stats |
| GET | `/api/billing/invoices` | Invoice history |
| GET | `/api/devices` | List paired devices |
| GET | `/api/devices/:id/trust` | Device trust score |
| POST | `/api/devices/pair` | Initiate pairing |
| POST | `/api/devices/:id/confirm` | Confirm pairing |
| GET | `/api/audit` | Query audit events |
| GET | `/api/audit/export?format=cef` | SIEM export (CEF/LEEF/JSON) |
| GET | `/api/audit/stats` | Audit statistics |

Rate limited: 100 requests / 10 seconds per IP (whitelisted: `/healthz`, `/ws`)

---

## Deployment

### Docker Compose

```bash
docker compose up -d
```

Services: relay, agent, postgres. Health checks enabled.

### Standalone

```bash
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

# Operator reconciles automatically:
# - ConfigMaps with shield config
# - MutatingWebhook (sidecar injection)
# - ValidatingWebhook (policy enforcement)
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

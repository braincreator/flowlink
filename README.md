<p align="center">
  <img src="https://flowlink.flow-masters.ru/favicon.svg" width="80" alt="FlowLink" />
</p>

<h1 align="center">FlowLink</h1>

<p align="center">
  <strong>AI-Native Remote Server Management</strong><br/>
  Open-source tool to manage your servers through AI assistants.
</p>

<p align="center">
  <a href="#-quick-start"><strong>Quick Start</strong></a> ·
  <a href="#-features">Features</a> ·
  <a href="#-architecture">Architecture</a> ·
  <a href="#-api-reference">API</a> ·
  <a href="#-pricing">Pricing</a> ·
  <a href="README_ru.md">Русский</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Go-1.24-00ADD8?logo=go" alt="Go" />
  <img src="https://img.shields.io/badge/License-MIT-green.svg" alt="License" />
  <img src="https://img.shields.io/badge/Docker-Ready-2496ED?logo=docker" alt="Docker" />
  <img src="https://img.shields.io/github/v/release/braincreator/flowlink?color=blue" alt="Release" />
</p>

---

**FlowLink** is a SaaS platform for remote server management through AI. Install a single binary (~5MB) on each server — then manage your entire infrastructure via OpenClaw, ChatGPT, Claude, Telegram, or web dashboard.

> **How it works:** FlowLink is a relay — it routes commands from AI assistants to your servers. You bring your own AI; we provide the infrastructure.

🔗 **Built by [FlowMasters](https://flow-masters.ru)** — chatbots, AI assistants, and automation for business.

---

## 🚀 Quick Start

### One-line install

```bash
# On the server (Linux / macOS)
curl -sSL https://install.flowlink.dev | bash
```

The script automatically:
- Downloads the binary for your platform
- Creates a systemd service (Linux) or LaunchAgent (macOS)
- Starts the agent and connects it to the relay

### Docker

```bash
# Run the relay
docker run -d \
  --name flowlink-relay \
  -p 8443:8443 -p 8080:8080 \
  -v flowlink-data:/var/lib/flowlink \
  -e FLOWLINK_API_TOKEN=your-secret-token \
  flowlink/relay:latest
```

### Build from source

```bash
git clone https://github.com/braincreator/flowlink.git
cd flowlink
make build

# Relay (on your VPS)
./bin/flowlink-relay -config relay.yaml

# Agent (on client servers)
./bin/flowlink -config agent.yaml
```

---

## 📡 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Client Machine                           │
│  ┌──────────────┐                                            │
│  │ flowlink     │ ← single binary, zero dependencies          │
│  │ (daemon)     │ ← sandbox, backup, approval, kill switch   │
│  └──────┬───────┘                                            │
└─────────┼───────────────────────────────────────────────────┘
          │ WSS (outbound, punches through NAT)
          ▼
┌─────────────────────────────────────────────────────────────┐
│                     Relay (VPS)                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    │
│  │ WSS      │  │ MCP      │  │ API      │  │ Billing  │    │
│  │ Server   │  │ Server   │  │ Gateway  │  │ Module   │    │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                  │
│  │ Auth     │  │ Audit    │  │ Agent    │                  │
│  │ Module   │  │ Log      │  │ Registry │                  │
│  └──────────┘  └──────────┘  └──────────┘                  │
└─────────┬───────────────────────────────────────────────────┘
          │ MCP (Streamable HTTP)
          ▼
┌─────────────────────────────────────────────────────────────┐
│                 Operator (OpenClaw / AI)                     │
│  ┌──────────────┐  ┌──────────────┐                         │
│  │ OpenClaw     │  │ Any MCP      │                         │
│  │ (AI brain)   │  │ Client       │                         │
│  └──────────────┘  └──────────────┘                         │
└─────────────────────────────────────────────────────────────┘
          │ HTTP API
          ▼
┌─────────────────────────────────────────────────────────────┐
│  Telegram Bot  │  Web Dashboard  │  Let's Encrypt (TLS)     │
└─────────────────────────────────────────────────────────────┘
```

**Components:**

| Component | Description |
|-----------|-------------|
| **Relay** | Central server on VPS. Accepts WSS from agents, provides HTTP API and MCP for AI assistants |
| **Agent** | Lightweight daemon on client machines. Executes commands, manages files, backups, kill switch |
| **MCP Server** | Integration with any MCP-compatible AI assistant (OpenClaw, Claude, ChatGPT) — 8 tools |
| **Telegram Bot** | Manage agents via Telegram: status, commands, approval, emergency stop |
| **Web Dashboard** | SPA panel with dark theme for monitoring and management |

---

## 🔐 Security Architecture

FlowLink implements a **7-layer security pipeline** that evaluates every command before execution:

```
Command received
       │
       ▼
  ┌──────────┐   NO
  │ KillSwitch├──────────► DROP (log + notify)
  └────┬─────┘
       │ YES
       ▼
  ┌──────────┐   YES
  │ Read-only├──────────► Execute read-only
  └────┬─────┘
       │ NO
       ▼
  ┌──────────┐   MATCH
  │ Blacklist├──────────► REJECT (log + notify)
  └────┬─────┘
       │ NO MATCH
       ▼
  ┌──────────┐   VIOLATION
  │ Sandbox  ├──────────► REJECT (out of bounds)
  └────┬─────┘
       │ OK
       ▼
  ┌──────────┐   NEEDS APPROVAL
  │ Approval ├──────────► Wait for human
  └────┬─────┘
       │ APPROVED / AUTO
       ▼
  ┌──────────┐
 │  Backup   │──► Snapshot before destructive ops
  └────┬─────┘
       │
       ▼
  ┌──────────┐
 │  Execute  │──► Run command, log result
  └──────────┘
```

### Key Security Properties

| Property | Implementation |
|----------|---------------|
| **E2EE** | X25519 ECDH key exchange + AES-256-GCM for all command/response payloads |
| **Relay is blind** | Relay forwards encrypted blobs — it has no access to plaintext commands |
| **Device trust** | Owner explicitly approves each device via Telegram before E2EE is established |
| **Zero-knowledge relay** | Even a compromised relay cannot read or modify commands |

---

## 🔒 End-to-End Encryption (E2EE)

FlowLink encrypts **all command and response payloads** with end-to-end encryption. The relay never sees plaintext.

### Key Generation

```bash
# Each owner has a keypair (auto-generated on first setup)
flowlink-agent keys generate
# → ~/.flowlink/e2ee_private.key  (X25519 private key)
# → ~/.flowlink/e2ee_public.key   (X25519 public key)
```

- **Algorithm:** X25519 (Curve25519 ECDH)
- **Symmetric cipher:** AES-256-GCM
- **Key derivation:** HKDF-SHA256
- **Private key permissions:** `0600` (owner read/write only)

### Key Storage

| File | Contents | Permissions |
|------|----------|-------------|
| `~/.flowlink/e2ee_private.key` | X25519 private key | `0600` |
| `~/.flowlink/e2ee_public.key` | X25519 public key | `0644` |
| `~/.flowlink/e2ee_devices.json` | Per-device shared secrets | `0600` |

### Key Exchange Flow

```
  Owner (Telegram)          Relay (VPS)           Device (Server)
       │                        │                       │
  1. /keys generate          │                       │
       │─── public key ────────►│                       │
       │                        │                       │
  2. Device pairing request   │                       │
       │                        │◄── CODE ──────────────│
       │                        │                       │
  3. /approve_device CODE     │                       │
       │                        │─── approval ──────────►│
       │                        │                       │
  4. E2EE handshake          │                       │
       │◄═══ ECDH ══════════════╪═══════════════════════►│
       │    (shared secret computed on both sides)       │
       │                        │                       │
  5. Encrypted communication │                       │
       │◄══════ AES-256-GCM ════╪══════════════════════►│
       │    (relay forwards opaque blobs)                │
```

### Key Rotation

```bash
# Rotate all keys (requires re-approval of all devices)
/rotate

# Automatic rotation (optional, in config)
# "auto_rotate": true  — rotates every 30 days
```

> **Forward secrecy:** After rotation, old keys are securely deleted. Previously captured encrypted traffic cannot be decrypted. However, FlowLink does **not** implement per-session key ratcheting (like Signal) — rotation is manual/explicit to keep the system simple for server management use cases.

---

## 📱 Device Management

FlowLink uses a **trust-on-first-approval** model. Every device must be explicitly approved by the owner via Telegram before it can execute commands.

### Commands

| Command | Description |
|---------|-------------|
| `/devices` | List all connected devices with status and E2EE info |
| `/approve_device CODE` | Approve a new device (starts E2EE handshake) |
| `/reject_device CODE` | Reject a pending pairing request |
| `/revoke NAME` | Revoke device access (immediate disconnect) |
| `/keys` | Show your E2EE key fingerprint and device count |
| `/rotate` | Rotate E2EE keys (all devices must re-pair) |
| `/device_info NAME` | Detailed device info (OS, IP, last seen, key fingerprint) |

### Pairing Flow

```
  ┌─────────┐         ┌─────────┐         ┌─────────┐
  │  Device  │         │  Relay   │         │  Owner  │
  │ (Agent)  │         │         │         │(Telegram)│
  └────┬────┘         └────┬────┘         └────┬────┘
       │                   │                   │
  1. flowlink-agent setup                      │
       │── connect + public key ──►│            │
       │                   │                   │
  2.                  ┌── pending ──►        │
       │                   │   /devices shows   │
       │                   │   "⚠️ PENDING"     │
       │                   │                   │
  3.                   │              /approve_device ABC123
       │                   │◄───────────────────│
       │                   │                   │
  4. ◄── E2EE established ──│                   │
       │                   │                   │
  5. ✅ Ready — encrypted commands flow         │
       │═══════════════════│                   │
```

---

## ✨ Features

- 🖥️ **Remote Shell** — execute commands with sandbox and timeouts
- 📁 **File Manager** — read, write, list directories
- 💾 **Backup & Recovery** — snapshots with retention policies
- 🛑 **Kill Switch** — emergency stop, pause, read-only mode
- ✅ **Approval System** — 3 modes: auto / soft_ask / hard_ask
- 📊 **Audit Log** — JSONL logging of every action with export
- 🔌 **MCP Server** — integrate with any AI assistant (8 tools)
- 🤖 **Telegram Bot** — 15 commands for management via Telegram
- 🖥️ **Web Dashboard** — SPA with dark theme
- 💰 **Billing** — 4 pricing plans, SaaS-ready
- 🔒 **TLS** — self-signed + Let's Encrypt
- 📡 **Event Streaming** — SSE for real-time notifications
- 🏢 **Multi-tenancy** — multiple clients on a single relay

---

## 🔧 Configuration

### Agent Setup

```bash
# Interactive setup (generates config + keys)
flowlink-agent setup --relay wss://your-relay.com --token YOUR_TOKEN
```

### Agent — `~/.flowlink/config.yaml`

```yaml
# Identity
agent_id: "auto-generated"
token: "pairwise-token-from-relay"
relay_url: "wss://relay.example.com/ws"

# Settings
heartbeat_sec: 30
label: "production-server-1"
work_dir: "/home/deploy"

# Sandbox — restrictions
sandbox:
  allowed_dirs:
    - "/home/deploy"
    - "/var/www"
  blocked_patterns:
    - "rm -rf /*"
    - "mkfs.*"
    - "dd if=*"
    - ":(){ :|:& };:"
  max_file_size: 104857600   # 100 MB
  max_exec_timeout: 300       # 5 min
  allow_sudo: false

# Approval — command confirmation
approval:
  mode: "soft_ask"            # auto | soft_ask | hard_ask
  soft_ask_notify: true
  hard_ask_timeout_sec: 3600
  max_retries: 3

# Backup
backup:
  enabled: true
  max_snapshots: 50
  max_total_size: 5368709120  # 5 GB
  retention_days: 7
  backup_dir: "~/.flowlink/backups"
```

### Relay — `relay.yaml`

```yaml
# Listeners
wss_addr: ":8443"
api_addr: ":8080"

# TLS
tls_mode: "letsencrypt"       # self-signed | letsencrypt | manual
tls_domain: "relay.example.com"
tls_cache: "/var/lib/flowlink/tls-cache"

# Auth
api_token: "your-secret-api-token"

# Multi-tenancy
data_dir: "/var/lib/flowlink"

# Rate limiting
rate_limit_rpm: 60            # requests per minute
```

### E2EE Configuration

```yaml
# ~/.flowlink/config.yaml — E2EE section
e2ee:
  enabled: true               # Enable end-to-end encryption
  auto_rotate: false           # Auto-rotate keys every 30 days
  key_dir: "~/.flowlink"      # Directory for key storage
```

---

## 🔐 Security

| Mechanism | Description |
|-----------|-------------|
| **JWT Auth** | Pairwise tokens for each agent, JWT for API access |
| **TLS** | All traffic encrypted (self-signed / Let's Encrypt / manual) |
| **Rate Limiting** | Configurable request limits |
| **Sandbox** | Blocks dangerous commands (rm -rf, fork bombs, mkfs) |
| **File Whitelist** | Directory access restrictions |
| **Approval System** | 3 command confirmation modes |
| **Audit Log** | JSONL logging of every action |
| **Timeout** | Configurable command execution time limits |

> **Liability:** FlowLink is a routing tool — like SSH. We do not control, modify, or initiate commands. The client (and their AI assistant) is fully responsible for all commands executed on their servers.

---

## 🛡️ Safety Features

FlowLink is designed to **prevent AI agents from causing damage**:

| Feature | How it works |
|---------|-------------|
| **Read-only by default** | New agents start in read-only mode |
| **Command blacklist** | `rm -rf /`, `mkfs`, `dd if=/dev/zero`, fork bombs — blocked |
| **Approval prompts** | Destructive commands require human confirmation (Telegram / Dashboard) |
| **Auto-backup** | Automatic snapshot before any destructive operation |
| **Kill switch** | Instant emergency stop via Telegram |
| **Sandbox** | Restricted directories and max file sizes |
| **Timeout** | Commands auto-killed after configurable timeout |

---

## 💰 Pricing

| Plan | Price/mo | Agents | Commands/mo | Backups | Storage | Features |
|------|---------|--------|-------------|---------|---------|----------|
| **Free** | $0 | 1 | 100 | 3 | 100 MB | Basic execution |
| **Starter** | $10 | 3 | 1,000 | 10 | 1 GB | + Telegram Bot, Audit |
| **Pro** | $30 | 25 | 10,000 | 50 | 10 GB | + MCP, API, Dashboard |
| **Enterprise** | Custom | 100+ | Unlimited | Unlimited | 100+ GB | All features, SLA, white-label |

> Self-hosted relay is always free (MIT license). Cloud pricing is for managed infrastructure.

---

## 🤖 Telegram Bot

| Command | Description |
|---------|-------------|
| `/start` | Greeting and pairing |
| `/help` | Command list |
| `/status` | Agent status |
| `/servers` | Server list |
| `/exec <cmd>` | Execute command |
| `/logs` | Recent logs |
| `/backups` | Backup list |
| `/restore <id>` | Restore backup |
| `/emergency` | Emergency stop |
| `/pause` | Pause agents |
| `/resume` | Resume agents |
| `/approve <id>` | Approve operation |
| `/reject <id>` | Reject operation |
| `/settings` | Settings |

---

## 📖 API Reference

### Auth

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/auth/login` | Login, get JWT |
| POST | `/api/v1/auth/refresh` | Refresh token |

### Agents

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/agents` | List connected agents |
| POST | `/api/v1/agents/register` | Register new agent |
| DELETE | `/api/v1/agents/delete/{id}` | Delete agent |
| POST | `/api/v1/agents/exec` | Execute command on agent |
| GET | `/api/v1/agents/files/read` | Read file |
| POST | `/api/v1/agents/files/write` | Write file |
| GET | `/api/v1/agents/files/list` | List files |
| GET | `/api/v1/agents/sysinfo` | System information |
| POST | `/api/v1/agents/task` | Send autonomous task |
| POST | `/api/v1/agents/task/cancel` | Cancel task |
| POST | `/api/v1/agents/pause` | Pause agent |
| POST | `/api/v1/agents/resume` | Resume agent |

### Backups

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/backups` | List backups |
| POST | `/api/v1/backups/{id}/restore` | Restore from backup |

### Audit

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/audit` | Query logs |
| GET | `/api/v1/audit/export` | Export to JSON/CSV |
| GET | `/api/v1/audit/stats` | Statistics |

### Clients

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/clients` | List clients |
| POST | `/api/v1/clients` | Create client |
| GET | `/api/v1/clients/{id}` | Client info |

### Billing

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/billing/usage` | Usage statistics |
| GET | `/api/v1/billing/plan` | Current plan |
| POST | `/api/v1/billing/plan/change` | Change plan |
| GET | `/api/v1/billing/invoices` | Invoice list |

### Events

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/events` | SSE event stream |

### MCP

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/mcp` | JSON-RPC MCP endpoint |

---

## 🔌 MCP Integration

Connect FlowLink to any MCP-compatible AI assistant:

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

**Available tools:**

| Tool | Description |
|------|-------------|
| `flowlink_agents` | List connected agents |
| `flowlink_exec` | Execute command on agent |
| `flowlink_read` | Read file |
| `flowlink_write` | Write file |
| `flowlink_list` | List directory |
| `flowlink_sysinfo` | System information |
| `flowlink_task` | Send autonomous task |
| `flowlink_task_status` | Task status |

---

## 🐳 Docker

```yaml
# docker-compose.yml
version: '3.8'
services:
  relay:
    image: flowlink/relay:latest
    ports:
      - "8443:8443"
      - "8080:8080"
    environment:
      - FLOWLINK_API_TOKEN=your-secret-token
      - FLOWLINK_TLS_MODE=letsencrypt
      - FLOWLINK_TLS_DOMAIN=relay.example.com
    volumes:
      - flowlink-data:/var/lib/flowlink
    restart: unless-stopped

volumes:
  flowlink-data:
```

---

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

---

## 📄 License

[MIT](LICENSE) © 2026 FlowMasters

---

<p align="center">
  Built with ❤️ by <a href="https://flow-masters.ru">FlowMasters</a>
</p>

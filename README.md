<div align="center">

# 🛡️ FlowLink

**MCP Gateway + AI-Native SecOps for Agents**

[![CI](https://img.shields.io/github/actions/workflow/status/braincreator/flowlink/ci.yml?branch=main&logo=github&label=CI)](https://github.com/braincreator/flowlink/actions)
[![Security](https://img.shields.io/github/actions/workflow/status/braincreator/flowlink/semgrep.yml?branch=main&logo=github&label=SAST)](https://github.com/braincreator/flowlink/actions)
[![Licenses](https://img.shields.io/github/actions/workflow/status/braincreator/flowlink/deny.yml?branch=main&logo=github&label=licenses)](https://github.com/braincreator/flowlink/actions)
[![Coverage](https://img.shields.io/codecov/c/github/braincreator/flowlink?logo=codecov&label=coverage)](https://app.codecov.io/gh/braincreator/flowlink)
[![Clippy](https://img.shields.io/badge/clippy-passing-green?logo=rust)](https://github.com/braincreator/flowlink/actions)
[![Rust](https://img.shields.io/badge/rust-1.80+-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-proprietary-red)]()

*Zero-trust MCP gateway · 7-level Shield · E2EE · RBAC · Forensics · SIEM · Billing*

</div>

---

## What is FlowLink?

FlowLink is the zero-trust control plane between AI agents and your infrastructure. The same class as Envoy AI Gateway and Operant MCP Gateway — focused on deep security and runtime control.

**Three pillars:**
- 🔍 **Visibility** — MCP tool discovery map, live audit log, forensic timeline, pattern learning, infrastructure map
- 🔑 **Governance** — Policy engine, approval workflow, RBAC, zero-trust secret injection, Shield profiles, SSO
- 🛡️ **Protection** — 7-level security pipeline, eBPF kernel interception, runtime blocking, redaction, sandbox, SIEM export

```
    AI Agents                    FlowLink                          Infrastructure
  ┌──────────┐              ┌───────────────┐                 ┌──────────────┐
  │ Claude   │──MCP/WS────→│  Relay        │──exec/stream──→│  Servers     │
  │ Cursor   │              │  ├─ Shield L1-L7              │  (via Agent) │
  │ Copilot  │              │  ├─ Policy Engine              │              │
  │ Windsurf │              │  ├─ Approval                   │  ┌─────────┐ │
  │ Codex    │              │  ├─ Secret Injection           │  │ ServerGuard│ │
  │ Custom   │              │  ├─ Audit + SIEM               │  │ (GitOps) │ │
  └──────────┘              │  └─ Billing                    │  └─────────┘ │
                            └───────────────┘                 └──────────────┘
                                    │
                              ┌─────┴─────┐
                              │ K8s       │
                              │ Operator  │
                              │ (CRD+WH)  │
                              └───────────┘
```

---

## Quick Start

```bash
# Install (Linux/macOS)
curl -fsSL https://flowlink.flow-masters.ru/install.sh | sh

# Register agent
flowlink agent register --name my-server

# Add to AI agent config (e.g. ~/.claude/mcp.json)
{
  "mcpServers": {
    "flowlink": {
      "command": "flowlink",
      "args": ["mcp"]
    }
  }
}

# Verify
flowlink version       # v0.3.1-dev
flowlink agent list    # ag_abc123  my-server  connected
flowlink mcp --test    # ✓ 12 tools available
```

📖 [Hello, Secure Agent — 10 min quickstart](https://flowlink.flow-masters.ru/docs/quickstart)

---

## Features

### 🔄 MCP Gateway + Relay
- **12 MCP tools** — agents, exec, read, write, list, sysinfo, kill, deregister, health, config_update, approve, policy
- **WebSocket relay** (axum) with typed JSON message protocol
- **REST API** — agents, billing, devices, audit, gitops endpoints
- **Config hot-reload** — file watcher → ConfigUpdate → agents update live
- **Streaming exec** — real-time command output via WebSocket

### 🛡️ Shield (7-Level Security Pipeline)
```
KillSwitch → ReadOnly → Blacklist → Policy → Sandbox → Approval → Backup → Execute
```
- **L1 Pattern matching** — regex blacklist for known attack patterns
- **L2 AST analysis** — command parsing, structural validation
- **L3 Deep analysis** — literal-only enforcement, command rewriting
- **eBPF** (Linux) — Kernel-level interception before `execve()`
- **Endpoint Security** (macOS) — Native process monitoring
- **Forensic timeline** — full incident reconstruction
- **Pattern learning** — behavioral baseline, anomaly detection

### 🔐 Security
- **E2EE** — X25519 + AES-256-GCM per-agent encryption
- **RBAC** — admin / operator / viewer roles
- **Zero-trust secret injection** — HashiCorp Vault integration, secrets never in agent context
- **Killswitch** — Per-agent emergency stop, instant command blocking
- **Policy engine** — allow/warn/block with Shield profiles

### 🔍 Observability
- **Audit trail** — Triple-write: DashMap + JSONL + PostgreSQL
- **SIEM export** — CEF, LEEF, JSON formats + RuSIEM + MaxPatrol connectors
- **Infrastructure map** — 80+ service types, live topology
- **Discovery** — automatic service catalog
- **Health monitoring** — `GET /healthz` → `{status, version}`

### 📋 Governance
- **Approval workflow** — Block → Alert → Approve/Reject (Telegram, Dashboard)
- **Change management** — track, approve, audit infrastructure changes
- **Compliance** — FSTEK/152-ФЗ, OWASP MCP risk mapping
- **SSO** — SAML 2.0 (Enterprise)

### ⚙️ GitOps
- **Config drift detection** — semantic diff current vs desired state
- **Auto-remediation** — classify action, auto-fix, backup before exec
- **Circuit breaker** — tempo control for AI agent command rate
- **ServerGuard** — file watching, Docker events, canary tokens

### ☸️ Kubernetes
- **CRD** — `FlowLinkShieldPolicy` for declarative configuration
- **Operator** — Reconciliation loop with status updates
- **Drift detection** — Compare CR spec vs actual cluster state
- **Admission webhook** — MutatingWebhook (sidecar injection) + ValidatingWebhook (enforcement)

### 💰 Billing
- **Plans** — Starter, Professional, Scale, Enterprise
- **Tochka Bank** — Russian payment provider integration
- **No agent limits** — unlimited agents per host
- **No request limits** — FlowLink is a security gateway, not an LLM provider

---

## Architecture

12 crates, ~158K lines of Rust, ~1187 tests.

| Crate | Purpose | Lines |
|-------|---------|-------|
| `core` | Message types, config, channels | ~15K |
| `crypto` | X25519 + AES-256-GCM encryption | ~3K |
| `db` | PostgreSQL repos (sqlx) | ~12K |
| `billing` | Plans, invoices, usage, Tochka Bank | ~8K |
| `agent` | Dispatch, policy, sandbox, killswitch, exec | ~25K |
| `relay` | WS server, REST API, RBAC, E2EE, MCP | ~35K |
| `shield` | eBPF/macOS ES, threat analysis, L1-L7 | ~20K |
| `gitops` | Drift detection, ServerGuard, backup engine | ~19K |
| `k8s` | Operator, CRD, webhooks | ~5K |
| `cli` | Binary entrypoint, MCP server | ~8K |
| `mcp` | MCP protocol types and server | ~3K |
| `sentinel` | AI Ops assistant, pattern learning | ~5K |

See [ARCHITECTURE.md](ARCHITECTURE.md) for full details.

---

## API Reference

| Method | Path | Description |
|--------|------|-------------|
| GET | `/healthz` | Health check |
| GET | `/api/v1/agents` | List connected agents |
| POST | `/api/v1/agents/:id/commands` | Send command |
| GET | `/api/v1/billing/usage` | Usage stats |
| GET | `/api/v1/devices/:id/trust` | Trust score |
| GET | `/api/v1/audit` | Audit events |
| GET | `/api/v1/audit/export` | SIEM export |
| GET | `/api/v1/gitops/drift/:id` | GitOps drift status |
| POST | `/api/v1/gitops/backup/:id` | Trigger backup |
| GET | `/api/v1/compliance/audit` | Compliance audit |

Rate limited: 100 req/10s per IP.

---

## Development

```bash
make build        # Release build
make test         # Run all tests (~1187)
make lint         # fmt + clippy
make check        # Fast compilation check
make docker       # Build Docker images
```

---

## Deployment

```bash
# VPS (flowlink.flow-masters.ru)
./scripts/deploy.sh              # Full deploy
./scripts/deploy-website-only.sh # Website only

# Docker
docker compose up -d

# Kubernetes
kubectl apply -f config/crd.yaml
kubectl apply -f config/policy.yaml
```

---

## License

Proprietary — © 2026 FlowMasters. [Website](https://flowlink.flow-masters.ru) · [Docs](https://flowlink.flow-masters.ru/docs) · [Pricing](https://flowlink.flow-masters.ru/pricing)

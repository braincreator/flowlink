<div align="center">

# 🛡️ FlowLink

**AI-native webhook gateway for managing AI agents and LLM operations**

[![Rust](https://img.shields.io/badge/rust-1.80+-orange?logo=rust)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/tests-1187 passing-green)]()
[![Crates](https://img.shields.io/badge/crates-10-blue)]()
[![SaaS](https://img.shields.io/badge/type-Cloud_SaaS-blue)](https://flowlink.flow-masters.ru)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS-lightgrey)]()

*WebSocket relay · E2EE · RBAC · Shield · K8s operator · Billing*

</div>

---

## What is FlowLink?

FlowLink is a production-ready gateway that sits between AI agents and the outside world. It provides:

- **Relay** — WebSocket hub connecting multiple AI agents with typed message protocol
- **Security** — E2EE (X25519 + AES-256-GCM), RBAC, device trust scoring, killswitch
- **Shield** — Kernel-level command interception (eBPF on Linux, Endpoint Security on macOS)
- **Observability** — Audit trail (triple-write), SIEM export (CEF/LEEF), health monitoring
- **Billing** — Per-agent usage tracking, plan management, invoicing
- **K8s** — Operator with CRD, reconciliation, drift detection, webhook injection
- **GitOps** — Config drift detection, automated restore engine

```
                    ┌─────────────┐
                    │  FlowLink   │
                    │   Relay     │
                    │  (WebSocket)│
                    └──┬──┬──┬──┬─┘
                       │  │  │  │
              ┌────────┘  │  │  └────────┐
              ▼           ▼  ▼           ▼
          Agent A     Agent B  Agent C  Agent D
          (Claude)    (Codex)  (Custom)  (GPT)
```

---

## Quick Start

```bash
# Build
cargo build --release

# Run relay
./flowlink relay --config relay.json

# Run agent
./flowlink agent --config agent.json

# Run shield (command interceptor)
./flowlink shield

# Docker
docker compose up -d
```

---

## Features

### 🔄 Relay
- **WebSocket server** (axum) with typed JSON message protocol
- **REST API** — agents, billing, devices, audit endpoints
- **Config hot-reload** — file watcher → ConfigUpdate → agents update live
- **Graceful shutdown** — SIGINT/SIGTERM, zero dropped connections
- **Rate limiting** — token bucket (100 req/10s), whitelist for health/WS

### 🔐 Security
- **E2EE** — Optional X25519 + AES-256-GCM per-agent encryption
- **RBAC** — admin / operator / viewer roles
- **Device trust** — Score 0-100, auto-deny < 20, GET /api/devices/:id/trust
- **Killswitch** — Per-agent emergency stop, instant command blocking
- **Policy engine** — 3-level evaluation (allow/warn/block)

### 🛡️ Shield
- **eBPF** (Linux) — Kernel-level command interception before `execve()`
- **Endpoint Security** (macOS) — Native macOS process monitoring
- **3-level threat analysis** — Pattern matching, risk scoring, canary files
- **Forensic snapshots** — ZFS snapshot on critical threats
- **Approval workflow** — Block → Alert → Approve/Reject via API

### 📊 Observability
- **Audit trail** — Triple-write: DashMap + JSONL + PostgreSQL (async)
- **SIEM export** — CEF, LEEF, JSON formats
- **Health check** — `GET /healthz` → `{status, version}`
- **Billing tracking** — Per-agent API requests, tokens, commands

### ☸️ Kubernetes
- **CRD** — `FlowLinkShieldPolicy` for declarative configuration
- **Operator** — Reconciliation loop with status updates
- **Drift detection** — Compare CR spec vs actual cluster state
- **Webhook injection** — MutatingWebhook (sidecar) + ValidatingWebhook (enforcement)
- **Auto cleanup** — Garbage-collected on CR deletion

### 💰 Billing
- **Plans** — Trial (7 days), Starter, Pro
- **Scale metric** — hosts × users × log retention
- **No agent limits** — unlimited agents per host
- **No request limits** — FlowLink is a security gateway, not an LLM provider

---

## Architecture

10 crates, 47K lines of Rust, 1187 tests.

| Crate | Purpose |
|-------|---------|
| `core` | Message types, config, channels |
| `crypto` | X25519 + AES-256-GCM encryption |
| `db` | PostgreSQL repos (sqlx) |
| `billing` | Plans, invoices, usage |
| `agent` | Dispatch, policy, sandbox, killswitch |
| `relay` | WS server, REST API, RBAC, E2EE |
| `shield` | eBPF/macOS ES, threat analysis |
| `k8s` | Operator, CRD, webhooks |
| `gitops` | Drift detection, restore |
| `cli` | Binary entrypoint |

See [ARCHITECTURE.md](ARCHITECTURE.md) for full details.

---

## API Reference

| Method | Path | Description |
|--------|------|-------------|
| GET | `/healthz` | Health check |
| GET | `/api/agents` | List connected agents |
| POST | `/api/agents/:id/commands` | Send command |
| GET | `/api/billing/usage` | Usage stats |
| GET | `/api/devices/:id/trust` | Trust score |
| GET | `/api/audit` | Audit events |
| GET | `/api/audit/export` | SIEM export |

Rate limited: 100 req/10s per IP.

---

## Development

```bash
make build        # Release build
make test         # Run all tests (1187)
make lint         # fmt + clippy
make check        # Fast compilation check
make docker       # Build Docker images
make website      # Build Next.js website
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

Proprietary — © 2026 FlowMasters. [Website](https://flowlink.flow-masters.ru)

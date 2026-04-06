# FlowLink

**Secure remote agent management over WebSocket with E2EE.**

FlowLink lets you manage remote machines through a relay server — execute commands, transfer files, manage backups — all encrypted end-to-end with X25519 + AES-256-GCM.

## Architecture

```
┌──────────┐     WSS/E2EE     ┌──────────┐     HTTP API     ┌──────────┐
│  Agent   │ ◄──────────────► │  Relay   │ ◄──────────────► │  Client  │
│ (on host)│                  │ (server) │                  │ (you)    │
└──────────┘                  └──────────┘                  └──────────┘
     │                              │
     ├── Shield (L1+L2+L3)         ├── Auth (JWT tokens)
     ├── Policy Engine              ├── Agent Pool
     ├── Snapshot/Backup            ├── Approval Queue
     └── Audit Log                  └── Event Bus (SSE)
```

## Quick Start

### Install

```bash
cargo build --release
# Binary: target/release/flowlink
```

### Generate Keypair

```bash
flowlink keygen
flowlink keygen --output keypair.json
```

### Start Relay

```bash
flowlink relay --config relay.json
```

### Start Agent

```bash
flowlink agent --config flowlink.json
```

## Crates

| Crate | Description |
|-------|-------------|
| `flowlink-core` | Protocol types, error codes, config |
| `flowlink-crypto` | X25519 + AES-256-GCM E2EE |
| `flowlink-shield` | 3-level command threat detection (L1 args + L2 AST + L3 interpreter) |
| `flowlink-agent` | Remote agent: executor, policy, WS connection |
| `flowlink-relay` | Relay server: agent pool, auth, event bus |
| `flowlink` | CLI binary |

## Shield — Command Protection

3-level threat detection engine:

- **L1 — Structured Args:** Pattern matching on binary + flags (rm -rf, dd, mkfs, docker rm -f)
- **L2 — AST Analysis:** tree-sitter-bash parsing for eval, pipe-to-shell, $() expansion
- **L3 — Interpreter Heuristics:** Pattern detection in python/perl/node/ruby scripts

113/113 tests pass. Zero false positives on safe commands.

## E2EE

Relay **cannot** read your data. X25519 key exchange + AES-256-GCM per-message encryption.

```
Agent ──[encrypt with relay's pub key]──► Relay ──[forward encrypted]──► Client
```

## Docker

```bash
docker build -f Dockerfile.relay -t flowlink-relay .
docker build -f Dockerfile.agent -t flowlink-agent .
```

## Config

See `examples/flowlink.json` and `examples/relay.json`.

## License

Private repository. All rights reserved.

<div align="center">

# 🛡️ FlowLink

**Kernel-level shield that stops AI agents from destroying your infrastructure**

[![Rust](https://img.shields.io/badge/rust-1.80+-orange?logo=rust)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-BSL_1.1-blue)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS-lightgrey)]()

*eBPF on Linux · Endpoint Security on macOS · 3-level threat analysis · Zero-trust agent management*

</div>

---

## The Problem

AI coding agents (Claude Code, Codex, Cursor, Devin…) can execute arbitrary shell commands on your machine. An AI agent running `rm -rf /` is indistinguishable from a human doing it — until it's too late.

```
# This actually happened.
# An AI agent decided to "clean up unused dependencies":
$ rm -rf /node_modules

# Oops — it ran as root. The trailing slash was missing.
# Production database gone. No snapshot. No undo.
```

**There is no permission boundary between an AI agent and your shell.** FlowLink creates one.

---

## The Demo

```bash
$ flowlink shield start
🛡️ FlowLink Shield active — monitoring all processes

# An AI agent (or anything) tries this:
$ rm -rf /etc
🚫 BLOCKED [L1/26ns] rm -rf /etc
   Threat: Critical | system_destroy
   Agent: claude-code (PID 48291)
   Snapshot: zfs snap pool/root@shield-20260406-211300
   Telegram alert → sent
   Approval required → waiting
```

The command **never executed**. The process was intercepted at the kernel level — before `execve()` returned.

---

## How It Works

```
  AI Agent → Command → Shield (kernel-level intercept)
                              │
                    ┌─────────┴──────────┐
                    │  L1: Pattern Match  │  ← 26ns  (structured args)
                    │  L2: AST Analysis   │  ← tree-sitter-bash
                    │  L3: Heuristics     │  ← interpreter detection
                    └─────────┬──────────┘
                              │
                    ┌─────────┴──────────┐
                    │ BLOCK + Alert +     │
                    │ Snapshot + Approve  │
                    └────────────────────┘
```

**On Linux:** eBPF program hooks `execve`/`execveat` syscalls. `bpf_send_signal(SIGSTOP)` freezes the process before it runs. The userspace daemon analyzes the command and decides: allow, block, or ask.

**On macOS:** Endpoint Security Framework (`ES_AUTH_EXEC`) intercepts execution at the same point — before the binary runs.

---

## Features

### 🛡️ Shield (command interception)
- **Race-free** process interception — eBPF on Linux, ES Framework on macOS
- **3-level threat analysis:**
  - L1: Structured argument pattern matching (26ns)
  - L2: AST analysis via tree-sitter-bash (pipes, redirects, chains)
  - L3: Interpreter heuristics (python `-c`, perl `-e`, ansible shell)
- **Forensic metadata** — binary, args, CWD, agent identity, PID, timestamp
- **Automatic snapshots** — ZFS/LVM snapshot before executing dangerous commands

### 📱 Agent Management
- Telegram bot for real-time command approval/rejection
- SSE/WebSocket relay for agent communication
- Device pairing with 6-digit codes (10-min TTL)
- Read-only mode toggle, kill switch, emergency pause

### 🔐 Security
- **E2EE** — X25519 ECDH key exchange + AES-256-GCM encryption
- **Zero-knowledge relay** — server cannot decrypt traffic by design
- **7-layer policy pipeline** — KillSwitch → ReadOnly → Blacklist → Sandbox → Approval → Backup → Execute
- **55+ blacklist rules** across system_destroy, security_bypass, data_theft, network_abuse
- **Audit log** with HMAC-SHA256 integrity chain
- **Key rotation** with automatic scheduling

---

## Quick Start

```bash
# Install
cargo install flowlink --features shield

# Or with one-liner (Linux/macOS)
curl -fsSL https://get.flowlink.dev | sh

# Run the shield
flowlink shield start

# Run the agent (connects to your relay)
flowlink agent start --relay wss://your-relay:8080
```

Minimal config (`~/.flowlink/config.toml`):

```toml
[shield]
enabled = true
policy = "ask"          # allow | deny | ask
snapshot = true         # auto-snapshot before dangerous commands
alert = "telegram"      # telegram | webhook | log

[shield.alerts.telegram]
bot_token = "..."
chat_id = "..."
```

---

## Benchmarks

Measured on Apple M2 Pro (macOS) / AMD EPYC 7763 (Linux) via Criterion.rs:

| Operation | Time | Notes |
|---|---|---|
| L1 pattern match (safe cmd) | **26 ns** | Structured arg inspection |
| L1 pattern match (dangerous) | **31 ns** | Early exit on match |
| L2 AST analysis (simple bash) | **~2 µs** | tree-sitter parse + walk |
| L2 complex pipe chain | **~8 µs** | Multi-stage pipe analysis |
| L3 interpreter heuristic | **~4 µs** | python/perl/ansible detection |
| Full pipeline (safe command) | **~3 µs** | L1 miss → L2 miss → L3 miss → allow |
| E2EE encrypt (1 KB) | **32 µs** | X25519 + AES-256-GCM |
| E2EE encrypt (10 KB) | **45 µs** | Same handshake, more data |
| Key generation | **~80 µs** | X25519 keypair |
| HKDF derivation | **~5 µs** | Shared secret → session keys |

**Zero perceptible overhead.** Even the full analysis pipeline adds under 10µs per command — millions of times faster than any human review.

---

## Architecture

```
flowlink/
├── crates/
│   ├── shield/     # eBPF + ES Framework interceptor, 3-level engine
│   ├── core/       # Protocol types, config, shared definitions
│   ├── crypto/     # X25519 + AES-256-GCM E2EE
│   ├── agent/      # Connects to relay, executes commands, manages backups
│   ├── relay/      # WebSocket hub, HTTP API, zero-knowledge message relay
│   └── cli/        # flowlink binary — agent and relay management
```

**Key dependencies:** aya (eBPF), tree-sitter-bash, x25519-dalek, aes-gcm, axum, tokio-tungstenite, dashmap.

---

## Comparison

| | **FlowLink** | Falco | Boundary | Manual SSH review |
|---|---|---|---|---|
| **Intercepts before exec** | ✅ eBPF/ES | ⚠️ After syscall | ❌ Userspace proxy | ❌ After execution |
| **AI agent awareness** | ✅ Agent identity | ❌ | ❌ | ❌ |
| **Real-time approval** | ✅ Telegram/WebSocket | ⚠️ Alerts only | ✅ CLI | ✅ Manual |
| **Auto-snapshot** | ✅ ZFS/LVM | ❌ | ❌ | ❌ |
| **E2EE** | ✅ X25519+AES | ❌ | ✅ | ✅ SSH |
| **Command analysis** | ✅ 3-level AST | ⚠️ Rule-based | ❌ | ✅ Human |
| **Setup complexity** | `cargo install` | Moderate | Moderate | N/A |

---

## Roadmap

- [x] **v0.1.0** — Core agent, relay, Telegram bot, dashboard
- [x] **v0.2.0** — E2EE, 7-layer policy, device pairing
- [x] **v0.3.0** — Cloud dashboard, backup management, approval queue
- [ ] **v0.4.0** — Windows Minifilter driver, cross-platform parity
- [ ] **v0.5.0** — CI/CD integration (GitHub Actions, GitLab CI hooks)
- [ ] **v1.0.0** — Enterprise: SSO, RBAC, compliance reports, cluster mode

---

## License

Business Source License 1.1 — see [LICENSE](LICENSE) for details.

---

<div align="center">

**If you're running AI agents on servers you care about, you need this.**

[GitHub](https://github.com/braincoder/flowlink) · [Website](https://flowlink.dev) · [Discord](https://discord.gg/flowlink)

</div>

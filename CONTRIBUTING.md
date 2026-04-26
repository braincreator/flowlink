# Contributing to FlowLink

Thank you for your interest in contributing! This guide covers everything you need to get started.

## Getting Started

### Prerequisites

- **Rust 1.80+** — [Install Rust](https://rustup.rs)
- **PostgreSQL 14+** — for relay database
- **Make** — build tool
- **Git** — version control

### Setup

```bash
# Clone the repository
git clone https://github.com/braincreator/flowlink.git
cd flowlink

# Build
make build

# Run tests (~1360)
make test

# Build with GitOps feature
cargo build --release --features gitops
```

## Project Structure

```
flowlink/
├── crates/
│   ├── core/       # Message types, config, channels
│   ├── crypto/     # X25519 + AES-256-GCM encryption
│   ├── db/         # PostgreSQL repos (sqlx)
│   ├── billing/    # Plans, invoices, usage, Tochka Bank
│   ├── agent/      # Dispatch, policy, sandbox, killswitch, exec
│   ├── relay/      # WS server, REST API, RBAC, E2EE, MCP
│   ├── shield/     # eBPF/macOS ES, threat analysis, L1-L7
│   ├── gitops/     # Drift detection, ServerGuard, backup engine
│   ├── k8s/        # Operator, CRD, admission webhooks
│   ├── mcp/        # MCP protocol types and server
│   ├── sentinel/   # AI Ops assistant, pattern learning
│   └── cli/        # Binary entrypoint, MCP server
├── scripts/        # Install/uninstall/update scripts
├── docs/           # Documentation
├── Cargo.toml      # Workspace config
└── Makefile
```

## Running Tests

```bash
# Run all tests
cargo test --workspace

# Run with verbose output
cargo test --workspace -- --nocapture

# Run specific crate
cargo test -p flowlink-relay

# Run with GitOps feature
cargo test --workspace --features gitops

# Run with coverage (requires tarpaulin)
cargo tarpaulin --workspace
```

## Code Style

We follow standard Rust conventions:

- **`cargo fmt`** — always format code before committing
- **`cargo clippy`** — run linter
- **`cargo check`** — fast compilation check

```bash
# Format
cargo fmt

# Lint
cargo clippy --workspace -- -D warnings

# Check
cargo check --workspace
```

### Naming Conventions

- Crates: snake_case (`flowlink-relay`, `flowlink-shield`)
- Types: PascalCase (`AppState`, `ShieldAlert`)
- Functions: snake_case (`handle_exec`, `new_pool`)
- Constants: SCREAMING_SNAKE_CASE (`MAX_BACKOFF`)
- Modules: snake_case (`gitops_bridge`, `policy_engine`)

### Error Handling

```rust
// ✅ DO: Use anyhow for application code, thiserror for library errors
fn read_config(path: &str) -> anyhow::Result<Config> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config: {}", path))?;
    Ok(serde_json::from_str(&data)?)
}

// ❌ DON'T: Unwrap in library code
fn read_config(path: &str) -> Config {
    let data = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&data).unwrap()
}
```

### Logging

Use `log` crate with structured messages:

```rust
log::info!("[gitops] Backup triggered for agent {}: {}", agent_id, backup_id);
log::warn!("Command blocked: {}", reason);
log::error!("Connection error: {}", e);
```

## Pull Request Process

1. **Fork** the repository
2. **Create a branch** from `main`:
   ```bash
   git checkout -b feature/your-feature-name
   ```
3. **Make changes** with tests
4. **Run tests** — all must pass:
   ```bash
   cargo test --workspace
   cargo clippy --workspace -- -D warnings
   ```
5. **Commit** with descriptive messages:
   ```bash
   git commit -m "feat(relay): add rate limiting middleware"
   ```
6. **Push** and open a Pull Request

### Commit Message Format

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): description

# Examples:
feat(agent): add gitops backup before destructive commands
fix(relay): handle WebSocket disconnect gracefully
docs(readme): update installation instructions
test(billing): add plan validation tests
refactor(shield): simplify pipeline levels
```

### Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation |
| `test` | Tests |
| `refactor` | Code restructuring |
| `chore` | Maintenance |
| `perf` | Performance improvement |

## Development Tips

### Running relay locally

```bash
cargo run -p flowlink -- relay --config dev/relay.json
```

### Running agent locally

```bash
cargo run -p flowlink -- agent --config dev/agent.json
```

### Running MCP server

```bash
cargo run -p flowlink -- mcp
```

### Running K8s operator

```bash
cargo run -p flowlink-k8s -- --relay-url http://localhost:8080
```

### Testing with WebSocket

```bash
# Install wscat
npm install -g wscat

# Connect to local relay
wscat -c ws://localhost:8080/ws
```

## Reporting Issues

When reporting bugs, please include:

1. **Rust version** (`rustc --version`)
2. **OS and architecture**
3. **Steps to reproduce**
4. **Expected vs actual behavior**
5. **Relevant logs**

---

Thank you for contributing to FlowLink! 🚀

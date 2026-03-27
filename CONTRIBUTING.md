# Contributing to FlowLink

Thank you for your interest in contributing! This guide covers everything you need to get started.

## Getting Started

### Prerequisites

- **Go 1.25+** — [Install Go](https://go.dev/doc/install)
- **Make** — build tool
- **Git** — version control

### Setup

```bash
# Clone the repository
git clone https://github.com/braincreator/flowlink.git
cd flowlink

# Install dependencies
go mod download

# Build
make build

# Run tests
make test
```

## Project Structure

```
flowlink/
├── cmd/
│   ├── agent/       # Agent entrypoint (main.go)
│   ├── bot/         # Telegram Bot entrypoint
│   └── relay/       # Relay entrypoint
├── internal/
│   ├── agent/       # Agent daemon (executor, sandbox, approval, backup, kill switch)
│   ├── billing/     # Plans, usage tracking, invoices
│   ├── config/      # Configuration loading
│   ├── dashboard/   # Web Dashboard SPA (embedded assets)
│   ├── protocol/    # WebSocket message types
│   ├── relay/       # Relay server (WSS, HTTP API, MCP, auth, audit, registry)
│   ├── tgbot/       # Telegram Bot (long polling)
│   └── transport/   # WebSocket transport layer
├── scripts/         # Install/uninstall/update scripts
├── docs/            # Documentation
├── web/             # Frontend assets
├── Makefile
├── go.mod
└── go.sum
```

## Running Tests

```bash
# Run all tests
go test ./...

# Run with verbose output
go test -v ./...

# Run specific package
go test ./internal/relay/...

# Run with coverage
go test -cover ./...

# Run with race detector
go test -race ./...
```

## Code Style

We follow standard Go conventions:

- **`gofmt`** — always format code before committing
- **`go vet`** — run static analysis
- **`golint`** — lint for style issues

```bash
# Format
gofmt -w .

# Vet
go vet ./...

# Lint (if golangci-lint installed)
golangci-lint run
```

### Naming Conventions

- Packages: lowercase, single word (`agent`, `relay`, `config`)
- Exported types: PascalCase (`AgentConn`, `RelayConfig`)
- Private types/functions: camelCase (`handleExec`, `newPool`)
- Constants: PascalCase for exported, camelCase for private
- Interfaces: `-er` suffix (`Handler`, `Provider`)

### Error Handling

```go
// ✅ DO: Return errors, don't panic
func readFile(path string) ([]byte, error) {
    data, err := os.ReadFile(path)
    if err != nil {
        return nil, fmt.Errorf("read file %s: %w", path, err)
    }
    return data, nil
}

// ❌ DON'T: Panic in library code
func readFile(path string) []byte {
    data, _ := os.ReadFile(path)
    return data
}
```

### Logging

Use structured logging with `log/slog`:

```go
logger.Info("agent connected",
    "agent_id", agent.ID,
    "hostname", agent.Hostname,
    "os", agent.OS,
)
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
   go test ./...
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
feat(agent): add backup rotation
fix(relay): handle WebSocket disconnect gracefully
docs(readme): update installation instructions
test(billing): add plan validation tests
refactor(protocol): simplify message types
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
go run ./cmd/relay -config ./dev/relay.yaml
```

### Running agent locally

```bash
go run ./cmd/agent -config ./dev/agent.yaml
```

### Running Telegram bot locally

```bash
TELEGRAM_BOT_TOKEN=your-token go run ./cmd/bot
```

### Testing with WebSocket

```bash
# Install wscat
npm install -g wscat

# Connect to local relay
wscat -c ws://localhost:8443/ws
```

## Reporting Issues

When reporting bugs, please include:

1. **Go version** (`go version`)
2. **OS and architecture**
3. **Steps to reproduce**
4. **Expected vs actual behavior**
5. **Relevant logs**

---

Thank you for contributing to FlowLink! 🚀

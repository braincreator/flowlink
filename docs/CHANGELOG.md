# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] - 2026-03-27

### Added

- **WebSocket relay** для управления агентами через WSS (outbound, пробивает NAT)
- **Remote command execution** с sandbox и configurable timeout
- **File manager** — read, write, list directory operations
- **Backup & restore** — снапшоты с retention, rotation, и configurable limits
- **Kill Switch** — emergency stop, pause, and readonly modes
- **Approval system** — 3 режима: auto / soft_ask / hard_ask
- **Audit logging** — JSONL format with query, export, and stats API
- **JWT authorization** with rate limiting
- **TLS** — self-signed, Let's Encrypt (auto), and manual certificate modes
- **MCP Server** — 8 инструментов для интеграции с OpenClaw via mcporter
- **Telegram Bot** — 15 команд для управления через Telegram (long polling)
- **Web Dashboard** — SPA with dark theme
- **Billing system** — 4 тарифных плана (Free / Starter / Business / Enterprise)
- **Event Streaming** — SSE for real-time notifications
- **Multi-tenancy** — clients and agents registry with data isolation
- **Install scripts** — systemd (Linux) and LaunchAgent (macOS) support
- **Autonomous tasks (L2)** — task submission, progress tracking, cancellation
- **Skill management** — push, list, delete skills on agents
- **LLM proxy** — route LLM requests through relay
- **System info** — CPU, RAM, disk, uptime monitoring

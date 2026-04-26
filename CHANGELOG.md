## [0.3.1-dev] - 2026-04-26

### 🏗️ Architecture
- Repositioned as MCP Gateway + AI-Native SecOps platform
- 12 crates, ~158K lines Rust, ~1187 tests
- GitOps module (19K lines, 183 tests) — feature-gated, ServerGuard + BackupEngine + DriftDetector
- K8s Operator (5K lines, 76 tests) — CRD + AdmissionWebhook + SidecarInjection

### 🛡️ Security
- Zero-Trust Secret Injection via HashiCorp Vault integration
- Forensic Timeline API (`/api/v1/forensics/timeline`)
- Compliance API (security_audit, policy_compliance, exec_summary, fstek)
- OWASP MCP risk mapping in docs

### 📋 Governance
- Approval Workflow (Telegram + Dashboard)
- Change Management API
- Service Catalog (80+ service types)
- Pattern Learning (behavioral baseline)

### 💰 Billing
- Plans updated: Starter (4 990₽), Professional (39 990₽), Scale (79 990₽), Enterprise (custom)
- PlanFeatures extended: forensics, service_catalog, ai_ops, change_management
- Tochka Bank payment integration

### 📊 Observability
- SIEM export: CEF, LEEF, JSON + RuSIEM + MaxPatrol connectors
- Infrastructure Map (live topology)
- Discovery (automatic service catalog)
- AI Ops Assistant

### 🌐 Website
- Hero repositioned: "MCP Gateway + AI-Native SecOps"
- New page: Hello, Secure Agent quickstart (/docs/quickstart)
- New page: FlowLink vs Competitors comparison (/docs/comparison)
- Favicon: multi-resolution ICO (16/32/48/64) + apple-touch-icon
- OG Image: Visibility/Governance/Protection triptych
- Pricing comparison table extended (18 rows)
- Features registry: 12 features (was 8)
- Docs index: NEW badges, Recommended Path updated

### 📝 Documentation
- README.md + README_ru.md fully updated
- ARCHITECTURE.md rewritten with Shield L1-L7, OWASP mapping
- ROADMAP.md: 7 of 15 phases marked as DONE
- GTM-PLAN.md: pricing synchronized with site

---

## [0.3.1] - 2026-04-05

### 🔧 Improvements
- Protocol versioning + version negotiation on connect
- Backup SHA-256 checksum verification
- Periodic backup scheduling (configurable interval)
- MCP tools: flowlink_backup_list, flowlink_backup_delete
- Public Delete() API for backups
- HealthChecker uses pkg/version instead of hardcoded value
- CORS defaults to wildcard
- Dashboard backup engine reads from config
- PID finder uses pgrep

---

## [0.3.0] - 2026-04-05

### 🖥️ Dashboard
- Backup management (create/restore/delete)
- Approval queue (approve/reject)
- Enhanced settings (storage, backup config, CORS)

### ⚙️ Backend
- 11 new DataProvider methods + API endpoints
- Relay-side BackupEngine integration

---

## [0.2.0] - 2026-04-03

**Security & Encryption — major release.**

### 🔐 Added: End-to-End Encryption (E2EE)
- X25519 ECDH key exchange for secure session establishment
- AES-256-GCM symmetric encryption for all command/response data
- Key generation, storage (chmod 0600), and rotation
- Symmetric key derivation for bidirectional communication
- Encrypted payload wire format — relay forwards blind blobs
- File encryption helpers for audit logs and backups
- Zero-knowledge relay: cannot decrypt traffic by design

### 🛡️ Added: 7-Layer Policy Pipeline
- **KillSwitch** → **Read-Only** → **Blacklist** → **Sandbox** → **Approval** → **Backup** → **Execute**
- 55+ blacklist rules across 4 categories (system_destroy, security_bypass, data_theft, network_abuse)
- Read-only mode by default for new agents
- Risk classification (low/medium/high/critical) for every command
- Regex-based pattern matching for blacklist
- Toggle read-only via Telegram: `/readonly on|off`

### 📱 Added: Device Management & Pairing
- Device registry with JSON persistence
- 6-digit pairing codes with 10-minute TTL
- Owner approves each device via Telegram (inline buttons)
- `/devices` — list connected machines with E2EE status
- `/approve_device` — approve new device
- `/reject_device` — reject pairing request
- `/revoke` — revoke device access
- `/keys` — show your encryption keys
- `/rotate` — rotate encryption keys
- `/device_info` — detailed device information

### 📄 Added: Terms of Service
- `TERMS.md` with safety guidelines and compliance

### 📚 Documentation
- README.md (EN) + README_ru.md (RU) updated with:
  - Security Architecture section
  - E2EE technical details
  - Device Management commands
  - Key exchange flow diagrams
  - Configuration reference

### 🔧 Configuration
- New `e2ee` config section (`enabled`, `auto_rotate`)

---

## [0.1.0] - 2026-04-03

Initial public release.

**Features:**
- 🖥️ Remote shell execution with sandbox and timeouts
- 📁 File manager — read, write, list directories
- 💾 Backup and restore with retention policies
- 🛑 Kill Switch — emergency stop, pause, readonly mode
- 🤖 MCP server for AI integration
- 📡 LLM Proxy — route AI requests through relay
- 🏢 Multi-tenancy with client registry
- 🔐 Authentication, rate limiting, audit logging
- 📊 Embedded web dashboard
- 🤖 Telegram bot for agent management
- 🔄 SSE real-time events
- 💳 Billing — plans, usage tracking, invoices
- 🐳 Docker support (multi-arch)
- 📦 One-line install script
- ⚙️ systemd / launchd / Windows service scripts

**Components:**
- `flowlink` — lightweight agent for client machines
- `flowlink-relay` — central relay server
- `flowlink-bot` — Telegram management interface

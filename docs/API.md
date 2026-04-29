# API Reference

Base URL: `https://relay.example.com`

All endpoints require `Authorization: Bearer ***` header unless noted otherwise.

---

## Agent WebSocket

### WS /ws

WebSocket endpoint for agent connections. Agents connect here to receive commands and send responses. Uses pairwise tokens issued at registration.

---

## Agents

### GET /api/v1/agents

List all registered agents with their connection status.

```bash
curl https://relay.example.com/api/v1/agents \
  -H "Authorization: Bearer ***"
```

**Response (200):**
```json
{
  "agents": [
    {
      "id": "agent-abc-123",
      "hostname": "prod-server-1",
      "os": "linux",
      "arch": "amd64",
      "version": "0.1.0",
      "connected": true,
      "last_seen": "2026-03-27T18:00:00Z",
      "client_id": "client-123",
      "label": "production"
    }
  ]
}
```

### POST /api/v1/agents/register

Register a new agent and receive a pairwise token.

```bash
curl -X POST https://relay.example.com/api/v1/agents/register \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{
    "client_id": "client-123",
    "hostname": "new-server",
    "label": "staging"
  }'
```

**Response (201):**
```json
{
  "agent_id": "agent-xyz-789",
  "token": "pairwi...cret",
  "relay_url": "wss://relay.example.com/ws"
}
```

### DELETE /api/v1/agents/delete/{agent_id}

Remove an agent from the registry.

```bash
curl -X DELETE https://relay.example.com/api/v1/agents/delete/agent-abc-123 \
  -H "Authorization: Bearer ***"
```

**Response (200):**
```json
{"status": "deleted"}
```

### PUT /api/v1/agents/config

Update agent configuration (e.g., labels, allowed skills).

```bash
curl -X PUT https://relay.example.com/api/v1/agents/config \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "agent-abc-123",
    "label": "production",
    "allowed_skills": ["deploy", "restart"]
  }'
```

**Response (200):**
```json
{"status": "updated"}
```

### POST /api/v1/agents/exec

Execute a shell command on a connected agent.

```bash
curl -X POST https://relay.example.com/api/v1/agents/exec \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "agent-abc-123",
    "command": "ls -la /var/log",
    "timeout_sec": 30
  }'
```

**Response (200):**
```json
{
  "request_id": "req-uuid",
  "exit_code": 0,
  "stdout": "total 128\ndrwxr-xr-x  2 root root 4096 ...",
  "stderr": "",
  "duration_ms": 45
}
```

### GET /api/v1/agents/files/read

Read a file from an agent.

```bash
curl "https://relay.example.com/api/v1/agents/files/read?agent_id=agent-abc-123&path=/etc/hostname" \
  -H "Authorization: Bearer ***"
```

**Response (200):**
```json
{
  "path": "/etc/hostname",
  "content": "prod-server-1\n",
  "size": 16
}
```

### POST /api/v1/agents/files/write

Write a file to an agent.

```bash
curl -X POST https://relay.example.com/api/v1/agents/files/write \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "agent-abc-123",
    "path": "/tmp/test.txt",
    "content": "Hello, flowlink!"
  }'
```

**Response (200):**
```json
{
  "path": "/tmp/test.txt",
  "size": 17,
  "status": "written"
}
```

### GET /api/v1/agents/files/list

List a directory on an agent.

```bash
curl "https://relay.example.com/api/v1/agents/files/list?agent_id=agent-abc-123&dir=/var/log" \
  -H "Authorization: Bearer ***"
```

**Response (200):**
```json
{
  "dir": "/var/log",
  "entries": [
    {"name": "syslog", "type": "file", "size": 102400, "modified": "2026-03-27T10:00:00Z"},
    {"name": "nginx", "type": "dir", "size": 0, "modified": "2026-03-26T08:00:00Z"}
  ]
}
```

### GET /api/v1/agents/sysinfo

Get system information from an agent.

```bash
curl "https://relay.example.com/api/v1/agents/sysinfo?agent_id=agent-abc-123" \
  -H "Authorization: Bearer ***"
```

**Response (200):**
```json
{
  "hostname": "prod-server-1",
  "os": "Ubuntu 24.04",
  "arch": "amd64",
  "cpu_count": 4,
  "cpu_usage_percent": 23.5,
  "ram_total_mb": 8192,
  "ram_used_mb": 4096,
  "disk_total_gb": 100,
  "disk_used_gb": 45,
  "uptime_sec": 864000
}
```

### POST /api/v1/agents/task

Submit an autonomous task to an agent.

```bash
curl -X POST https://relay.example.com/api/v1/agents/task \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{
    "agent_id": "agent-abc-123",
    "task": "Check disk usage and clean up /tmp if > 80%",
    "description": "Automated disk cleanup"
  }'
```

**Response (200):**
```json
{
  "task_id": "task-uuid",
  "status": "submitted"
}
```

### POST /api/v1/agents/task/cancel

Cancel a running task.

```bash
curl -X POST https://relay.example.com/api/v1/agents/task/cancel \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "agent-abc-123", "task_id": "task-uuid"}'
```

### Skills

#### POST /api/v1/agents/skills/push

Push a skill (script) to an agent.

```bash
curl -X POST https://relay.example.com/api/v1/agents/skills/push \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "agent-abc-123", "name": "deploy", "content": "..."}'
```

#### GET /api/v1/agents/skills/list

List skills available on an agent.

```bash
curl "https://relay.example.com/api/v1/agents/skills/list?agent_id=agent-abc-123" \
  -H "Authorization: Bearer ***"
```

#### POST /api/v1/agents/skills/delete

Delete a skill from an agent.

```bash
curl -X POST https://relay.example.com/api/v1/agents/skills/delete \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "agent-abc-123", "name": "deploy"}'
```

---

## Backups

### POST /api/v1/agents/backup

Trigger a backup snapshot for an agent.

```bash
curl -X POST https://relay.example.com/api/v1/agents/backup \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "agent-abc-123", "description": "Pre-deployment snapshot"}'
```

**Response (200):**
```json
{
  "backup_id": "snap-uuid",
  "status": "created"
}
```

### GET /api/v1/agents/backup/list

List available backups for an agent.

```bash
curl "https://relay.example.com/api/v1/agents/backup/list?agent_id=agent-abc-123" \
  -H "Authorization: Bearer ***"
```

**Response (200):**
```json
{
  "backups": [
    {
      "id": "snap-uuid",
      "created_at": "2026-03-27T18:00:00Z",
      "size_bytes": 52428800,
      "description": "Auto-backup"
    }
  ]
}
```

### POST /api/v1/agents/backup/restore

Restore from a backup snapshot.

```bash
curl -X POST https://relay.example.com/api/v1/agents/backup/restore \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{"agent_id": "agent-abc-123", "backup_id": "snap-uuid"}'
```

### DELETE /api/v1/agents/backup/{id}

Delete a backup snapshot.

```bash
curl -X DELETE https://relay.example.com/api/v1/agents/backup/snap-uuid \
  -H "Authorization: Bearer ***"
```

**Response (200):**
```json
{"status": "deleted"}
```

---

## LLM

### POST /api/v1/llm/chat

Send a chat completion request through the relay's LLM proxy.

```bash
curl -X POST https://relay.example.com/api/v1/llm/chat \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o",
    "messages": [{"role": "user", "content": "Hello"}],
    "agent_id": "agent-abc-123"
  }'
```

**Response (200):**
```json
{
  "id": "chatcmpl-uuid",
  "choices": [
    {"message": {"role": "assistant", "content": "Hello! How can I help?"}}
  ]
}
```

### GET /api/v1/llm/backends

List configured LLM backends.

```bash
curl https://relay.example.com/api/v1/llm/backends \
  -H "Authorization: Bearer ***"
```

**Response (200):**
```json
{
  "backends": [
    {"id": "openai", "provider": "openai", "models": ["gpt-4o", "gpt-4o-mini"], "default": true}
  ]
}
```

### GET /api/v1/llm/health

Check health of LLM backend connections.

```bash
curl https://relay.example.com/api/v1/llm/health \
  -H "Authorization: Bearer ***"
```

**Response (200):**
```json
{
  "openai": {"status": "ok", "latency_ms": 120},
  "anthropic": {"status": "ok", "latency_ms": 95}
}
```

---

## Audit

### GET /api/v1/audit

Query audit logs with filters.

```bash
curl "https://relay.example.com/api/v1/audit?agent_id=agent-abc-123&limit=50&offset=0" \
  -H "Authorization: Bearer ***"
```

**Response (200):**
```json
{
  "entries": [
    {
      "timestamp": "2026-03-27T18:00:00Z",
      "agent_id": "agent-abc-123",
      "action": "exec",
      "details": {"command": "ls -la"},
      "client_id": "client-123",
      "ip": "1.2.3.4"
    }
  ],
  "total": 142
}
```

### GET /api/v1/audit/export

Export audit logs in JSON or CSV format.

```bash
curl "https://relay.example.com/api/v1/audit/export?format=csv&from=2026-03-20&to=2026-03-27" \
  -H "Authorization: Bearer ***" \
  -o audit-export.csv
```

### GET /api/v1/audit/stats

Get audit statistics.

```bash
curl "https://relay.example.com/api/v1/audit/stats?period=7d" \
  -H "Authorization: Bearer ***"
```

---

## Approvals

### GET /api/v1/approvals

List pending approval requests.

```bash
curl https://relay.example.com/api/v1/approvals \
  -H "Authorization: Bearer ***"
```

**Response (200):**
```json
{
  "approvals": [
    {
      "id": "appr-uuid",
      "agent_id": "agent-abc-123",
      "action": "exec",
      "details": {"command": "rm -rf /tmp/old-cache"},
      "status": "pending",
      "created_at": "2026-03-27T18:00:00Z"
    }
  ]
}
```

### POST /api/v1/approvals/{id}/approve

Approve a pending action.

```bash
curl -X POST https://relay.example.com/api/v1/approvals/appr-uuid/approve \
  -H "Authorization: Bearer ***"
```

### POST /api/v1/approvals/{id}/reject

Reject a pending action.

```bash
curl -X POST https://relay.example.com/api/v1/approvals/appr-uuid/reject \
  -H "Authorization: Bearer ***"
```

---

## Clients

### GET /api/v1/clients

List all registered clients.

```bash
curl https://relay.example.com/api/v1/clients \
  -H "Authorization: Bearer ***"
```

### POST /api/v1/clients

Create a new client.

```bash
curl -X POST https://relay.example.com/api/v1/clients \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{"name": "Acme Corp", "email": "admin@acme.com", "plan": "business"}'
```

### GET /api/v1/clients/{id}

Get client details.

```bash
curl https://relay.example.com/api/v1/clients/client-123 \
  -H "Authorization: Bearer ***"
```

### POST /api/v1/clients/{id}/agents

Register an agent under a specific client.

```bash
curl -X POST https://relay.example.com/api/v1/clients/client-123/agents \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{"hostname": "web-01", "label": "production"}'
```

---

## Billing

### GET /api/v1/billing/usage

Get usage statistics for the current billing period.

```bash
curl https://relay.example.com/api/v1/billing/usage \
  -H "Authorization: Bearer ***"
```

**Response (200):**
```json
{
  "period": "2026-03",
  "commands_used": 542,
  "commands_limit": 10000,
  "agents_used": 3,
  "agents_limit": 25,
  "backups_count": 8,
  "storage_used_bytes": 1073741824,
  "storage_limit_bytes": 10737418240
}
```

### GET /api/v1/billing/plan

Get current plan details.

```bash
curl https://relay.example.com/api/v1/billing/plan \
  -H "Authorization: Bearer ***"
```

### POST /api/v1/billing/plan/change

Change the subscription plan.

```bash
curl -X POST https://relay.example.com/api/v1/billing/plan/change \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{"plan_id": "enterprise"}'
```

### GET /api/v1/billing/invoices

List invoices.

```bash
curl https://relay.example.com/api/v1/billing/invoices \
  -H "Authorization: Bearer ***"
```

### POST /api/v1/billing/invoices/{id}/pay

Pay an invoice.

```bash
curl -X POST https://relay.example.com/api/v1/billing/invoices/inv-123/pay \
  -H "Authorization: Bearer ***"
```

### GET /api/v1/billing/payment-methods

List available payment methods.

```bash
curl https://relay.example.com/api/v1/billing/payment-methods \
  -H "Authorization: Bearer ***"
```

### POST /api/v1/billing/webhook

Incoming webhook from payment provider (Tochka). No auth required — validated via webhook signature.

```bash
curl -X POST https://relay.example.com/api/v1/billing/webhook \
  -H "Content-Type: application/json" \
  -d '{"event": "payment.completed", "invoice_id": "inv-123"}'
```

---

## Nginx Config

### GET /api/v1/nginx-config

Generate an nginx configuration snippet for the relay (useful for reverse proxy setup).

```bash
curl https://relay.example.com/api/v1/nginx-config \
  -H "Authorization: Bearer ***"
```

**Response (200):** Returns a text/plain nginx configuration block.

---

## Rate Limits

### GET /api/v1/rate-limits

List rate limit configurations for all clients.

```bash
curl https://relay.example.com/api/v1/rate-limits \
  -H "Authorization: Bearer ***"
```

### POST /api/v1/rate-limits

Reset rate limit statistics (admin action).

```bash
curl -X POST https://relay.example.com/api/v1/rate-limits \
  -H "Authorization: Bearer ***"
```

### GET /api/v1/rate-limits/{client_id}

Get rate limit configuration and current usage for a specific client.

```bash
curl https://relay.example.com/api/v1/rate-limits/client-123 \
  -H "Authorization: Bearer ***"
```

### PUT /api/v1/rate-limits/{client_id}

Update rate limit configuration for a specific client.

```bash
curl -X PUT https://relay.example.com/api/v1/rate-limits/client-123 \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{"requests_per_minute": 100, "requests_per_hour": 1000}'
```

### DELETE /api/v1/rate-limits/{client_id}

Remove custom rate limit configuration for a client (reverts to defaults).

```bash
curl -X DELETE https://relay.example.com/api/v1/rate-limits/client-123 \
  -H "Authorization: Bearer ***"
```

### GET /api/v1/rate-limits/stats

Get aggregate rate limit statistics across all clients.

```bash
curl https://relay.example.com/api/v1/rate-limits/stats \
  -H "Authorization: Bearer ***"
```

---

## Health

### GET /api/v1/health

Full health report including all subsystems (DB, agents, LLM, etc.).

```bash
curl https://relay.example.com/api/v1/health
```

No auth required.

**Response (200):**
```json
{
  "status": "ok",
  "version": "0.1.0",
  "uptime_sec": 86400,
  "agents_connected": 3,
  "db": {"status": "ok"},
  "llm": {"status": "ok"}
}
```

### GET /api/v1/health/ready

Readiness probe. Returns 200 if the relay is ready to accept traffic, 503 otherwise.

```bash
curl https://relay.example.com/api/v1/health/ready
```

No auth required.

### GET /api/v1/health/live

Liveness probe. Returns 200 if the relay process is alive, 503 otherwise.

```bash
curl https://relay.example.com/api/v1/health/live
```

No auth required.

---

## Integration Proxy

### * /api/v1/integration/*

Proxy endpoint for external service integrations (billing, S3, etc.). Only enabled when `integration_url` is configured. Routes are forwarded to the configured integration backend.

```bash
curl https://relay.example.com/api/v1/integration/billing/status \
  -H "Authorization: Bearer ***"
```

---

## Events (SSE)

### GET /api/v1/events

Server-Sent Events stream for real-time notifications.

```bash
curl -N https://relay.example.com/api/v1/events \
  -H "Authorization: Bearer ***"
```

**Event format:**
```
event: agent_connected
data: {"agent_id": "agent-abc-123", "hostname": "prod-server-1", "timestamp": "..."}

event: exec_output
data: {"request_id": "req-uuid", "agent_id": "agent-abc-123", "chunk": "output line...", "stream": "stdout"}

event: exec_done
data: {"request_id": "req-uuid", "exit_code": 0, "duration_ms": 45}

event: agent_disconnected
data: {"agent_id": "agent-abc-123", "reason": "heartbeat_timeout"}
```

**Event types:** `agent_connected`, `agent_disconnected`, `exec_output`, `exec_done`, `approval_request`, `backup_created`, `task_progress`, `task_done`.

---

## MCP

### POST /mcp

JSON-RPC 2.0 MCP endpoint. Used by OpenClaw via mcporter.

```bash
curl -X POST https://relay.example.com/mcp \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/list"
  }'
```

```bash
curl -X POST https://relay.example.com/mcp \
  -H "Authorization: Bearer ***" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
      "name": "flowlink_exec",
      "arguments": {
        "agent_id": "agent-abc-123",
        "command": "uptime"
      }
    }
  }'
```

---

## Dashboard

### GET /dashboard/

Web Dashboard SPA (dark theme). Authenticated via API token.

```bash
curl https://relay.example.com/dashboard/ \
  -H "Authorization: Bearer ***"
```

---

## Error Responses

All errors follow a consistent format:

```json
{
  "error": "agent not found",
  "code": "NOT_FOUND",
  "status": 404
}
```

| HTTP Status | Code | Description |
|-------------|------|-------------|
| 400 | BAD_REQUEST | Invalid request body or parameters |
| 401 | UNAUTHORIZED | Missing or invalid token |
| 403 | FORBIDDEN | Insufficient permissions |
| 404 | NOT_FOUND | Resource not found |
| 409 | CONFLICT | Agent already registered |
| 429 | RATE_LIMITED | Too many requests |
| 500 | INTERNAL_ERROR | Server error |

---

## GitOps

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/gitops/drift/{agent_id}` | Drift status for agent |
| POST | `/api/v1/gitops/backup/{agent_id}` | Trigger backup |
| GET | `/api/v1/gitops/backups/{agent_id}` | List backups |
| POST | `/api/v1/gitops/restore/{agent_id}` | Restore from backup |
| GET | `/api/v1/gitops/guard/{agent_id}` | Server guard status |

## Compliance & Forensics

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/compliance/audit` | Compliance audit report |
| GET | `/api/v1/compliance/policy` | Policy compliance check |
| GET | `/api/v1/compliance/exec-summary` | Executive security summary |
| GET | `/api/v1/compliance/fstek` | FSTEK/152-FZ compliance |
| GET | `/api/v1/forensics/timeline` | Forensic incident timeline |

## Discovery & Infrastructure

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/discovery/services` | Discovered services catalog |
| GET | `/api/v1/health/agents` | Agent health overview |
| GET | `/api/v1/infra/map` | Infrastructure topology map |

## Authentication

All auth endpoints are **public** (no JWT required) unless noted.

### Email Authentication

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/auth/email/send-code` | No | Send email verification code (rate-limited) |
| POST | `/api/auth/email/verify` | No | Verify email code and get JWT |
| POST | `/api/auth/email/change-start` | JWT | Start email change flow |
| POST | `/api/auth/email/change-confirm` | JWT | Confirm email change with code |

### OAuth2 Social Login

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/auth/oauth-url?provider={vk\|yandex\|github}` | No | Get OAuth2 authorization URL |
| GET | `/api/auth/vk/callback` | No | VK OAuth2 callback |
| GET | `/api/auth/yandex/callback` | No | Yandex OAuth2 callback |
| GET | `/api/auth/github/callback` | No | GitHub OAuth2 callback |

### SAML SSO

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/auth/saml/login` | No | Redirect to IdP |
| POST | `/auth/saml/acs` | No | SAML Assertion Consumer Service |
| GET | `/auth/saml/metadata` | No | SAML SP metadata XML |

### Session & Account Management

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/auth/me` | JWT | Current user info |
| GET | `/api/auth/account` | JWT | Full account details |
| POST | `/api/auth/logout` | JWT | Invalidate current session |
| POST | `/api/auth/refresh` | Refresh Token | Renew access token |
| GET | `/api/auth/providers` | No | List enabled auth providers |
| POST | `/api/auth/link-email` | JWT | Link email to social account |

### Two-Factor Authentication (2FA)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/auth/2fa/setup` | JWT | Generate TOTP secret + QR |
| POST | `/api/auth/2fa/enable` | JWT | Enable 2FA with valid TOTP code |
| POST | `/api/auth/2fa/complete` | Temp Token | Complete 2FA during login |
| GET | `/api/auth/2fa/status` | JWT | Check if 2FA is enabled |
| POST | `/api/auth/2fa/disable` | JWT | Disable 2FA |

### Session Management

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/auth/sessions` | JWT | List all active sessions |
| DELETE | `/api/auth/sessions` | JWT | Revoke all other sessions |
| DELETE | `/api/auth/sessions/{id}` | JWT | Revoke specific session |

## Account Management

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/account` | JWT | Get account info |
| GET | `/api/account/info` | JWT | Detailed account profile |
| DELETE | `/api/account` | JWT | Request account deletion (30-day grace) |
| POST | `/api/account/cancel-deletion` | JWT | Cancel pending deletion |
| DELETE | `/api/account/hard` | JWT | Immediate hard delete (admin) |
| GET | `/api/account/settings` | JWT | User preferences |
| PATCH | `/api/account/settings` | JWT | Update preferences |
| GET | `/api/account/notifications` | JWT | Account notification history |
| POST | `/api/account/notifications/{id}/read` | JWT | Mark notification as read |

## Integrations Marketplace

All integration endpoints require JWT authentication.

### Catalog & Discovery

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/integrations/catalog` | JWT | List available integration types |
| GET | `/api/integrations` | JWT | List installed integrations |
| POST | `/api/integrations` | JWT | Install an integration |
| DELETE | `/api/integrations/{id}` | JWT | Uninstall an integration |

**Install request body:**
```json
{
  "kind": "telegram",
  "name": "My Telegram Bot",
  "config": { "bot_token": "123456:ABC-DEF" },
  "subscribed_events": ["shield.alert", "approval.request"],
  "org_id": "optional-org-id"
}
```

**Available kinds:** `telegram`, `slack`, `discord`, `github`, `max`, `webhook`

### OAuth2 Flow

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/integrations/oauth/begin` | JWT | Start OAuth2 (returns `authorize_url`) |
| GET | `/api/integrations/oauth/callback` | No | OAuth2 callback (provider redirects here) |

**OAuth begin request body:**
```json
{
  "kind": "slack",
  "redirect_after": "https://app.example.com/integrations"
}
```

**OAuth begin response:**
```json
{
  "authorize_url": "https://slack.com/oauth/v2/authorize?...",
  "integration_id": "uuid"
}
```

## Organizations

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/orgs` | JWT | Create organization |
| POST | `/api/orgs/onboard` | JWT | Onboarding wizard |
| POST | `/api/orgs/switch` | JWT | Switch active organization |
| GET | `/api/orgs/invites/accept?code={code}` | JWT | Accept org invitation |

### Organization Management

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/orgs/{org_id}` | JWT | Organization details |
| GET | `/api/orgs/{org_id}/health` | JWT | Organization health status |
| GET | `/api/orgs/{org_id}/members` | JWT | List org members |
| DELETE | `/api/orgs/{org_id}/members/{account_id}` | JWT | Remove member |
| POST | `/api/orgs/{org_id}/members/{account_id}/assign-role` | JWT | Assign RBAC role |
| GET | `/api/orgs/{org_id}/invites` | JWT | List pending invitations |
| GET | `/api/orgs/{org_id}/audit` | JWT | Organization audit log |

### Organization RBAC

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/orgs/{org_id}/roles` | JWT | List custom roles |
| POST | `/api/orgs/{org_id}/roles` | JWT | Create custom role |
| PUT | `/api/orgs/{org_id}/roles/{role_id}` | JWT | Update role |
| DELETE | `/api/orgs/{org_id}/roles/{role_id}` | JWT | Delete role |

### Organization Webhooks

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/orgs/{org_id}/webhooks` | JWT | List org webhooks |
| POST | `/api/orgs/{org_id}/webhooks` | JWT | Create webhook |
| GET | `/api/orgs/{org_id}/webhooks/{id}` | JWT | Get webhook details |
| PUT | `/api/orgs/{org_id}/webhooks/{id}` | JWT | Update webhook |
| DELETE | `/api/orgs/{org_id}/webhooks/{id}` | JWT | Delete webhook |
| POST | `/api/orgs/{org_id}/webhooks/{id}/test` | JWT | Send test webhook |

### Secrets & Vault

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/orgs/{org_id}/secrets/config` | JWT | Get secrets configuration |
| POST | `/api/orgs/{org_id}/secrets/config/key-setup` | JWT | Setup encryption key |
| POST | `/api/orgs/{org_id}/secrets/config/vault-setup` | JWT | Configure external vault |
| GET | `/api/orgs/{org_id}/secrets/config/vault` | JWT | Get vault config |
| GET | `/api/orgs/{org_id}/vault/health` | JWT | Vault health check |

### Service Map & Discovery

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/orgs/{org_id}/map/summary` | JWT | Infrastructure summary |
| GET | `/api/orgs/{org_id}/map/services` | JWT | List discovered services |
| GET | `/api/orgs/{org_id}/map/service/{service_id}/topology` | JWT | Service topology |
| GET | `/api/orgs/{org_id}/map/service/{service_id}/secrets` | JWT | Service secrets |
| POST | `/api/orgs/{org_id}/discovery/start` | JWT | Start infrastructure discovery |
| GET | `/api/orgs/{org_id}/discovery/results` | JWT | Get discovery results |
| POST | `/api/orgs/{org_id}/discovery/submit` | JWT | Submit discovery data |
| POST | `/api/orgs/{org_id}/discovery/{scan_id}/approve` | JWT | Approve discovery scan |

## Billing (Full)

### Public Endpoints (No Auth)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/plans` | No | List available plans |
| POST | `/api/billing/webhook/tochka` | No | Точка Банк payment webhook (HMAC-verified) |
| POST | `/api/billing/check-expiry` | API Key | Check and expire overdue subscriptions |
| POST | `/api/billing/cleanup-expired-deletions` | API Key | GDPR cleanup of expired deletions |

### Subscription Management

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/billing` | JWT | Get billing overview |
| GET | `/api/billing/plans` | JWT | List plans with current subscription |
| GET | `/api/billing/my-plan` | JWT | Current plan details |
| GET | `/api/billing/check-feature?feature={name}` | JWT | Check if feature is available on current plan |
| POST | `/api/billing/change-plan` | JWT | Change plan (upgrade/downgrade) |
| GET | `/api/billing/subscription` | JWT | Current subscription details |
| POST | `/api/billing/subscription/change-plan` | JWT | Change subscription plan |
| POST | `/api/billing/subscription/pause` | JWT | Pause subscription |
| POST | `/api/billing/subscription/resume` | JWT | Resume subscription |
| DELETE | `/api/billing/subscription` | JWT | Cancel Точка subscription |
| GET | `/api/billing/subscriptions` | JWT | List all subscriptions |
| POST | `/api/billing/subscriptions/{id}/cancel` | JWT | Cancel specific subscription |

### Payments & Invoices

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/billing/subscribe` | JWT | Create subscription (Точка Банк) |
| POST | `/api/billing/create-payment` | JWT | Create one-time payment |
| GET | `/api/billing/invoices` | JWT | List invoices |
| GET | `/api/billing/invoices/{id}` | JWT | Get invoice details |
| GET | `/api/billing/payments/methods` | JWT | List payment methods |
| GET | `/api/billing/orders` | JWT | List orders |
| POST | `/api/billing/orders` | JWT | Create order |
| GET | `/api/billing/usage` | JWT | Usage statistics |

**5-Tier Plan System:**

| Plan | Price | Agents | Key Features |
|------|-------|--------|-------------|
| Free | 0₽ | 1 | Basic shield, approval, audit |
| Starter | 2,990₽/mo | 3 | Policy engine, E2EE, redaction |
| Professional | 19,990₽/mo | 10 | RBAC, patterns, SIEM, forensics |
| Scale | 49,990₽/mo | 30 | SSO, AI ops, service catalog |
| Enterprise | Custom | Unlimited | On-prem, dedicated support |

## Notifications

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/notifications/channels` | JWT | List notification channels |
| POST | `/api/notifications/channels` | JWT | Add notification channel |
| GET | `/api/notifications/channels/{id}` | JWT | Get channel details |
| PUT | `/api/notifications/channels/{id}` | JWT | Update channel |
| DELETE | `/api/notifications/channels/{id}` | JWT | Remove channel |
| POST | `/api/notifications/channels/{id}/verify` | JWT | Verify channel ownership |
| POST | `/api/notifications/channels/{id}/primary` | JWT | Set as primary channel |
| POST | `/api/notifications/link-code` | JWT | Generate linking code |
| POST | `/api/notifications/confirm-code` | JWT | Confirm linking code |
| POST | `/api/notifications/test` | JWT | Send test notification |

## Devices

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/devices/pair` | JWT | Start device pairing |
| POST | `/api/devices/confirm` | JWT | Confirm pairing with code |
| GET | `/api/devices` | JWT | List paired devices |
| DELETE | `/api/devices/{id}` | JWT | Remove device |
| POST | `/api/devices/{id}/trust` | JWT | Trust a device |

## Shield

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/shield/alerts` | JWT | List shield alerts |
| GET | `/api/shield/stats` | JWT | Shield statistics |
| POST | `/api/shield/resolve` | JWT | Resolve alert |
| POST | `/api/shield/approve/{pid}` | JWT | Approve process |
| POST | `/api/shield/reject/{pid}` | JWT | Reject process |
| POST | `/api/shield/ingest` | JWT | Ingest alert data |
| GET | `/api/shield/canary` | JWT | Canary token status |

## Admin

All admin endpoints require JWT with `is_admin = true`.

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/admin/config` | Admin | Get server configuration |
| POST | `/api/admin/config/reload` | Admin | Hot-reload configuration |
| GET | `/api/admin/dashboard-stats` | Admin | Dashboard statistics |
| GET | `/api/admin/accounts` | Admin | List all accounts |
| POST | `/api/admin/accounts/{id}/plan` | Admin | Change account plan |
| POST | `/api/admin/accounts/{id}/toggle` | Admin | Enable/disable account |
| GET | `/api/admin/clients` | Admin | List registered clients |
| GET | `/api/admin/orders` | Admin | List all orders |
| GET | `/api/admin/subscriptions` | Admin | List all subscriptions |
| GET | `/api/admin/plans` | Admin | Manage plans |
| PUT | `/api/admin/plans/{id}` | Admin | Update plan |
| GET | `/api/admin/audit/query` | Admin | Query audit log |
| GET | `/api/admin/audit/stats` | Admin | Audit statistics |
| GET | `/api/admin/audit/export` | Admin | Export audit log |
| GET | `/api/admin/shield/alerts` | Admin | Shield alerts (admin view) |
| GET | `/api/admin/shield/stats` | Admin | Shield statistics (admin) |
| GET | `/api/admin/waitlist` | Admin | List waitlist entries |
| POST | `/api/admin/waitlist/notify` | Admin | Send waitlist notification |
| GET | `/api/admin/llm/backends` | Admin | List LLM backends |
| GET | `/api/admin/llm/health` | Admin | LLM backends health |

## API Keys

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/v1/api-keys` | JWT | Create API key |
| GET | `/api/v1/api-keys` | JWT | List API keys |
| DELETE | `/api/v1/api-keys/{key_id}` | JWT | Delete API key |
| POST | `/api/v1/api-keys/{key_id}/revoke` | JWT | Revoke API key |
| POST | `/api/v1/api-keys/{key_id}/rotate` | JWT | Rotate API key secret |

## Secrets Management

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/secrets` | JWT | List secrets |
| POST | `/api/v1/secrets` | JWT | Create secret |
| GET | `/api/v1/secrets/{id}` | JWT | Get secret |
| POST | `/api/v1/secrets/inject` | JWT | Inject secrets into agent |
| GET | `/api/v1/secret-mappings` | JWT | List secret mappings |
| POST | `/api/v1/secret-mappings` | JWT | Create secret mapping |
| DELETE | `/api/v1/secret-mappings/{id}` | JWT | Delete secret mapping |

## Policies

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/policies` | JWT | List policies |
| POST | `/api/v1/policies` | JWT | Create policy |
| GET | `/api/v1/policies/{id}` | JWT | Get policy |
| DELETE | `/api/v1/policies/{id}` | JWT | Delete policy |
| POST | `/api/v1/policies/bind` | JWT | Bind policy to agent |
| POST | `/api/v1/policies/unbind` | JWT | Unbind policy from agent |

## Command Patterns & History

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/patterns` | JWT | List learned command patterns |
| POST | `/api/v1/patterns/apply` | JWT | Apply pattern as policy |
| GET | `/api/v1/commands/history` | JWT | Command execution history |
| GET | `/api/v1/commands/history/{id}` | JWT | Get command details |
| GET | `/api/v1/commands/stats` | JWT | Command statistics |

## Agent Health & Tags

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/agents/health/overview` | JWT | All agents health overview |
| GET | `/api/v1/agents/{agent_id}/health` | JWT | Single agent health |
| GET | `/api/v1/agents/{agent_id}/health/timeseries` | JWT | Health metrics timeline |
| GET | `/api/v1/agents/tags` | JWT | List all tags |
| POST | `/api/v1/agents/tags` | JWT | Create tag |
| POST | `/api/v1/agents/{agent_id}/tags` | JWT | Tag an agent |
| GET | `/api/v1/tags` | JWT | Global tag list |

## Interactive Sessions

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/sessions` | JWT | List interactive sessions |
| GET | `/api/v1/sessions/{id}` | JWT | Get session details |

## Forensics (Extended)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/forensics/snapshots` | JWT | List forensic snapshots |
| POST | `/api/v1/forensics/snapshot` | JWT | Create snapshot |
| GET | `/api/v1/forensics/snapshot/{snapshot_id}` | JWT | Get snapshot |
| GET | `/api/v1/forensics/diff/{ida}/{idb}` | JWT | Diff two snapshots |
| POST | `/api/v1/forensics/reconstruct/{agent_id}` | JWT | Reconstruct agent state |
| GET | `/api/v1/forensics/report` | JWT | Generate forensics report |

## Change Management

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/changes` | JWT | List pending changes |
| POST | `/api/v1/changes/{change_id}/approve` | JWT | Approve change |
| POST | `/api/v1/changes/{change_id}/rollback` | JWT | Rollback change |

## Service Catalog

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/catalog/services` | JWT | Service catalog |
| GET | `/api/v1/catalog/summary` | JWT | Catalog summary |
| GET | `/api/v1/catalog/efficiency` | JWT | Efficiency metrics |

## Compliance Reports

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/compliance/reports` | JWT | List compliance reports |
| GET | `/api/v1/compliance/reports/{id}` | JWT | Get specific report |

## AI Ops

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/v1/ops/ask` | JWT | Ask AI operations question |
| POST | `/api/v1/shield/dry-run` | JWT | Dry-run shield policy |

## Control Plane

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/v1/signup` | API Key | Register new agent/client |
| POST | `/api/v1/heartbeat` | API Key | Agent heartbeat |

## External Webhooks (Public)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/webhooks/alertmanager` | No | Prometheus Alertmanager webhook |
| POST | `/api/webhooks/generic-alert` | No | Generic alert ingestion (Prometheus/Zabbix/Grafana) |
| POST | `/api/tg/webhook` | No | Telegram bot webhook (receives updates) |

## Config & Agent Management

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/config` | JWT | Get agent configuration |
| POST | `/api/config/push/{agent_id}` | JWT | Push config to agent |
| POST | `/api/exec/{agent_id}` | JWT | Execute command on agent |
| GET | `/api/agents` | JWT | List connected agents |
| GET | `/api/approvals` | JWT | List pending approvals |
| POST | `/api/approvals/{id}/approve` | JWT | Approve request |
| POST | `/api/approvals/{id}/reject` | JWT | Reject request |

## LLM

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/llm` | JWT | LLM proxy status |

## Audit (User)

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/audit` | JWT | User audit log |
| POST | `/api/audit/event` | JWT | Create audit event |
| GET | `/api/audit/stats` | JWT | Audit statistics |
| GET | `/api/audit/export` | JWT | Export audit log |
| GET | `/api/trace/{correlation_id}` | JWT | Trace correlation ID |

## Other

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/playground/scan` | No | Public security playground scan |
| POST | `/api/waitlist` | No | Waitlist signup |
| GET | `/api/events` | JWT/SSE Token | SSE event stream |
| GET | `/health` | No | Detailed health check (DB, agents, shield) |
| GET | `/healthz` | No | Simple liveness check |
| GET | `/metrics` | JWT | Prometheus metrics |
| POST | `/mcp` | JWT | MCP JSON-RPC |
| GET | `/mcp/stream` | JWT | MCP SSE stream |
| POST | `/mcp/stream` | JWT | MCP stream POST |
| DELETE | `/mcp/stream` | JWT | MCP stream close |
| GET | `/ws` | Token | WebSocket (agent connection) |
| GET | `/dashboard` | No | Dashboard SPA |
| GET | `/dashboard/{*path}` | No | Dashboard SPA (all routes) |

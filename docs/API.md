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

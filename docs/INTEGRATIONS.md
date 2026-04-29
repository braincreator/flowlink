# FlowLink Integrations Marketplace

## Overview

FlowLink includes a built-in integration marketplace that allows users and organizations to connect external services for notifications, automation, and workflows.

**Key features:**
- 🔌 **Hot-pluggable** — install/uninstall integrations while relay is running
- 🔐 **OAuth2 support** — automatic token management with refresh
- 👥 **Per-user & per-org** — personal bots and organization-wide integrations
- 📡 **Event subscriptions** — fine-grained control over which events to receive
- 🔄 **Persistent** — integrations survive relay restarts (stored in DB)
- 🧩 **Extensible** — add new integration types with a single Rust file

---

## API Endpoints

All endpoints require JWT authentication unless noted.

### Browse Catalog

```
GET /api/integrations/catalog
```

Returns available integration types with metadata and configuration schemas.

**Response:**
```json
{
  "integrations": [
    {
      "kind": "telegram",
      "display_name": "Telegram Bot",
      "description": "Connect your own Telegram bot...",
      "icon": "🤖",
      "category": "messenger",
      "requires_oauth": false,
      "config_schema": {
        "type": "object",
        "required": ["bot_token"],
        "properties": {
          "bot_token": { "type": "string", "title": "Bot Token" }
        }
      },
      "available_events": [
        { "event_type": "agent_connected", "display_name": "Agent Connected" }
      ]
    }
  ]
}
```

### List Installed Integrations

```
GET /api/integrations
```

Returns all integrations for the authenticated user, including org integrations where user is a member.

**Response:**
```json
[
  {
    "id": "550e8400-...",
    "kind": "telegram",
    "name": "My Work Bot",
    "status": "active",
    "config": { "bot_token": "***" },
    "subscribed_events": ["agent_connected", "shield_alert"],
    "org_id": null,
    "has_tokens": false,
    "created_at": "2026-04-29T12:00:00Z"
  }
]
```

### Install Integration (non-OAuth)

```
POST /api/integrations
```

Install and start a new integration immediately.

**Request:**
```json
{
  "kind": "telegram",
  "name": "My Telegram Bot",
  "config": {
    "bot_token": "123456:ABC-DEF..."
  },
  "subscribed_events": ["agent_connected", "shield_alert", "approval_requested"],
  "org_id": null
}
```

**Response (201):**
```json
{
  "id": "550e8400-...",
  "kind": "telegram",
  "status": "active"
}
```

### Begin OAuth2 Flow

```
POST /api/integrations/oauth/begin
```

For integrations requiring OAuth2 (Slack, Discord, GitHub). Returns the authorization URL to redirect the user to.

**Request:**
```json
{
  "kind": "slack",
  "name": "Company Slack",
  "subscribed_events": ["shield_alert", "approval_requested"],
  "org_id": "org-uuid-here"
}
```

**Response:**
```json
{
  "authorize_url": "https://slack.com/oauth/v2/authorize?client_id=...&scope=...&state=...",
  "integration_id": "550e8400-..."
}
```

The frontend should redirect the user to `authorize_url`.

### OAuth2 Callback

```
GET /api/integrations/oauth/callback?code=xxx&state=yyy
```

**Public endpoint** — called by the OAuth2 provider after user authorization. Exchanges the authorization code for access tokens, persists them, and redirects back to the dashboard.

**Flow:**
```
User → authorize_url → Provider (Slack/Discord/GitHub)
                                    ↓ redirects to
                         /api/integrations/oauth/callback?code=X&state=Y
                                    ↓
                         Relay: exchange code → tokens → save to DB → start integration
                                    ↓ redirects to
                         /dashboard/settings/integrations?status=connected
```

### Uninstall Integration

```
DELETE /api/integrations/{id}
```

Stops the integration and marks it as uninstalled in the database.

**Response:**
```json
{ "status": "uninstalled" }
```

---

## Available Integrations

### 🤖 Telegram Bot

| Property | Value |
|---|---|
| **Kind** | `telegram` |
| **OAuth** | No |
| **Config** | `bot_token` (required), `admin_chat_id`, `webhook_url` |
| **Scopes** | Personal, Organization |

Each user connects their own bot from @BotFather. The bot sends notifications and accepts commands (`/status`, `/billing`, `/shield`, `/help`).

```bash
# Install
curl -X POST https://relay/api/integrations \
  -H "Authorization: Bearer TOKEN" \
  -d '{"kind":"telegram","config":{"bot_token":"123456:ABC"},"subscribed_events":["agent_connected"]}'
```

### 💬 Slack

| Property | Value |
|---|---|
| **Kind** | `slack` |
| **OAuth** | Yes |
| **Scopes** | `chat:write chat:write.public channels:read groups:read im:write` |
| **Org only** | Yes (workspace-level) |

Requires creating a Slack App. The OAuth flow handles installation to the workspace.

```bash
# Begin OAuth
curl -X POST https://relay/api/integrations/oauth/begin \
  -H "Authorization: Bearer TOKEN" \
  -d '{"kind":"slack","subscribed_events":["shield_alert","approval_requested"]}'
```

### 🎮 Discord

| Property | Value |
|---|---|
| **Kind** | `discord` |
| **OAuth** | Yes |
| **Scopes** | `bot identify webhooks.incoming` |
| **Org only** | Yes |

### 🐙 GitHub

| Property | Value |
|---|---|
| **Kind** | `github` |
| **OAuth** | Yes |
| **Scopes** | `repo read:org` |
| **Personal, Org** | Both |

Can create issues for security alerts and link commits to approval workflows.

### 🔗 Custom Webhook

| Property | Value |
|---|---|
| **Kind** | `webhook` |
| **OAuth** | No |
| **Config** | `url` (required), `secret`, `headers`, `method`, `timeout_secs`, `retries`, `event_filter` |
| **Personal, Org** | Both |

Forwards all subscribed events as JSON POST requests to any HTTP endpoint. Includes HMAC-SHA256 signature verification (`X-FlowLink-Signature`) when `secret` is configured. Retries up to 3 times with exponential backoff on failure.

**Payload format:**
```json
{
  "event": "shield_alert",
  "timestamp": "2025-01-01T00:00:00Z",
  "account_id": "...",
  "integration_id": "...",
  "data": { "agent_id": "...", "risk": "high", "command": "..." },
  "signature": "sha256=<hex>"
}
```

**Headers sent:**
- `Content-Type: application/json`
- `X-FlowLink-Event: <event_type>`
- `X-FlowLink-Delivery: <integration_id>`
- `X-FlowLink-Signature: sha256=<hex>` (if secret configured)
- All custom headers from config

```bash
curl -X POST https://relay/api/integrations \
  -d '{"kind":"webhook","config":{"url":"https://example.com/hook","secret":"my-secret"},"subscribed_events":["shield_alert"]}'
```

### 📱 MAX Messenger

| Property | Value |
|---|---|
| **Kind** | `max` |
| **OAuth** | No |
| **Config** | `access_token` (required), `chat_id`, `webhook_url` |
| **Personal, Org** | Both |

VK MAX messenger integration. Sends HTML-formatted notifications to chats via `platform-api.max.ru`. Supports bot commands (`/status`, `/servers`, `/billing`, `/shield`, etc.) via FlowLinkClient — same as Telegram.

**API:**
- Send: `POST https://platform-api.max.ru/messages?chat_id={id}`
- Auth: `Authorization: <access_token>` header
- Format: HTML
- Rate limit: 30 rps
- Webhook: `POST https://platform-api.max.ru/subscriptions`

**Deep links:**
```
https://max.ru/<botName>?start=<payload>
```

```bash
curl -X POST https://relay/api/integrations \
  -d '{"kind":"max","config":{"access_token":"AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw","chat_id":123456789},"subscribed_events":["agent_connected","agent_disconnected","shield_alert","approval_requested"]}'
```

---

## Event Types

| Event | Description |
|---|---|
| `agent_connected` | Server agent connected to relay |
| `agent_disconnected` | Server agent disconnected |
| `shield_alert` | Shield detected a risky command |
| `approval_requested` | Command requires manual approval |
| `approval_resolved` | Approval was approved or denied |
| `payment_received` | Payment processed |
| `subscription_changed` | Plan subscription changed |
| `system_alert` | System-level alert |

---

## Organization Integrations

Org-level integrations are visible to all org members but can only be managed by **owners** and **admins**.

```bash
# Install Slack for an organization
curl -X POST https://relay/api/integrations/oauth/begin \
  -d '{"kind":"slack","org_id":"org-uuid","subscribed_events":["shield_alert"]}'

# List integrations (returns personal + org where member)
curl https://relay/api/integrations
```

**RBAC rules:**
- **Owner/Admin**: install, configure, uninstall org integrations
- **Member**: view org integrations, manage personal integrations
- **Non-member**: no access to org integrations

---

## OAuth2 Token Lifecycle

```
┌─────────────────────────────────────────┐
│ Integration installed                    │
│ status: pending_auth                     │
└─────────────┬───────────────────────────┘
              │ User clicks "Connect"
              ▼
┌─────────────────────────────────────────┐
│ POST /oauth/begin → authorize_url       │
│ User redirected to provider             │
└─────────────┬───────────────────────────┘
              │ User authorizes
              ▼
┌─────────────────────────────────────────┐
│ GET /oauth/callback?code=X&state=Y      │
│ Relay exchanges code for tokens         │
│ Tokens stored in DB (encrypted)         │
│ status: active                          │
└─────────────┬───────────────────────────┘
              │ Token expires
              ▼
┌─────────────────────────────────────────┐
│ Auto-refresh using refresh_token        │
│ New tokens saved to DB                  │
│ If refresh fails → status: token_expired│
│ User must re-authorize                  │
└─────────────────────────────────────────┘
```

---

## Adding a New Integration Type

### 1. Create the integration crate

```rust
// crates/integrations-myservice/src/lib.rs
use flowlink_integrations_core::*;

pub struct MyServiceIntegration;

#[async_trait]
impl Integration for MyServiceIntegration {
    fn kind(&self) -> IntegrationKind { IntegrationKind("myservice".into()) }

    fn meta(&self) -> IntegrationMeta {
        IntegrationMeta {
            kind: IntegrationKind("myservice".into()),
            display_name: "My Service".into(),
            description: "Connect My Service for ...".into(),
            icon: "🔧".into(),
            category: IntegrationCategory::Monitoring,
            config_schema: serde_json::json!({ /* JSON Schema */ }),
            available_events: EventDescriptor::all_events(),
            supports_user_instances: true,
            supports_org_instances: true,
            requires_oauth: false, // or true + oauth_config()
            oauth_config: None,
            author: "FlowLink".into(),
            version: "1.0.0".into(),
        }
    }

    async fn validate_config(&self, config: &serde_json::Value) -> Result<(), IntegrationError> {
        // Validate config fields
        Ok(())
    }

    async fn start(&self, id: IntegrationId, config: IntegrationConfig, mut event_rx: broadcast::Receiver<IntegrationEvent>) -> Result<(), IntegrationError> {
        // Start event processing loop
        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => { /* handle event */ }
                    Err(broadcast::error::RecvError::Closed) => break,
                    _ => continue,
                }
            }
        });
        Ok(())
    }

    async fn stop(&self, id: &IntegrationId) -> Result<(), IntegrationError> { Ok(()) }
    async fn handle_event(&self, event: &IntegrationEvent, config: &IntegrationConfig) -> Vec<IntegrationAction> { vec![] }
    async fn handle_command(&self, cmd: &str, args: &serde_json::Value, config: &IntegrationConfig) -> Result<serde_json::Value, IntegrationError> { Ok(serde_json::json!({})) }
    async fn health_check(&self, id: &IntegrationId) -> Result<bool, IntegrationError> { Ok(true) }
}
```

### 2. Register in relay startup

```rust
// crates/relay/src/lib.rs — in the IntegrationManager block
mgr.register(Arc::new(flowlink_myservice::MyServiceIntegration));
```

**Currently registered:** Telegram, MAX, Webhook

### 3. Add to catalog in `integrations-core/src/lib.rs`

Add a new `IntegrationMeta` entry to `builtin_catalog()`. The dashboard automatically renders cards from the catalog API.

### 4. Done! The integration appears in the marketplace automatically.

---

## Database Schema

```sql
CREATE TABLE integrations (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    account_id TEXT NOT NULL,
    org_id TEXT,
    name TEXT NOT NULL DEFAULT '',
    config JSONB NOT NULL DEFAULT '{}',
    oauth_tokens JSONB,
    subscribed_events JSONB NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'installed',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**Status values:** `pending_auth`, `installed`, `configured`, `active`, `paused`, `token_expired`, `error`, `uninstalled`

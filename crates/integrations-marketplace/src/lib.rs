//! FlowLink Integrations Marketplace — API and database layer.
//!
//! Provides:
//! - REST API for users to browse, install, configure integrations
//! - OAuth2 callback handling
//! - DB persistence for integration configs + OAuth tokens
//! - Per-user and per-org integration management with RBAC
//! - Hot-pluggable: install/uninstall while relay is running

use std::sync::Arc;
use axum::{
    extract::{Path, Query},
    response::IntoResponse,
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use flowlink_integrations_core::*;

// ═══════════════════════════════════════════════
// Marketplace State
// ═══════════════════════════════════════════════

pub struct MarketplaceState {
    pub db: Option<Arc<flowlink_db::DbPool>>,
    pub manager: Arc<tokio::sync::Mutex<IntegrationManager>>,
}

// ═══════════════════════════════════════════════
// API Types
// ═══════════════════════════════════════════════

#[derive(Deserialize)]
pub struct InstallRequest {
    pub kind: String,
    pub name: Option<String>,
    pub config: serde_json::Value,
    pub subscribed_events: Vec<String>,
    pub org_id: Option<String>,
}

#[derive(Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: String,
}

#[derive(Deserialize)]
pub struct BeginOAuthRequest {
    pub kind: String,
    pub org_id: Option<String>,
    pub name: Option<String>,
    pub redirect_after: Option<String>,
}

#[derive(Serialize)]
pub struct BeginOAuthResponse {
    pub authorize_url: String,
    pub integration_id: String,
}

#[derive(Serialize)]
pub struct IntegrationResponse {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub status: String,
    pub config: serde_json::Value,
    pub subscribed_events: Vec<String>,
    pub org_id: Option<String>,
    pub requires_oauth: bool,
    pub has_tokens: bool,
    pub created_at: String,
}

// ═══════════════════════════════════════════════
// DB Operations
// ═══════════════════════════════════════════════

pub async fn ensure_table(pool: &flowlink_db::DbPool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS integrations (
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
        CREATE INDEX IF NOT EXISTS idx_integrations_account ON integrations(account_id);
        CREATE INDEX IF NOT EXISTS idx_integrations_org ON integrations(org_id) WHERE org_id IS NOT NULL;
        CREATE INDEX IF NOT EXISTS idx_integrations_status ON integrations(status);
        "#,
    )
    .execute(pool.pool())
    .await?;
    Ok(())
}

pub async fn save_integration(pool: &flowlink_db::DbPool, config: &IntegrationConfig) -> anyhow::Result<()> {
    let oauth_json = config.oauth_tokens.as_ref()
        .map(|t| serde_json::to_value(t))
        .transpose()?
        .unwrap_or(serde_json::Value::Null);

    sqlx::query(
        r#"
        INSERT INTO integrations (id, kind, account_id, org_id, name, config, oauth_tokens, subscribed_events, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (id) DO UPDATE SET
            config = EXCLUDED.config,
            oauth_tokens = EXCLUDED.oauth_tokens,
            subscribed_events = EXCLUDED.subscribed_events,
            status = EXCLUDED.status,
            name = EXCLUDED.name,
            org_id = EXCLUDED.org_id,
            updated_at = NOW()
        "#,
    )
    .bind(&config.id)
    .bind(&config.kind.0)
    .bind(&config.account_id)
    .bind(&config.org_id)
    .bind(&config.name)
    .bind(serde_json::to_value(&config.config)?)
    .bind(oauth_json)
    .bind(serde_json::to_value(&config.subscribed_events)?)
    .bind(serde_json::to_string(&config.status)?)
    .execute(pool.pool())
    .await?;
    Ok(())
}

/// Update only OAuth tokens for an integration
pub async fn update_oauth_tokens(pool: &flowlink_db::DbPool, id: &str, tokens: &OAuthTokens) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE integrations SET oauth_tokens = $2, status = 'active', updated_at = NOW() WHERE id = $1"
    )
    .bind(id)
    .bind(serde_json::to_value(tokens)?)
    .execute(pool.pool())
    .await?;
    Ok(())
}

pub async fn load_active_integrations(pool: &flowlink_db::DbPool) -> anyhow::Result<Vec<IntegrationConfig>> {
    let rows = sqlx::query_as::<_, (
        String, String, String, Option<String>, String,
        serde_json::Value, Option<serde_json::Value>,
        serde_json::Value, String,
        chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>
    )>(
        "SELECT id, kind, account_id, org_id, name, config, oauth_tokens, subscribed_events, status, created_at, updated_at FROM integrations WHERE status IN ('active', 'configured', 'paused', 'pending_auth') ORDER BY created_at",
    )
    .fetch_all(pool.pool())
    .await?;

    Ok(rows.into_iter().map(|(id, kind, account_id, org_id, name, config, oauth_tokens, events, status, created_at, updated_at)| {
        IntegrationConfig {
            id,
            kind: IntegrationKind(kind),
            account_id,
            org_id,
            name,
            config,
            oauth_tokens: oauth_tokens.and_then(|v| serde_json::from_value(v).ok()),
            subscribed_events: serde_json::from_value(events).unwrap_or_default(),
            status: match status.as_str() {
                "pending_auth" => IntegrationStatus::PendingAuth,
                "installed" => IntegrationStatus::Installed,
                "configured" => IntegrationStatus::Configured,
                "active" => IntegrationStatus::Active,
                "paused" => IntegrationStatus::Paused,
                "token_expired" => IntegrationStatus::TokenExpired,
                s => IntegrationStatus::Error(s.to_string()),
            },
            created_at,
            updated_at,
        }
    }).collect())
}

pub async fn list_integrations_for_account(
    pool: &flowlink_db::DbPool,
    account_id: &str,
) -> anyhow::Result<Vec<IntegrationConfig>> {
    let rows = sqlx::query_as::<_, (
        String, String, String, Option<String>, String,
        serde_json::Value, Option<serde_json::Value>,
        serde_json::Value, String,
        chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>
    )>(
        r#"
        SELECT i.id, i.kind, i.account_id, i.org_id, i.name, i.config, i.oauth_tokens, i.subscribed_events, i.status, i.created_at, i.updated_at
        FROM integrations i
        WHERE i.status != 'uninstalled'
          AND (
            i.account_id = $1
            OR EXISTS (SELECT 1 FROM org_members om WHERE om.org_id::text = i.org_id AND om.account_id = $1)
          )
        ORDER BY i.created_at DESC
        "#,
    )
    .bind(account_id)
    .fetch_all(pool.pool())
    .await?;

    Ok(rows.into_iter().map(|(id, kind, account_id, org_id, name, config, oauth_tokens, events, status, created_at, updated_at)| {
        IntegrationConfig {
            id, kind: IntegrationKind(kind), account_id, org_id, name, config,
            oauth_tokens: oauth_tokens.and_then(|v| serde_json::from_value(v).ok()),
            subscribed_events: serde_json::from_value(events).unwrap_or_default(),
            status: match status.as_str() {
                "pending_auth" => IntegrationStatus::PendingAuth,
                "installed" => IntegrationStatus::Installed,
                "configured" => IntegrationStatus::Configured,
                "active" => IntegrationStatus::Active,
                "paused" => IntegrationStatus::Paused,
                "token_expired" => IntegrationStatus::TokenExpired,
                s => IntegrationStatus::Error(s.to_string()),
            },
            created_at, updated_at,
        }
    }).collect())
}

pub async fn delete_integration(pool: &flowlink_db::DbPool, id: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE integrations SET status = 'uninstalled', updated_at = NOW() WHERE id = $1")
        .bind(id).execute(pool.pool()).await?;
    Ok(())
}

pub async fn can_manage_org_integrations(pool: &flowlink_db::DbPool, account_id: &str, org_id: &str) -> bool {
    if let Ok(Some(member)) = flowlink_db::orgs::OrgRepo::get_member(pool.pool(), org_id.parse().unwrap_or_default(), account_id).await {
        matches!(member.role.as_str(), "owner" | "admin")
    } else {
        false
    }
}

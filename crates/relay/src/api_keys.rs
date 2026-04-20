// API Key management — CRUD + validation for MCP and API auth
// Keys are prefixed with `flk_` for identification, stored as SHA-256 hashes.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

// ═══════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiKeyRole {
    Admin,
    Operator,
    Viewer,
}

impl ApiKeyRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Operator => "operator",
            Self::Viewer => "viewer",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(Self::Admin),
            "operator" => Some(Self::Operator),
            "viewer" => Some(Self::Viewer),
            _ => None,
        }
    }

    /// Check if this role is allowed to call a given MCP tool.
    pub fn can_call(&self, tool: &str) -> bool {
        match self {
            Self::Admin => true, // Full access
            Self::Operator => !matches!(
                tool,
                "flowlink_kill" | "flowlink_deregister" | "flowlink_config_update"
            ),
            Self::Viewer => matches!(
                tool,
                "flowlink_agents" | "flowlink_health" | "flowlink_read" | "flowlink_list" | "flowlink_sysinfo"
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyInfo {
    pub id: Uuid,
    pub org_id: Uuid,
    pub account_id: String,
    pub key_prefix: String,
    pub name: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyWithSecret {
    pub info: ApiKeyInfo,
    /// The full secret key — only returned once at creation time.
    pub secret: String,
}

/// Identity resolved from a validated API key — attached to request context.
#[derive(Debug, Clone)]
pub struct KeyIdentity {
    pub key_id: Uuid,
    pub org_id: Uuid,
    pub account_id: String,
    pub role: ApiKeyRole,
    pub name: String,
}

// ═══════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════

/// Generate a new API key: `flk_` + 32 random hex chars.
pub fn generate_key() -> String {
    let bytes: [u8; 32] = rand::random();
    format!("flk_{}", hex::encode(bytes))
}

/// Hash a key with SHA-256 for storage.
pub fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

/// Extract the prefix (first 12 chars) for display.
pub fn key_prefix(key: &str) -> String {
    key.chars().take(12).collect()
}

// ═══════════════════════════════════════════════
// Repository
// ═══════════════════════════════════════════════

pub struct ApiKeyRepo;

impl ApiKeyRepo {
    /// Create a new API key. Returns the info + the secret (shown once).
    pub async fn create(
        db: &PgPool,
        org_id: Uuid,
        account_id: &str,
        name: &str,
        role: &ApiKeyRole,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ApiKeyWithSecret> {
        let secret = generate_key();
        let hash = hash_key(&secret);
        let prefix = key_prefix(&secret);

        let row = sqlx::query_as::<_, (Uuid, DateTime<Utc>)>(
            "INSERT INTO api_keys (org_id, account_id, key_hash, key_prefix, name, role, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id, created_at",
        )
        .bind(org_id)
        .bind(account_id)
        .bind(&hash)
        .bind(&prefix)
        .bind(name)
        .bind(role.as_str())
        .bind(expires_at)
        .fetch_one(db)
        .await?;

        Ok(ApiKeyWithSecret {
            info: ApiKeyInfo {
                id: row.0,
                org_id,
                account_id: account_id.to_string(),
                key_prefix: prefix,
                name: name.to_string(),
                role: role.as_str().to_string(),
                created_at: row.1,
                last_used: None,
                expires_at,
                active: true,
            },
            secret,
        })
    }

    /// Validate an API key by hash. Returns identity if valid.
    pub async fn validate(db: &PgPool, key: &str) -> Result<Option<KeyIdentity>> {
        if !key.starts_with("flk_") {
            return Ok(None);
        }

        let hash = hash_key(key);

        let row = sqlx::query_as::<_, (Uuid, Uuid, String, String, Option<DateTime<Utc>>, bool)>(
            "SELECT id, org_id, account_id, role, expires_at, active
             FROM api_keys WHERE key_hash = $1",
        )
        .bind(&hash)
        .fetch_optional(db)
        .await?;

        let (id, org_id, account_id, role_str, expires_at, active) = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        if !active {
            return Ok(None);
        }

        // Check expiration
        if let Some(exp) = expires_at {
            if exp < Utc::now() {
                return Ok(None);
            }
        }

        let role = ApiKeyRole::from_str(&role_str)
            .unwrap_or(ApiKeyRole::Viewer);

        // Update last_used (fire and forget)
        let db2 = db.clone();
        tokio::spawn(async move {
            let _ = sqlx::query("UPDATE api_keys SET last_used = NOW() WHERE id = $1")
                .bind(id)
                .execute(&db2)
                .await;
        });

        Ok(Some(KeyIdentity {
            key_id: id,
            org_id,
            account_id,
            role,
            name: String::new(), // not needed for auth context
        }))
    }

    /// List API keys for an organization. Admin sees all, others see only their own.
    pub async fn list_by_org(
        db: &PgPool,
        org_id: Uuid,
        caller_account_id: &str,
        caller_role: &ApiKeyRole,
    ) -> Result<Vec<ApiKeyInfo>> {
        let rows = if matches!(caller_role, ApiKeyRole::Admin) {
            sqlx::query_as::<_, (Uuid, Uuid, String, String, String, String, DateTime<Utc>, Option<DateTime<Utc>>, Option<DateTime<Utc>>, bool)>(
                "SELECT id, org_id, account_id, key_prefix, name, role, created_at, last_used, expires_at, active
                 FROM api_keys WHERE org_id = $1 ORDER BY created_at DESC",
            )
            .bind(org_id)
            .fetch_all(db)
            .await?
        } else {
            sqlx::query_as::<_, (Uuid, Uuid, String, String, String, String, DateTime<Utc>, Option<DateTime<Utc>>, Option<DateTime<Utc>>, bool)>(
                "SELECT id, org_id, account_id, key_prefix, name, role, created_at, last_used, expires_at, active
                 FROM api_keys WHERE org_id = $1 AND account_id = $2 ORDER BY created_at DESC",
            )
            .bind(org_id)
            .bind(caller_account_id)
            .fetch_all(db)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|(id, org_id, account_id, key_prefix, name, role, created_at, last_used, expires_at, active)| {
                ApiKeyInfo {
                    id,
                    org_id,
                    account_id,
                    key_prefix,
                    name,
                    role,
                    created_at,
                    last_used,
                    expires_at,
                    active,
                }
            })
            .collect())
    }

    /// Revoke (deactivate) an API key. Only admin or key owner can do this.
    pub async fn revoke(
        db: &PgPool,
        key_id: Uuid,
        caller_account_id: &str,
        caller_is_admin: bool,
    ) -> Result<bool> {
        let result = if caller_is_admin {
            sqlx::query("UPDATE api_keys SET active = false WHERE id = $1 AND active = true")
                .bind(key_id)
                .execute(db)
                .await?
        } else {
            sqlx::query(
                "UPDATE api_keys SET active = false WHERE id = $1 AND account_id = $2 AND active = true",
            )
            .bind(key_id)
            .bind(caller_account_id)
            .execute(db)
            .await?
        };

        Ok(result.rows_affected() > 0)
    }

    /// Delete an API key permanently. Admin only.
    pub async fn delete(db: &PgPool, key_id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM api_keys WHERE id = $1")
            .bind(key_id)
            .execute(db)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

// API Key management — CRUD + validation for MCP and API auth
// Industry-standard pattern: fine-grained scopes, role inheritance,
// last_used tracking, rotation support.
//
// Inspired by: GitHub PAT (fine-grained), Stripe restricted keys, AWS IAM access keys.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;
use tokio::sync::RwLock;
use std::collections::HashMap;

// ═══════════════════════════════════════════════
// Scopes (fine-grained permissions)
// ═══════════════════════════════════════════════

/// Individual permission scopes for API keys.
/// Following the pattern: `resource:action` (like GitHub's `repo:read`, `repo:write`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Scope {
    // Agent management
    #[serde(rename = "agents:read")]
    AgentsRead,
    #[serde(rename = "agents:write")]
    AgentsWrite,    // exec, write files
    #[serde(rename = "agents:admin")]
    AgentsAdmin,    // kill, deregister, config

    // Approvals
    #[serde(rename = "approvals:read")]
    ApprovalsRead,
    #[serde(rename = "approvals:write")]
    ApprovalsWrite, // approve/reject

    // Policy
    #[serde(rename = "policy:read")]
    PolicyRead,
    #[serde(rename = "policy:write")]
    PolicyWrite,

    // System
    #[serde(rename = "system:read")]
    SystemRead,     // health, sysinfo
    #[serde(rename = "system:write")]
    SystemWrite,    // config_update
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AgentsRead => "agents:read",
            Self::AgentsWrite => "agents:write",
            Self::AgentsAdmin => "agents:admin",
            Self::ApprovalsRead => "approvals:read",
            Self::ApprovalsWrite => "approvals:write",
            Self::PolicyRead => "policy:read",
            Self::PolicyWrite => "policy:write",
            Self::SystemRead => "system:read",
            Self::SystemWrite => "system:write",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "agents:read" => Some(Self::AgentsRead),
            "agents:write" => Some(Self::AgentsWrite),
            "agents:admin" => Some(Self::AgentsAdmin),
            "approvals:read" => Some(Self::ApprovalsRead),
            "approvals:write" => Some(Self::ApprovalsWrite),
            "policy:read" => Some(Self::PolicyRead),
            "policy:write" => Some(Self::PolicyWrite),
            "system:read" => Some(Self::SystemRead),
            "system:write" => Some(Self::SystemWrite),
            _ => None,
        }
    }

    /// All available scopes (for admin role).
    pub fn all() -> Vec<Scope> {
        vec![
            Self::AgentsRead, Self::AgentsWrite, Self::AgentsAdmin,
            Self::ApprovalsRead, Self::ApprovalsWrite,
            Self::PolicyRead, Self::PolicyWrite,
            Self::SystemRead, Self::SystemWrite,
        ]
    }

    /// Scopes for operator role (no agents:admin, no system:write, no policy:write).
    pub fn operator_scopes() -> Vec<Scope> {
        vec![
            Self::AgentsRead, Self::AgentsWrite,
            Self::ApprovalsRead, Self::ApprovalsWrite,
            Self::PolicyRead,
            Self::SystemRead,
        ]
    }

    /// Scopes for viewer role (read-only).
    pub fn viewer_scopes() -> Vec<Scope> {
        vec![
            Self::AgentsRead,
            Self::ApprovalsRead,
            Self::PolicyRead,
            Self::SystemRead,
        ]
    }

    /// Scopes granted to an org role.
    pub fn for_org_role(role: &str) -> Vec<Scope> {
        match role {
            "owner" | "admin" => Self::all(),
            "member" => Self::operator_scopes(),
            "viewer" => Self::viewer_scopes(),
            _ => Self::viewer_scopes(),
        }
    }

    /// Parse a comma-separated list of scopes.
    pub fn parse_list(s: &str) -> Vec<Scope> {
        s.split(',')
            .filter_map(|p| Self::from_str(p.trim()))
            .collect()
    }

    /// Serialize a list of scopes to comma-separated string.
    pub fn join(scopes: &[Scope]) -> String {
        scopes.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(",")
    }
}

/// Map MCP tool names to required scopes.
pub fn required_scopes(tool: &str) -> Vec<Scope> {
    match tool {
        "flowlink_agents" | "flowlink_health" | "flowlink_read" | "flowlink_list" | "flowlink_sysinfo" => {
            vec![Scope::AgentsRead]
        }
        "flowlink_exec" | "flowlink_write" => {
            vec![Scope::AgentsWrite]
        }
        "flowlink_kill" | "flowlink_deregister" => {
            vec![Scope::AgentsAdmin]
        }
        "flowlink_approve" => {
            vec![Scope::ApprovalsWrite]
        }
        "flowlink_policy" => {
            vec![Scope::PolicyRead]
        }
        "flowlink_config_update" => {
            vec![Scope::SystemWrite]
        }
        _ => vec![Scope::AgentsRead], // default: at least read
    }
}

// ═══════════════════════════════════════════════
// Legacy role enum (kept for backward compat)
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

    /// Scopes granted by this role.
    pub fn scopes(&self) -> Vec<Scope> {
        match self {
            Self::Admin => Scope::all(),
            Self::Operator => Scope::operator_scopes(),
            Self::Viewer => Scope::viewer_scopes(),
        }
    }

    /// Check if this role can call a given MCP tool.
    pub fn can_call(&self, tool: &str) -> bool {
        let required = required_scopes(tool);
        let granted = self.scopes();
        required.iter().all(|r| granted.contains(r))
    }
}

// ═══════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyInfo {
    pub id: Uuid,
    pub org_id: Uuid,
    pub account_id: String,
    pub key_prefix: String,
    pub name: String,
    pub role: String,
    pub scopes: String,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyWithSecret {
    pub id: Uuid,
    pub key: String,
    pub key_prefix: String,
    pub name: String,
    pub role: String,
    pub scopes: String,
}

/// Identity resolved from a validated API key.
#[derive(Debug, Clone)]
pub struct KeyIdentity {
    pub key_id: Uuid,
    pub org_id: Uuid,
    pub account_id: String,
    pub role: ApiKeyRole,
    pub scopes: Vec<Scope>,
    pub name: String,
}

impl KeyIdentity {
    /// Check if this identity has the required scopes for a tool.
    pub fn can_call(&self, tool: &str) -> bool {
        let required = required_scopes(tool);
        required.iter().all(|r| self.scopes.contains(r))
    }
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
    /// Create a new API key.
    /// `scopes` overrides role-based defaults if provided (fine-grained).
    /// Scopes are capped to caller's org permissions (no privilege escalation).
    pub async fn create(
        db: &PgPool,
        org_id: Uuid,
        account_id: &str,
        name: &str,
        role: &ApiKeyRole,
        custom_scopes: Option<&[Scope]>,
        caller_max_scopes: &[Scope], // caller's org-granted scopes (ceiling)
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ApiKeyWithSecret> {
        let secret = generate_key();
        let hash = hash_key(&secret);
        let prefix = key_prefix(&secret);

        // Determine final scopes: custom (capped) or role defaults (capped)
        let final_scopes = if let Some(custom) = custom_scopes {
            // Cap custom scopes to caller's permissions
            custom.iter()
                .filter(|s| caller_max_scopes.contains(s))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            // Role defaults, capped
            role.scopes().into_iter()
                .filter(|s| caller_max_scopes.contains(s))
                .collect::<Vec<_>>()
        };

        let scopes_str = Scope::join(&final_scopes);

        let row = sqlx::query_as::<_, (Uuid, DateTime<Utc>)>(
            "INSERT INTO api_keys (org_id, account_id, key_hash, key_prefix, name, role, scopes, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id, created_at",
        )
        .bind(org_id)
        .bind(account_id)
        .bind(&hash)
        .bind(&prefix)
        .bind(name)
        .bind(role.as_str())
        .bind(&scopes_str)
        .bind(expires_at)
        .fetch_one(db)
        .await?;

        Ok(ApiKeyWithSecret {
            id: row.0,
            key: secret,
            key_prefix: prefix,
            name: name.to_string(),
            role: role.as_str().to_string(),
            scopes: scopes_str,
        })
    }

    /// Validate an API key by hash. Returns identity with scopes.
    pub async fn validate(db: &PgPool, key: &str) -> Result<Option<KeyIdentity>> {
        if !key.starts_with("flk_") {
            return Ok(None);
        }

        let hash = hash_key(key);

        let row = sqlx::query_as::<_, (Uuid, Uuid, String, String, String, Option<DateTime<Utc>>, bool)>(
            "SELECT id, org_id, account_id, role, scopes, expires_at, active
             FROM api_keys WHERE key_hash = $1",
        )
        .bind(&hash)
        .fetch_optional(db)
        .await?;

        let (id, org_id, account_id, role_str, scopes_str, expires_at, active) = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        if !active {
            return Ok(None);
        }

        if let Some(exp) = expires_at {
            if exp < Utc::now() {
                return Ok(None);
            }
        }

        let role = ApiKeyRole::from_str(&role_str).unwrap_or(ApiKeyRole::Viewer);
        let scopes = Scope::parse_list(&scopes_str);
        // If scopes empty, fall back to role defaults
        let scopes = if scopes.is_empty() { role.scopes() } else { scopes };

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
            scopes,
            name: String::new(),
        }))
    }

    /// List API keys for an organization.
    /// Admin/owner sees all org keys. Others see only their own.
    pub async fn list_by_org(
        db: &PgPool,
        org_id: Uuid,
        caller_account_id: &str,
        caller_org_role: &str,
    ) -> Result<Vec<ApiKeyInfo>> {
        let rows = if caller_org_role == "owner" || caller_org_role == "admin" {
            sqlx::query_as::<_, (Uuid, Uuid, String, String, String, String, String, DateTime<Utc>, Option<DateTime<Utc>>, Option<DateTime<Utc>>, bool)>(
                "SELECT id, org_id, account_id, key_prefix, name, role, scopes, created_at, last_used, expires_at, active
                 FROM api_keys WHERE org_id = $1 ORDER BY created_at DESC",
            )
            .bind(org_id)
            .fetch_all(db)
            .await?
        } else {
            sqlx::query_as::<_, (Uuid, Uuid, String, String, String, String, String, DateTime<Utc>, Option<DateTime<Utc>>, Option<DateTime<Utc>>, bool)>(
                "SELECT id, org_id, account_id, key_prefix, name, role, scopes, created_at, last_used, expires_at, active
                 FROM api_keys WHERE org_id = $1 AND account_id = $2 ORDER BY created_at DESC",
            )
            .bind(org_id)
            .bind(caller_account_id)
            .fetch_all(db)
            .await?
        };

        Ok(rows.into_iter().map(|(id, org_id, account_id, key_prefix, name, role, scopes, created_at, last_used, expires_at, active)| {
            ApiKeyInfo { id, org_id, account_id, key_prefix, name, role, scopes, created_at, last_used, expires_at, active }
        }).collect())
    }

    /// Revoke (deactivate) an API key. Admin/owner can revoke any, others only own.
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

    /// Permanently delete an API key. Admin only.
    pub async fn delete(db: &PgPool, key_id: Uuid) -> Result<bool> {
        let result = sqlx::query("DELETE FROM api_keys WHERE id = $1")
            .bind(key_id)
            .execute(db)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Rotate: revoke old key and create a new one with same permissions.
    /// Returns the new key's secret (shown once).
    pub async fn rotate(
        db: &PgPool,
        old_key_id: Uuid,
        caller_account_id: &str,
        caller_is_admin: bool,
        caller_max_scopes: &[Scope],
    ) -> Result<Option<ApiKeyWithSecret>> {
        // Fetch old key info
        let row = sqlx::query_as::<_, (Uuid, String, String, String, String, Option<DateTime<Utc>>)>(
            "SELECT org_id, account_id, name, role, scopes, expires_at FROM api_keys WHERE id = $1",
        )
        .bind(old_key_id)
        .fetch_optional(db)
        .await?;

        let (org_id, owner_id, name, role_str, scopes_str, expires_at) = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        // Only admin or key owner can rotate
        if !caller_is_admin && owner_id != caller_account_id {
            return Ok(None);
        }

        // Create new key with same permissions
        let role = ApiKeyRole::from_str(&role_str).unwrap_or(ApiKeyRole::Viewer);
        let old_scopes = Scope::parse_list(&scopes_str);
        let new_key = Self::create(
            db, org_id, caller_account_id,
            &format!("{} (rotated)", name),
            &role,
            Some(&old_scopes),
            caller_max_scopes,
            expires_at,
        ).await?;

        // Revoke old key
        sqlx::query("UPDATE api_keys SET active = false WHERE id = $1")
            .bind(old_key_id)
            .execute(db)
            .await?;

        Ok(Some(new_key))
    }
}

// ═══════════════════════════════════════════════
// Per-key Rate Limiter (sliding window)
// ═══════════════════════════════════════════════

#[derive(Default)]
struct KeyBucket {
    count: u64,
    window_start: Option<std::time::Instant>,
}

pub struct KeyRateLimiter {
    buckets: RwLock<HashMap<String, KeyBucket>>,
    max_requests: u64,
    window_secs: u64,
}

impl KeyRateLimiter {
    pub fn new(max_requests: u64, window_secs: u64) -> Self {
        Self {
            buckets: RwLock::new(HashMap::new()),
            max_requests,
            window_secs,
        }
    }

    /// Returns true if the request is allowed, false if rate limited
    pub async fn check(&self, key_id: &str) -> bool {
        let now = std::time::Instant::now();
        let window_dur = std::time::Duration::from_secs(self.window_secs);
        let mut buckets = self.buckets.write().await;

        let bucket = buckets.entry(key_id.to_string()).or_insert_with(KeyBucket::default);

        // Reset window if expired
        let window_start = bucket.window_start.get_or_insert(std::time::Instant::now());
        if now.duration_since(*window_start) > window_dur {
            bucket.count = 0;
            bucket.window_start = Some(now);
        }

        bucket.count += 1;
        bucket.count <= self.max_requests
    }

    /// Get current usage for a key
    pub async fn usage(&self, key_id: &str) -> (u64, u64) {
        let buckets = self.buckets.read().await;
        if let Some(bucket) = buckets.get(key_id) {
            (bucket.count, self.max_requests)
        } else {
            (0, self.max_requests)
        }
    }

    /// Clean up stale buckets (call periodically)
    pub async fn cleanup(&self) {
        let now = std::time::Instant::now();
        let window_dur = std::time::Duration::from_secs(self.window_secs);
        let mut buckets = self.buckets.write().await;
        buckets.retain(|_, b| b.window_start.is_some_and(|ws| now.duration_since(ws) < window_dur));
    }

    /// Check with a per-plan limit override. If plan_limit is 0, uses default.
    pub async fn check_plan(&self, key_id: &str, plan_limit: u32) -> bool {
        let effective_limit = if plan_limit > 0 { plan_limit as u64 } else { self.max_requests };
        let now = std::time::Instant::now();
        let window_dur = std::time::Duration::from_secs(self.window_secs);
        let mut buckets = self.buckets.write().await;
        let bucket = buckets.entry(key_id.to_string()).or_insert_with(KeyBucket::default);
        let window_start = bucket.window_start.get_or_insert(now);
        if now.duration_since(*window_start) > window_dur {
            bucket.count = 0;
            bucket.window_start = Some(now);
        }
        bucket.count += 1;
        bucket.count <= effective_limit
    }
}

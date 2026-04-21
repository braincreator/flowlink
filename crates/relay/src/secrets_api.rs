use sqlx::Row;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};

use crate::server::AppState;
use flowlink_core::rbac::Permission;

fn gp(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, String)> {
    state.db.as_ref().map(|db| db.pool()).ok_or((StatusCode::SERVICE_UNAVAILABLE, "Database not configured".to_string()))
}

#[derive(Debug, Serialize)]
pub struct SecretEntry {
    pub id: Uuid,
    pub org_id: Uuid,
    pub key: String,
    pub description: String,
    pub tags: Vec<String>,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
    // Note: encrypted_value and nonce are NEVER returned via API
}

#[derive(Debug, Deserialize)]
pub struct CreateSecretRequest {
    pub key: String,
    pub value: String, // plaintext — will be encrypted server-side
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSecretRequest {
    pub value: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct SecretQuery {
    pub org_id: Option<Uuid>,
    pub tag: Option<String>,
    pub prefix: Option<String>,
}

/// Get encryption key from ENV or fallback to a config-derived key
fn hex_str_to_bytes(s: &str) -> Result<Vec<u8>, String> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i+2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn get_encryption_key() -> Result<[u8; 32], String> {
    // Production: must set FLOWLINK_SECRETS_KEY env var (hex-encoded 32 bytes)
    if let Ok(key_hex) = std::env::var("FLOWLINK_SECRETS_KEY") {
        if key_hex.len() == 64 {
            if let Ok(bytes) = hex_str_to_bytes(&key_hex) {
                if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    return Ok(key);
                }
            }
        }
        return Err("FLOWLINK_SECRETS_KEY must be 64 hex chars (32 bytes)".into());
    }
    // Dev-only fallback with warning
    log::warn!("⚠️  FLOWLINK_SECRETS_KEY not set — using dev fallback. DO NOT use in production!");
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    // Include machine-specific salt to make fallback less predictable
    hasher.update(b"flowlink-secrets-dev-key");
    hasher.update(std::env::var("HOSTNAME").unwrap_or_default().as_bytes());
    hasher.update(std::env::var("USER").unwrap_or_default().as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    Ok(key)
}

fn encrypt(plaintext: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
    let key = get_encryption_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Cipher init: {}", e))?;
    let nonce_bytes: [u8; 12] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let encrypted = cipher.encrypt(nonce, plaintext.as_bytes()).map_err(|e| format!("Encrypt: {}", e))?;
    Ok((encrypted, nonce_bytes.to_vec()))
}

fn decrypt(encrypted: &[u8], nonce: &[u8]) -> Result<String, String> {
    let key = get_encryption_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Cipher init: {}", e))?;
    let nonce = Nonce::from_slice(nonce);
    let decrypted = cipher.decrypt(nonce, encrypted).map_err(|e| format!("Decrypt: {}", e))?;
    String::from_utf8(decrypted).map_err(|e| e.to_string())
}

pub async fn list_secrets(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Query(q): Query<SecretQuery>,
) -> Result<(StatusCode, Json<Vec<SecretEntry>>), (StatusCode, String)> {
    // RBAC check
    if let Err(e) = state.rbac.check_permission(&claims.sub, &Permission::SecretsRead) {
        // Fallback: admins always allowed in dev mode
        if !claims.is_admin {
            return Err((StatusCode::FORBIDDEN, format!("Permission denied: {e}")));
        }
    }
    let pool = gp(&state)?;
    // Org-scope: filter by user's org from claims
    let org_filter: Option<Uuid> = match &claims.org_id {
        Some(id) => Uuid::parse_str(id).ok(),
        None => q.org_id, // fallback to query param if no org in claims
    };
    let rows = sqlx::query(
        "SELECT id, org_id, key, description, tags, created_by, created_at::text, updated_at::text FROM secrets WHERE ($1::uuid IS NULL OR org_id = $1) ORDER BY key"
    ).bind(org_filter).fetch_all(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let entries: Vec<SecretEntry> = rows.iter().map(|r| SecretEntry {
        id: r.get("id"), org_id: r.get("org_id"), key: r.get("key"),
        description: r.get::<Option<String>, _>("description").unwrap_or_default(),
        tags: r.get::<Vec<String>, _>("tags"), created_by: r.get("created_by"),
        created_at: r.get("created_at"), updated_at: r.get("updated_at"),
    }).collect();
    Ok((StatusCode::OK, Json(entries)))
}

pub async fn create_secret(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Json(body): Json<CreateSecretRequest>,
) -> Result<(StatusCode, Json<SecretEntry>), (StatusCode, String)> {
    // RBAC check
    if let Err(e) = state.rbac.check_permission(&claims.sub, &Permission::SecretsWrite) {
        if !claims.is_admin { return Err((StatusCode::FORBIDDEN, format!("Permission denied: {e}"))); }
    }
    let pool = gp(&state)?;
    if body.key.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Key is required".into()));
    }
    if body.key.len() > 128 {
        return Err((StatusCode::BAD_REQUEST, "Key too long (max 128 chars)".into()));
    }
    if !body.key.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.') {
        return Err((StatusCode::BAD_REQUEST, "Key must contain only alphanumeric, underscore, hyphen, dot".into()));
    }
    if body.value.len() > 65536 {
        return Err((StatusCode::BAD_REQUEST, "Value too large (max 64KB)".into()));
    }
    let (encrypted, nonce) = encrypt(&body.value).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let org_id: uuid::Uuid = match &claims.org_id {
        Some(id) => uuid::Uuid::parse_str(id).unwrap_or_default(),
        None => return Err((StatusCode::FORBIDDEN, "No organization selected".into())),
    };

    let row = sqlx::query(
        "INSERT INTO secrets (org_id, key, encrypted_value, nonce, description, tags, created_by) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id, org_id, key, description, tags, created_by, created_at::text, updated_at::text"
    ).bind(org_id).bind(body.key.trim()).bind(&encrypted).bind(&nonce).bind(&body.description).bind(&body.tags).bind(&claims.account_id)
    .fetch_one(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(SecretEntry {
        id: row.get("id"), org_id: row.get("org_id"), key: row.get("key"),
        description: row.get::<Option<String>, _>("description").unwrap_or_default(),
        tags: row.get::<Vec<String>, _>("tags"), created_by: row.get("created_by"),
        created_at: row.get("created_at"), updated_at: row.get("updated_at"),
    })))
}

#[derive(Debug, Serialize)]
pub struct SecretWithValue {
    pub id: Uuid,
    pub key: String,
    pub value: String,
}

pub async fn get_secret_value(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<SecretWithValue>), (StatusCode, String)> {
    // RBAC check
    if let Err(e) = state.rbac.check_permission(&claims.sub, &Permission::SecretsRead) {
        if !claims.is_admin { return Err((StatusCode::FORBIDDEN, format!("Permission denied: {e}"))); }
    }
    let pool = gp(&state)?;
    // Org-scope: only allow access to secrets in user's org
    let org_id = match &claims.org_id {
        Some(id) => Uuid::parse_str(id).ok(),
        None => None,
    };
    let query = if org_id.is_some() {
        "SELECT id, key, encrypted_value, nonce FROM secrets WHERE id = $1 AND org_id = $2"
    } else {
        "SELECT id, key, encrypted_value, nonce FROM secrets WHERE id = $1"
    };
    let q = sqlx::query(query).bind(id);
    let q = if let Some(oid) = org_id { q.bind(oid) } else { q };
    let row = q.fetch_optional(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match row {
        Some(r) => {
            let enc: Vec<u8> = r.get("encrypted_value");
            let nonce: Vec<u8> = r.get("nonce");
            let value = decrypt(&enc, &nonce).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            Ok((StatusCode::OK, Json(SecretWithValue {
                id: r.get("id"), key: r.get("key"), value,
            })))
        }
        None => Err((StatusCode::NOT_FOUND, "Secret not found".into())),
    }
}

pub async fn update_secret(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateSecretRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    // RBAC check
    if let Err(e) = state.rbac.check_permission(&claims.sub, &Permission::SecretsWrite) {
        if !claims.is_admin { return Err((StatusCode::FORBIDDEN, format!("Permission denied: {e}"))); }
    }
    let pool = gp(&state)?;

    if let Some(ref value) = body.value {
        let (encrypted, nonce) = encrypt(value).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        sqlx::query("UPDATE secrets SET encrypted_value = $2, nonce = $3, updated_at = NOW() WHERE id = $1")
            .bind(id).bind(&encrypted).bind(&nonce).execute(pool).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(ref desc) = body.description {
        sqlx::query("UPDATE secrets SET description = $2, updated_at = NOW() WHERE id = $1")
            .bind(id).bind(desc).execute(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(ref tags) = body.tags {
        sqlx::query("UPDATE secrets SET tags = $2, updated_at = NOW() WHERE id = $1")
            .bind(id).bind(tags).execute(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    Ok(StatusCode::OK)
}

pub async fn delete_secret(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    // RBAC check
    if let Err(e) = state.rbac.check_permission(&claims.sub, &Permission::SecretsDelete) {
        if !claims.is_admin { return Err((StatusCode::FORBIDDEN, format!("Permission denied: {e}"))); }
    }
    let pool = gp(&state)?;
    let result = sqlx::query("DELETE FROM secrets WHERE id = $1").bind(id)
        .execute(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if result.rows_affected() == 0 { return Err((StatusCode::NOT_FOUND, "Secret not found".into())); }
    Ok(StatusCode::NO_CONTENT)
}

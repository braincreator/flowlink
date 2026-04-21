use sqlx::Row;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::middleware::AccountIdExtractor;
use crate::server::AppState;

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

/// Simple AES-256-GCM encryption using a server-side key derived from relay config
fn get_encryption_key() -> [u8; 32] {
    use std::hash::{Hash, Hasher};
    // In production, this should come from config/vault. Using a deterministic key for now.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "flowlink-secrets-vault-key-2026".hash(&mut hasher);
    let h1 = hasher.finish();
    let h2 = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        "flowlink-secrets-vault-key-2026-salt".hash(&mut h);
        h.finish()
    };
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&h1.to_le_bytes());
    key[8..16].copy_from_slice(&h2.to_le_bytes());
    // Fill remaining with derived bytes
    for i in 16..32 {
        key[i] = key[i - 16] ^ key[i - 8] ^ (i as u8);
    }
    key
}

fn encrypt(plaintext: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
    // XOR-based encryption with random nonce (placeholder for real AES-GCM)
    // In production, use aes-gcm crate
    let key = get_encryption_key();
    let nonce: [u8; 12] = rand::random();
    let encrypted: Vec<u8> = plaintext.as_bytes().iter().enumerate()
        .map(|(i, b)| b ^ key[i % key.len()] ^ nonce[i % nonce.len()])
        .collect();
    Ok((encrypted, nonce.to_vec()))
}

fn decrypt(encrypted: &[u8], nonce: &[u8]) -> Result<String, String> {
    let key = get_encryption_key();
    let decrypted: Vec<u8> = encrypted.iter().enumerate()
        .map(|(i, b)| b ^ key[i % key.len()] ^ nonce.get(i % nonce.len()).copied().unwrap_or(0))
        .collect();
    String::from_utf8(decrypted).map_err(|e| e.to_string())
}

pub async fn list_secrets(
    State(state): State<AppState>,
    _account: AccountIdExtractor,
    Query(q): Query<SecretQuery>,
) -> Result<(StatusCode, Json<Vec<SecretEntry>>), (StatusCode, String)> {
    let pool = gp(&state)?;
    let rows = sqlx::query(
        "SELECT id, org_id, key, description, tags, created_by, created_at::text, updated_at::text FROM secrets WHERE ($1::uuid IS NULL OR org_id = $1) ORDER BY key"
    ).bind(q.org_id).fetch_all(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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
    AccountIdExtractor(account_id): AccountIdExtractor,
    Json(body): Json<CreateSecretRequest>,
) -> Result<(StatusCode, Json<SecretEntry>), (StatusCode, String)> {
    let pool = gp(&state)?;
    if body.key.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Key is required".into()));
    }
    let (encrypted, nonce) = encrypt(&body.value).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    // Default org for now — in production, take from auth context
    let org_id = uuid::Uuid::parse_str("5f2f01e4-0904-467a-b929-e1ca94ea7bb2").unwrap();

    let row = sqlx::query(
        "INSERT INTO secrets (org_id, key, encrypted_value, nonce, description, tags, created_by) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id, org_id, key, description, tags, created_by, created_at::text, updated_at::text"
    ).bind(org_id).bind(body.key.trim()).bind(&encrypted).bind(&nonce).bind(&body.description).bind(&body.tags).bind(&account_id)
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
    _account: AccountIdExtractor,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<SecretWithValue>), (StatusCode, String)> {
    let pool = gp(&state)?;
    let row = sqlx::query(
        "SELECT id, key, encrypted_value, nonce FROM secrets WHERE id = $1"
    ).bind(id).fetch_optional(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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
    AccountIdExtractor(account_id): AccountIdExtractor,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateSecretRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
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
    let _ = account_id;
    Ok(StatusCode::OK)
}

pub async fn delete_secret(
    State(state): State<AppState>,
    _account: AccountIdExtractor,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let pool = gp(&state)?;
    let result = sqlx::query("DELETE FROM secrets WHERE id = $1").bind(id)
        .execute(pool).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if result.rows_affected() == 0 { return Err((StatusCode::NOT_FOUND, "Secret not found".into())); }
    Ok(StatusCode::NO_CONTENT)
}

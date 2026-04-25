// Zero-Trust Secret Management API
// Endpoints for org key setup, external vault config, verification
// All endpoints require org owner/admin role

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::Claims;
use crate::server::AppState;
use crate::zero_trust_secrets::{
    ExternalVaultSetupRequest, OrgKeySetupRequest, OrgSecretConfig, VaultMode,
};

/// Verify org admin role — returns 403 if not owner/admin
async fn require_org_admin(
    pool: &sqlx::PgPool,
    org_id: Uuid,
    account_id: &str,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let role = sqlx::query_scalar::<_, String>(
        "SELECT role FROM org_members WHERE org_id = $1 AND account_id = $2"
    )
    .bind(org_id)
    .bind(account_id)
    .fetch_optional(pool)
    .await;

    match role {
        Ok(Some(r)) if r == "owner" || r == "admin" => Ok(r),
        Ok(Some(_)) => Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"ok": false, "error": "Only org owner/admin can manage secret configuration"})),
        )),
        Ok(None) => Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"ok": false, "error": "Not a member of this organization"})),
        )),
        Err(e) => {
            log::error!("DB error: {e}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"ok": false, "error": "Internal error"})),
            ))
        }
    }
}

fn get_pool(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, Json<serde_json::Value>)> {
    state.db.as_ref().map(|p| &p.write_pool).ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"ok": false, "error": "Database unavailable"})),
        )
    })
}

/// POST /api/orgs/{org_id}/secrets/config/key-setup
/// Set up or rotate the org's encryption public key.
/// The PRIVATE key never touches the relay — admin keeps it locally.
pub async fn setup_org_key(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<OrgKeySetupRequest>,
) -> impl IntoResponse {
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(r) => return r.into_response(),
    };

    if let Err(r) = require_org_admin(pool, org_id, &claims.account_id).await {
        return r.into_response();
    }

    // Validate public key format (should be base64-encoded X25519 public key = 32 bytes)
    let key_bytes = match base64::decode(&req.org_public_key) {
        Ok(bytes) if bytes.len() == 32 => bytes,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "ok": false,
                    "error": "Invalid public key format. Expected base64-encoded 32-byte X25519 public key."
                })),
            ).into_response();
        }
    };

    // Compute key ID (SHA-256 of public key)
    let key_id = {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(&key_bytes);
        let mut hex = String::with_capacity(16);
        for byte in hash.iter().take(8) {
            use std::fmt::Write;
            write!(&mut hex, "{byte:02x}").unwrap();
        }
        hex
    };

    // Upsert org secret config
    let result = sqlx::query(
        r#"INSERT INTO org_secret_configs (org_id, org_public_key, org_key_id, vault_mode, key_set_up_by, created_at, updated_at)
           VALUES ($1, $2, $3, 'none', $4, NOW(), NOW())
           ON CONFLICT (org_id) DO UPDATE SET
             org_public_key = $2,
             org_key_id = $3,
             key_set_up_by = $4,
             updated_at = NOW()"#
    )
    .bind(org_id)
    .bind(&req.org_public_key)
    .bind(&key_id)
    .bind(&claims.account_id)
    .execute(pool)
    .await;

    if let Err(e) = result {
        log::error!("Failed to save org key config: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": "Failed to save configuration"})),
        ).into_response();
    }

    log::info!(
        "🔑 Org key setup: org={} key_id={} by={} replacing={:?}",
        org_id, key_id, claims.account_id, req.replacing_key_id
    );

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "key_id": key_id,
            "message": "Organization encryption key configured. Private key should NEVER be uploaded to relay."
        })),
    ).into_response()
}

/// POST /api/orgs/{org_id}/secrets/config/vault-setup
/// Configure external Vault for this organization
pub async fn setup_external_vault(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ExternalVaultSetupRequest>,
) -> impl IntoResponse {
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(r) => return r.into_response(),
    };

    if let Err(r) = require_org_admin(pool, org_id, &claims.account_id).await {
        return r.into_response();
    }

    // Validate address
    if !req.address.starts_with("https://") && !req.address.starts_with("http://") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "ok": false,
                "error": "Vault address must start with https:// or http://"
            })),
        ).into_response();
    }

    let vault_config = VaultMode::External {
        address: req.address,
        auth: req.auth,
        mount_path: req.mount_path,
        mtls: req.mtls,
        ca_cert_pem: req.ca_cert_pem,
        response_wrapping: req.response_wrapping,
    };

    let vault_json = match serde_json::to_string(&vault_config) {
        Ok(j) => j,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"ok": false, "error": format!("Invalid vault config: {e}")})),
            ).into_response();
        }
    };

    // Update org secret config with vault settings
    let result = sqlx::query(
        "UPDATE org_secret_configs SET vault_mode = $2::text, vault_config = $3, updated_at = NOW() WHERE org_id = $1"
    )
    .bind(org_id)
    .bind("external")
    .bind(&vault_json)
    .execute(pool)
    .await;

    if let Err(e) = result {
        log::error!("Failed to save external vault config: {e}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": "Failed to save vault configuration"})),
        ).into_response();
    }

    log::info!("🏦 External Vault configured: org={} by={}", org_id, claims.account_id);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "message": "External HashiCorp Vault configured. Secrets will be stored in your own Vault."
        })),
    ).into_response()
}

/// GET /api/orgs/{org_id}/secrets/config
/// Get current zero-trust configuration for the org
pub async fn get_secret_config(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(r) => return r.into_response(),
    };

    if let Err(r) = require_org_admin(pool, org_id, &claims.account_id).await {
        return r.into_response();
    }

    let row = sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT org_public_key, org_key_id, vault_mode, vault_config FROM org_secret_configs WHERE org_id = $1"
    )
    .bind(org_id)
    .fetch_optional(pool)
    .await;

    let (public_key, key_id, vault_mode, vault_config) = match row {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "ok": true,
                    "configured": false,
                    "message": "No secret configuration found. Set up org encryption key first."
                })),
            ).into_response();
        }
        Err(e) => {
            log::error!("DB error: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"ok": false, "error": "Internal error"})),
            ).into_response();
        }
    };

    let vault: VaultMode = match (vault_mode.as_deref(), vault_config) {
        (Some("external"), Some(vc)) => serde_json::from_str(&vc).unwrap_or(VaultMode::None),
        (Some("embedded"), _) => VaultMode::Embedded {
            namespace: org_id.to_string(),
        },
        _ => VaultMode::None,
    };

    let config = OrgSecretConfig {
        org_id: org_id.to_string(),
        org_public_key: public_key.unwrap_or_default(),
        org_key_id: key_id.unwrap_or_default(),
        vault,
        key_rotated_at: None,
        key_set_up_by: None,
    };

    let verification = config.verify_zero_trust();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "configured": true,
            "org_key_id": config.org_key_id,
            "has_public_key": !config.org_public_key.is_empty(),
            "vault_mode": config.vault,
            "zero_trust_verification": verification,
        })),
    ).into_response()
}

/// DELETE /api/orgs/{org_id}/secrets/config/vault
/// Remove external vault configuration, switch back to embedded/none
pub async fn remove_external_vault(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> impl IntoResponse {
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(r) => return r.into_response(),
    };

    if let Err(r) = require_org_admin(pool, org_id, &claims.account_id).await {
        return r.into_response();
    }

    let _ = sqlx::query(
        "UPDATE org_secret_configs SET vault_mode = 'none', vault_config = NULL, updated_at = NOW() WHERE org_id = $1"
    )
    .bind(org_id)
    .execute(pool)
    .await;

    log::info!("🏦 External Vault removed: org={} by={}", org_id, claims.account_id);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "message": "External Vault configuration removed. Secrets will use embedded storage."
        })),
    ).into_response()
}

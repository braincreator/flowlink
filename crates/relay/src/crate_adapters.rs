//! Integration marketplace adapter.
//!
//! Thin adapter that bridges relay's AppState with the integration crates.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use flowlink_integrations_marketplace as marketplace;

use crate::server::AppState;

// ═══════════════════════════════════════════════
// Integration Marketplace
// ═══════════════════════════════════════════════

/// GET /api/integrations/catalog
pub async fn integrations_catalog(
    State(state): State<AppState>,
) -> axum::response::Response {
    use flowlink_integrations_core::IntegrationManager;
    let manager = state.integration_manager.lock().await;
    let catalog = manager.list_available();
    Json(serde_json::json!({ "integrations": catalog })).into_response()
}

/// GET /api/integrations — list user's installed integrations
pub async fn integrations_list(
    State(state): State<AppState>,
    claims: crate::middleware::ClaimsExtractor,
) -> axum::response::Response {
    let account_id = &claims.0.sub;
    if let Some(db) = &state.db {
        match marketplace::list_integrations_for_account(db, account_id).await {
            Ok(integrations) => {
                let resp: Vec<serde_json::Value> = integrations.iter().map(|i: &flowlink_integrations_core::IntegrationConfig| {
                    serde_json::json!({
                        "id": i.id,
                        "kind": i.kind.0,
                        "name": i.name,
                        "status": i.status,
                        "subscribed_events": i.subscribed_events,
                        "created_at": i.created_at.to_rfc3339(),
                    })
                }).collect();
                Json(resp).into_response()
            }
            Err(e) => {
                log::error!("Failed to list integrations: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
            }
        }
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Database not available").into_response()
    }
}

/// POST /api/integrations — install a new integration
pub async fn integrations_install(
    State(state): State<AppState>,
    claims: crate::middleware::ClaimsExtractor,
    Json(body): Json<serde_json::Value>,
) -> axum::response::Response {
    use flowlink_integrations_core::{IntegrationConfig, IntegrationKind, IntegrationStatus};

    let account_id = claims.0.sub.clone();
    let kind_str = body.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let kind = IntegrationKind(kind_str.to_string());

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let config = IntegrationConfig {
        id: id.clone(),
        kind: kind.clone(),
        account_id: account_id.clone(),
        org_id: body.get("org_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
        name: body.get("name").and_then(|v| v.as_str()).unwrap_or(kind_str).to_string(),
        config: body.get("config").cloned().unwrap_or(serde_json::json!({})),
        oauth_tokens: None,
        subscribed_events: body.get("subscribed_events")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        status: IntegrationStatus::Configured,
        created_at: now,
        updated_at: now,
    };

    let mut manager = state.integration_manager.lock().await;
    match manager.install(&kind, config.clone()).await {
        Ok(_) => {
            if let Some(db) = &state.db {
                if let Err(e) = marketplace::save_integration(db, &config).await {
                    log::error!("Failed to save integration: {}", e);
                }
            }
            Json(serde_json::json!({
                "id": id,
                "kind": kind_str,
                "status": "active"
            })).into_response()
        }
        Err(e) => {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": e.to_string()
            }))).into_response()
        }
    }
}

/// DELETE /api/integrations/{id}
pub async fn integrations_uninstall(
    State(state): State<AppState>,
    claims: crate::middleware::ClaimsExtractor,
    Path(id): Path<String>,
) -> axum::response::Response {
    let mut manager = state.integration_manager.lock().await;
    match manager.uninstall(&id).await {
        Ok(_) => {
            if let Some(db) = &state.db {
                if let Err(e) = marketplace::delete_integration(db, &id).await {
                    log::error!("Failed to delete integration: {}", e);
                }
            }
            Json(serde_json::json!({"status": "uninstalled"})).into_response()
        }
        Err(e) => {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": e.to_string()
            }))).into_response()
        }
    }
}

// ═══════════════════════════════════════════════
// OAuth2 Flow
// ═══════════════════════════════════════════════

/// POST /api/integrations/oauth/begin — initiate OAuth2 flow
pub async fn integrations_oauth_begin(
    State(state): State<AppState>,
    claims: crate::middleware::ClaimsExtractor,
    Json(body): Json<serde_json::Value>,
) -> axum::response::Response {
    use flowlink_integrations_core::{IntegrationConfig, IntegrationKind, IntegrationStatus};

    let account_id = claims.0.sub.clone();
    let kind_str = body.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let kind = IntegrationKind(kind_str.to_string());
    let redirect_after = body.get("redirect_after").and_then(|v| v.as_str()).map(|s| s.to_string());

    let integration_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let config = IntegrationConfig {
        id: integration_id.clone(),
        kind: kind.clone(),
        account_id: account_id.clone(),
        org_id: body.get("org_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
        name: body.get("name").and_then(|v| v.as_str()).unwrap_or(kind_str).to_string(),
        config: serde_json::json!({}),
        oauth_tokens: None,
        subscribed_events: body.get("subscribed_events")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        status: IntegrationStatus::PendingAuth,
        created_at: now,
        updated_at: now,
    };

    if let Some(db) = &state.db {
        if let Err(e) = marketplace::save_integration(db, &config).await {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": e.to_string()
            }))).into_response();
        }
    }

    let callback_url = format!("{}/api/integrations/oauth/callback", crate::server_base_url());

    let mut manager = state.integration_manager.lock().await;
    match manager.begin_oauth(&kind, integration_id.clone(), account_id, &callback_url, redirect_after) {
        Ok(authorize_url) => {
            Json(serde_json::json!({
                "authorize_url": authorize_url,
                "integration_id": integration_id
            })).into_response()
        }
        Err(e) => {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": e.to_string()
            }))).into_response()
        }
    }
}

/// GET /api/integrations/oauth/callback — OAuth2 callback
pub async fn integrations_oauth_callback(
    State(state): State<AppState>,
    Query(params): Query<serde_json::Value>,
) -> axum::response::Response {
    let code = params.get("code").and_then(|v| v.as_str()).unwrap_or("");
    let state_param = params.get("state").and_then(|v| v.as_str()).unwrap_or("");

    let callback_url = format!("{}/api/integrations/oauth/callback", crate::server_base_url());

    let mut manager = state.integration_manager.lock().await;
    match manager.complete_oauth(state_param, code, &callback_url).await {
        Ok((integration_id, tokens)) => {
            manager.update_oauth_tokens(&integration_id, tokens.clone());
            drop(manager);

            if let Some(db) = &state.db {
                if let Err(e) = marketplace::update_oauth_tokens(db, &integration_id, &tokens).await {
                    log::error!("Failed to persist OAuth tokens: {}", e);
                }
            }

            let redirect = "https://flowlink.flow-masters.ru/settings/integrations?status=connected";
            axum::response::Redirect::to(redirect).into_response()
        }
        Err(e) => {
            log::error!("OAuth callback failed: {}", e);
            let redirect = format!("https://flowlink.flow-masters.ru/settings/integrations?error={}", urlencoding::encode(&e.to_string()));
            axum::response::Redirect::to(&redirect).into_response()
        }
    }
}

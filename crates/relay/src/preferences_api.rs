//! User preferences & notifications API
//!
//! GET  /api/account/notifications        — list notifications for current user
//! POST /api/account/notifications/{id}/read — mark as read
//!
//! Backed by in-memory NotificationStore. Migrates to DB when notifications table is added.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use chrono::Utc;

use crate::server::AppState;

// --------------------------------------------------------------------------- //
// Types
// --------------------------------------------------------------------------- //

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub account_id: String,
    pub kind: String,        // "security", "billing", "system", "agent"
    pub title: String,
    pub message: String,
    pub read: bool,
    pub created_at: i64,
}

/// In-memory notification store (per account_id)
pub struct NotificationStore {
    notifications: tokio::sync::RwLock<std::collections::HashMap<String, Vec<Notification>>>,
}

impl Default for NotificationStore {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationStore {
    pub fn new() -> Self {
        Self {
            notifications: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    pub async fn add(&self, account_id: &str, kind: &str, title: &str, message: &str) -> Notification {
        let notif = Notification {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: account_id.to_string(),
            kind: kind.to_string(),
            title: title.to_string(),
            message: message.to_string(),
            read: false,
            created_at: Utc::now().timestamp(),
        };
        let mut map = self.notifications.write().await;
        map.entry(account_id.to_string())
            .or_insert_with(Vec::new)
            .push(notif.clone());
        notif
    }

    pub async fn list(&self, account_id: &str) -> Vec<Notification> {
        let map = self.notifications.read().await;
        map.get(account_id).cloned().unwrap_or_default()
    }

    pub async fn mark_read(&self, account_id: &str, notif_id: &str) -> bool {
        let mut map = self.notifications.write().await;
        if let Some(list) = map.get_mut(account_id) {
            for n in list.iter_mut() {
                if n.id == notif_id {
                    n.read = true;
                    return true;
                }
            }
        }
        false
    }
}

// --------------------------------------------------------------------------- //
// Helpers
// --------------------------------------------------------------------------- //

/// Extract account_id from JWT in Authorization header.
/// Returns (account_id, response_if_error).
fn extract_account_from_jwt(
    headers: &HeaderMap,
    auth_engine: &Option<Arc<crate::auth::AuthEngine>>,
) -> Result<String, axum::response::Response> {
    let token = headers.get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "missing authorization header"}))
        ).into_response())?;

    let engine = auth_engine.as_ref()
        .ok_or_else(|| (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "auth not configured"}))
        ).into_response())?;

    let claims = engine.validate_access_token(token)
        .map_err(|_| (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid or expired token"}))
        ).into_response())?;

    Ok(claims.account_id)
}

// --------------------------------------------------------------------------- //
// Route Handlers
// --------------------------------------------------------------------------- //

/// GET /api/account/notifications
pub async fn get_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let account_id = match extract_account_from_jwt(&headers, &state.auth_engine) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let notifs = if let Some(ref store) = state.notification_store {
        store.list(&account_id).await
    } else {
        vec![]
    };

    (StatusCode::OK, Json(json!(notifs))).into_response()
}

/// POST /api/account/notifications/{id}/read
pub async fn mark_notification_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let account_id = match extract_account_from_jwt(&headers, &state.auth_engine) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let found = if let Some(ref store) = state.notification_store {
        store.mark_read(&account_id, &id).await
    } else {
        false
    };

    if found {
        (StatusCode::OK, Json(json!({ "ok": true }))).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(json!({ "ok": false, "error": "notification not found" }))).into_response()
    }
}

//! User preferences & notifications API
//!
//! GET  /api/account/notifications        — list notifications for current user
//! POST /api/account/notifications/{id}/read — mark as read
//!
//! Backed by in-memory NotificationStore. Migrates to DB when notifications table is added.
//!
//! This is the standalone crate version — uses `NotificationState` instead of relay's `AppState`.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use chrono::Utc;

// ═══════════════════════════════════════════════
// State
// ═══════════════════════════════════════════════

/// Re-export NotificationState from api module
pub use crate::api::NotificationState;

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

/// Extract account_id from Extension (set by auth middleware).
fn require_account_id_from_ext(account_id: &Option<Extension<String>>) -> Result<String, axum::response::Response> {
    match account_id {
        Some(Extension(id)) => Ok(id.clone()),
        None => Err((StatusCode::UNAUTHORIZED, Json(json!({"error": "missing account_id"}))).into_response()),
    }
}

// --------------------------------------------------------------------------- //
// Route Handlers
// --------------------------------------------------------------------------- //

/// GET /api/account/notifications
pub async fn get_notifications(
    Extension(account_id): Extension<String>,
) -> impl IntoResponse {
    // In standalone mode, we just return empty — the real store is accessed via NotificationState
    // This handler is kept for API compatibility
    let notifs: Vec<Notification> = vec![];
    (StatusCode::OK, Json(json!(notifs))).into_response()
}

/// GET /api/account/notifications (with state for store access)
pub async fn get_notifications_with_store(
    Extension(state): Extension<std::sync::Arc<crate::api::NotificationState>>,
    Extension(account_id): Extension<String>,
) -> impl IntoResponse {
    let notifs = state.store.list(&account_id).await;
    (StatusCode::OK, Json(json!(notifs))).into_response()
}

/// POST /api/account/notifications/{id}/read
pub async fn mark_notification_read(
    Extension(state): Extension<std::sync::Arc<crate::api::NotificationState>>,
    Extension(account_id): Extension<String>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let found = state.store.mark_read(&account_id, &id).await;

    if found {
        (StatusCode::OK, Json(json!({ "ok": true }))).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(json!({ "ok": false, "error": "notification not found" }))).into_response()
    }
}

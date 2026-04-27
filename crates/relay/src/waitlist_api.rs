//! Waitlist API — public signup and admin notification

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

use crate::AppState;

#[derive(Deserialize)]
pub struct WaitlistSignup {
    pub email: String,
    pub feature_id: String,
    pub feature_name: String,
}

#[derive(Deserialize)]
pub struct WaitlistNotify {
    pub feature_id: String,
}

/// POST /api/waitlist — public signup (no auth)
pub async fn waitlist_signup(
    State(state): State<AppState>,
    Json(req): Json<WaitlistSignup>,
) -> impl IntoResponse {
    if req.email.is_empty() || !req.email.contains('@') {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid email"}))).into_response();
    }
    if req.feature_id.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Missing feature_id"}))).into_response();
    }

    match &state.db {
        Some(db) => {
            match flowlink_db::waitlist::waitlist_signup(db.pool(), &req.email, &req.feature_id, &req.feature_name).await {
                Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
            }
        }
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "DB unavailable"}))).into_response(),
    }
}

/// GET /api/admin/waitlist — list all entries (admin only)
pub async fn admin_waitlist_list(State(state): State<AppState>) -> impl IntoResponse {
    match &state.db {
        Some(db) => {
            match flowlink_db::waitlist::get_waitlist(db.pool()).await {
                Ok(entries) => (StatusCode::OK, Json(entries)).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
            }
        }
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "DB unavailable"}))).into_response(),
    }
}

/// POST /api/admin/waitlist/notify — notify all for a feature (admin only)
pub async fn admin_waitlist_notify(
    State(state): State<AppState>,
    Json(req): Json<WaitlistNotify>,
) -> impl IntoResponse {
    match &state.db {
        Some(db) => {
            match flowlink_db::waitlist::notify_waitlist(db.pool(), &req.feature_id).await {
                Ok(count) => (StatusCode::OK, Json(serde_json::json!({"notified": count}))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
            }
        }
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "DB unavailable"}))).into_response(),
    }
}

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use crate::server::AppState;

/// GET /api/account/notifications
pub async fn get_notifications(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    // TODO: query from DB notifications table
    (StatusCode::OK, Json(serde_json::json!([]))).into_response()
}

/// POST /api/account/notifications/{id}/read
pub async fn mark_notification_read(
    State(_state): State<AppState>,
    Path(_id): Path<String>,
) -> impl IntoResponse {
    // TODO: update notification in DB
    (StatusCode::OK, Json(json!({ "ok": true }))).into_response()
}

// TODO: Restore original auth_api.rs — currently stubbed for compilation
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Response},
    routing::{get, post},
    http::StatusCode,
    Json, Router,
};
use serde_json::json;

use crate::server::AppState;

pub async fn list_providers(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(json!({
        "providers": ["telegram", "email"]
    })))
}

pub async fn vk_callback(Query(_query): Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    // TODO: implement
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "not implemented"})))
}

pub async fn yandex_callback(Query(_query): Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "not implemented"})))
}

pub async fn github_callback(Query(_query): Query<std::collections::HashMap<String, String>>) -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "not implemented"})))
}

pub async fn refresh_token(State(_state): State<AppState>) -> impl IntoResponse {
    // TODO: implement
    (StatusCode::NOT_IMPLEMENTED, Json(json!({"error": "not implemented"})))
}

pub async fn auth_me(
    State(_state): State<AppState>,
    Query(_query): Query<()>,
) -> impl IntoResponse {
    // Mock response for now
    (StatusCode::OK, Json(json!({
        "account_id": "mock_account_id",
        "email": "user@example.com",
        "name": Some("Mock User"),
        "avatar_url": None::<String>,
        "tg_id": None::<String>,
        "created_at": 1704067200,
        "plan": Some("free"),
        "active": true,
        "servers_count": 0
    })))
}

pub async fn logout(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"message": "Logged out successfully"})))
}

pub async fn account_info(
    State(state): State<AppState>,
) -> impl IntoResponse {
    auth_me(axum::extract::State(state), Query(())).await
}

// Router for auth endpoints
pub fn auth_router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/providers", get(list_providers))
        .route("/api/auth/vk/callback", get(vk_callback))
        .route("/api/auth/yandex/callback", get(yandex_callback))
        .route("/api/auth/github/callback", get(github_callback))
        .route("/api/auth/refresh", post(refresh_token))
        .route("/api/auth/me", get(auth_me))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/account", get(account_info))
}

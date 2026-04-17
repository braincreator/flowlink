//! GDPR Account Deletion API handlers

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
    Router,
};
use serde_json::json;

use crate::server::AppState;

fn extract_account_id(
    state: &AppState,
    headers: &HeaderMap,
) -> std::result::Result<String, (StatusCode, Json<serde_json::Value>)> {
    let token = headers
        .get("authorization")
        .and_then(|v: &axum::http::HeaderValue| v.to_str().ok())
        .and_then(|v: &str| v.strip_prefix("Bearer "))
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "отсутствует авторизация"})),
            )
        })?;

    let engine = state.auth_engine.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "авторизация не настроена"})),
        )
    })?;

    let claims = engine.validate_access_token(token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "неверный токен"})),
        )
    })?;

    Ok(claims.account_id)
}

fn get_pool(
    state: &AppState,
) -> std::result::Result<&sqlx::PgPool, (StatusCode, Json<serde_json::Value>)> {
    state.db.as_ref().map(|db| db.pool()).ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "база данных недоступна"})),
        )
    })
}

/// DELETE /api/account — Request soft deletion (30-day grace period)
pub async fn request_deletion(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (
    StatusCode,
    Json<serde_json::Value>,
) {
    let account_id = match extract_account_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return e,
    };
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(e) => return e,
    };

    // Check if already requested
    let acc = match flowlink_db::accounts::AccountRepo::get(pool, &account_id).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "аккаунт не найден"})),
            )
        }
        Err(e) => {
            log::error!("Failed to get account for deletion: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "ошибка базы данных"})),
            );
        }
    };

    if acc.deletion_requested_at.is_some() {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "удаление уже запрошено",
                "deleted_at": acc.deleted_at,
                "deletion_requested_at": acc.deletion_requested_at,
            })),
        );
    }

    match flowlink_db::accounts::AccountRepo::request_deletion(pool, &account_id).await {
        Ok(()) => {
            log::info!("Account {} requested deletion", account_id);
            // TODO: send email notification
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "message": "аккаунт будет удалён через 30 дней",
                    "deleted_at": chrono::Utc::now() + chrono::Duration::days(30),
                })),
            )
        }
        Err(e) => {
            log::error!("Failed to request deletion for {}: {e}", account_id);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "ошибка базы данных"})),
            )
        }
    }
}

/// POST /api/account/cancel-deletion — Cancel pending deletion
pub async fn cancel_deletion(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (
    StatusCode,
    Json<serde_json::Value>,
) {
    let account_id = match extract_account_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return e,
    };
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(e) => return e,
    };

    match flowlink_db::accounts::AccountRepo::cancel_deletion(pool, &account_id).await {
        Ok(true) => {
            log::info!("Account {} cancelled deletion", account_id);
            (
                StatusCode::OK,
                Json(json!({"ok": true, "message": "удаление отменено"})),
            )
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "нет ожидающего удаления"})),
        ),
        Err(e) => {
            log::error!("Failed to cancel deletion for {}: {e}", account_id);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "ошибка базы данных"})),
            )
        }
    }
}

/// DELETE /api/account/hard — Immediate hard delete (requires confirmation_code)
pub async fn hard_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> (
    StatusCode,
    Json<serde_json::Value>,
) {
    let account_id = match extract_account_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return e,
    };
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let confirmation_code = match body.get("confirmation_code").and_then(|v| v.as_str()) {
        Some(c) if c == "DELETE_MY_ACCOUNT" => c,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "требуется подтверждение: отправьте confirmation_code = \"DELETE_MY_ACCOUNT\""})),
            )
        }
    };
    let _ = confirmation_code;

    // Verify account has a recent soft-delete request
    let acc = match flowlink_db::accounts::AccountRepo::get(pool, &account_id).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "аккаунт не найден"})),
            )
        }
        Err(e) => {
            log::error!("Failed to get account for hard delete: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "ошибка базы данных"})),
            );
        }
    };

    if acc.deletion_requested_at.is_none() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "сначала запросите мягкое удаление"})),
        );
    }

    match flowlink_db::accounts::AccountRepo::hard_delete(pool, &account_id).await {
        Ok(()) => {
            log::warn!("Account {} hard-deleted by user", account_id);
            (
                StatusCode::OK,
                Json(json!({"ok": true, "message": "аккаунт удалён навсегда"})),
            )
        }
        Err(e) => {
            log::error!("Failed to hard-delete account {}: {e}", account_id);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "ошибка базы данных"})),
            )
        }
    }
}

/// Scheduled cleanup: hard-delete accounts past their deletion date
pub async fn cleanup_expired_deletions(
    State(state): State<AppState>,
) -> (
    StatusCode,
    Json<serde_json::Value>,
) {
    let pool = match get_pool(&state) {
        Ok(p) => p,
        Err(e) => return e,
    };

    let expired = match flowlink_db::accounts::AccountRepo::find_expired_deletions(pool).await {
        Ok(ids) => ids,
        Err(e) => {
            log::error!("Failed to find expired deletions: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "ошибка базы данных"})),
            );
        }
    };

    let mut deleted = Vec::new();
    for account_id in &expired {
        match flowlink_db::accounts::AccountRepo::hard_delete(pool, account_id).await {
            Ok(()) => {
                log::info!("Auto-hard-deleted expired account {}", account_id);
                deleted.push(account_id.clone());
            }
            Err(e) => {
                log::error!("Failed to auto-delete account {}: {e}", account_id);
            }
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "deleted_count": deleted.len(),
            "deleted_accounts": deleted,
        })),
    )
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/account", axum::routing::delete(request_deletion))
        .route("/api/account/cancel-deletion", axum::routing::post(cancel_deletion))
        .route("/api/account/hard", axum::routing::delete(hard_delete))
}

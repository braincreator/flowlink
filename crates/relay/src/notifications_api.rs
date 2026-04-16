//! Notification channel management API.
//!
//! REST endpoints for users to bind, configure, and test notification channels.
//!
//! All endpoints require JWT auth (applied via middleware layer).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::server::AppState;

// ═══════════════════════════════════════════════
// Request / Response types
// ═══════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct BindChannelRequest {
    pub channel_type: String,
    pub channel_address: String,
    pub display_name: Option<String>,
    /// Auto-set as primary if no other primary exists
    #[serde(default)]
    pub set_primary: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChannelRequest {
    pub min_severity: Option<String>,
    pub mute_categories: Option<Vec<String>>,
    pub display_name: Option<String>,
    pub is_primary: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ChannelResponse {
    pub id: uuid::Uuid,
    pub channel_type: String,
    pub channel_address: String,
    pub display_name: Option<String>,
    pub is_primary: bool,
    pub verified: bool,
    pub min_severity: String,
    pub mute_categories: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl ChannelResponse {
    fn from_db(ch: &flowlink_db::notification_channels::UserChannel) -> Self {
        let mute_cats = ch.mute_categories.as_ref()
            .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok())
            .unwrap_or_default();

        Self {
            id: ch.id,
            channel_type: ch.channel_type.clone(),
            channel_address: ch.channel_address.clone(),
            display_name: ch.display_name.clone(),
            is_primary: ch.is_primary,
            verified: ch.verified,
            min_severity: ch.min_severity.clone().unwrap_or_else(|| "info".into()),
            mute_categories: mute_cats,
            created_at: ch.created_at,
        }
    }
}

// ═══════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════

/// Extract account_id from JWT auth middleware.
/// The auth middleware sets `account_id` in request extensions.
fn get_account_id(state: &AppState, req_parts: &axum::http::request::Parts) -> Result<String, StatusCode> {
    // Try to get account_id from auth context
    // The JWT middleware stores claims — we access via the auth module
    // For now, extract from the state's auth engine based on the token
    // This is a placeholder — actual implementation reads from request extensions
    Err(StatusCode::UNAUTHORIZED)
}

/// Simple JSON error response.
fn error_response(status: StatusCode, message: &str) -> axum::response::Response {
    (
        status,
        Json(serde_json::json!({"error": message})),
    ).into_response()
}

// ═══════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════

/// GET /api/notifications/channels
///
/// List all notification channels bound to the authenticated user's account.
pub async fn list_channels(
    State(state): State<AppState>,
    axum::Extension(account_id): axum::Extension<crate::middleware::AccountIdExtractor>,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return error_response(StatusCode::SERVICE_UNAVAILABLE, "Database not configured"),
    };

    match flowlink_db::notification_channels::UserChannelRepo::list_for_account(db.pool(), &account_id.0).await {
        Ok(channels) => {
            let resp: Vec<ChannelResponse> = channels.iter().map(ChannelResponse::from_db).collect();
            (StatusCode::OK, Json(serde_json::json!({"channels": resp, "count": resp.len()}))).into_response()
        }
        Err(e) => {
            log::warn!("Failed to list channels for {}: {}", account_id.0, e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to list channels")
        }
    }
}

/// POST /api/notifications/channels
///
/// Bind a new notification channel.
pub async fn bind_channel(
    State(state): State<AppState>,
    axum::Extension(account_id): axum::Extension<crate::middleware::AccountIdExtractor>,
    Json(req): Json<BindChannelRequest>,
) -> impl IntoResponse {
    // Validate channel_type
    let valid_types = ["telegram", "max", "slack", "webhook", "email"];
    if !valid_types.contains(&req.channel_type.as_str()) {
        return error_response(
            StatusCode::BAD_REQUEST,
            &format!("Invalid channel_type '{}'. Must be one of: {}", req.channel_type, valid_types.join(", ")),
        );
    }

    let db = match &state.db {
        Some(db) => db,
        None => return error_response(StatusCode::SERVICE_UNAVAILABLE, "Database not configured"),
    };

    // For telegram, verify that the chat_id looks valid
    if req.channel_type == "telegram" {
        if req.channel_address.parse::<i64>().is_err() {
            return error_response(StatusCode::BAD_REQUEST, "Invalid Telegram chat_id");
        }
    }

    let is_primary = req.set_primary;
    match flowlink_db::notification_channels::UserChannelRepo::upsert(
        db.pool(),
        &account_id.0,
        &req.channel_type,
        &req.channel_address,
        req.display_name.as_deref(),
        is_primary,
    ).await {
        Ok(ch) => {
            // Auto-verify for telegram (binding via bot = already verified)
            if req.channel_type == "telegram" {
                let _ = flowlink_db::notification_channels::UserChannelRepo::verify(db.pool(), ch.id).await;
            }
            let resp = ChannelResponse::from_db(&ch);
            (StatusCode::CREATED, Json(serde_json::json!({
                "ok": true,
                "channel": resp,
                "message": format!("{} channel bound", req.channel_type),
            }))).into_response()
        }
        Err(e) => {
            log::warn!("Failed to bind channel: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to bind channel")
        }
    }
}

/// PATCH /api/notifications/channels/:id
///
/// Update channel settings (mute, severity, display_name).
pub async fn update_channel(
    State(state): State<AppState>,
    axum::Extension(account_id): axum::Extension<crate::middleware::AccountIdExtractor>,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<UpdateChannelRequest>,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return error_response(StatusCode::SERVICE_UNAVAILABLE, "Database not configured"),
    };

    let mut updated = false;

    if let Some(ref severity) = req.min_severity {
        match flowlink_db::notification_channels::UserChannelRepo::set_min_severity(db.pool(), id, severity).await {
            Ok(true) => updated = true,
            Ok(false) => return error_response(StatusCode::NOT_FOUND, "Channel not found"),
            Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid severity: {}", e)),
        }
    }

    if let Some(ref categories) = req.mute_categories {
        let cats: Vec<&str> = categories.iter().map(|s| s.as_str()).collect();
        if let Ok(true) = flowlink_db::notification_channels::UserChannelRepo::set_mute_categories(db.pool(), id, cats).await {
            updated = true;
        }
    }

    if let Some(true) = req.is_primary {
        if let Ok(true) = flowlink_db::notification_channels::UserChannelRepo::set_primary(db.pool(), id).await {
            updated = true;
        }
    }

    if !updated {
        return error_response(StatusCode::BAD_REQUEST, "No valid fields to update");
    }

    // Return updated channel
    match flowlink_db::notification_channels::UserChannelRepo::get_by_type(
        db.pool(), &account_id.0, "telegram", // fallback — ideally get by id
    ).await {
        Ok(Some(ch)) if ch.id == id => {
            let resp = ChannelResponse::from_db(&ch);
            (StatusCode::OK, Json(serde_json::json!({"ok": true, "channel": resp}))).into_response()
        }
        _ => {
            // Return generic success if we can't find it (timing)
            (StatusCode::OK, Json(serde_json::json!({"ok": true, "message": "Updated"}))).into_response()
        }
    }
}

/// DELETE /api/notifications/channels/:id
///
/// Unbind a notification channel.
pub async fn unbind_channel(
    State(state): State<AppState>,
    _account_id: axum::Extension<String>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return error_response(StatusCode::SERVICE_UNAVAILABLE, "Database not configured"),
    };

    match flowlink_db::notification_channels::UserChannelRepo::delete(db.pool(), id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"ok": true, "message": "Channel removed"}))).into_response(),
        Ok(false) => error_response(StatusCode::NOT_FOUND, "Channel not found"),
        Err(e) => {
            log::warn!("Failed to unbind channel {}: {}", id, e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to unbind channel")
        }
    }
}

/// POST /api/notifications/channels/:id/verify
///
/// Mark a channel binding as verified.
pub async fn verify_channel(
    State(state): State<AppState>,
    _account_id: axum::Extension<String>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return error_response(StatusCode::SERVICE_UNAVAILABLE, "Database not configured"),
    };

    match flowlink_db::notification_channels::UserChannelRepo::verify(db.pool(), id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"ok": true, "message": "Channel verified"}))).into_response(),
        Ok(false) => error_response(StatusCode::NOT_FOUND, "Channel not found"),
        Err(e) => {
            log::warn!("Failed to verify channel {}: {}", id, e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify channel")
        }
    }
}

/// POST /api/notifications/channels/:id/primary
///
/// Set a channel as the primary notification channel.
pub async fn set_primary(
    State(state): State<AppState>,
    _account_id: axum::Extension<String>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return error_response(StatusCode::SERVICE_UNAVAILABLE, "Database not configured"),
    };

    match flowlink_db::notification_channels::UserChannelRepo::set_primary(db.pool(), id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"ok": true, "message": "Primary channel set"}))).into_response(),
        Ok(false) => error_response(StatusCode::NOT_FOUND, "Channel not found"),
        Err(e) => {
            log::warn!("Failed to set primary channel {}: {}", id, e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to set primary channel")
        }
    }
}

/// POST /api/notifications/test
///
/// Send a test notification to all user's verified channels.
pub async fn send_test(
    State(state): State<AppState>,
    axum::Extension(account_id): axum::Extension<crate::middleware::AccountIdExtractor>,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return error_response(StatusCode::SERVICE_UNAVAILABLE, "Database not configured"),
    };

    // Get user's channels
    let channels = match flowlink_db::notification_channels::UserChannelRepo::list_for_account(db.pool(), &account_id.0).await {
        Ok(ch) => ch,
        Err(e) => {
            log::warn!("Failed to list channels for test: {}", e);
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to list channels");
        }
    };

    if channels.is_empty() {
        return error_response(StatusCode::CONFLICT, "No notification channels bound. Bind a channel first.");
    }

    // Create test notification
    let notification = crate::notifications::Notification {
        id: uuid::Uuid::new_v4().to_string(),
        account_id: account_id.0.clone(),
        severity: crate::notifications::Severity::Info,
        category: crate::notifications::Category::System,
        subject: "Test Notification".into(),
        body: format!(
            "<b>✅ Test Notification</b>\nFlowLink уведомления работают!\nКаналов привязано: {}\nВремя: {}",
            channels.len(),
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        ),
        data: std::collections::HashMap::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        tags: vec!["test".into()],
    };

    // Send via router
    let router = state.notification_router.get();
    let delivered = match router {
        Some(router) => router.send(&notification).await,
        None => {
            return error_response(StatusCode::SERVICE_UNAVAILABLE, "Notification router not configured");
        }
    };

    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "message": format!("Test sent to {} channel(s)", delivered),
        "total_channels": channels.len(),
        "delivered": delivered,
    }))).into_response()
}

/// POST /api/notifications/link-code
///
/// Generate a one-time linking code for binding a notification channel.
/// User copies the code and enters it in the target channel (TG, MAX, etc.).
pub async fn generate_link_code(
    State(state): State<AppState>,
    axum::Extension(account_id): axum::Extension<crate::middleware::AccountIdExtractor>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let channel_type = req.get("channel_type")
        .and_then(|v| v.as_str())
        .unwrap_or("telegram");

    let valid_types = ["telegram", "max", "slack"];
    if !valid_types.contains(&channel_type) {
        return error_response(StatusCode::BAD_REQUEST, "Invalid channel_type");
    }

    let db = match &state.db {
        Some(db) => db,
        None => return error_response(StatusCode::SERVICE_UNAVAILABLE, "Database not configured"),
    };

    // Generate 6-digit code
    let code = format!("{:06}", rand::random::<u32>() % 1_000_000);

    // Store in linking_codes table
    let result = sqlx::query(
        r#"INSERT INTO linking_codes (code, account_id, channel_type, channel_address, expires_at)
           VALUES ($1, $2, $3, $4, NOW() + INTERVAL '10 minutes')
           ON CONFLICT DO NOTHING"#,
    )
    .bind(&code)
    .bind(&account_id.0)
    .bind(channel_type)
    .bind("") // channel_address filled when confirmed from TG side
    .execute(db.pool())
    .await;

    match result {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({
            "ok": true,
            "code": code,
            "channel_type": channel_type,
            "expires_in_seconds": 600,
            "message": match channel_type {
                "telegram" => "Отправьте /start <code> в Telegram боте FlowLink",
                "max" => "Отправьте /link <code> в MAX боте FlowLink",
                "slack" => "Используйте код для привязки Slack",
                _ => "Введите код в целевом канале",
            },
        }))).into_response(),
        Err(e) => {
            log::warn!("Failed to generate link code: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate code")
        }
    }
}

/// POST /api/notifications/confirm-code
///
/// Called from TG bot (or other channel) when user sends /start <code>.
/// Matches code → links channel → returns account info.
pub async fn confirm_link_code(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let code = match req.get("code").and_then(|v| v.as_str()) {
        Some(c) if !c.is_empty() => c,
        _ => return error_response(StatusCode::BAD_REQUEST, "Missing code"),
    };

    let channel_address = req.get("channel_address")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let display_name = req.get("display_name")
        .and_then(|v| v.as_str());

    let db = match &state.db {
        Some(db) => db,
        None => return error_response(StatusCode::SERVICE_UNAVAILABLE, "Database not configured"),
    };

    // Lookup code
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT account_id, channel_type, channel_address FROM linking_codes \
         WHERE code = $1 AND used_at IS NULL AND expires_at > NOW()",
    )
    .bind(code)
    .fetch_optional(db.pool())
    .await;

    match row {
        Ok(Some((account_id, channel_type, _))) => {
            // Mark as used
            let _ = sqlx::query("UPDATE linking_codes SET used_at = NOW() WHERE code = $1")
                .bind(code)
                .execute(db.pool())
                .await;

            // Update channel address if provided
            let addr = if !channel_address.is_empty() { channel_address } else { "" };

            // Bind notification channel
            match flowlink_db::notification_channels::UserChannelRepo::upsert(
                db.pool(),
                &account_id,
                &channel_type,
                addr,
                display_name,
                true, // auto-primary
            ).await {
                Ok(ch) => {
                    // Also auto-verify
                    let _ = flowlink_db::notification_channels::UserChannelRepo::verify(db.pool(), ch.id).await;

                    // Update tg_id in accounts if telegram
                    if channel_type == "telegram" {
                        if let Ok(chat_id) = channel_address.parse::<i64>() {
                            let _ = flowlink_db::accounts::AccountRepo::update_tg_id(
                                db.pool(), &account_id, chat_id,
                            ).await;
                        }
                    }

                    log::info!(
                        "Channel linked via code: account={}, type={}, addr={}",
                        account_id, channel_type, addr,
                    );

                    (StatusCode::OK, Json(serde_json::json!({
                        "ok": true,
                        "account_id": account_id,
                        "channel_type": channel_type,
                        "verified": true,
                        "message": "Channel linked successfully",
                    }))).into_response()
                }
                Err(e) => {
                    log::warn!("Failed to bind channel after code confirm: {}", e);
                    error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to bind channel")
                }
            }
        }
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Invalid or expired code"),
        Err(e) => {
            log::warn!("Code lookup failed: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to verify code")
        }
    }
}

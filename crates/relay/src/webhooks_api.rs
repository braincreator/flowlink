//! Webhooks API — CRUD + test ping for organization webhooks

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::middleware::AccountIdExtractor;
use crate::orgs_api::require_org_role;
use crate::server::AppState;
use flowlink_db::webhooks::{WebhookRepo, trigger_webhooks};

#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    pub url: String,
    pub events: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListAuditQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub action: Option<String>,
}

// ═══════════════════════════════════════════════
// Audit Log Endpoint
// ═══════════════════════════════════════════════

/// GET /api/orgs/{org_id}/audit
pub async fn list_org_audit(
    State(state): State<AppState>,
    AccountIdExtractor(account_id): AccountIdExtractor,
    Path(org_id): Path<Uuid>,
    Query(params): Query<ListAuditQuery>,
) -> impl IntoResponse {
    let pool = match state.db.as_ref() {
        Some(db) => db.pool(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "database not configured"}))).into_response(),
    };

    // Only owner/admin can view
    if let Err(err) = require_org_role(pool, org_id, &account_id, &["owner", "admin"]).await {
        return err.into_response();
    }

    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(50).clamp(1, 200);

    match flowlink_db::audit::query_org_audit(
        pool,
        &org_id.to_string(),
        page,
        limit,
        params.action.as_deref(),
    )
    .await
    {
        Ok((rows, total)) => {
            let items: Vec<Value> = rows
                .into_iter()
                .map(|r| {
                    json!({
                        "id": r.id,
                        "org_id": r.org_id,
                        "account_id": r.account_id,
                        "action": r.action,
                        "resource_type": r.resource_type,
                        "resource_id": r.resource_id,
                        "details": r.details,
                        "ip_address": r.ip_address,
                        "timestamp": r.timestamp,
                    })
                })
                .collect();
            let pages = (total + limit - 1) / limit;
            Json(json!({
                "items": items,
                "total": total,
                "page": page,
                "limit": limit,
                "pages": pages,
            }))
            .into_response()
        }
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Internal error"})),
        )
            .into_response(),
    }
}

// ═══════════════════════════════════════════════
// Webhook Endpoints
// ═══════════════════════════════════════════════

fn json_webhook(row: &flowlink_db::webhooks::WebhookRow) -> Value {
    json!({
        "id": row.id,
        "org_id": row.org_id,
        "url": row.url,
        "events": row.events,
        "is_active": row.is_active,
        "created_at": row.created_at,
        "last_triggered_at": row.last_triggered_at,
    })
}

/// GET /api/orgs/{org_id}/webhooks
pub async fn list_webhooks(
    State(state): State<AppState>,
    AccountIdExtractor(account_id): AccountIdExtractor,
    Path(org_id): Path<Uuid>,
) -> impl IntoResponse {
    let pool = match state.db.as_ref() {
        Some(db) => db.pool(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "database not configured"}))).into_response(),
    };

    if let Err(err) = require_org_role(pool, org_id, &account_id, &["owner", "admin"]).await {
        return err.into_response();
    }

    match WebhookRepo::list_by_org(pool, &org_id.to_string()).await {
        Ok(rows) => {
            let items: Vec<Value> = rows.iter().map(json_webhook).collect();
            Json(json!({"items": items})).into_response()
        }
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Internal error"})),
        )
            .into_response(),
    }
}

/// POST /api/orgs/{org_id}/webhooks
pub async fn create_webhook(
    State(state): State<AppState>,
    AccountIdExtractor(account_id): AccountIdExtractor,
    Path(org_id): Path<Uuid>,
    Json(body): Json<CreateWebhookRequest>,
) -> impl IntoResponse {
    let pool = match state.db.as_ref() {
        Some(db) => db.pool(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "database not configured"}))).into_response(),
    };

    if let Err(err) = require_org_role(pool, org_id, &account_id, &["owner", "admin"]).await {
        return err.into_response();
    }

    // Validate URL
    let parsed_url = match url::Url::parse(&body.url) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid url"}))).into_response(),
    };

    // SSRF protection: reject internal/private IPs
    let host = parsed_url.host_str().unwrap_or("");
    let blocked = ["127.0.0.1", "0.0.0.0", "localhost", "::1", "169.254.169.254",
        "10.", "172.16.", "172.17.", "172.18.", "172.19.", "172.2", "172.3",
        "192.168.", "fc00:", "fe80:", "fd"];
    if blocked.iter().any(|b| host.starts_with(*b) || host == *b) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Webhook URL must be publicly accessible"}))).into_response();
    }
    if parsed_url.scheme() != "https" {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Webhook URL must use HTTPS"}))).into_response();
    }

    if body.events.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "at least one event required"})),
        )
            .into_response();
    }

    let secret = uuid::Uuid::new_v4().to_string();

    match WebhookRepo::create(pool, &org_id.to_string(), &body.url, &secret, &body.events).await {
        Ok(row) => (StatusCode::CREATED, Json(json_webhook(&row))).into_response(),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Internal error"})),
        )
            .into_response(),
    }
}

/// DELETE /api/orgs/{org_id}/webhooks/{id}
pub async fn delete_webhook(
    State(state): State<AppState>,
    AccountIdExtractor(account_id): AccountIdExtractor,
    Path((org_id, id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    let pool = match state.db.as_ref() {
        Some(db) => db.pool(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "database not configured"}))).into_response(),
    };

    if let Err(err) = require_org_role(pool, org_id, &account_id, &["owner", "admin"]).await {
        return err.into_response();
    }

    match WebhookRepo::delete(pool, id, &org_id.to_string()).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, Json(json!({"error": "webhook not found"}))).into_response(),
        Err(_e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Internal error"})),
        )
            .into_response(),
    }
}

/// POST /api/orgs/{org_id}/webhooks/{id}/test
pub async fn test_webhook(
    State(state): State<AppState>,
    AccountIdExtractor(account_id): AccountIdExtractor,
    Path((org_id, id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    let pool = match state.db.as_ref() {
        Some(db) => db.pool(),
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "database not configured"}))).into_response(),
    };

    if let Err(err) = require_org_role(pool, org_id, &account_id, &["owner", "admin"]).await {
        return err.into_response();
    }

    let _wh = match WebhookRepo::get(pool, id, &org_id.to_string()).await {
        Ok(Some(w)) => w,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "webhook not found"}))).into_response(),
        Err(_e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Internal error"}))).into_response(),
    };

    // Fire a test ping
    let payload = json!({
        "event": "webhook.test",
        "webhook_id": id,
        "org_id": org_id,
        "timestamp": chrono::Utc::now(),
    });
    trigger_webhooks(pool, &org_id.to_string(), "webhook.test", payload).await;

    Json(json!({"ok": true, "message": "test ping sent"})).into_response()
}

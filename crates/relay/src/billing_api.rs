//! Billing API endpoints for the relay server
//!
//! REST API для управления тарифами, подписками, платежами и вебхуками Точка Банка.

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    body::Bytes,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::middleware::AccountIdExtractor;
use crate::server::AppState;

// ═══════════════════════════════════════════════
// Request / Response types
// ═══════════════════════════════════════════════

#[derive(Deserialize)]
pub struct ChangePlanRequest {
    pub plan_id: String,
}

#[derive(Deserialize)]
pub struct TopUpRequest {
    pub amount_kopecks: u64,
    pub method: String,
}

#[derive(Deserialize)]
pub struct CreateSubscriptionRequest {
    pub plan_id: String,
    pub period: String,
    pub amount_kopecks: i64,
    pub tochka_subscription_id: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateOrderRequest {
    pub amount_kopecks: i64,
    pub description: Option<String>,
    pub payment_method: String,
}

#[derive(Serialize)]
pub struct BillingInfo {
    pub plan_id: String,
    pub plan_name: String,
    pub active: bool,
    pub balance_rub: String,
    pub expires_at: Option<String>,
    pub usage: Value,
    pub available_plans: Vec<Value>,
}

// ═══════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════

fn get_billing_engine(state: &AppState) -> Result<&Arc<flowlink_billing::BillingEngine>, axum::response::Response> {
    match &state.billing {
        Some(engine) => Ok(engine),
        None => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "Billing not configured"})),
        ).into_response()),
    }
}

/// Constant-time string comparison для предотвращения timing-атак
fn const_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut result = 0u8;
    for i in 0..a_bytes.len() {
        result |= a_bytes[i] ^ b_bytes[i];
    }
    result == 0
}

// ═══════════════════════════════════════════════
// Handlers — Existing billing endpoints
// ═══════════════════════════════════════════════

/// GET /api/billing — get billing info for the authenticated account
pub async fn get_billing_info(
    State(state): State<AppState>,
    account: AccountIdExtractor,
) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) {
        Ok(e) => e,
        Err(r) => return r,
    };

    // Ensure account exists in DB
    if let Some(db) = &state.db {
        if let Err(e) = flowlink_db::accounts::AccountRepo::get_or_create(
            db.pool(), &account.0, flowlink_billing::plans::PlanId::Trial.as_str(),
        ).await {
            log::warn!("DB account lookup failed: {e}");
        }
    }

    let account_billing = billing_engine.get_or_create_account(&account.0);
    let plan = billing_engine.plans().get(&account_billing.plan_id);
    let usage = billing_engine.usage().get_snapshot(&account.0);

    let plan_name = plan.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let available_plans: Vec<Value> = billing_engine.plans()
        .list_available()
        .iter()
        .map(|p| json!({
            "id": p.id,
            "name": p.name,
            "price_rub": p.format_monthly(),
            "tier": p.tier,
        }))
        .collect();

    let info = BillingInfo {
        plan_id: account_billing.plan_id.clone(),
        plan_name,
        active: account_billing.active,
        balance_rub: flowlink_billing::payment::PaymentConfig::format_rub(
            account_billing.balance_kopecks
        ),
        expires_at: account_billing.expires_at.map(|dt| dt.to_rfc3339()),
        usage: serde_json::to_value(&usage).unwrap_or(json!(null)),
        available_plans,
    };

    (StatusCode::OK, Json(info)).into_response()
}

/// GET /api/billing/usage — get current usage snapshot
pub async fn get_usage(
    State(state): State<AppState>,
    account: AccountIdExtractor,
) -> impl IntoResponse {
    let all_tracker_usage = state.usage_tracker.get_all_usage().await;
    let (daily_requests, daily_tokens) = state.usage_tracker.today_stats().await;

    let mut response = serde_json::json!({
        "tracker": {
            "agents": all_tracker_usage,
            "daily_requests": daily_requests,
            "daily_tokens": daily_tokens,
            "active_agents": all_tracker_usage.len(),
        }
    });

    if let Some(billing_engine) = &state.billing {
        let snapshot = billing_engine.usage().get_snapshot(&account.0);
        response["billing"] = serde_json::to_value(&snapshot).unwrap_or(json!(null));
    }

    (StatusCode::OK, Json(response)).into_response()
}

/// GET /api/billing/plans — list available plans
pub async fn list_plans(State(state): State<AppState>) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) {
        Ok(e) => e,
        Err(r) => return r,
    };

    let plans = billing_engine.plans().list_available();
    (StatusCode::OK, Json(plans)).into_response()
}

/// POST /api/billing/change-plan — change plan
pub async fn change_plan(
    State(state): State<AppState>,
    account: AccountIdExtractor,
    Json(body): Json<ChangePlanRequest>,
) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) {
        Ok(e) => e,
        Err(r) => return r,
    };

    let mut account_billing = billing_engine.get_or_create_account(&account.0);

    match billing_engine.change_plan(&mut account_billing, &body.plan_id) {
        Ok(created_invoice) => {
            if let Some(db) = &state.db {
                if let Err(e) = flowlink_db::accounts::AccountRepo::update_plan(
                    db.pool(), &account.0, &body.plan_id,
                ).await {
                    log::warn!("Failed to persist plan change to DB: {e}");
                }
                if let Some(ref inv) = created_invoice {
                    let row = flowlink_db::invoices::InvoiceRow {
                        id: inv.id.clone(),
                        account_id: inv.account_id.clone(),
                        number: inv.number.clone(),
                        status: format!("{:?}", inv.status).to_lowercase(),
                        subtotal_kopecks: inv.subtotal_kopecks as i64,
                        tax_kopecks: inv.tax_kopecks as i64,
                        total_kopecks: inv.total_kopecks as i64,
                        currency: inv.currency.clone(),
                        payment_method: inv.payment_method.as_ref().map(|m| format!("{:?}", m).to_lowercase()),
                        created_at: inv.created_at,
                        paid_at: inv.paid_at,
                        due_at: inv.due_at,
                        notes: inv.notes.clone(),
                    };
                    if let Err(e) = flowlink_db::invoices::InvoiceRepo::create(
                        db.pool(), &row, &[],
                    ).await {
                        log::warn!("Failed to persist invoice to DB: {e}");
                    }
                }
            }
            let mut resp = json!({
                "plan_id": account_billing.plan_id,
                "message": "Plan changed successfully",
            });
            if let Some(inv) = created_invoice {
                resp["invoice"] = serde_json::to_value(&inv).unwrap_or(json!(null));
            }
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => {
            (StatusCode::BAD_REQUEST, Json(json!({
                "error": e.to_string(),
            }))).into_response()
        }
    }
}

/// GET /api/billing/invoices — list invoices
pub async fn list_invoices(
    State(state): State<AppState>,
    account: AccountIdExtractor,
) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) {
        Ok(e) => e,
        Err(r) => return r,
    };

    let invoices = billing_engine.invoices().list_for_account(&account.0);
    (StatusCode::OK, Json(invoices)).into_response()
}

/// GET /api/billing/invoices/{id} — get specific invoice
pub async fn get_invoice(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) {
        Ok(e) => e,
        Err(r) => return r,
    };

    match billing_engine.invoices().get(&id) {
        Some(invoice) => (StatusCode::OK, Json(invoice)).into_response(),
        None => {
            (StatusCode::NOT_FOUND, Json(json!({
                "error": "Invoice not found",
            }))).into_response()
        }
    }
}

/// GET /api/billing/payments/methods — list available payment methods
pub async fn list_payment_methods(State(state): State<AppState>) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) {
        Ok(e) => e,
        Err(r) => return r,
    };

    let methods: Vec<Value> = billing_engine.payments().available_methods()
        .iter()
        .map(|m| json!({
            "id": m.as_str(),
            "name": m.display_name(),
        }))
        .collect();

    (StatusCode::OK, Json(methods)).into_response()
}

// ═══════════════════════════════════════════════
// Handlers — Subscriptions (Точка Банк)
// ═══════════════════════════════════════════════

/// GET /api/billing/subscriptions — список подписок аккаунта
pub async fn list_subscriptions(
    State(state): State<AppState>,
    account: AccountIdExtractor,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response(),
    };
    match flowlink_db::subscriptions::SubscriptionRepo::list_for_account(db.pool(), &account.0).await {
        Ok(subs) => (StatusCode::OK, Json(json!(subs))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/billing/subscriptions — создать подписку
pub async fn create_subscription(
    State(state): State<AppState>,
    account: AccountIdExtractor,
    Json(body): Json<CreateSubscriptionRequest>,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response(),
    };
    let id = uuid::Uuid::new_v4().to_string();
    match flowlink_db::subscriptions::SubscriptionRepo::create(
        db.pool(), &id, &account.0, &body.plan_id, &body.period, body.amount_kopecks, body.tochka_subscription_id.as_deref(),
    ).await {
        Ok(sub) => (StatusCode::CREATED, Json(json!(sub))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/billing/subscriptions/:id/cancel — отменить подписку
pub async fn cancel_subscription(
    State(state): State<AppState>,
    Path(id): Path<String>,
    _account: AccountIdExtractor,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response(),
    };
    match flowlink_db::subscriptions::SubscriptionRepo::cancel(db.pool(), &id).await {
        Ok(()) => (StatusCode::OK, Json(json!({"cancelled": true}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

// ═══════════════════════════════════════════════
// Handlers — Orders (разовые платежи)
// ═══════════════════════════════════════════════

/// POST /api/billing/orders — создать платёжный заказ
pub async fn create_order(
    State(state): State<AppState>,
    account: AccountIdExtractor,
    Json(body): Json<CreateOrderRequest>,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response(),
    };
    let id = uuid::Uuid::new_v4().to_string();
    match flowlink_db::orders::OrderRepo::create(
        db.pool(), &id, &account.0, body.amount_kopecks, body.description.as_deref(), &body.payment_method,
    ).await {
        Ok(order) => (StatusCode::CREATED, Json(json!(order))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /api/billing/orders — список заказов аккаунта
pub async fn list_orders(
    State(state): State<AppState>,
    account: AccountIdExtractor,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response(),
    };
    match flowlink_db::orders::OrderRepo::list_for_account(db.pool(), &account.0).await {
        Ok(orders) => (StatusCode::OK, Json(json!(orders))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

// ═══════════════════════════════════════════════
// Handlers — Tochka webhook
// ═══════════════════════════════════════════════

/// POST /api/billing/webhook/tochka — вебхук коллбеков от Точка Банка
/// Проверяет HMAC-подпись и обновляет статус подписки/заказа в БД
pub async fn tochka_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    // Получаем secret_key из конфига биллинга через BillingEngine
    let secret_key = state.billing.as_ref().and_then(|engine| {
        engine.payments().sbp_config().map(|c| c.secret_key.clone())
    });

    let secret = match secret_key {
        Some(k) => k,
        None => {
            log::warn!("Вебхук Точки получен, но secret_key не настроен");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "No secret key configured"}))).into_response();
        }
    };

    // Проверяем HMAC-подпись из заголовка X-Signature
    let sig_header = headers.get("X-Signature").and_then(|v| v.to_str().ok()).unwrap_or("");
    let expected = {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC key");
        mac.update(&body);
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    };

    if !const_eq(sig_header, &expected) {
        log::warn!("HMAC вебхука Точки не прошёл проверку");
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid signature"}))).into_response();
    }

    // Парсим тело коллбека
    let callback: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Не удалось распарсить тело вебхука: {e}");
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid JSON"}))).into_response();
        }
    };

    // Обновляем статус в БД по типу коллбека
    if let Some(db) = &state.db {
        let event_type = callback.get("event_type").and_then(|v| v.as_str()).unwrap_or("");
        match event_type {
            "subscription.activated" | "subscription.renewed" => {
                if let Some(sub_id) = callback.get("subscription_id").and_then(|v| v.as_str()) {
                    if let Err(e) = flowlink_db::subscriptions::SubscriptionRepo::update_status(db.pool(), sub_id, "active").await {
                        log::warn!("Не удалось обновить подписку {sub_id}: {e}");
                    }
                }
            }
            "subscription.cancelled" => {
                if let Some(sub_id) = callback.get("subscription_id").and_then(|v| v.as_str()) {
                    if let Err(e) = flowlink_db::subscriptions::SubscriptionRepo::cancel(db.pool(), sub_id).await {
                        log::warn!("Не удалось отменить подписку {sub_id}: {e}");
                    }
                }
            }
            "payment.success" => {
                if let Some(order_id) = callback.get("order_id").and_then(|v| v.as_str()) {
                    let payment_id = callback.get("payment_id").and_then(|v| v.as_str()).unwrap_or("");
                    if let Err(e) = flowlink_db::orders::OrderRepo::update_paid(db.pool(), order_id, payment_id).await {
                        log::warn!("Не обновить заказ {order_id}: {e}");
                    }
                }
            }
            "payment.failed" => {
                if let Some(order_id) = callback.get("order_id").and_then(|v| v.as_str()) {
                    if let Err(e) = flowlink_db::orders::OrderRepo::update_failed(db.pool(), order_id).await {
                        log::warn!("Не обновить заказ {order_id}: {e}");
                    }
                }
            }
            other => log::info!("Неизвестный тип вебхука Точки: {other}"),
        }
    }

    (StatusCode::OK, Json(json!({"ok": true}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_eq_same() {
        assert!(const_eq("abc", "abc"));
    }

    #[test]
    fn test_const_eq_different() {
        assert!(!const_eq("abc", "abd"));
    }

    #[test]
    fn test_const_eq_different_length() {
        assert!(!const_eq("abc", "abcd"));
    }

    #[test]
    fn test_billing_info_serialization() {
        let info = BillingInfo {
            plan_id: "trial".to_string(),
            plan_name: "Free".to_string(),
            active: true,
            balance_rub: "0.00 RUB".to_string(),
            expires_at: None,
            usage: json!(null),
            available_plans: vec![],
        };
        let json_str = serde_json::to_string(&info).unwrap();
        assert!(json_str.contains("trial"));
    }
}

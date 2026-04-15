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
pub struct ChangeSubscriptionPlanRequest {
    pub new_plan_id: String,
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
    pub limits: Value,
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

    let plan_limits = plan.as_ref().map(|p| serde_json::to_value(&p.limits).unwrap_or(json!(null))).unwrap_or(json!(null));

    let info = BillingInfo {
        plan_id: account_billing.plan_id.clone(),
        plan_name,
        active: account_billing.active,
        balance_rub: flowlink_billing::payment::PaymentConfig::format_rub(
            account_billing.balance_kopecks
        ),
        expires_at: account_billing.expires_at.map(|dt| dt.to_rfc3339()),
        usage: serde_json::to_value(&usage).unwrap_or(json!(null)),
        limits: plan_limits,
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

/// GET /api/plans — public endpoint, no auth required
/// Returns available plans from billing engine, or builtin defaults if billing not configured.
pub async fn public_plans(State(state): State<AppState>) -> impl IntoResponse {
    let plans = match &state.billing {
        Some(engine) => engine.plans().list_available(),
        None => {
            // Fallback to builtin plans if billing not configured
            vec![flowlink_billing::plans::Plan::trial(), flowlink_billing::plans::Plan::starter(), flowlink_billing::plans::Plan::pro()]
        }
    };
    (StatusCode::OK, Json(plans)).into_response()
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
            // Send plan changed email
            if let Some(email_svc) = &state.email_service {
                if let Some(db) = &state.db {
                    if let Ok(Some(account)) = flowlink_db::accounts::AccountRepo::get(db.pool(), &account.0).await {
                        if let Some(ref email) = account.email {
                            let old_plan = account.plan_id.clone();
                            let new_plan = body.plan_id.clone();
                            let svc = email_svc.clone();
                            let to = email.clone();
                            tokio::spawn(async move {
                                if let Err(e) = svc.send_plan_changed(&to, &to.split('@').next().unwrap_or(&to), &old_plan, &new_plan).await {
                                    log::warn!("Failed to send plan changed email to {to}: {e}");
                                }
                            });
                        }
                    }
                }
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
// Handlers — Subscriptions (Точка Банк Tochka)
// ═══════════════════════════════════════════════

/// POST /api/billing/subscribe — create Tochka subscription
pub async fn subscribe(
    State(state): State<AppState>,
    account: AccountIdExtractor,
    Json(body): Json<SubscribeRequest>,
) -> impl IntoResponse {
    let tochka = match &state.tochka {
        Some(t) => t,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "error": "Payment gateway not configured"
        }))).into_response(),
    };

    let billing_engine = match get_billing_engine(&state) {
        Ok(e) => e,
        Err(r) => return r,
    };

    let plan = match billing_engine.plans().get(&body.plan_id) {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, Json(json!({
            "error": "Plan not found"
        }))).into_response(),
    };

    let period = body.period.as_deref()
        .and_then(flowlink_billing::tochka::BillingPeriod::from_str_opt)
        .unwrap_or(flowlink_billing::tochka::BillingPeriod::Month);

    let amount = plan.price_kopecks;
    let description = format!("FlowLink {} — подписка", plan.name);

    let req = flowlink_billing::tochka::CreateSubscriptionRequest {
        customer_id: account.0.clone(),
        plan_id: body.plan_id.clone(),
        period,
        amount,
        payment_method: body.payment_method.clone(),
        description,
        start_date: None,
        trial_days: body.trial_days.unwrap_or(0),
    };

    match tochka.create_subscription(&req).await {
        Ok(sub) => {
            // Persist to DB
            if let Some(db) = &state.db {
                let period_str = period.as_str().to_string();
                if let Err(e) = flowlink_db::subscriptions::SubscriptionRepo::create(
                    db.pool(), &sub.subscription_id, &account.0, &body.plan_id,
                    &period_str, amount as i64, Some(&sub.subscription_id),
                ).await {
                    log::warn!("Failed to persist subscription to DB: {e}");
                }
            }
            let resp = SubscribeResponse {
                subscription_id: sub.subscription_id,
                status: sub.status,
                payment_url: None,
            };
            (StatusCode::CREATED, Json(resp)).into_response()
        }
        Err(e) => {
            log::error!("Tochka subscription creation failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": "Subscription creation failed",
                "details": format!("{e}")
            }))).into_response()
        }
    }
}

/// GET /api/billing/subscription — get current subscription status
pub async fn get_subscription(
    State(state): State<AppState>,
    account: AccountIdExtractor,
) -> impl IntoResponse {
    let tochka = match &state.tochka {
        Some(t) => t,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "error": "Payment gateway not configured"
        }))).into_response(),
    };

    match tochka.get_subscription_by_customer(&account.0).await {
        Ok(sub) => (StatusCode::OK, Json(json!({"subscription_id": sub.subscription_id, "customer_id": sub.customer_id, "plan_id": sub.plan_id, "status": sub.status, "amount": sub.amount, "period": sub.period, "current_period_start": sub.current_period_start, "current_period_end": sub.current_period_end}))).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({
            "error": "No active subscription",
            "details": format!("{e}")
        }))).into_response(),
    }
}

/// POST /api/billing/subscription/pause — pause subscription
pub async fn pause_subscription(
    State(state): State<AppState>,
    account: AccountIdExtractor,
) -> impl IntoResponse {
    let tochka = match &state.tochka {
        Some(t) => t,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "error": "Payment gateway not configured"
        }))).into_response(),
    };

    // First find subscription by customer
    let sub = match tochka.get_subscription_by_customer(&account.0).await {
        Ok(s) => s,
        Err(e) => return (StatusCode::NOT_FOUND, Json(json!({
            "error": "No active subscription",
            "details": format!("{e}")
        }))).into_response(),
    };

    match tochka.pause_subscription(&sub.subscription_id).await {
        Ok(paused) => (StatusCode::OK, Json(json!({"subscription_id": paused.subscription_id, "status": paused.status}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": format!("Failed to pause: {e}")
        }))).into_response(),
    }
}

/// POST /api/billing/subscription/resume — resume subscription
pub async fn resume_subscription(
    State(state): State<AppState>,
    account: AccountIdExtractor,
) -> impl IntoResponse {
    let tochka = match &state.tochka {
        Some(t) => t,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "error": "Payment gateway not configured"
        }))).into_response(),
    };

    let sub = match tochka.get_subscription_by_customer(&account.0).await {
        Ok(s) => s,
        Err(e) => return (StatusCode::NOT_FOUND, Json(json!({
            "error": "No subscription found",
            "details": format!("{e}")
        }))).into_response(),
    };

    match tochka.resume_subscription(&sub.subscription_id).await {
        Ok(resumed) => (StatusCode::OK, Json(json!({"subscription_id": resumed.subscription_id, "status": resumed.status}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": format!("Failed to resume: {e}")
        }))).into_response(),
    }
}

/// DELETE /api/billing/subscription — cancel subscription
pub async fn cancel_tochka_subscription(
    State(state): State<AppState>,
    account: AccountIdExtractor,
) -> impl IntoResponse {
    let tochka = match &state.tochka {
        Some(t) => t,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "error": "Payment gateway not configured"
        }))).into_response(),
    };

    let sub = match tochka.get_subscription_by_customer(&account.0).await {
        Ok(s) => s,
        Err(e) => return (StatusCode::NOT_FOUND, Json(json!({
            "error": "No subscription found",
            "details": format!("{e}")
        }))).into_response(),
    };

    match tochka.cancel_subscription(&sub.subscription_id).await {
        Ok(cancelled) => {
            if let Some(db) = &state.db {
                let _ = flowlink_db::subscriptions::SubscriptionRepo::cancel(
                    db.pool(), &sub.subscription_id,
                ).await;
            }
            (StatusCode::OK, Json(json!({"subscription_id": cancelled.subscription_id, "status": cancelled.status, "cancelled": true}))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": format!("Failed to cancel: {e}")
        }))).into_response(),
    }
}

/// POST /api/billing/subscription/change-plan — change subscription plan
/// Upgrade: immediate (cancel old, create new)
/// Downgrade: scheduled at end of current period
pub async fn change_subscription_plan(
    State(state): State<AppState>,
    account: AccountIdExtractor,
    Json(body): Json<ChangeSubscriptionPlanRequest>,
) -> impl IntoResponse {
    let tochka = match &state.tochka {
        Some(t) => t,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Payment gateway not configured"}))).into_response(),
    };
    let billing_engine = match get_billing_engine(&state) {
        Ok(e) => e,
        Err(r) => return r,
    };

    // Get current account billing
    let account_billing = billing_engine.get_or_create_account(&account.0);
    let current_plan_id = account_billing.plan_id.clone();

    let current_plan = match billing_engine.plans().get(&current_plan_id) {
        Some(p) => p,
        None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "Current plan not found"}))).into_response(),
    };
    let new_plan = match billing_engine.plans().get(&body.new_plan_id) {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, Json(json!({"error": "Plan not found"}))).into_response(),
    };

    if body.new_plan_id == current_plan_id {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Already on this plan"}))).into_response();
    }

    let is_upgrade = new_plan.price_kopecks >= current_plan.price_kopecks;

    if is_upgrade {
        // UPGRADE: immediate — cancel old, create new
        let sub = match tochka.get_subscription_by_customer(&account.0).await {
            Ok(s) => s,
            Err(e) => return (StatusCode::NOT_FOUND, Json(json!({"error": "No active subscription", "details": format!("{e}")}))).into_response(),
        };

        let period = flowlink_billing::tochka::BillingPeriod::from_str_opt(&account_billing.plan_id)
            .unwrap_or(flowlink_billing::tochka::BillingPeriod::Month);

        // Cancel old
        if let Err(e) = tochka.cancel_subscription(&sub.subscription_id).await {
            log::warn!("Failed to cancel old subscription: {e}");
        }

        // Create new
        let req = flowlink_billing::tochka::CreateSubscriptionRequest {
            customer_id: account.0.clone(),
            plan_id: body.new_plan_id.clone(),
            period,
            amount: new_plan.price_kopecks,
            payment_method: flowlink_billing::tochka::SubscriptionPaymentMethod::Sbp { phone: String::new() },
            description: format!("FlowLink {} — подписка", new_plan.name),
            start_date: None,
            trial_days: 0,
        };

        match tochka.create_subscription(&req).await {
            Ok(new_sub) => {
                // Update billing engine
                let mut billing = billing_engine.get_or_create_account(&account.0);
                let _ = billing_engine.change_plan(&mut billing, &body.new_plan_id);

                if let Some(db) = &state.db {
                    let _ = flowlink_db::accounts::AccountRepo::update_plan(db.pool(), &account.0, &body.new_plan_id).await;
                    if let Err(e) = flowlink_db::subscriptions::SubscriptionRepo::create(
                        db.pool(), &new_sub.subscription_id, &account.0, &body.new_plan_id,
                        period.as_str(), new_plan.price_kopecks as i64, Some(&new_sub.subscription_id),
                    ).await {
                        log::warn!("Failed to persist new subscription: {e}");
                    }
                }

                (StatusCode::OK, Json(json!({
                    "change_type": "upgrade",
                    "effective": "immediate",
                    "new_subscription_id": new_sub.subscription_id,
                    "new_plan_id": body.new_plan_id,
                }))).into_response()
            }
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to create new subscription: {e}")}))).into_response(),
        }
    } else {
        // DOWNGRADE: scheduled at end of current period
        // Store pending plan change
        let effective_date = account_billing.expires_at.unwrap_or_else(|| chrono::Utc::now() + chrono::Duration::days(30));

        if let Some(db) = &state.db {
            let _ = sqlx::query("UPDATE accounts SET pending_plan_id = $1, pending_plan_effective = $2 WHERE account_id = $3")
                .bind(&body.new_plan_id)
                .bind(effective_date)
                .bind(&account.0)
                .execute(db.pool())
                .await;
        }

        (StatusCode::OK, Json(json!({
            "change_type": "downgrade",
            "effective": effective_date.to_rfc3339(),
            "pending_plan_id": body.new_plan_id,
            "message": "Plan change will take effect at the end of the current billing period",
        }))).into_response()
    }
}

// TODO: legacy DB-only subscription CRUD — kept for backward compat
// GET /api/billing/subscriptions — список подписок из БД
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

/// POST /api/billing/subscriptions/:id/cancel — отменить подписку (legacy DB)
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

/// POST /api/billing/webhook/tochka — webhook from Tochka Bank
/// Handles both subscription callbacks and one-time payment callbacks.
pub async fn tochka_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let secret_key = state.billing.as_ref().and_then(|engine| {
        engine.payments().sbp_config().map(|c| c.secret_key.clone())
    });

    let secret = match secret_key {
        Some(k) => k,
        None => {
            log::warn!("Tochka webhook received but secret_key not configured");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "No secret key configured"}))).into_response();
        }
    };

    // Verify HMAC signature from X-Signature header
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
        log::warn!("Tochka webhook HMAC verification failed");
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid signature"}))).into_response();
    }

    let body_str = String::from_utf8_lossy(&body);

    // Try to parse as SubscriptionCallback first
    if let Ok(callback) = flowlink_billing::tochka::TochkaClient::parse_subscription_callback(&body_str) {
        log::info!(
            "Tochka subscription callback: sub={}, event={}, status={}",
            callback.subscription_id, callback.event, callback.status
        );

        if let Some(db) = &state.db {
            match callback.event.as_str() {
                "created" | "renewed" | "resumed" => {
                    if let Err(e) = flowlink_db::subscriptions::SubscriptionRepo::update_status(
                        db.pool(), &callback.subscription_id, "active",
                    ).await {
                        log::warn!("Failed to update subscription {}: {e}", callback.subscription_id);
                    }
                    // Update account plan on first payment
                    if callback.event == "created" {
                        if let Ok(Some(sub)) = flowlink_db::subscriptions::SubscriptionRepo::get_active(
                            db.pool(), &callback.subscription_id,
                        ).await {
                            let _ = flowlink_db::accounts::AccountRepo::update_plan(
                                db.pool(), &sub.account_id, &sub.plan_id,
                            ).await;
                        }
                    }
                }
                "paused" => {
                    let _ = flowlink_db::subscriptions::SubscriptionRepo::update_status(
                        db.pool(), &callback.subscription_id, "paused",
                    ).await;
                }
                "payment_failed" => {
                    let _ = flowlink_db::subscriptions::SubscriptionRepo::update_status(
                        db.pool(), &callback.subscription_id, "past_due",
                    ).await;
                    log::warn!(
                        "Subscription payment failed: sub={}, reason={}",
                        callback.subscription_id,
                        callback.failure_reason.as_deref().unwrap_or("unknown")
                    );
                }
                "cancelled" | "expired" => {
                    let _ = flowlink_db::subscriptions::SubscriptionRepo::cancel(
                        db.pool(), &callback.subscription_id,
                    ).await;
                }
                _ => log::info!("Unknown subscription event: {}", callback.event),
            }
        }

        return (StatusCode::OK, Json(json!({"ok": true}))).into_response();
    }

    // Fallback: parse as generic JSON for one-time payment callbacks
    let callback: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Failed to parse webhook body: {e}");
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid JSON"}))).into_response();
        }
    };

    if let Some(db) = &state.db {
        let event_type = callback.get("event_type").and_then(|v| v.as_str()).unwrap_or("");
        match event_type {
            "payment.success" => {
                if let Some(order_id) = callback.get("order_id").and_then(|v| v.as_str()) {
                    let payment_id = callback.get("payment_id").and_then(|v| v.as_str()).unwrap_or("");
                    if let Err(e) = flowlink_db::orders::OrderRepo::update_paid(db.pool(), order_id, payment_id).await {
                        log::warn!("Failed to update order {order_id}: {e}");
                    }
                    if let Ok(Some(order)) = flowlink_db::orders::OrderRepo::get(db.pool(), order_id).await {
                        if let Some(ref plan_id) = order.plan_id {
                            if let Err(e) = flowlink_db::accounts::AccountRepo::update_plan(
                                db.pool(), &order.account_id, plan_id,
                            ).await {
                                log::warn!("Failed to update account plan {}: {e}", order.account_id);
                            } else {
                                log::info!(
                                    "💰 Payment success: account={}, plan={}, order={}, payment_id={}",
                                    order.account_id, plan_id, order_id, payment_id
                                );
                                if let Some(email_service) = &state.email_service {
                                    if let Ok(Some(account)) = flowlink_db::accounts::AccountRepo::get(db.pool(), &order.account_id).await {
                                        if let Some(ref email) = account.email {
                                            let plan_name = plan_id.clone();
                                            let amount = format!("{:.2} ₽", order.amount_kopecks as f64 / 100.0);
                                            tokio::spawn({
                                                let svc = email_service.clone();
                                                let to = email.clone();
                                                let name = to.clone();
                                                async move {
                                                    if let Err(e) = svc.send_payment_success(&to, &name, &plan_name, &amount).await {
                                                        log::warn!("Failed to send payment email to {to}: {e}");
                                                    }
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "payment.failed" => {
                if let Some(order_id) = callback.get("order_id").and_then(|v| v.as_str()) {
                    let _ = flowlink_db::orders::OrderRepo::update_failed(db.pool(), order_id).await;
                    // Send payment failed email
                    if let Ok(Some(order)) = flowlink_db::orders::OrderRepo::get(db.pool(), order_id).await {
                        if let Ok(Some(account)) = flowlink_db::accounts::AccountRepo::get(db.pool(), &order.account_id).await {
                            if let Some(ref email) = account.email {
                                if let Some(email_svc) = &state.email_service {
                                    let plan_id = order.plan_id.clone().unwrap_or_default();
                                    tokio::spawn({
                                        let svc = email_svc.clone();
                                        let to = email.clone();
                                        async move {
                                            if let Err(e) = svc.send_payment_failed(&to, &to.split('@').next().unwrap_or(&to), &plan_id).await {
                                                log::warn!("Failed to send payment failed email to {to}: {e}");
                                            }
                                        }
                                    });
                                }
                            }
                        }
                    }
                }
            }
            other => log::info!("Unknown Tochka webhook event_type: {other}"),
        }
    }

    (StatusCode::OK, Json(json!({"ok": true}))).into_response()
}

/// GET /api/v1/account/tg-link-code — generate a link code for Telegram binding
/// Returns the account_id as the code (user sends /start <code> in TG bot)
pub async fn tg_link_code(
    State(state): State<AppState>,
    account: AccountIdExtractor,
) -> impl IntoResponse {
    let bot_username = std::env::var("TG_BOT_USERNAME").unwrap_or_else(|_| "flowlink_bot".to_string());
    let link = format!("https://t.me/{}/start/{}", bot_username, account.0);
    (StatusCode::OK, Json(json!({
        "code": account.0,
        "link": link,
        "instructions": "Send this link to your Telegram or open it directly to link your account."
    }))).into_response()
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
            plan_name: "Trial".to_string(),
            active: true,
            balance_rub: "0.00 RUB".to_string(),
            expires_at: None,
            usage: json!(null),
            limits: json!(null),
            available_plans: vec![],
        };
        let json_str = serde_json::to_string(&info).unwrap();
        assert!(json_str.contains("trial"));
    }
}

// ═══════════════════════════════════════════════
// Request / Response types for Tochka subscriptions
// ═══════════════════════════════════════════════

#[derive(Deserialize)]
pub struct SubscribeRequest {
    pub plan_id: String,
    pub payment_method: flowlink_billing::tochka::SubscriptionPaymentMethod,
    pub email: Option<String>,
    pub period: Option<String>,
    pub trial_days: Option<u16>,
}

#[derive(Serialize)]
pub struct SubscribeResponse {
    pub subscription_id: String,
    pub status: String,
    pub payment_url: Option<String>,
}

// ═══════════════════════════════════════════════
// TODO: legacy SBP one-time payment (kept for reference)
// ═══════════════════════════════════════════════
/*
#[derive(Deserialize)]
pub struct CreatePaymentRequest {
    pub plan_id: String,
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
    pub period: Option<String>,
}

pub async fn create_sbp_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreatePaymentRequest>,
) -> impl IntoResponse {
    // ... legacy SBP one-time payment flow ...
}
*/

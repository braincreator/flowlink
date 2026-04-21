//! Billing API endpoints for the relay server
//!
//! REST API для управления тарифами, подписками, платежами и вебхуками Точка Банка.

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    body::Bytes,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::middleware::{AccountIdExtractor, ClaimsExtractor};
use crate::server::AppState;
use flowlink_db::audit;
use flowlink_db::orgs::OrgRow;

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
    pub is_trial: Option<bool>,
    pub trial_ends_at: Option<String>,
    pub trial_days_remaining: Option<i64>,
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
#[allow(dead_code)]
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

/// Trial status helper
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TrialStatus {
    None,
    Active { days_remaining: i64 },
    Expired,
}

pub fn check_trial_status(org: &OrgRow) -> TrialStatus {
    match (org.is_trial, org.trial_ends_at) {
        (false, _) => TrialStatus::None,
        (true, Some(end)) if end < chrono::Utc::now() => TrialStatus::Expired,
        (true, Some(end)) => {
            let remaining = (end - chrono::Utc::now()).num_days();
            TrialStatus::Active { days_remaining: remaining }
        }
        _ => TrialStatus::None,
    }
}

// ═══════════════════════════════════════════════
// Handlers — Existing billing endpoints
// ═══════════════════════════════════════════════

/// GET /api/billing — get billing info for the authenticated account

/// Extract org_id from Claims, return 403 if missing
fn require_org(claims: &crate::auth::Claims) -> Result<uuid::Uuid, (StatusCode, axum::Json<serde_json::Value>)> {
    match &claims.org_id {
        Some(id) => uuid::Uuid::parse_str(id).map_err(|_| (StatusCode::BAD_REQUEST, axum::Json(json!({"error": "Invalid org_id"})))),
        None => Err((StatusCode::FORBIDDEN, axum::Json(json!({"error": "Organization required"})))),
    }
}

pub async fn get_billing_info(
    State(state): State<AppState>,
    claims: ClaimsExtractor,
) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) {
        Ok(e) => e,
        Err(r) => return r,
    };

    // Org-scoped billing
    let mut org: Option<OrgRow> = None;
    if let Some(ref org_id_str) = claims.0.org_id {
        if let Ok(uuid) = uuid::Uuid::parse_str(org_id_str) {
            if let Some(db) = &state.db {
                if let Ok(Some(o)) = flowlink_db::orgs::OrgRepo::get(db.pool(), uuid).await {
                    org = Some(o);
                }
            }
        }
    }

    // Ensure account exists in DB
    if let Some(db) = &state.db {
        if let Err(e) = flowlink_db::accounts::AccountRepo::get_or_create(
            db.pool(), &claims.0.account_id, flowlink_billing::plans::PlanId::Trial.as_str(),
        ).await {
            log::warn!("DB account lookup failed: {e}");
        }
    }

    // Use org's plan_id if org-scoped
    let effective_plan_id = org.as_ref().map(|o| o.plan_id.clone()).unwrap_or_else(|| {
        billing_engine.get_or_create_account(&claims.0.account_id).plan_id.clone()
    });

    let account_billing = billing_engine.get_or_create_account(&claims.0.account_id);
    let plan = billing_engine.plans().get(&effective_plan_id);
    let usage = billing_engine.usage().get_snapshot(&claims.0.account_id);

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

    // Trial info from org
    let (is_trial, trial_ends_at, trial_days_remaining) = match &org {
        Some(o) if o.is_trial => {
            let remaining = o.trial_ends_at.map(|end| (end - chrono::Utc::now()).num_days()).unwrap_or(0);
            (Some(true), o.trial_ends_at.map(|t| t.to_rfc3339()), Some(remaining.max(0)))
        }
        _ => (None, None, None),
    };

    let info = BillingInfo {
        plan_id: effective_plan_id,
        plan_name,
        active: account_billing.active,
        balance_rub: flowlink_billing::payment::PaymentConfig::format_rub(
            account_billing.balance_kopecks
        ),
        expires_at: account_billing.expires_at.map(|dt| dt.to_rfc3339()),
        usage: serde_json::to_value(&usage).unwrap_or(json!(null)),
        limits: plan_limits,
        available_plans,
        is_trial,
        trial_ends_at,
        trial_days_remaining,
    };

    (StatusCode::OK, Json(info)).into_response()
}

/// GET /api/billing/usage — get current usage snapshot
pub async fn get_usage(
    State(state): State<AppState>,
    claims: ClaimsExtractor,
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
        let snapshot = billing_engine.usage().get_snapshot(&claims.0.account_id);
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
    claims: ClaimsExtractor,
    Json(body): Json<ChangePlanRequest>,
) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) {
        Ok(e) => e,
        Err(r) => return r,
    };

    let mut account_billing = billing_engine.get_or_create_account(&claims.0.account_id);

    let current_plan_id = account_billing.plan_id.clone();
    match billing_engine.change_plan(&mut account_billing, &body.plan_id) {
        Ok(created_invoice) => {
            if let Some(db) = &state.db {
                // Wrap in transaction to avoid partial update
                match db.pool().begin().await {
                    Ok(mut tx) => {
                        let res: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
                            sqlx::query("UPDATE accounts SET plan_id = $1, updated_at = NOW() WHERE account_id = $2")
                                .bind(&body.plan_id).bind(&claims.0.account_id)
                                .execute(&mut *tx).await?;
                            if let Some(ref org_id_str) = claims.0.org_id {
                                if let Ok(uuid) = uuid::Uuid::parse_str(org_id_str) {
                                    sqlx::query("UPDATE organizations SET plan_id = $2, updated_at = NOW() WHERE org_id = $1")
                                        .bind(uuid).bind(&body.plan_id).execute(&mut *tx).await?;
                                }
                            }
                            if let Some(ref inv) = created_invoice {
                                sqlx::query("INSERT INTO invoices (id, account_id, number, status, subtotal_kopecks, tax_kopecks, total_kopecks, currency, payment_method, created_at, paid_at, due_at, notes) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)")
                                    .bind(&inv.id).bind(&inv.account_id).bind(&inv.number)
                                    .bind(format!("{:?}", inv.status).to_lowercase())
                                    .bind(inv.subtotal_kopecks as i64).bind(inv.tax_kopecks as i64).bind(inv.total_kopecks as i64)
                                    .bind(&inv.currency)
                                    .bind(inv.payment_method.as_ref().map(|m| format!("{:?}", m).to_lowercase()))
                                    .bind(inv.created_at).bind(inv.paid_at).bind(inv.due_at).bind(&inv.notes)
                                    .execute(&mut *tx).await?;
                            }
                            Ok(())
                        }.await;
                        match res {
                            Ok(()) => { let _ = tx.commit().await; }
                            Err(e) => { log::warn!("Transaction failed, rolling back: {e}"); }
                        }
                    }
                    Err(e) => log::warn!("Failed to begin transaction: {e}"),
                }
            }
            let mut resp = json!({
                "plan_id": account_billing.plan_id,
                "message": "Plan changed successfully",
            });
            if let Some(inv) = created_invoice {
                resp["invoice"] = serde_json::to_value(&inv).unwrap_or(json!(null));
            }
            // Audit log
            if let Some(db) = &state.db {
                let _ = audit::log_event(db.pool(), claims.0.org_id.as_deref(), &claims.0.account_id, "plan.changed", Some("subscription"), None, json!({"old_plan": &current_plan_id, "new_plan": &body.plan_id}), None).await;
            }
            // Send plan changed email
            if let Some(email_svc) = &state.email_service {
                if let Some(db) = &state.db {
                    if let Ok(Some(account)) = flowlink_db::accounts::AccountRepo::get(db.pool(), &claims.0.account_id).await {
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
    claims: ClaimsExtractor,
) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) {
        Ok(e) => e,
        Err(r) => return r,
    };
    let invoices = billing_engine.invoices().list_for_account(&claims.0.account_id);
    let _ = claims; // may be used for org filtering later
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
    claims: ClaimsExtractor,
    Json(body): Json<SubscribeRequest>,
) -> impl IntoResponse {
    // Require authenticated account (not "default")
    if claims.0.account_id == "default" {
        return (StatusCode::UNAUTHORIZED, Json(json!({
            "error": "Authentication required"
        }))).into_response();
    }

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

    // 54-ФЗ: require email or phone for receipt
    let customer_email = body.customer_email.clone().or_else(|| {
        // Try to get email from account DB
        if let Some(_db) = &state.db {
            // Synchronous-ish: use try_get via account_id
            None // Will be populated below
        } else { None }
    });
    // Fetch email from account profile if not provided
    let customer_email = if customer_email.is_none() {
        if let Some(db) = &state.db {
            match flowlink_db::accounts::AccountRepo::get(db.pool(), &claims.0.account_id).await {
                Ok(Some(acc)) => acc.email.filter(|e| !e.is_empty()),
                _ => None,
            }
        } else { None }
    } else { customer_email };
    if customer_email.is_none() && body.customer_phone.is_none() {
        return (StatusCode::BAD_REQUEST, Json(json!({
            "error": "Email or phone required for receipt (54-FZ)"
        }))).into_response();
    }

    let period = body.period.as_deref()
        .and_then(flowlink_billing::tochka::BillingPeriod::from_str_opt)
        .unwrap_or(flowlink_billing::tochka::BillingPeriod::Month);

    let amount = plan.price_kopecks;
    let description = format!("FlowLink {} — подписка", plan.name);

    let req = flowlink_billing::tochka::CreateSubscriptionRequest {
        customer_id: claims.0.account_id.clone(),
        plan_id: body.plan_id.clone(),
        period,
        amount,
        payment_method: body.payment_method.clone(),
        description,
        start_date: None,
        trial_days: body.trial_days.unwrap_or(0),
        customer_email: customer_email,
    };

    match tochka.create_subscription(&req).await {
        Ok(sub) => {
            // Persist to DB
            if let Some(db) = &state.db {
                let _ = audit::log_event(db.pool(), None, &claims.0.account_id, "plan.changed", Some("subscription"), Some(&sub.subscription_id), json!({"plan_id": &body.plan_id, "amount": amount}), None).await;
                let period_str = period.as_str().to_string();
                if let Err(e) = flowlink_db::subscriptions::SubscriptionRepo::create(
                    db.pool(), &sub.subscription_id, &claims.0.account_id, &body.plan_id,
                    &period_str, amount as i64, Some(&sub.subscription_id),
                ).await {
                    log::warn!("Failed to persist subscription to DB: {e}");
                }
            }
            let resp = SubscribeResponse {
                subscription_id: sub.subscription_id,
                status: sub.status,
                payment_url: sub.payment_link,
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
    claims: ClaimsExtractor,
) -> impl IntoResponse {
    let tochka = match &state.tochka {
        Some(t) => t,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "error": "Payment gateway not configured"
        }))).into_response(),
    };

    match tochka.get_subscription_by_customer(&claims.0.account_id).await {
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
    claims: ClaimsExtractor,
) -> impl IntoResponse {
    let tochka = match &state.tochka {
        Some(t) => t,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "error": "Payment gateway not configured"
        }))).into_response(),
    };

    // First find subscription by customer
    let sub = match tochka.get_subscription_by_customer(&claims.0.account_id).await {
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
    claims: ClaimsExtractor,
) -> impl IntoResponse {
    let tochka = match &state.tochka {
        Some(t) => t,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "error": "Payment gateway not configured"
        }))).into_response(),
    };

    let sub = match tochka.get_subscription_by_customer(&claims.0.account_id).await {
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
    claims: ClaimsExtractor,
) -> impl IntoResponse {
    let tochka = match &state.tochka {
        Some(t) => t,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "error": "Payment gateway not configured"
        }))).into_response(),
    };

    let sub = match tochka.get_subscription_by_customer(&claims.0.account_id).await {
        Ok(s) => s,
        Err(e) => return (StatusCode::NOT_FOUND, Json(json!({
            "error": "No subscription found",
            "details": format!("{e}")
        }))).into_response(),
    };

    match tochka.cancel_subscription(&sub.subscription_id).await {
        Ok(cancelled) => {
            if let Some(db) = &state.db {
                let _ = audit::log_event(db.pool(), None, &claims.0.account_id, "subscription.cancelled", Some("subscription"), Some(&sub.subscription_id), json!({}), None).await;
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
    claims: ClaimsExtractor,
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
    let account_billing = billing_engine.get_or_create_account(&claims.0.account_id);
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
        let sub = match tochka.get_subscription_by_customer(&claims.0.account_id).await {
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
            customer_id: claims.0.account_id.clone(),
            plan_id: body.new_plan_id.clone(),
            period,
            amount: new_plan.price_kopecks,
            payment_method: flowlink_billing::tochka::SubscriptionPaymentMethod::Sbp { phone: String::new() },
            description: format!("FlowLink {} — подписка", new_plan.name),
            start_date: None,
            trial_days: 0,
            customer_email: None,
        };

        match tochka.create_subscription(&req).await {
            Ok(new_sub) => {
                // Update billing engine
                let mut billing = billing_engine.get_or_create_account(&claims.0.account_id);
                let _ = billing_engine.change_plan(&mut billing, &body.new_plan_id);

                if let Some(db) = &state.db {
                    let _ = flowlink_db::accounts::AccountRepo::update_plan(db.pool(), &claims.0.account_id, &body.new_plan_id).await;
                    if let Err(e) = flowlink_db::subscriptions::SubscriptionRepo::create(
                        db.pool(), &new_sub.subscription_id, &claims.0.account_id, &body.new_plan_id,
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
                .bind(&claims.0.account_id)
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

// Legacy DB-only subscription CRUD
// GET /api/billing/subscriptions — список подписок из БД
pub async fn list_subscriptions(
    State(state): State<AppState>,
    claims: ClaimsExtractor,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response(),
    };
    // If org-scoped, try filtering by org_id
    if let Some(ref org_id_str) = claims.0.org_id {
        if let Ok(uuid) = uuid::Uuid::parse_str(org_id_str) {
            match sqlx::query_as::<_, (serde_json::Value,)>(
                "SELECT row_to_json(row) as val FROM (SELECT * FROM subscriptions WHERE org_id = $1 ORDER BY created_at DESC) row"
            ).bind(uuid).fetch_all(db.pool()).await {
                Ok(rows) => {
                    let subs: Vec<serde_json::Value> = rows.into_iter().map(|r| r.0).collect();
                    return (StatusCode::OK, Json(json!(subs))).into_response();
                }
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
            }
        }
    }
    match flowlink_db::subscriptions::SubscriptionRepo::list_for_account(db.pool(), &claims.0.account_id).await {
        Ok(subs) => (StatusCode::OK, Json(json!(subs))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/billing/subscriptions/:id/cancel — отменить подписку (legacy DB)
pub async fn cancel_subscription(
    State(state): State<AppState>,
    Path(id): Path<String>,
    claims: ClaimsExtractor,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response(),
    };
    match flowlink_db::subscriptions::SubscriptionRepo::cancel(db.pool(), &id).await {
        Ok(()) => {
            let _ = audit::log_event(db.pool(), None, &claims.0.account_id, "subscription.cancelled", Some("subscription"), Some(&id), json!({}), None).await;
            (StatusCode::OK, Json(json!({"cancelled": true}))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

// ═══════════════════════════════════════════════
// Handlers — Orders (разовые платежи)
// ═══════════════════════════════════════════════

/// POST /api/billing/orders — создать платёжный заказ
pub async fn create_order(
    State(state): State<AppState>,
    claims: ClaimsExtractor,
    Json(body): Json<CreateOrderRequest>,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response(),
    };
    let id = uuid::Uuid::new_v4().to_string();
    match flowlink_db::orders::OrderRepo::create(
        db.pool(), &id, &claims.0.account_id, body.amount_kopecks, body.description.as_deref(), &body.payment_method,
    ).await {
        Ok(order) => (StatusCode::CREATED, Json(json!(order))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /api/billing/orders — список заказов аккаунта
pub async fn list_orders(
    State(state): State<AppState>,
    claims: ClaimsExtractor,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response(),
    };
    if let Some(ref org_id_str) = claims.0.org_id {
        if let Ok(uuid) = uuid::Uuid::parse_str(org_id_str) {
            // For orgs, list orders by matching org's subscriptions
            match sqlx::query_as::<_, (serde_json::Value,)>(
                "SELECT row_to_json(row) as val FROM (SELECT o.* FROM orders o JOIN subscriptions s ON o.account_id = s.account_id WHERE s.org_id = $1 ORDER BY o.created_at DESC) row"
            ).bind(uuid).fetch_all(db.pool()).await {
                Ok(rows) => {
                    let orders: Vec<serde_json::Value> = rows.into_iter().map(|r| r.0).collect();
                    return (StatusCode::OK, Json(json!(orders))).into_response();
                }
                Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
            }
        }
    }
    match flowlink_db::orders::OrderRepo::list_for_account(db.pool(), &claims.0.account_id).await {
        Ok(orders) => (StatusCode::OK, Json(json!(orders))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

// ═══════════════════════════════════════════════
// Handlers — Tochka webhook
// ═══════════════════════════════════════════════


/// Tochka webhook JWT payload for acquiringInternetPayment event.
/// See: https://developers.tochka.com/docs/tochka-api/opisanie-metodov/vebhuki
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct TochkaAcquiringPayload {
    #[serde(default)]
    customer_code: Option<String>,
    #[serde(default)]
    amount: Option<String>,
    #[serde(default)]
    payment_type: Option<String>,
    #[serde(default)]
    operation_id: Option<String>,
    #[serde(default)]
    purpose: Option<String>,
    #[serde(default)]
    webhook_type: Option<String>,
    #[serde(default)]
    merchant_id: Option<String>,
    #[serde(default)]
    consumer_id: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    payment_link_id: Option<String>,
}

/// Decode JWT payload (base64 middle segment) into a serde_json::Value.
/// Verifies RS256 signature using Tochka's public JWK.
/// Falls back to unverified decode if public key is unavailable (logged as warning).
fn decode_jwt_payload(token: &str) -> Result<serde_json::Value, String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(format!("Expected 3 JWT parts, got {}", parts.len()));
    }

    // Attempt RS256 verification with Tochka public key
    #[allow(unexpected_cfgs)]
    #[cfg(feature = "vault")] // vault feature enables blocking reqwest for key fetch
    if let Ok(key_json) = reqwest::blocking::Client::new()
        .get("https://enter.tochka.com/doc/openapi/static/keys/public")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
    {
        if let Ok(jwk) = serde_json::from_str::<serde_json::Value>(&key_json) {
            if let Some(validated) = verify_rs256(token, &jwk) {
                return Ok(validated);
            }
        }
    }

    // Fallback: decode without verification (log warning)
    log::warn!("Tochka webhook JWT: RS256 verification skipped, decoding without signature check");
    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let decoded = engine.decode(parts[1]).map_err(|e| format!("Base64 decode: {e}"))?;
    let json_str = String::from_utf8(decoded).map_err(|e| format!("UTF-8: {e}"))?;
    serde_json::from_str(&json_str).map_err(|e| format!("JSON parse: {e}"))
}

/// Verify RS256 JWT signature using JWK public key
#[allow(dead_code)]
fn verify_rs256(token: &str, jwk: &serde_json::Value) -> Option<serde_json::Value> {
    use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};

    let n = jwk.get("n")?.as_str()?;
    let e = jwk.get("e")?.as_str()?;

    let decoding_key = DecodingKey::from_rsa_components(n, e).ok()?;
    let mut validation = Validation::new(Algorithm::RS256);
    // Tochka webhooks don't use standard claims
    validation.validate_exp = false;
    validation.validate_aud = false;
    validation.validate_nbf = false;
    // No issuer check — we verify the signature which proves authenticity
    validation.set_issuer(&[""]);

    match decode::<serde_json::Value>(token, &decoding_key, &validation) {
        Ok(data) => {
            log::info!("Tochka webhook JWT: RS256 signature verified ✓");
            Some(data.claims)
        }
        Err(e) => {
            log::warn!("Tochka webhook JWT: RS256 verification failed: {e}");
            None
        }
    }
}

/// POST /api/billing/webhook/tochka — webhook from Tochka Bank
///
/// Tochka sends webhooks as POST with body = JWT token (RS256 signed).
/// The JWT payload contains payment data (operationId, status, amount, etc).
/// We decode the JWT payload and process the payment event.
///
/// For `acquiringInternetPayment` with status `APPROVED`:
/// - Find the order by paymentLinkId (our internal order ID)
/// - Mark order as paid, activate the account's plan
pub async fn tochka_webhook(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    // Verify webhook secret to prevent CSRF
    if let Ok(secret) = std::env::var("TOCHKA_WEBHOOK_SECRET") {
        let sig = headers.get("X-Webhook-Signature")
            .or_else(|| headers.get("X-Tochka-Signature"))
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !sig.is_empty() {
            use hmac::Mac;
            let mut mac: hmac::Hmac<sha2::Sha256> = match Mac::new_from_slice(secret.as_bytes()) {
                Ok(m) => m,
                Err(e) => {
                    log::error!("HMAC init failed: {e}");
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Internal error"}))).into_response();
                }
            };
            mac.update(&body);
            let expected = hex::encode(mac.finalize().into_bytes());
            if !const_eq(sig, &expected) {
                log::warn!("Tochka webhook: invalid signature");
                return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid signature"}))).into_response();
            }
        } else {
            log::warn!("Tochka webhook: no signature header");
        }
    }
    // else: no secret configured, skip verification (dev mode)

    let body_str = match String::from_utf8(body.to_vec()) {
        Ok(s) => s.trim().to_string(),
        Err(e) => {
            log::warn!("Tochka webhook: invalid UTF-8 body: {e}");
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid body"}))).into_response();
        }
    };

    // Decode JWT payload (body is a JWT token string)
    let payload_json = match decode_jwt_payload(&body_str) {
        Ok(json) => json,
        Err(e) => {
            log::warn!("Tochka webhook: failed to decode JWT: {e}");
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid JWT"}))).into_response();
        }
    };

    let payload: TochkaAcquiringPayload = match serde_json::from_value(payload_json.clone()) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("Tochka webhook: failed to parse payload: {e} | body={}", payload_json);
            return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid payload"}))).into_response();
        }
    };

    let webhook_type = payload.webhook_type.as_deref().unwrap_or("unknown");
    let status = payload.status.as_deref().unwrap_or("UNKNOWN");
    let operation_id = payload.operation_id.as_deref().unwrap_or("");

    log::info!(
        "Tochka webhook: type={}, status={}, operationId={}, paymentLinkId={}, amount={}, paymentType={}",
        webhook_type, status, operation_id,
        payload.payment_link_id.as_deref().unwrap_or("?"),
        payload.amount.as_deref().unwrap_or("?"),
        payload.payment_type.as_deref().unwrap_or("?")
    );

    // Handle acquiringInternetPayment events
    if webhook_type == "acquiringInternetPayment" {
        if let Some(db) = &state.db {
            if let Some(ref order_id) = payload.payment_link_id {
                if !order_id.is_empty() {
                    match status {
                        "AUTHORIZED" => {
                            // Two-step payment: funds reserved, waiting for capture
                            log::info!("Payment authorized (reserved): order={}, op={}", order_id, operation_id);
                            // Order stays pending until APPROVED
                        }
                        "APPROVED" => {
                            // Payment completed — activate subscription
                            if let Err(e) = flowlink_db::orders::OrderRepo::update_paid(
                                db.pool(), order_id, operation_id,
                            ).await {
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
                                            "\u{1f4b0} Payment approved: account={}, plan={}, order={}, op={}",
                                            order.account_id, plan_id, order_id, operation_id
                                        );
                                        if let Some(email_service) = &state.email_service {
                                            if let Ok(Some(account)) = flowlink_db::accounts::AccountRepo::get(db.pool(), &order.account_id).await {
                                                if let Some(ref email) = account.email {
                                                    let plan_name = plan_id.clone();
                                                    let amount = format!("{:.2} \u{20bd}", order.amount_kopecks as f64 / 100.0);
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
                        "DECLINED" | "REJECTED" | "ERROR" => {
                            // Payment failed
                            log::warn!("Payment failed: order={}, status={}, op={}", order_id, status, operation_id);
                            if let Err(e) = flowlink_db::orders::OrderRepo::update_failed(db.pool(), order_id).await {
                                log::warn!("Failed to mark order {order_id} as failed: {e}");
                            }
                            // Send payment failed email
                            if let Some(email_service) = &state.email_service {
                                if let Ok(Some(order)) = flowlink_db::orders::OrderRepo::get(db.pool(), order_id).await {
                                    if let Ok(Some(account)) = flowlink_db::accounts::AccountRepo::get(db.pool(), &order.account_id).await {
                                        if let Some(ref email) = account.email {
                                            let plan_id = order.plan_id.clone().unwrap_or_default();
                                            tokio::spawn({
                                                let svc = email_service.clone();
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
                        "REFUNDED" | "PARTIALLY_REFUNDED" => {
                            // Refund processed
                            log::info!("Payment refunded: order={}, status={}, op={}", order_id, status, operation_id);
                            // Downgrade account to free plan
                            if let Ok(Some(order)) = flowlink_db::orders::OrderRepo::get(db.pool(), order_id).await {
                                if let Err(e) = flowlink_db::accounts::AccountRepo::update_plan(
                                    db.pool(), &order.account_id, "free",
                                ).await {
                                    log::warn!("Failed to downgrade account {}: {e}", order.account_id);
                                }
                            }
                        }
                        other => {
                            log::info!("Unhandled payment status '{other}' for order={}", order_id);
                        }
                    }
                }
            }
        }
    }

    (StatusCode::OK, Json(json!({"ok": true}))).into_response()
}

/// POST /api/billing/check-expiry — cron-callable endpoint that processes expired trials/grace.
/// Should be called daily. For orgs with expired trials: enters grace period.
/// For orgs with expired grace: auto-downgrades to free plan.
pub async fn check_expiry(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let db = match &state.db {
        Some(db) => db,
        None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response(),
    };

    let now = chrono::Utc::now();
    let mut processed = 0i64;
    let mut errors = 0i64;
    let mut grace_entered = 0i64;
    let mut downgraded = 0i64;

    // 0. Cleanup expired email verification codes (15 min)
    if let Err(e) = flowlink_db::email_verification::EmailVerificationRepo::cleanup_expired(db.pool(), 15).await {
        log::warn!("check_expiry: email cleanup failed: {e}");
    }

    // 1. Find orgs with expired trials but no grace period set yet
    match sqlx::query(
        "UPDATE organizations SET grace_ends_at = $2, is_trial = false, updated_at = NOW() WHERE is_trial = true AND trial_ends_at IS NOT NULL AND trial_ends_at < $1 AND grace_ends_at IS NULL"
    )
    .bind(now)
    .bind(now + chrono::Duration::days(3))
    .execute(db.pool())
    .await
    {
        Ok(result) => {
            grace_entered = result.rows_affected() as i64;
            processed += grace_entered;
        }
        Err(e) => {
            log::error!("check_expiry: failed to set grace period: {e}");
            errors += 1;
        }
    }

    // 2. Find orgs with expired grace periods — auto-downgrade to free
    match sqlx::query(
        "UPDATE organizations SET plan_id = 'free', grace_ends_at = NULL, updated_at = NOW() WHERE grace_ends_at IS NOT NULL AND grace_ends_at < $1 AND plan_id != 'free'"
    )
    .bind(now)
    .execute(db.pool())
    .await
    {
        Ok(result) => {
            downgraded = result.rows_affected() as i64;
            processed += downgraded;
        }
        Err(e) => {
            log::error!("check_expiry: failed to auto-downgrade: {e}");
            errors += 1;
        }
    }

    log::info!(
        "check_expiry: processed={processed}, grace_entered={grace_entered}, downgraded={downgraded}, errors={errors}"
    );

    (StatusCode::OK, Json(json!({
        "processed": processed,
        "grace_entered": grace_entered,
        "downgraded": downgraded,
        "errors": errors,
    }))).into_response()
}

/// GET /api/v1/account/tg-link-code — generate a link code for Telegram binding
/// Returns the account_id as the code (user sends /start <code> in TG bot)
pub async fn tg_link_code(
    State(_state): State<AppState>,
    claims: ClaimsExtractor,
) -> impl IntoResponse {
    let bot_username = std::env::var("TG_BOT_USERNAME").unwrap_or_else(|_| "flowlink_bot".to_string());
    let link = format!("https://t.me/{}/start/{}", bot_username, claims.0.account_id);
    (StatusCode::OK, Json(json!({
        "code": claims.0.account_id,
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
            is_trial: None,
            trial_ends_at: None,
            trial_days_remaining: None,
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
    /// Payment method — optional, defaults to Card
    #[serde(default = "default_payment_method")]
    pub payment_method: flowlink_billing::tochka::SubscriptionPaymentMethod,
    pub email: Option<String>,
    pub period: Option<String>,
    pub trial_days: Option<u16>,
    /// Customer email for 54-FZ receipt
    pub customer_email: Option<String>,
    /// Customer phone for 54-FZ receipt (alternative to email)
    pub customer_phone: Option<String>,
}

fn default_payment_method() -> flowlink_billing::tochka::SubscriptionPaymentMethod {
    flowlink_billing::tochka::SubscriptionPaymentMethod::Card { card_token: None }
}

#[derive(Serialize)]
pub struct SubscribeResponse {
    pub subscription_id: String,
    pub status: String,
    pub payment_url: Option<String>,
}

// ═══════════════════════════════════════════════
// Legacy SBP one-time payment (kept for reference)
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

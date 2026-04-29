//! Billing API endpoints.
//!
//! REST API для управления тарифами, подписками, платежами и вебхуками Точка Банка.

use axum::{
    extract::{Path, Extension},
    response::IntoResponse,
    body::Bytes,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::ClaimsExtractor;
use crate::{BillingState, server_base_url};
use flowlink_db::audit;
use flowlink_db::orgs::OrgRow;

// ═══════════════════════════════════════════════
// Request / Response types
// ═══════════════════════════════════════════════

#[derive(Deserialize)]
pub struct ChangePlanRequest { pub plan_id: String }

#[derive(Deserialize)]
pub struct ChangeSubscriptionPlanRequest { pub new_plan_id: String }

#[derive(Deserialize)]
pub struct TopUpRequest { pub amount_kopecks: u64, pub method: String }

#[derive(Deserialize)]
pub struct CreateSubscriptionRequest {
    pub plan_id: String, pub period: String, pub amount_kopecks: i64,
    pub tochka_subscription_id: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateOrderRequest {
    pub amount_kopecks: i64, pub description: Option<String>, pub payment_method: String,
}

#[derive(Serialize)]
pub struct BillingInfo {
    pub plan_id: String, pub plan_name: String, pub active: bool, pub balance_rub: String,
    pub expires_at: Option<String>, pub usage: Value, pub limits: Value,
    pub available_plans: Vec<Value>, pub is_trial: Option<bool>,
    pub trial_ends_at: Option<String>, pub trial_days_remaining: Option<i64>,
}

#[derive(Deserialize)]
pub struct SubscribeRequest {
    pub plan_id: String,
    #[serde(default = "default_payment_method")]
    pub payment_method: crate::tochka::SubscriptionPaymentMethod,
    pub email: Option<String>,
    pub period: Option<String>,
    pub trial_days: Option<u16>,
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
}

fn default_payment_method() -> crate::tochka::SubscriptionPaymentMethod {
    crate::tochka::SubscriptionPaymentMethod::Card { card_token: None }
}

#[derive(Serialize)]
pub struct SubscribeResponse {
    pub subscription_id: String, pub status: String, pub payment_url: Option<String>,
}

// ═══════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════

fn get_billing_engine(state: &BillingState) -> Result<&Arc<crate::BillingEngine>, axum::response::Response> {
    match &state.billing {
        Some(engine) => Ok(engine),
        None => Err((StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Billing not configured"}))).into_response()),
    }
}

/// Constant-time string comparison для предотвращения timing-атак
#[allow(dead_code)]
fn const_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() { return false; }
    let mut result = 0u8;
    for i in 0..a.len() { result |= a.as_bytes()[i] ^ b.as_bytes()[i]; }
    result == 0
}

/// Trial status helper
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TrialStatus { None, Active { days_remaining: i64 }, Expired }

pub fn check_trial_status(org: &OrgRow) -> TrialStatus {
    match (org.is_trial, org.trial_ends_at) {
        (false, _) => TrialStatus::None,
        (true, Some(end)) if end < chrono::Utc::now() => TrialStatus::Expired,
        (true, Some(end)) => TrialStatus::Active { days_remaining: (end - chrono::Utc::now()).num_days() },
        _ => TrialStatus::None,
    }
}

#[allow(dead_code)]
fn require_org(claims: &flowlink_auth::Claims) -> Result<uuid::Uuid, (StatusCode, axum::Json<serde_json::Value>)> {
    match &claims.org_id {
        Some(id) => uuid::Uuid::parse_str(id).map_err(|_| (StatusCode::BAD_REQUEST, axum::Json(json!({"error": "Invalid org_id"})))),
        None => Err((StatusCode::FORBIDDEN, axum::Json(json!({"error": "Organization required"})))),
    }
}

// ═══════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════

/// GET /api/billing — get billing info for the authenticated account
pub async fn get_billing_info(
    Extension(state): Extension<Arc<BillingState>>,
    claims: ClaimsExtractor,
) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) { Ok(e) => e, Err(r) => return r };

    let mut org: Option<OrgRow> = None;
    if let Some(ref org_id_str) = claims.0.org_id {
        if let Ok(uuid) = uuid::Uuid::parse_str(org_id_str) {
            if let Some(db) = &state.db {
                if let Ok(Some(o)) = flowlink_db::orgs::OrgRepo::get(db.pool(), uuid).await { org = Some(o); }
            }
        }
    }

    if let Some(db) = &state.db {
        if let Err(e) = flowlink_db::accounts::AccountRepo::get_or_create(
            db.pool(), &claims.0.account_id, crate::plans::PlanId::Free.as_str(),
        ).await { log::warn!("DB account lookup failed: {e}"); }
    }

    let effective_plan_id = org.as_ref().map(|o| o.plan_id.clone()).unwrap_or_else(|| {
        billing_engine.get_or_create_account(&claims.0.account_id).plan_id.clone()
    });

    let account_billing = billing_engine.get_or_create_account(&claims.0.account_id);
    let plan = billing_engine.plans().get(&effective_plan_id);
    let usage = billing_engine.usage().get_snapshot(&claims.0.account_id);
    let plan_name = plan.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let available_plans: Vec<Value> = billing_engine.plans().list_available().iter().map(|p| json!({
        "id": p.id, "name": p.name, "price_rub": p.format_monthly(), "tier": p.tier,
    })).collect();
    let plan_limits = plan.as_ref().map(|p| serde_json::to_value(&p.limits).unwrap_or(json!(null))).unwrap_or(json!(null));

    let (is_trial, trial_ends_at, trial_days_remaining) = match &org {
        Some(o) if o.is_trial => {
            let remaining = o.trial_ends_at.map(|end| (end - chrono::Utc::now()).num_days()).unwrap_or(0);
            (Some(true), o.trial_ends_at.map(|t| t.to_rfc3339()), Some(remaining.max(0)))
        }
        _ => (None, None, None),
    };

    let info = BillingInfo {
        plan_id: effective_plan_id, plan_name, active: account_billing.active,
        balance_rub: crate::payment::PaymentConfig::format_rub(account_billing.balance_kopecks),
        expires_at: account_billing.expires_at.map(|dt| dt.to_rfc3339()),
        usage: serde_json::to_value(&usage).unwrap_or(json!(null)),
        limits: plan_limits, available_plans, is_trial, trial_ends_at, trial_days_remaining,
    };
    (StatusCode::OK, Json(info)).into_response()
}

/// GET /api/billing/usage
pub async fn get_usage(
    Extension(state): Extension<Arc<BillingState>>,
    claims: ClaimsExtractor,
) -> impl IntoResponse {
    let all_tracker_usage = state.usage_tracker.get_all_usage().await;
    let (daily_requests, daily_tokens) = state.usage_tracker.today_stats().await;

    let mut response = serde_json::json!({
        "tracker": { "agents": all_tracker_usage, "daily_requests": daily_requests, "daily_tokens": daily_tokens, "active_agents": all_tracker_usage.len() }
    });

    if let Some(billing_engine) = &state.billing {
        let snapshot = billing_engine.usage().get_snapshot(&claims.0.account_id);
        response["billing"] = serde_json::to_value(&snapshot).unwrap_or(json!(null));
    }
    (StatusCode::OK, Json(response)).into_response()
}

/// GET /api/plans — public endpoint, no auth required
pub async fn public_plans(Extension(state): Extension<Arc<BillingState>>) -> impl IntoResponse {
    let plans = match &state.billing {
        Some(engine) => engine.plans().list_available(),
        None => {
            let registry = crate::plans::PlanRegistry::new();
            registry.seed_defaults();
            registry.list_available()
        }
    };
    let promotions = match &state.db {
        Some(db) => flowlink_db::promotions::get_active_promotions(db.pool()).await.unwrap_or_default(),
        None => vec![],
    };
    (StatusCode::OK, Json(json!({"plans": plans, "promotions": promotions}))).into_response()
}

/// GET /api/billing/plans
pub async fn list_plans(Extension(state): Extension<Arc<BillingState>>) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) { Ok(e) => e, Err(r) => return r };
    let plans = billing_engine.plans().list_available();
    (StatusCode::OK, Json(plans)).into_response()
}

/// GET /api/billing/my-plan
pub async fn my_plan(Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) { Ok(e) => e, Err(r) => return r };
    let billing = billing_engine.get_or_create_account(&claims.0.account_id);
    let plan = match billing_engine.plans().get(&billing.plan_id) {
        Some(p) => p, None => return (StatusCode::OK, Json(json!({"plan_id": billing.plan_id, "plan": null, "error": "Plan not found"}))).into_response(),
    };
    (StatusCode::OK, Json(json!({"plan_id": plan.id, "plan": plan}))).into_response()
}

/// GET /api/billing/check-feature
pub async fn check_feature(
    Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) { Ok(e) => e, Err(r) => return r };
    let feature = match params.get("feature") {
        Some(f) => f.clone(), None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "Missing ?feature= parameter"}))).into_response(),
    };
    let billing = billing_engine.get_or_create_account(&claims.0.account_id);
    let plan = match billing_engine.plans().get(&billing.plan_id) {
        Some(p) => p, None => return (StatusCode::OK, Json(json!({"allowed": false, "reason": format!("Plan '{}' not found", billing.plan_id)}))).into_response(),
    };
    match plan.require_feature(&feature) {
        Ok(()) => (StatusCode::OK, Json(json!({"allowed": true, "reason": null}))).into_response(),
        Err(e) => (StatusCode::OK, Json(json!({"allowed": false, "reason": e.to_string()}))).into_response(),
    }
}

/// POST /api/billing/change-plan
pub async fn change_plan(
    Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor, Json(body): Json<ChangePlanRequest>,
) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) { Ok(e) => e, Err(r) => return r };
    let mut account_billing = billing_engine.get_or_create_account(&claims.0.account_id);
    let current_plan_id = account_billing.plan_id.clone();
    match billing_engine.change_plan(&mut account_billing, &body.plan_id) {
        Ok(created_invoice) => {
            if let Some(db) = &state.db {
                match db.pool().begin().await {
                    Ok(mut tx) => {
                        let res: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
                            sqlx::query("UPDATE accounts SET plan_id = $1, updated_at = NOW() WHERE account_id = $2")
                                .bind(&body.plan_id).bind(&claims.0.account_id).execute(&mut *tx).await?;
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
            let mut resp = json!({"plan_id": account_billing.plan_id, "message": "Plan changed successfully"});
            if let Some(inv) = created_invoice { resp["invoice"] = serde_json::to_value(&inv).unwrap_or(json!(null)); }
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
                            let lang = account.preferred_language.as_deref().unwrap_or("ru").to_string();
                            let svc = email_svc.clone(); let to = email.clone();
                            tokio::spawn(async move {
                                if let Err(e) = svc.send_plan_changed(&to, to.split('@').next().unwrap_or(&to), &old_plan, &new_plan, &lang).await {
                                    log::warn!("Failed to send plan changed email to {to}: {e}");
                                }
                            });
                        }
                    }
                }
            }
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(_e) => (StatusCode::BAD_REQUEST, Json(json!({"error": "Internal error"}))).into_response(),
    }
}

/// GET /api/billing/invoices
pub async fn list_invoices(Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) { Ok(e) => e, Err(r) => return r };
    let invoices = billing_engine.invoices().list_for_account(&claims.0.account_id);
    (StatusCode::OK, Json(invoices)).into_response()
}

/// GET /api/billing/invoices/{id}
pub async fn get_invoice(Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor, Path(id): Path<String>) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) { Ok(e) => e, Err(r) => return r };
    match billing_engine.invoices().get(&id) {
        Some(invoice) => {
            if invoice.account_id != claims.0.sub && !claims.0.is_admin {
                return (StatusCode::FORBIDDEN, Json(json!({"error": "Not your invoice"}))).into_response();
            }
            (StatusCode::OK, Json(invoice)).into_response()
        }
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "Invoice not found"}))).into_response(),
    }
}

/// GET /api/billing/payments/methods
pub async fn list_payment_methods(Extension(state): Extension<Arc<BillingState>>) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) { Ok(e) => e, Err(r) => return r };
    let methods: Vec<Value> = billing_engine.payments().available_methods().iter().map(|m| json!({"id": m.as_str(), "name": m.display_name()})).collect();
    (StatusCode::OK, Json(methods)).into_response()
}

// ═══════════════════════════════════════════════
// Subscriptions (Tochka Bank)
// ═══════════════════════════════════════════════

/// POST /api/billing/subscribe
pub async fn subscribe(
    Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor, Json(body): Json<SubscribeRequest>,
) -> impl IntoResponse {
    if claims.0.account_id == "default" { return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Authentication required"}))).into_response(); }
    let tochka = match &state.tochka {
        Some(t) => t, None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Payment gateway not configured"}))).into_response(),
    };
    let billing_engine = match get_billing_engine(&state) { Ok(e) => e, Err(r) => return r };

    // Check for existing active subscription
    if let Some(db) = &state.db {
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT plan_id FROM subscriptions WHERE account_id = $1 AND status IN ('active', 'trialing') LIMIT 1"
        ).bind(&claims.0.account_id).fetch_optional(db.pool()).await.unwrap_or(None);
        if let Some(current_plan) = existing {
            return (StatusCode::CONFLICT, Json(json!({"error": "Active subscription exists", "current_plan": current_plan}))).into_response();
        }
    }

    let plan = match billing_engine.plans().get(&body.plan_id) {
        Some(p) => p, None => return (StatusCode::NOT_FOUND, Json(json!({"error": "Plan not found"}))).into_response(),
    };
    if plan.id == "enterprise" {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Enterprise plan requires manual setup. Please contact admin@flow-masters.ru for custom pricing and onboarding."}))).into_response();
    }

    // 54-ФЗ: require email or phone
    let customer_phone = body.customer_phone.clone();
    let customer_email = if body.customer_email.is_none() {
        if let Some(db) = &state.db {
            match flowlink_db::accounts::AccountRepo::get(db.pool(), &claims.0.account_id).await {
                Ok(Some(acc)) => acc.email.filter(|e| !e.is_empty()), _ => None,
            }
        } else { None }
    } else { body.customer_email.clone() };
    if customer_email.is_none() && customer_phone.is_none() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "Email or phone required for receipt (54-FZ)"}))).into_response();
    }

    let period = body.period.as_deref().and_then(crate::tochka::BillingPeriod::from_str_opt).unwrap_or(crate::tochka::BillingPeriod::Month);
    let amount = plan.price_kopecks;
    let description = format!("FlowLink {} — подписка", plan.name);

    let req = crate::tochka::CreateSubscriptionRequest {
        customer_id: claims.0.account_id.clone(), plan_id: body.plan_id.clone(), period, amount,
        payment_method: body.payment_method.clone(), description, start_date: None, trial_days: body.trial_days.unwrap_or(0),
        customer_email, customer_phone,
        return_url: Some(format!("{}/dashboard/billing?plan={}&status=success", server_base_url(), body.plan_id)),
        fail_url: Some(format!("{}/checkout/{}?status=failed", server_base_url(), body.plan_id)),
    };

    match tochka.create_subscription(&req).await {
        Ok(sub) => {
            if let Some(db) = &state.db {
                let _ = audit::log_event(db.pool(), None, &claims.0.account_id, "plan.changed", Some("subscription"), Some(&sub.subscription_id), json!({"plan_id": &body.plan_id, "amount": amount}), None).await;
                let period_str = period.as_str().to_string();
                if let Err(e) = flowlink_db::subscriptions::SubscriptionRepo::create(
                    db.pool(), &sub.subscription_id, &claims.0.account_id, &body.plan_id, &period_str, amount as i64, Some(&sub.subscription_id),
                ).await { log::warn!("Failed to persist subscription to DB: {e}"); }
            }
            (StatusCode::CREATED, Json(SubscribeResponse { subscription_id: sub.subscription_id, status: sub.status, payment_url: sub.payment_link })).into_response()
        }
        Err(e) => { log::error!("Tochka subscription creation failed: {e}"); (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Subscription creation failed", "details": "Check logs for details"}))).into_response() }
    }
}

/// GET /api/billing/subscription
pub async fn get_subscription(Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor) -> impl IntoResponse {
    let tochka = match &state.tochka { Some(t) => t, None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Payment gateway not configured"}))).into_response() };
    match tochka.get_subscription_by_customer(&claims.0.account_id).await {
        Ok(sub) => (StatusCode::OK, Json(json!({"subscription_id": sub.subscription_id, "customer_id": sub.customer_id, "plan_id": sub.plan_id, "status": sub.status, "amount": sub.amount, "period": sub.period, "current_period_start": sub.current_period_start, "current_period_end": sub.current_period_end}))).into_response(),
        Err(_e) => (StatusCode::NOT_FOUND, Json(json!({"error": "No active subscription", "details": "Check logs for details"}))).into_response(),
    }
}

/// POST /api/billing/subscription/pause
pub async fn pause_subscription(Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor) -> impl IntoResponse {
    let tochka = match &state.tochka { Some(t) => t, None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Payment gateway not configured"}))).into_response() };
    let sub = match tochka.get_subscription_by_customer(&claims.0.account_id).await {
        Ok(s) => s, Err(_e) => return (StatusCode::NOT_FOUND, Json(json!({"error": "No active subscription"}))).into_response(),
    };
    match tochka.pause_subscription(&sub.subscription_id).await {
        Ok(paused) => (StatusCode::OK, Json(json!({"subscription_id": paused.subscription_id, "status": paused.status}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to pause: {e}")}))).into_response(),
    }
}

/// POST /api/billing/subscription/resume
pub async fn resume_subscription(Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor) -> impl IntoResponse {
    let tochka = match &state.tochka { Some(t) => t, None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Payment gateway not configured"}))).into_response() };
    let sub = match tochka.get_subscription_by_customer(&claims.0.account_id).await {
        Ok(s) => s, Err(_e) => return (StatusCode::NOT_FOUND, Json(json!({"error": "No subscription found"}))).into_response(),
    };
    match tochka.resume_subscription(&sub.subscription_id).await {
        Ok(resumed) => (StatusCode::OK, Json(json!({"subscription_id": resumed.subscription_id, "status": resumed.status}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to resume: {e}")}))).into_response(),
    }
}

/// DELETE /api/billing/subscription
pub async fn cancel_tochka_subscription(Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor) -> impl IntoResponse {
    let tochka = match &state.tochka { Some(t) => t, None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Payment gateway not configured"}))).into_response() };
    let sub = match tochka.get_subscription_by_customer(&claims.0.account_id).await {
        Ok(s) => s, Err(_e) => return (StatusCode::NOT_FOUND, Json(json!({"error": "No subscription found"}))).into_response(),
    };
    match tochka.cancel_subscription(&sub.subscription_id).await {
        Ok(cancelled) => {
            if let Some(db) = &state.db {
                let _ = audit::log_event(db.pool(), None, &claims.0.account_id, "subscription.cancelled", Some("subscription"), Some(&sub.subscription_id), json!({}), None).await;
                let _ = flowlink_db::subscriptions::SubscriptionRepo::cancel(db.pool(), &sub.subscription_id).await;
            }
            (StatusCode::OK, Json(json!({"subscription_id": cancelled.subscription_id, "status": cancelled.status, "cancelled": true}))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to cancel: {e}")}))).into_response(),
    }
}

/// POST /api/billing/subscription/change-plan
pub async fn change_subscription_plan(
    Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor, Json(body): Json<ChangeSubscriptionPlanRequest>,
) -> impl IntoResponse {
    let tochka = match &state.tochka { Some(t) => t, None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "Payment gateway not configured"}))).into_response() };
    let billing_engine = match get_billing_engine(&state) { Ok(e) => e, Err(r) => return r };

    let account_billing = billing_engine.get_or_create_account(&claims.0.account_id);
    let current_plan_id = account_billing.plan_id.clone();
    let current_plan = match billing_engine.plans().get(&current_plan_id) { Some(p) => p, None => return (StatusCode::BAD_REQUEST, Json(json!({"error": "Current plan not found"}))).into_response() };
    let new_plan = match billing_engine.plans().get(&body.new_plan_id) { Some(p) => p, None => return (StatusCode::NOT_FOUND, Json(json!({"error": "Plan not found"}))).into_response() };
    if body.new_plan_id == current_plan_id { return (StatusCode::BAD_REQUEST, Json(json!({"error": "Already on this plan"}))).into_response(); }
    let is_upgrade = new_plan.price_kopecks >= current_plan.price_kopecks;

    if is_upgrade {
        let sub = match tochka.get_subscription_by_customer(&claims.0.account_id).await {
            Ok(s) => s, Err(_e) => return (StatusCode::NOT_FOUND, Json(json!({"error": "No active subscription"}))).into_response(),
        };
        let period = crate::tochka::BillingPeriod::from_str_opt(&account_billing.plan_id).unwrap_or(crate::tochka::BillingPeriod::Month);
        if let Err(e) = tochka.cancel_subscription(&sub.subscription_id).await { log::warn!("Failed to cancel old subscription: {e}"); }

        let req = crate::tochka::CreateSubscriptionRequest {
            customer_id: claims.0.account_id.clone(), plan_id: body.new_plan_id.clone(), period,
            amount: new_plan.price_kopecks, payment_method: crate::tochka::SubscriptionPaymentMethod::Sbp { phone: String::new() },
            description: format!("FlowLink {} — подписка", new_plan.name), start_date: None, trial_days: 0,
            customer_email: None, customer_phone: None, return_url: None, fail_url: None,
        };

        match tochka.create_subscription(&req).await {
            Ok(new_sub) => {
                let mut billing = billing_engine.get_or_create_account(&claims.0.account_id);
                let _ = billing_engine.change_plan(&mut billing, &body.new_plan_id);
                if let Some(db) = &state.db {
                    let _ = flowlink_db::accounts::AccountRepo::update_plan(db.pool(), &claims.0.account_id, &body.new_plan_id).await;
                    if let Err(e) = flowlink_db::subscriptions::SubscriptionRepo::create(
                        db.pool(), &new_sub.subscription_id, &claims.0.account_id, &body.new_plan_id,
                        period.as_str(), new_plan.price_kopecks as i64, Some(&new_sub.subscription_id),
                    ).await { log::warn!("Failed to persist new subscription: {e}"); }
                }
                (StatusCode::OK, Json(json!({"change_type": "upgrade", "effective": "immediate", "new_subscription_id": new_sub.subscription_id, "new_plan_id": body.new_plan_id}))).into_response()
            }
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to create new subscription: {e}")}))).into_response(),
        }
    } else {
        let effective_date = account_billing.expires_at.unwrap_or_else(|| chrono::Utc::now() + chrono::Duration::days(30));
        if let Some(db) = &state.db {
            let _ = sqlx::query("UPDATE accounts SET pending_plan_id = $1, pending_plan_effective = $2 WHERE account_id = $3")
                .bind(&body.new_plan_id).bind(effective_date).bind(&claims.0.account_id)
                .execute(db.pool()).await;
        }
        (StatusCode::OK, Json(json!({"change_type": "downgrade", "effective": effective_date.to_rfc3339(), "pending_plan_id": body.new_plan_id, "message": "Plan change will take effect at the end of the current billing period"}))).into_response()
    }
}

// ═══════════════════════════════════════════════
// Legacy DB subscription CRUD
// ═══════════════════════════════════════════════

/// GET /api/billing/subscriptions
pub async fn list_subscriptions(Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor) -> impl IntoResponse {
    let db = match &state.db { Some(db) => db, None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response() };
    if let Some(ref org_id_str) = claims.0.org_id {
        if let Ok(uuid) = uuid::Uuid::parse_str(org_id_str) {
            match sqlx::query_as::<_, (serde_json::Value,)>("SELECT row_to_json(row) as val FROM (SELECT * FROM subscriptions WHERE org_id = $1 ORDER BY created_at DESC) row")
                .bind(uuid).fetch_all(db.pool()).await {
                Ok(rows) => { let subs: Vec<serde_json::Value> = rows.into_iter().map(|r| r.0).collect(); return (StatusCode::OK, Json(json!(subs))).into_response(); }
                Err(_e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Internal error"}))).into_response(),
            }
        }
    }
    match flowlink_db::subscriptions::SubscriptionRepo::list_for_account(db.pool(), &claims.0.account_id).await {
        Ok(subs) => (StatusCode::OK, Json(json!(subs))).into_response(),
        Err(_e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Internal error"}))).into_response(),
    }
}

/// POST /api/billing/subscriptions/:id/cancel
pub async fn cancel_subscription(Extension(state): Extension<Arc<BillingState>>, Path(id): Path<String>, claims: ClaimsExtractor) -> impl IntoResponse {
    let db = match &state.db { Some(db) => db, None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response() };
    match flowlink_db::subscriptions::SubscriptionRepo::get_by_id(db.pool(), &id).await {
        Ok(Some(sub)) if sub.account_id == claims.0.account_id || claims.0.is_admin => {},
        Ok(Some(_)) => return (StatusCode::FORBIDDEN, Json(json!({"error": "Not your subscription"}))).into_response(),
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "Subscription not found"}))).into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Internal error"}))).into_response(),
    }
    match flowlink_db::subscriptions::SubscriptionRepo::cancel(db.pool(), &id).await {
        Ok(()) => { let _ = audit::log_event(db.pool(), None, &claims.0.account_id, "subscription.cancelled", Some("subscription"), Some(&id), json!({}), None).await; (StatusCode::OK, Json(json!({"cancelled": true}))).into_response() }
        Err(_e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Internal error"}))).into_response(),
    }
}

// ═══════════════════════════════════════════════
// Orders (разовые платежи)
// ═══════════════════════════════════════════════

/// POST /api/billing/orders
pub async fn create_order(Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor, Json(body): Json<CreateOrderRequest>) -> impl IntoResponse {
    let db = match &state.db { Some(db) => db, None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response() };
    let id = uuid::Uuid::new_v4().to_string();
    match flowlink_db::orders::OrderRepo::create(db.pool(), &id, &claims.0.account_id, body.amount_kopecks, body.description.as_deref(), &body.payment_method).await {
        Ok(order) => (StatusCode::CREATED, Json(json!(order))).into_response(),
        Err(_e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Internal error"}))).into_response(),
    }
}

/// GET /api/billing/orders
pub async fn list_orders(Extension(state): Extension<Arc<BillingState>>, claims: ClaimsExtractor) -> impl IntoResponse {
    let db = match &state.db { Some(db) => db, None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response() };
    if let Some(ref org_id_str) = claims.0.org_id {
        if let Ok(uuid) = uuid::Uuid::parse_str(org_id_str) {
            match sqlx::query_as::<_, (serde_json::Value,)>("SELECT row_to_json(row) as val FROM (SELECT o.* FROM orders o JOIN subscriptions s ON o.account_id = s.account_id WHERE s.org_id = $1 ORDER BY o.created_at DESC) row")
                .bind(uuid).fetch_all(db.pool()).await {
                Ok(rows) => { let orders: Vec<serde_json::Value> = rows.into_iter().map(|r| r.0).collect(); return (StatusCode::OK, Json(json!(orders))).into_response(); }
                Err(_e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Internal error"}))).into_response(),
            }
        }
    }
    match flowlink_db::orders::OrderRepo::list_for_account(db.pool(), &claims.0.account_id).await {
        Ok(orders) => (StatusCode::OK, Json(json!(orders))).into_response(),
        Err(_e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Internal error"}))).into_response(),
    }
}

// ═══════════════════════════════════════════════
// Tochka webhook
// ═══════════════════════════════════════════════

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct TochkaAcquiringPayload {
    #[serde(default)] customer_code: Option<String>, #[serde(default)] amount: Option<String>,
    #[serde(default)] payment_type: Option<String>, #[serde(default)] operation_id: Option<String>,
    #[serde(default)] purpose: Option<String>, #[serde(default)] webhook_type: Option<String>,
    #[serde(default)] merchant_id: Option<String>, #[serde(default)] consumer_id: Option<String>,
    #[serde(default)] status: Option<String>, #[serde(default)] payment_link_id: Option<String>,
}

fn decode_jwt_payload(token: &str) -> Result<serde_json::Value, String> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 { return Err(format!("Expected 3 JWT parts, got {}", parts.len())); }

    // RS256 verification skipped in billing crate (relay can add vault feature later)
    log::warn!("Tochka webhook JWT: RS256 verification skipped, decoding without signature check");
    use base64::Engine;
    let engine = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let decoded = engine.decode(parts[1]).map_err(|e| format!("Base64 decode: {e}"))?;
    let json_str = String::from_utf8(decoded).map_err(|e| format!("UTF-8: {e}"))?;
    serde_json::from_str(&json_str).map_err(|e| format!("JSON parse: {e}"))
}

/// POST /api/billing/webhook/tochka
pub async fn tochka_webhook(
    Extension(state): Extension<Arc<BillingState>>,
    headers: axum::http::HeaderMap, body: Bytes,
) -> impl IntoResponse {
    // Verify webhook secret
    if let Ok(secret) = std::env::var("TOCHKA_WEBHOOK_SECRET") {
        let sig = headers.get("X-Webhook-Signature").or_else(|| headers.get("X-Tochka-Signature")).and_then(|v| v.to_str().ok()).unwrap_or("");
        if !sig.is_empty() {
            use hmac::Mac;
            let mut mac: hmac::Hmac<sha2::Sha256> = match hmac::Mac::new_from_slice(secret.as_bytes()) {
                Ok(m) => m, Err(e) => { log::error!("HMAC init failed: {e}"); return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Internal error"}))).into_response(); }
            };
            mac.update(&body);
            let expected = hex::encode(mac.finalize().into_bytes());
            if !const_eq(sig, &expected) { log::warn!("Tochka webhook: invalid signature"); return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid signature"}))).into_response(); }
        } else { log::warn!("Tochka webhook: no signature header"); }
    }

    let body_str = match String::from_utf8(body.to_vec()) {
        Ok(s) => s.trim().to_string(), Err(e) => { log::warn!("Tochka webhook: invalid UTF-8 body: {e}"); return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid body"}))).into_response(); }
    };

    let payload_json = match decode_jwt_payload(&body_str) { Ok(j) => j, Err(e) => { log::warn!("Tochka webhook: failed to decode JWT: {e}"); return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid JWT"}))).into_response(); } };
    let payload: TochkaAcquiringPayload = match serde_json::from_value(payload_json.clone()) {
        Ok(p) => p, Err(e) => { log::warn!("Tochka webhook: failed to parse payload: {e} | body={}", payload_json); return (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid payload"}))).into_response(); }
    };

    let webhook_type = payload.webhook_type.as_deref().unwrap_or("unknown");
    let status = payload.status.as_deref().unwrap_or("UNKNOWN");
    let operation_id = payload.operation_id.as_deref().unwrap_or("");

    log::info!("Tochka webhook: type={}, status={}, operationId={}, paymentLinkId={}, amount={}", webhook_type, status, operation_id, payload.payment_link_id.as_deref().unwrap_or("?"), payload.amount.as_deref().unwrap_or("?"));

    if webhook_type == "acquiringInternetPayment" {
        if let Some(db) = &state.db {
            if let Some(ref order_id) = payload.payment_link_id {
                if !order_id.is_empty() {
                    match status {
                        "AUTHORIZED" => { log::info!("Payment authorized (reserved): order={}, op={}", order_id, operation_id); }
                        "APPROVED" => {
                            let _ = flowlink_db::orders::OrderRepo::update_paid(db.pool(), order_id, operation_id).await;
                            if let Ok(Some(order)) = flowlink_db::orders::OrderRepo::get(db.pool(), order_id).await {
                                if let Some(ref plan_id) = order.plan_id {
                                    if let Err(e) = flowlink_db::accounts::AccountRepo::update_plan(db.pool(), &order.account_id, plan_id).await {
                                        log::warn!("Failed to update account plan {}: {e}", order.account_id);
                                    } else {
                                        log::info!("\u{1f4b0} Payment approved: account={}, plan={}, order={}, op={}", order.account_id, plan_id, order_id, operation_id);
                                        state.metrics.inc_billing_payments("approved");
                                        if let Some(ref billing_engine) = state.billing {
                                            state.metrics.set_subscriptions_active(billing_engine.active_subscription_count() as f64);
                                            state.metrics.set_billing_revenue_monthly(billing_engine.monthly_revenue_rub());
                                        }
                                        if let Some(email_service) = &state.email_service {
                                            if let Ok(Some(account)) = flowlink_db::accounts::AccountRepo::get(db.pool(), &order.account_id).await {
                                                if let Some(ref email) = account.email {
                                                    let plan_name = plan_id.clone();
                                                    let amount = format!("{:.2} \u{20bd}", order.amount_kopecks as f64 / 100.0);
                                                    let lang = account.preferred_language.as_deref().unwrap_or("ru").to_string();
                                                    tokio::spawn({ let svc = email_service.clone(); let to = email.clone(); async move {
                                                        if let Err(e) = svc.send_payment_success(&to, to.split('@').next().unwrap_or(&to), &plan_name, &amount, &lang).await { log::warn!("Failed to send payment email to {to}: {e}"); }
                                                    }});
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        "DECLINED" | "REJECTED" | "ERROR" => {
                            log::warn!("Payment failed: order={}, status={}, op={}", order_id, status, operation_id);
                            state.metrics.inc_billing_payments("failed");
                            let _ = flowlink_db::orders::OrderRepo::update_failed(db.pool(), order_id).await;
                            if let Some(email_service) = &state.email_service {
                                if let Ok(Some(order)) = flowlink_db::orders::OrderRepo::get(db.pool(), order_id).await {
                                    if let Ok(Some(account)) = flowlink_db::accounts::AccountRepo::get(db.pool(), &order.account_id).await {
                                        if let Some(ref email) = account.email {
                                            let plan_id = order.plan_id.clone().unwrap_or_default();
                                            let lang = account.preferred_language.as_deref().unwrap_or("ru").to_string();
                                            tokio::spawn({ let svc = email_service.clone(); let to = email.clone(); async move {
                                                if let Err(e) = svc.send_payment_failed(&to, to.split('@').next().unwrap_or(&to), &plan_id, &lang).await { log::warn!("Failed to send payment failed email to {to}: {e}"); }
                                            }});
                                        }
                                    }
                                }
                            }
                        }
                        "REFUNDED" | "PARTIALLY_REFUNDED" => {
                            log::info!("Payment refunded: order={}, status={}, op={}", order_id, status, operation_id);
                            if let Ok(Some(order)) = flowlink_db::orders::OrderRepo::get(db.pool(), order_id).await {
                                let _ = flowlink_db::accounts::AccountRepo::update_plan(db.pool(), &order.account_id, "free").await;
                            }
                        }
                        other => { log::info!("Unhandled payment status '{other}' for order={}", order_id); }
                    }
                }
            }
        }
    }
    (StatusCode::OK, Json(json!({"ok": true}))).into_response()
}

// ═══════════════════════════════════════════════
// Expiry check
// ═══════════════════════════════════════════════

/// POST /api/billing/check-expiry
pub async fn check_expiry(Extension(state): Extension<Arc<BillingState>>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    let admin_key = std::env::var("FLOWLINK_ADMIN_KEY").unwrap_or_default();
    if !admin_key.is_empty() {
        let key = headers.get("x-admin-key").and_then(|v| v.to_str().ok()).unwrap_or("");
        if key != admin_key { return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Admin key required"}))).into_response(); }
    }
    let db = match &state.db { Some(db) => db, None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error": "DB not configured"}))).into_response() };

    let now = chrono::Utc::now();
    let mut processed = 0i64; let mut errors = 0i64; let mut grace_entered = 0i64; let mut downgraded = 0i64;

    if let Err(e) = flowlink_db::email_verification::EmailVerificationRepo::cleanup_expired(db.pool(), 15).await { log::warn!("check_expiry: email cleanup failed: {e}"); }

    match sqlx::query("UPDATE organizations SET grace_ends_at = $2, is_trial = false, updated_at = NOW() WHERE is_trial = true AND trial_ends_at IS NOT NULL AND trial_ends_at < $1 AND grace_ends_at IS NULL")
        .bind(now).bind(now + chrono::Duration::days(3)).execute(db.pool()).await {
        Ok(result) => { grace_entered = result.rows_affected() as i64; processed += grace_entered; }
        Err(e) => { log::error!("check_expiry: failed to set grace period: {e}"); errors += 1; }
    }

    match sqlx::query("UPDATE organizations SET plan_id = 'free', grace_ends_at = NULL, updated_at = NOW() WHERE grace_ends_at IS NOT NULL AND grace_ends_at < $1 AND plan_id != 'free'")
        .bind(now).execute(db.pool()).await {
        Ok(result) => { downgraded = result.rows_affected() as i64; processed += downgraded; }
        Err(e) => { log::error!("check_expiry: failed to auto-downgrade: {e}"); errors += 1; }
    }

    log::info!("check_expiry: processed={processed}, grace_entered={grace_entered}, downgraded={downgraded}, errors={errors}");
    (StatusCode::OK, Json(json!({"processed": processed, "grace_entered": grace_entered, "downgraded": downgraded, "errors": errors}))).into_response()
}

/// Background version of check_expiry
pub async fn check_expiry_bg(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    let now = chrono::Utc::now();
    let grace = sqlx::query("UPDATE subscriptions SET status = 'grace' WHERE status = 'active' AND current_period_end < $1 AND current_period_end > $2")
        .bind(now).bind(now - chrono::Duration::hours(24)).execute(pool).await?.rows_affected();
    let downgraded = sqlx::query("UPDATE subscriptions SET status = 'cancelled' WHERE status = 'grace' AND current_period_end < $1")
        .bind(now - chrono::Duration::days(7)).execute(pool).await?.rows_affected();
    if grace > 0 || downgraded > 0 { log::info!("check_expiry_bg: grace_entered={grace}, downgraded={downgraded}"); }
    Ok(())
}

/// GET /api/v1/account/tg-link-code
pub async fn tg_link_code(Extension(_state): Extension<Arc<BillingState>>, claims: ClaimsExtractor) -> impl IntoResponse {
    let bot_username = std::env::var("TG_BOT_USERNAME").unwrap_or_else(|_| "flowlink_bot".to_string());
    let link = format!("https://t.me/{}/start/{}", bot_username, claims.0.account_id);
    (StatusCode::OK, Json(json!({"code": claims.0.account_id, "link": link, "instructions": "Send this link to your Telegram or open it directly to link your account."}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_const_eq_same() { assert!(const_eq("abc", "abc")); }
    #[test]
    fn test_const_eq_different() { assert!(!const_eq("abc", "abd")); }
    #[test]
    fn test_const_eq_different_length() { assert!(!const_eq("abc", "abcd")); }
    #[test]
    fn test_billing_info_serialization() {
        let info = BillingInfo { plan_id: "trial".to_string(), plan_name: "Trial".to_string(), active: true, balance_rub: "0.00 RUB".to_string(), expires_at: None, usage: json!(null), limits: json!(null), available_plans: vec![], is_trial: None, trial_ends_at: None, trial_days_remaining: None };
        let json_str = serde_json::to_string(&info).unwrap();
        assert!(json_str.contains("trial"));
    }
}

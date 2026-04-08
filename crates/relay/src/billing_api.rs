//! Billing API endpoints for the relay server
//!
//! REST API for plan management, usage checking, and invoices.

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
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
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "Billing not configured"})),
        ).into_response()),
    }
}

// ═══════════════════════════════════════════════
// Handlers
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
            db.pool(), &account.0, "free",
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

    (axum::http::StatusCode::OK, Json(info)).into_response()
}

/// GET /api/billing/usage — get current usage snapshot
pub async fn get_usage(
    State(state): State<AppState>,
    account: AccountIdExtractor,
) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) {
        Ok(e) => e,
        Err(r) => return r,
    };

    let snapshot = billing_engine.usage().get_snapshot(&account.0);
    (axum::http::StatusCode::OK, Json(snapshot)).into_response()
}

/// GET /api/billing/plans — list available plans
pub async fn list_plans(State(state): State<AppState>) -> impl IntoResponse {
    let billing_engine = match get_billing_engine(&state) {
        Ok(e) => e,
        Err(r) => return r,
    };

    let plans = billing_engine.plans().list_available();
    (axum::http::StatusCode::OK, Json(plans)).into_response()
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
        Ok(()) => {
            billing_engine.update_account(&account_billing);
            // Persist to DB if available
            if let Some(db) = &state.db {
                if let Err(e) = flowlink_db::accounts::AccountRepo::update_plan(
                    db.pool(), &account.0, &body.plan_id,
                ).await {
                    log::warn!("Failed to persist plan change to DB: {e}");
                }
            }
            (axum::http::StatusCode::OK, Json(json!({
                "plan_id": account_billing.plan_id,
                "message": "Plan changed successfully",
            }))).into_response()
        }
        Err(e) => {
            (axum::http::StatusCode::BAD_REQUEST, Json(json!({
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
    (axum::http::StatusCode::OK, Json(invoices)).into_response()
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
        Some(invoice) => (axum::http::StatusCode::OK, Json(invoice)).into_response(),
        None => {
            (axum::http::StatusCode::NOT_FOUND, Json(json!({
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

    (axum::http::StatusCode::OK, Json(methods)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_billing_info_serialization() {
        let info = BillingInfo {
            plan_id: "free".to_string(),
            plan_name: "Free".to_string(),
            active: true,
            balance_rub: "0.00 ₽".to_string(),
            expires_at: None,
            usage: json!(null),
            available_plans: vec![],
        };
        let json_str = serde_json::to_string(&info).unwrap();
        assert!(json_str.contains("free"));
    }
}

//! Dynamic plan enforcement middleware.
//!
//! Instead of per-handler `require_feature!` calls, this middleware checks
//! route prefixes against a feature/limit requirement map. Plan is resolved
//! from request extensions (set by auth middleware).
//!
//! # Configuration
//!
//! Add entries to `ROUTE_REQUIREMENTS` to protect routes:
//!
//! ```ignore
//! ("/api/approvals", Require::Feature("approval")),
//! ("/api/v1/policies", Require::Feature("policy_engine")),
//! ("/api/orgs/{org_id}/webhooks", Require::Feature("webhooks")),
//! ("/api/orgs/{org_id}/webhooks", Require::OnWrite(LimitCheck::Count("max_webhooks"))),
//! ```
//!
//! All checks are dynamic — they read from the Plan struct which comes from DB.

use axum::{
    extract::State,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::sync::Arc;

use crate::server::AppState;

/// What a route requires from the plan.
#[derive(Debug, Clone)]
pub enum Require {
    /// A boolean feature must be enabled (e.g. "approval", "rbac").
    Feature(&'static str),
    /// A numeric limit must be checked only on write methods (POST/PUT/PATCH/DELETE).
    /// The actual count is fetched by the handler and validated via `PlanGateError`.
    LimitOnWrite(&'static str),
    /// SSO endpoints — requires "sso" feature.
    SSO,
}

/// Route prefix → requirement mapping.
/// Checked in order, first match wins. Use specific paths before wildcard prefixes.
pub static ROUTE_REQUIREMENTS: &[(&str, Require)] = &[
    // Shield approval (requires approval feature)
    ("/api/shield/approve", Require::Feature("approval")),
    ("/api/shield/reject", Require::Feature("approval")),
    // Approvals
    ("/api/approvals", Require::Feature("approval")),
    // Audit log
    ("/api/audit", Require::Feature("audit_log")),
    ("/api/v1/commands/history", Require::Feature("audit_log")),
    // RBAC (custom roles)
    ("/api/orgs/{org_id}/roles", Require::Feature("rbac")),
    ("/api/orgs/{org_id}/roles/{role_id}", Require::Feature("rbac")),
    // Webhooks
    ("/api/orgs/{org_id}/webhooks", Require::Feature("webhooks")),
    ("/api/orgs/{org_id}/webhooks/{id}", Require::Feature("webhooks")),
    // Policies
    ("/api/v1/policies", Require::Feature("policy_engine")),
    // Patterns
    ("/api/v1/patterns/apply", Require::Feature("pattern_learning")),
    // SIEM export
    ("/api/v1/siem", Require::Feature("siem_export")),
    // SAML/SSO
    ("/auth/saml", Require::SSO),
];

/// Error response for plan gate violations via middleware.
#[derive(Debug, Clone, Serialize)]
pub struct GateBlocked {
    pub error: &'static str,
    pub feature: Option<&'static str>,
    pub limit: Option<&'static str>,
    pub required_plan: Option<&'static str>,
    pub upgrade_url: Option<String>,
    pub message: String,
}

impl IntoResponse for GateBlocked {
    fn into_response(self) -> Response {
        (StatusCode::FORBIDDEN, axum::Json(self)).into_response()
    }
}

/// Feature → minimum plan tier mapping.
/// This is the SINGLE SOURCE OF TRUTH for upgrade hints.
pub fn feature_min_tier(feature: &str) -> &'static str {
    match feature {
        "approval" | "rbac" => "professional",
        "pattern_learning" | "siem_export" | "webhooks" | "policy_engine" => "scale",
        "sso" | "on_premise" => "enterprise",
        _ => "professional",
    }
}

/// Check if a feature is enabled on the plan.
fn has_feature(plan: &flowlink_billing::plans::Plan, feature: &str) -> bool {
    match feature {
        "shield" => plan.features.shield,
        "mcp_gateway" => plan.features.mcp_gateway,
        "policy_engine" => plan.features.policy_engine,
        "approval" => plan.features.approval,
        "rbac" => plan.features.rbac,
        "pattern_learning" => plan.features.pattern_learning,
        "e2ee" => plan.features.e2ee,
        "audit_log" => plan.features.audit_log,
        "webhooks" => plan.features.webhooks,
        "siem_export" => plan.features.siem_export,
        "sso" => plan.features.sso,
        "on_premise" => plan.features.on_premise,
        _ => false,
    }
}

/// Get a numeric limit value from the plan (0 = unlimited).
fn get_limit(plan: &flowlink_billing::plans::Plan, limit: &str) -> u64 {
    match limit {
        "max_agents" => plan.limits.max_agents,
        "max_users" => plan.limits.max_users,
        "max_custom_rules" => plan.limits.max_custom_rules,
        "max_policies" => plan.limits.max_policies,
        "max_webhooks" => plan.limits.max_webhooks,
        "audit_retention_days" => plan.limits.audit_retention_days,
        _ => 0, // unknown = unlimited
    }
}

/// Plan enforcement middleware.
///
/// Runs after auth middleware (Plan extension available).
/// Checks route against ROUTE_REQUIREMENTS table.
pub async fn plan_enforcement_middleware(
    State(_state): State<Arc<AppState>>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let path = req.uri().path();
    let method = req.method();

    // Find matching requirement
    let requirement = ROUTE_REQUIREMENTS.iter().find(|(prefix, _)| {
        path == *prefix || path.starts_with(&format!("{}/", prefix))
    });

    let req_rule = match requirement {
        None => return next.run(req).await,
        Some((_prefix, rule)) => rule,
    };

    // Skip enforcement paths that don't need auth (public endpoints, health, etc.)
    let skip_prefixes = [
        "/api/plans",
        "/api/billing",
        "/api/auth",
        "/api/account",
        "/health",
        "/healthz",
        "/metrics",
        "/api/events",
        "/api/shield/ingest",
        "/api/audit/event",
        "/api/shield/canary",
        "/api/devices",
        "/api/notifications",
        "/api/preferences",
        "/api/agents",
        "/api/servers",
        "/api/api-keys",
        "/api/v1/agents",
        "/api/v1/health",
        "/api/dashboard",
        "/ws",
        "/mcp",
        "/playground",
        "/auth/login",
        "/auth/register",
        "/auth/callback",
    ];
    for prefix in &skip_prefixes {
        if path.starts_with(prefix) {
            return next.run(req).await;
        }
    }

    // Extract plan from extensions
    let plan = req.extensions().get::<flowlink_billing::plans::Plan>().cloned();

    let plan = match plan {
        Some(p) => p,
        None => return next.run(req).await, // No plan = no enforcement
    };

    match req_rule {
        Require::Feature(feature) => {
            if !has_feature(&plan, feature) {
                let tier = feature_min_tier(feature);
                log::info!(
                    "Plan gate blocked: feature '{}' not on plan '{}' (path={})",
                    feature, plan.id, path
                );
                return GateBlocked {
                    error: "feature_not_available",
                    feature: Some(feature),
                    limit: None,
                    required_plan: Some(tier),
                    upgrade_url: Some(format!("/pricing?upgrade={}", tier)),
                    message: format!(
                        "Feature '{}' is not available on your current plan. Upgrade to {} or higher.",
                        feature, tier
                    ),
                }.into_response();
            }
        }
        Require::LimitOnWrite(limit_name) => {
            // Only enforce on write methods
            let is_write = method == axum::http::Method::POST
                || method == axum::http::Method::PUT
                || method == axum::http::Method::PATCH
                || method == axum::http::Method::DELETE;

            if is_write {
                let max = get_limit(&plan, limit_name);
                if max == 0 {
                    // 0 = unlimited, skip
                }
                // Note: actual count check happens in the handler.
                // Here we only check if the limit is defined (max > 0).
                // The handler calls `check_limit!` with the actual count.
            }

            // For /api/orgs routes, also check specific sub-paths
            if path.contains("/webhooks") && is_write {
                if !plan.features.webhooks {
                    let tier = feature_min_tier("webhooks");
                    return GateBlocked {
                        error: "feature_not_available",
                        feature: Some("webhooks"),
                        limit: None,
                        required_plan: Some(tier),
                        upgrade_url: Some(format!("/pricing?upgrade={}", tier)),
                        message: format!("Webhooks are not available on your current plan. Upgrade to {} or higher.", tier),
                    }.into_response();
                }
            }
            if path.contains("/roles") && is_write {
                if !plan.features.rbac {
                    let tier = feature_min_tier("rbac");
                    return GateBlocked {
                        error: "feature_not_available",
                        feature: Some("rbac"),
                        limit: None,
                        required_plan: Some(tier),
                        upgrade_url: Some(format!("/pricing?upgrade={}", tier)),
                        message: format!("RBAC is not available on your current plan. Upgrade to {} or higher.", tier),
                    }.into_response();
                }
            }
        }
        Require::SSO => {
            if !plan.features.sso {
                let tier = feature_min_tier("sso");
                return GateBlocked {
                    error: "feature_not_available",
                    feature: Some("sso"),
                    limit: None,
                    required_plan: Some(tier),
                    upgrade_url: Some(format!("/pricing?upgrade={}", tier)),
                    message: format!("SSO is not available on your current plan. Upgrade to {} or higher.", tier),
                }.into_response();
            }
        }
    }

    // Insert plan into a typed wrapper for handler access
    // (Plan is already in extensions from auth middleware)

    next.run(req).await
}

/// Helper for handlers that need to check a limit dynamically.
///
/// Usage in handler:
/// ```ignore
/// let plan = req.extensions().get::<Plan>().cloned();
/// enforce_limit!(plan, "max_policies", current_policy_count)?;
/// ```
#[macro_export]
macro_rules! enforce_limit {
    ($plan:expr, $limit:expr, $current:expr) => {{
        let plan = $plan.as_ref();
        match plan {
            Some(p) => {
                let max = match $limit {
                    "max_agents" => p.limits.max_agents,
                    "max_users" => p.limits.max_users,
                    "max_custom_rules" => p.limits.max_custom_rules,
                    "max_policies" => p.limits.max_policies,
                    "max_webhooks" => p.limits.max_webhooks,
                    _ => 0u64,
                };
                if max != 0 && ($current as u64) >= max {
                    let tier = $crate::plan_enforcement::feature_min_tier($limit);
                    return Err($crate::plan_gate::PlanGateError::limit_exceeded(
                        $limit,
                        $current as u64,
                        max,
                        Some(tier),
                    ));
                }
            }
            None => {} // No plan = no enforcement
        }
        Ok::<(), $crate::plan_gate::PlanGateError>(())
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowlink_billing::plans::{PlanFeatures, PlanLimits};

    fn make_starter() -> flowlink_billing::plans::Plan {
        flowlink_billing::plans::Plan {
            id: "starter".into(),
            name: "Starter".into(),
            description: "Free".into(),
            tier: 0,
            price_kopecks: 0,
            annual_price_kopecks: None,
            annual_discount_percent: 0,
            features: PlanFeatures {
                shield: true, shield_level: "basic".into(),
                mcp_gateway: true, policy_engine: true,
                approval: false, rbac: false, pattern_learning: false,
                e2ee: true, audit_log: true, webhooks: false,
                siem_export: false, sso: false, on_premise: false,
                forensics: false, service_catalog: false,
                ai_ops: false, change_management: false,
            },
            limits: PlanLimits {
                max_agents: 1, max_users: 1, audit_retention_days: 30,
                api_rate_limit: 100, api_rate_window_secs: 60,
                max_custom_rules: 3, max_policies: 1, max_webhooks: 0,
                approval_channels: vec![], siem_formats: vec![],
                allowed_shield_levels: vec![], support_tier: "community".into(),
            },
            available: true, legacy: false, trial_days: None,
            billing_period: "month".into(),
        }
    }

    fn make_pro() -> flowlink_billing::plans::Plan {
        flowlink_billing::plans::Plan {
            id: "professional".into(),
            name: "Professional".into(),
            description: "Pro".into(),
            tier: 1,
            price_kopecks: 199000,
            annual_price_kopecks: Some(1910400),
            annual_discount_percent: 20,
            features: PlanFeatures {
                shield: true, shield_level: "advanced".into(),
                mcp_gateway: true, policy_engine: true,
                approval: true, rbac: true, pattern_learning: false,
                e2ee: true, audit_log: true, webhooks: true,
                siem_export: true, sso: false, on_premise: false,
                forensics: false, service_catalog: false,
                ai_ops: false, change_management: false,
            },
            limits: PlanLimits {
                max_agents: 5, max_users: 5, audit_retention_days: 60,
                api_rate_limit: 500, api_rate_window_secs: 60,
                max_custom_rules: 50, max_policies: 5, max_webhooks: 3,
                approval_channels: vec!["telegram".into()],
                siem_formats: vec!["json".into()],
                allowed_shield_levels: vec![], support_tier: "email".into(),
            },
            available: true, legacy: false, trial_days: None,
            billing_period: "month".into(),
        }
    }

    #[test]
    fn test_has_feature_starter() {
        let plan = make_starter();
        assert!(has_feature(&plan, "shield"));
        assert!(has_feature(&plan, "policy_engine"));
        assert!(!has_feature(&plan, "approval"));
        assert!(!has_feature(&plan, "rbac"));
        assert!(!has_feature(&plan, "webhooks"));
        assert!(!has_feature(&plan, "sso"));
    }

    #[test]
    fn test_has_feature_pro() {
        let plan = make_pro();
        assert!(has_feature(&plan, "approval"));
        assert!(has_feature(&plan, "rbac"));
        assert!(has_feature(&plan, "webhooks"));
        assert!(has_feature(&plan, "siem_export"));
        assert!(!has_feature(&plan, "pattern_learning"));
        assert!(!has_feature(&plan, "sso"));
    }

    #[test]
    fn test_get_limit() {
        let plan = make_starter();
        assert_eq!(get_limit(&plan, "max_agents"), 1);
        assert_eq!(get_limit(&plan, "max_webhooks"), 0); // unlimited
        assert_eq!(get_limit(&plan, "unknown"), 0); // unknown = unlimited
    }

    #[test]
    fn test_feature_min_tier() {
        assert_eq!(feature_min_tier("approval"), "professional");
        assert_eq!(feature_min_tier("policy_engine"), "scale");
        assert_eq!(feature_min_tier("sso"), "enterprise");
    }

    #[test]
    fn test_route_requirements_coverage() {
        // Verify key routes are covered
        let paths = [
            "/api/approvals",
            "/api/approvals/123/approve",
            "/api/v1/policies",
            "/api/v1/policies/123",
            "/api/v1/patterns/apply",
            "/auth/saml/login",
        ];
        for path in &paths {
            let matched = ROUTE_REQUIREMENTS.iter().any(|(prefix, _)| {
                *path == *prefix || path.starts_with(&format!("{}/", prefix))
            });
            assert!(matched, "Route '{}' not covered by ROUTE_REQUIREMENTS", path);
        }
    }

    #[test]
    fn test_gate_blocked_serialization() {
        let blocked = GateBlocked {
            error: "feature_not_available",
            feature: Some("approval"),
            limit: None,
            required_plan: Some("professional"),
            upgrade_url: Some("/pricing?upgrade=professional".into()),
            message: "Feature 'approval' not available".into(),
        };
        let json = serde_json::to_string(&blocked).unwrap();
        assert!(json.contains("\"error\":\"feature_not_available\""));
        assert!(json.contains("\"upgrade_url\":\"/pricing?upgrade=professional\""));
    }
}

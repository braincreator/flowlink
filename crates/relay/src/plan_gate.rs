//! Plan gate — feature/limit enforcement for all request paths.
//!
//! # Usage
//!
//! ```ignore
//! use crate::plan_gate::{require_feature, check_limit, PlanGateError};
//!
//! async fn handler(Extension(plan): Extension<OptionalPlan>) -> impl IntoResponse {
//!     require_feature!(&plan.0, "approval")?;
//!     check_limit!(&plan.0, "max_agents", current_agents)?;
//!     // ... handler logic
//! }
//! ```
//!
//! The `Plan` extension is inserted by auth middleware (JWT + API key paths).
//! If no plan is found, requests proceed without enforcement (backward compat).

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};
use flowlink_billing::plans::Plan;
use serde::Serialize;

/// Extract the current user's plan from request extensions.
///
/// Returns `None` if plan is not set (unauthenticated or dev mode).
pub struct OptionalPlan(pub Option<Plan>);

impl<S: Send + Sync> FromRequestParts<S> for OptionalPlan {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(OptionalPlan(parts.extensions.get::<Plan>().cloned()))
    }
}

/// Error response for plan gate violations.
#[derive(Debug, Serialize)]
pub struct PlanGateError {
    pub error: String,
    pub feature: Option<String>,
    pub limit: Option<String>,
    pub current: Option<u64>,
    pub maximum: Option<u64>,
    pub required_plan: Option<String>,
    pub upgrade_url: Option<String>,
    pub message: String,
}

impl PlanGateError {
    /// Feature not available on current plan.
    pub fn feature_not_available(feature: &str, required_plan: Option<&str>) -> Self {
        Self {
            error: "feature_not_available".into(),
            feature: Some(feature.into()),
            limit: None,
            current: None,
            maximum: None,
            required_plan: required_plan.map(String::from),
            upgrade_url: required_plan.map(|p| format!("/pricing?upgrade={}", p)),
            message: required_plan
                .map(|p| format!("Feature '{}' requires {} plan or higher", feature, p))
                .unwrap_or_else(|| format!("Feature '{}' is not available on your plan", feature)),
        }
    }

    /// Limit exceeded on current plan.
    pub fn limit_exceeded(limit: &str, current: u64, maximum: u64, required_plan: Option<&str>) -> Self {
        Self {
            error: "limit_exceeded".into(),
            feature: None,
            limit: Some(limit.into()),
            current: Some(current),
            maximum: Some(maximum),
            required_plan: required_plan.map(String::from),
            upgrade_url: required_plan.map(|p| format!("/pricing?upgrade={}", p)),
            message: required_plan
                .map(|p| format!("Limit '{}' exceeded ({} of {}). Requires {} plan", limit, current, maximum, p))
                .unwrap_or_else(|| format!("Limit '{}' exceeded ({} of {})", limit, current, maximum)),
        }
    }
}

impl axum::response::IntoResponse for PlanGateError {
    fn into_response(self) -> axum::response::Response {
        let status = axum::http::StatusCode::FORBIDDEN;
        (status, axum::Json(self)).into_response()
    }
}

/// Check that a boolean feature is enabled on the plan.
///
/// # Returns
/// - `Ok(())` if feature is enabled or plan is not set (passthrough)
/// - `Err(PlanGateError)` with upgrade hint if feature is disabled
pub fn require_feature(plan: &Option<Plan>, feature: &str, required_plan: Option<&str>) -> Result<(), PlanGateError> {
    let plan = match plan {
        Some(p) => p,
        None => return Ok(()), // No plan = no enforcement (dev/unauthenticated)
    };

    let enabled = match feature {
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
    };

    if enabled {
        Ok(())
    } else {
        Err(PlanGateError::feature_not_available(feature, required_plan))
    }
}

/// Check that a numeric limit is not exceeded.
///
/// # Returns
/// - `Ok(())` if under limit or plan is not set
/// - `Err(PlanGateError)` with upgrade hint if limit exceeded
pub fn check_limit(plan: &Option<Plan>, limit: &str, current: u64, required_plan: Option<&str>) -> Result<(), PlanGateError> {
    let plan = match plan {
        Some(p) => p,
        None => return Ok(()),
    };

    let maximum = match limit {
        "max_agents" => plan.limits.max_agents,
        "max_users" => plan.limits.max_users,
        "audit_retention_days" => plan.limits.audit_retention_days,
        "api_rate_limit" => plan.limits.api_rate_limit as u64,
        "max_custom_rules" => plan.limits.max_custom_rules,
        "max_policies" => plan.limits.max_policies,
        "max_webhooks" => plan.limits.max_webhooks,
        _ => return Ok(()), // Unknown limit = no enforcement
    };

    // 0 means unlimited
    if maximum == 0 || current < maximum {
        Ok(())
    } else {
        Err(PlanGateError::limit_exceeded(limit, current, maximum, required_plan))
    }
}

/// Get the minimum plan tier required for a feature.
pub fn feature_min_tier(feature: &str) -> &'static str {
    match feature {
        "shield" | "e2ee" | "policy_engine" | "audit_log" => "starter",
        "approval" | "rbac" | "serverguard" | "forensics" | "service_catalog" | "ai_ops" => "professional",
        "pattern_learning" | "siem_export" | "webhooks" | "change_management" => "scale",
        "sso" | "on_premise" => "enterprise",
        _ => "professional",
    }
}

/// Get the minimum plan tier required when a limit is exceeded.
pub fn limit_min_tier(limit: &str) -> &'static str {
    match limit {
        "max_agents" | "max_users" | "max_custom_rules" => "professional",
        "max_policies" | "max_webhooks" => "scale",
        _ => "professional",
    }
}

/// Convenience macro: require a feature, auto-resolve required plan tier.
#[macro_export]
macro_rules! require_feature {
    ($plan:expr, $feature:expr) => {
        $crate::plan_gate::require_feature($plan, $feature, Some($crate::plan_gate::feature_min_tier($feature)))
    };
    ($plan:expr, $feature:expr, $required:expr) => {
        $crate::plan_gate::require_feature($plan, $feature, Some($required))
    };
}

/// Convenience macro: check a numeric limit, auto-resolve required plan tier.
#[macro_export]
macro_rules! check_limit {
    ($plan:expr, $limit:expr, $current:expr) => {
        $crate::plan_gate::check_limit($plan, $limit, $current as u64, Some($crate::plan_gate::limit_min_tier($limit)))
    };
    ($plan:expr, $limit:expr, $current:expr, $required:expr) => {
        $crate::plan_gate::check_limit($plan, $limit, $current as u64, Some($required))
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowlink_billing::plans::{PlanFeatures, PlanLimits};

    fn make_plan() -> Plan {
        Plan {
            id: "professional".into(),
            name: "Professional".into(),
            description: "Pro plan".into(),
            tier: 1,
            price_kopecks: 199000,
            annual_price_kopecks: Some(1910400),
            annual_discount_percent: 20,
            features: PlanFeatures {
                shield: true,
                shield_level: "advanced".into(),
                mcp_gateway: true,
                policy_engine: true,
                approval: true,
                rbac: true,
                pattern_learning: false,
                e2ee: true,
                audit_log: true,
                webhooks: true,
                siem_export: true,
                sso: false,
                on_premise: false,
                forensics: false,
                service_catalog: false,
                ai_ops: false,
                change_management: false,
                serverguard: false,
                serverguard_level: String::new(),
            },
            limits: PlanLimits {
                max_agents: 5,
                max_users: 5,
                audit_retention_days: 60,
                api_rate_limit: 500,
                api_rate_window_secs: 60,
                max_custom_rules: 50,
                max_policies: 5,
                max_webhooks: 3,
                approval_channels: vec!["telegram".into()],
                siem_formats: vec!["json".into()],
                allowed_shield_levels: vec![],
                support_tier: "email".into(),
            },
            available: true,
            legacy: false,
            trial_days: None,
            billing_period: "month".into(),
        }
    }

    fn make_starter_plan() -> Plan {
        Plan {
            id: "starter".into(),
            name: "Starter".into(),
            description: "Free".into(),
            tier: 0,
            price_kopecks: 0,
            annual_price_kopecks: None,
            annual_discount_percent: 0,
            features: PlanFeatures {
                shield: true,
                shield_level: "basic".into(),
                mcp_gateway: true,
                policy_engine: true,
                approval: false,
                rbac: false,
                pattern_learning: false,
                e2ee: true,
                audit_log: true,
                webhooks: false,
                siem_export: false,
                sso: false,
                on_premise: false,
                forensics: false,
                service_catalog: false,
                ai_ops: false,
                change_management: false,
                serverguard: false,
                serverguard_level: String::new(),
            },
            limits: PlanLimits {
                max_agents: 1,
                max_users: 1,
                audit_retention_days: 30,
                api_rate_limit: 100,
                api_rate_window_secs: 60,
                max_custom_rules: 3,
                max_policies: 1,
                max_webhooks: 0,
                approval_channels: vec![],
                siem_formats: vec![],
                allowed_shield_levels: vec![],
                support_tier: "community".into(),
            },
            available: true,
            legacy: false,
            trial_days: Some(14),
            billing_period: "month".into(),
        }
    }

    #[test]
    fn test_require_feature_enabled() {
        let plan = Some(make_plan());
        assert!(require_feature(&plan, "approval", None).is_ok());
    }

    #[test]
    fn test_require_feature_disabled() {
        let plan = Some(make_starter_plan());
        let err = require_feature(&plan, "approval", Some("professional")).unwrap_err();
        assert_eq!(err.error, "feature_not_available");
        assert_eq!(err.feature.as_deref(), Some("approval"));
        assert_eq!(err.required_plan.as_deref(), Some("professional"));
        assert!(err.upgrade_url.is_some());
    }

    #[test]
    fn test_require_feature_no_plan() {
        let plan: Option<Plan> = None;
        assert!(require_feature(&plan, "anything", None).is_ok());
    }

    #[test]
    fn test_check_limit_under() {
        let plan = Some(make_plan());
        assert!(check_limit(&plan, "max_agents", 3, None).is_ok());
    }

    #[test]
    fn test_check_limit_exceeded() {
        let plan = Some(make_plan());
        let err = check_limit(&plan, "max_agents", 7, Some("scale")).unwrap_err();
        assert_eq!(err.error, "limit_exceeded");
        assert_eq!(err.current, Some(7));
        assert_eq!(err.maximum, Some(5));
    }

    #[test]
    fn test_check_limit_unlimited() {
        let plan = Some(make_plan());
        // max_policies is 5, but let's test with a field that's 0 in starter
        let starter = Some(make_starter_plan());
        assert!(check_limit(&starter, "max_webhooks", 9999, None).is_ok()); // 0 = unlimited
    }

    #[test]
    fn test_check_limit_no_plan() {
        let plan: Option<Plan> = None;
        assert!(check_limit(&plan, "max_agents", 9999, None).is_ok());
    }

    #[test]
    fn test_feature_min_tier() {
        // Starter features
        assert_eq!(feature_min_tier("shield"), "starter");
        assert_eq!(feature_min_tier("e2ee"), "starter");
        assert_eq!(feature_min_tier("policy_engine"), "starter");
        assert_eq!(feature_min_tier("audit_log"), "starter");
        // Professional features
        assert_eq!(feature_min_tier("approval"), "professional");
        assert_eq!(feature_min_tier("rbac"), "professional");
        assert_eq!(feature_min_tier("serverguard"), "professional");
        assert_eq!(feature_min_tier("forensics"), "professional");
        assert_eq!(feature_min_tier("service_catalog"), "professional");
        assert_eq!(feature_min_tier("ai_ops"), "professional");
        // Scale features
        assert_eq!(feature_min_tier("pattern_learning"), "scale");
        assert_eq!(feature_min_tier("siem_export"), "scale");
        assert_eq!(feature_min_tier("webhooks"), "scale");
        assert_eq!(feature_min_tier("change_management"), "scale");
        // Enterprise features
        assert_eq!(feature_min_tier("sso"), "enterprise");
        assert_eq!(feature_min_tier("on_premise"), "enterprise");
        // Unknown defaults to professional
        assert_eq!(feature_min_tier("unknown_feature"), "professional");
    }

    #[test]
    fn test_limit_min_tier() {
        assert_eq!(limit_min_tier("max_agents"), "professional");
        assert_eq!(limit_min_tier("max_policies"), "scale");
    }

    #[test]
    fn test_plan_gate_error_serialization() {
        let err = PlanGateError::feature_not_available("sso", Some("enterprise"));
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"error\":\"feature_not_available\""));
        assert!(json.contains("\"upgrade_url\":\"/pricing?upgrade=enterprise\""));
    }

    #[test]
    fn test_limit_exceeded_serialization() {
        let err = PlanGateError::limit_exceeded("max_agents", 10, 5, Some("professional"));
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"current\":10"));
        assert!(json.contains("\"maximum\":5"));
        assert!(json.contains("\"message\":\"Limit 'max_agents' exceeded (10 of 5). Requires professional plan\""));
    }
}

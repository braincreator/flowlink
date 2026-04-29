use flowlink_service_traits::*;

#[test]
fn service_mode_equality() {
    assert_eq!(ServiceMode::Standalone, ServiceMode::Standalone);
    assert_ne!(ServiceMode::Standalone, ServiceMode::Cloud);
}

#[test]
fn service_mode_serde() {
    let mode = ServiceMode::Standalone;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, "\"standalone\"");
    let back: ServiceMode = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ServiceMode::Standalone);
}

#[test]
fn service_endpoints_default() {
    let ep = ServiceEndpoints::default();
    assert!(ep.billing_url.is_none());
    assert!(ep.auth_url.is_none());
    assert!(ep.agent_url.is_none());
    assert!(ep.shield_url.is_none());
}

#[test]
fn service_endpoints_with_urls() {
    let json = serde_json::json!({
        "billing_url": "http://billing:8080",
        "auth_url": "http://auth:8081",
    });
    let ep: ServiceEndpoints = serde_json::from_value(json).unwrap();
    assert_eq!(ep.billing_url.as_deref(), Some("http://billing:8080"));
    assert_eq!(ep.auth_url.as_deref(), Some("http://auth:8081"));
    assert!(ep.shield_url.is_none());
}

#[test]
fn agent_status_serialization() {
    let status = AgentStatus {
        agent_id: "a1".into(),
        hostname: "server1".into(),
        online: true,
        os: "linux".into(),
        version: Some("1.0".into()),
        last_seen: None,
    };
    let json = serde_json::to_string(&status).unwrap();
    assert!(json.contains("\"agent_id\":\"a1\""));
    assert!(json.contains("\"online\":true"));
    let back: AgentStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(back.agent_id, "a1");
    assert!(back.online);
}

#[test]
fn plan_info_serialization() {
    let plan = PlanInfo {
        plan_id: "pro".into(),
        name: "Pro".into(),
        price_kopecks: 9900,
        description: "Pro plan".into(),
        features: vec!["shield".into(), "audit".into()],
    };
    let json = serde_json::to_string(&plan).unwrap();
    let back: PlanInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.plan_id, "pro");
    assert_eq!(back.price_kopecks, 9900);
    assert_eq!(back.features.len(), 2);
}

#[test]
fn billing_account_info_serialization() {
    let info = BillingAccountInfo {
        account_id: "acc1".into(),
        plan_id: "free".into(),
        plan_name: "Free".into(),
        status: "active".into(),
        balance_kopecks: 5000,
        trial_ends_at: None,
    };
    let json = serde_json::to_string(&info).unwrap();
    let back: BillingAccountInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.balance_kopecks, 5000);
}

#[test]
fn auth_check_result_serialization() {
    let result = AuthCheckResult {
        account_id: "u1".into(),
        is_admin: true,
        org_id: Some("org1".into()),
        plan_id: Some("pro".into()),
    };
    let json = serde_json::to_string(&result).unwrap();
    let back: AuthCheckResult = serde_json::from_str(&json).unwrap();
    assert!(back.is_admin);
    assert_eq!(back.org_id.unwrap(), "org1");
}

#[test]
fn licence_info_serialization() {
    let info = LicenceInfo {
        key: "LIC-123".into(),
        customer: "ACME".into(),
        tier: "team".into(),
        max_agents: 20,
        max_users: 10,
        expires_at: chrono::Utc::now(),
        features: vec!["shield".into()],
        offline_until: chrono::Utc::now() + chrono::Duration::days(30),
    };
    let json = serde_json::to_string(&info).unwrap();
    let back: LicenceInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.max_agents, 20);
    assert_eq!(back.tier, "team");
}

#[test]
fn shield_alert_info_serialization() {
    let alert = ShieldAlertInfo {
        id: "al1".into(),
        agent_id: "a1".into(),
        risk: "high".into(),
        command: "rm -rf /".into(),
        resolved: false,
        created_at: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&alert).unwrap();
    let back: ShieldAlertInfo = serde_json::from_str(&json).unwrap();
    assert!(!back.resolved);
    assert_eq!(back.risk, "high");
}

#[test]
fn invoice_info_serialization() {
    let inv = InvoiceInfo {
        id: "inv1".into(),
        amount_kopecks: 9900,
        status: "paid".into(),
        created_at: chrono::Utc::now(),
        description: "Pro plan".into(),
    };
    let json = serde_json::to_string(&inv).unwrap();
    let back: InvoiceInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.amount_kopecks, 9900);
}

#[test]
fn usage_info_serialization() {
    let usage = UsageInfo {
        agents_connected: 5,
        commands_total: 100,
        commands_blocked: 3,
        storage_used_bytes: 1024,
        period: "2025-01".into(),
    };
    let json = serde_json::to_string(&usage).unwrap();
    let back: UsageInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.agents_connected, 5);
}

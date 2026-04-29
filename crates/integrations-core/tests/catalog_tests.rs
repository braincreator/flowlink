use flowlink_integrations_core::*;

#[test]
fn catalog_has_six_entries() {
    let catalog = builtin_catalog();
    assert_eq!(catalog.len(), 6);
}

#[test]
fn catalog_kinds() {
    let catalog = builtin_catalog();
    let kinds: Vec<&str> = catalog.iter().map(|m| m.kind.0.as_str()).collect();
    assert!(kinds.contains(&"telegram"));
    assert!(kinds.contains(&"slack"));
    assert!(kinds.contains(&"discord"));
    assert!(kinds.contains(&"github"));
    assert!(kinds.contains(&"max"));
    assert!(kinds.contains(&"webhook"));
}

#[test]
fn catalog_meta_complete() {
    for meta in builtin_catalog() {
        assert!(!meta.display_name.is_empty());
        assert!(!meta.description.is_empty());
        assert!(!meta.version.is_empty());
        assert!(!meta.icon.is_empty());
    }
}

#[test]
fn oauth_flags() {
    let catalog = builtin_catalog();
    for meta in &catalog {
        match meta.kind.0.as_str() {
            "slack" | "discord" | "github" => assert!(meta.requires_oauth, "{} should require oauth", meta.kind.0),
            "telegram" | "max" | "webhook" => assert!(!meta.requires_oauth, "{} should not require oauth", meta.kind.0),
            _ => {}
        }
    }
}

#[test]
fn event_descriptors_complete() {
    let events = EventDescriptor::all_events();
    assert!(events.len() >= 8);
    let types: Vec<&str> = events.iter().map(|e| e.event_type.as_str()).collect();
    for expected in &["agent_connected", "agent_disconnected", "shield_alert", "approval_requested", "approval_resolved", "payment_received", "subscription_changed", "system_alert"] {
        assert!(types.contains(expected), "missing event: {}", expected);
    }
}

#[test]
fn integration_event_roundtrip() {
    let events: Vec<IntegrationEvent> = vec![
        IntegrationEvent::AgentConnected { agent_id: "a".into(), hostname: "h".into() },
        IntegrationEvent::AgentDisconnected { agent_id: "a".into(), hostname: "h".into() },
        IntegrationEvent::ShieldAlert { agent_id: "a".into(), risk: "high".into(), command: "rm -rf /".into() },
        IntegrationEvent::ApprovalRequested { approval_id: "ap1".into(), agent_id: "a".into(), command: "cmd".into(), risk: "medium".into() },
        IntegrationEvent::ApprovalResolved { approval_id: "ap1".into(), decision: "approved".into(), resolved_by: "admin".into() },
        IntegrationEvent::PaymentReceived { account_id: "acc".into(), amount_kopecks: 10000, description: "plan".into() },
        IntegrationEvent::SubscriptionChanged { account_id: "acc".into(), plan: "pro".into() },
        IntegrationEvent::PlanExpiring { account_id: "acc".into(), days_left: 3 },
        IntegrationEvent::SystemAlert { level: "warn".into(), message: "disk full".into() },
        IntegrationEvent::Custom { name: "test".into(), data: "payload".into() },
    ];
    for event in &events {
        let json = serde_json::to_string(event).unwrap();
        let back: IntegrationEvent = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2);
    }
}

#[test]
fn event_deserialization_from_json() {
    let json = r#"{"type":"AgentConnected","payload":{"agent_id":"a1","hostname":"srv1"}}"#;
    let event: IntegrationEvent = serde_json::from_str(json).unwrap();
    match event {
        IntegrationEvent::AgentConnected { agent_id, hostname } => {
            assert_eq!(agent_id, "a1");
            assert_eq!(hostname, "srv1");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn integration_manager_starts_empty() {
    let mgr = IntegrationManager::new();
    assert!(mgr.list_available().is_empty());
}

#[test]
fn integration_kind_string() {
    let kind = IntegrationKind("telegram".into());
    assert_eq!(kind.0, "telegram");
    let kind2: IntegrationKind = "webhook".into();
    assert_eq!(kind2.0, "webhook");
}

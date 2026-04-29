use flowlink_integrations_webhook::*;
use flowlink_integrations_core::*;

#[test]
fn parse_minimal_config() {
    let json = serde_json::json!({
        "url": "https://example.com/hook"
    });
    let config = WebhookIntegration::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(config.validate_config(&json)).unwrap();
}

#[test]
fn parse_full_config() {
    let json = serde_json::json!({
        "url": "https://example.com/hook",
        "secret": "my-secret-key",
        "method": "POST",
        "timeout_secs": 15,
        "retries": 5,
        "event_filter": ["shield_alert", "approval_requested"],
        "headers": {
            "X-Custom": "value",
            "Authorization": "Bearer token123"
        }
    });
    let config = WebhookIntegration::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(config.validate_config(&json)).unwrap();
}

#[test]
fn reject_empty_url() {
    let json = serde_json::json!({"url": ""});
    let config = WebhookIntegration::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(config.validate_config(&json));
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        IntegrationError::ConfigError(msg) => assert!(msg.contains("url is required")),
        _ => panic!("wrong error type"),
    }
}

#[test]
fn reject_invalid_url_scheme() {
    let json = serde_json::json!({"url": "ftp://example.com/hook"});
    let config = WebhookIntegration::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(config.validate_config(&json));
    assert!(result.is_err());
}

#[test]
fn reject_missing_url() {
    let json = serde_json::json!({"secret": "abc"});
    let config = WebhookIntegration::new();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(config.validate_config(&json));
    assert!(result.is_err());
}

#[test]
fn webhook_payload_serialization() {
    let payload = WebhookPayload {
        event: "shield_alert".into(),
        timestamp: "2025-01-01T00:00:00Z".into(),
        account_id: "acc-1".into(),
        integration_id: "inst-1".into(),
        data: serde_json::json!({"agent_id": "a1", "risk": "high"}),
        signature: Some("sha256=abc123".into()),
    };
    let json = serde_json::to_string(&payload).unwrap();
    assert!(json.contains("shield_alert"));
    assert!(json.contains("acc-1"));
    assert!(json.contains("sha256=abc123"));
}

#[test]
fn webhook_payload_without_signature() {
    let payload = WebhookPayload {
        event: "agent_connected".into(),
        timestamp: "2025-01-01T00:00:00Z".into(),
        account_id: "acc-1".into(),
        integration_id: "inst-1".into(),
        data: serde_json::json!({}),
        signature: None,
    };
    let json = serde_json::to_string(&payload).unwrap();
    assert!(!json.contains("signature"), "signature should be omitted when None");
}

#[test]
fn hmac_signature_deterministic() {
    let body = b"test payload data";
    let sig1 = WebhookIntegration::sign_payload(body, "secret");
    let sig2 = WebhookIntegration::sign_payload(body, "secret");
    assert_eq!(sig1, sig2, "same input should produce same signature");
    assert!(sig1.starts_with("sha256="));
}

#[test]
fn hmac_signature_different_keys() {
    let body = b"test payload data";
    let sig1 = WebhookIntegration::sign_payload(body, "secret1");
    let sig2 = WebhookIntegration::sign_payload(body, "secret2");
    assert_ne!(sig1, sig2, "different keys should produce different signatures");
}

#[test]
fn hmac_signature_different_bodies() {
    let sig1 = WebhookIntegration::sign_payload(b"body1", "secret");
    let sig2 = WebhookIntegration::sign_payload(b"body2", "secret");
    assert_ne!(sig1, sig2, "different bodies should produce different signatures");
}

#[test]
fn config_defaults() {
    let json = serde_json::json!({"url": "https://example.com/hook"});
    let config: WebhookConfig = serde_json::from_value(json).unwrap();
    assert_eq!(config.method, "POST");
    assert_eq!(config.timeout_secs, 10);
    assert_eq!(config.retries, 3);
    assert!(config.event_filter.is_empty());
    assert!(config.headers.is_empty());
}

#[test]
fn integration_kind() {
    let integration = WebhookIntegration::new();
    assert_eq!(integration.kind().0, "webhook");
}

#[test]
fn webhook_does_not_support_commands() {
    let integration = WebhookIntegration::new();
    let config = IntegrationConfig {
        id: "test".into(),
        account_id: "acc".into(),
        org_id: None,
        name: "test".into(),
        kind: IntegrationKind("webhook".into()),
        config: serde_json::json!({"url": "https://example.com"}),
        subscribed_events: vec![],
        status: IntegrationStatus::Active,
        created_at: chrono::Utc::now(),
        oauth_tokens: None,
        updated_at: chrono::Utc::now(),
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(integration.handle_command("/start", &serde_json::json!({}), &config));
    assert!(result.is_err());
}

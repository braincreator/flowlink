use flowlink_integrations_max::*;
use flowlink_integrations_core::*;

#[test]
fn parse_valid_config() {
    let json = serde_json::json!({
        "access_token": "AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw",
        "chat_id": 1234567890
    });
    let config: MaxConfig = serde_json::from_value(json).unwrap();
    assert_eq!(config.access_token, "AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw");
    assert_eq!(config.chat_id, Some(1234567890));
}

#[test]
fn reject_empty_token() {
    let integration = MaxIntegration::new(None);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(integration.validate_config(&serde_json::json!({"access_token": ""})));
    assert!(result.is_err());
}

#[test]
fn integration_kind() {
    let integration = MaxIntegration::new(None);
    assert_eq!(integration.kind().0, "max");
}

#[test]
fn config_with_webhook() {
    let json = serde_json::json!({
        "access_token": "test-token",
        "chat_id": 999,
        "webhook_url": "https://example.com/max/webhook",
        "dashboard_url": "https://dash.example.com",
        "api_url": "http://localhost:3000",
        "api_token": "jwt"
    });
    let config: MaxConfig = serde_json::from_value(json).unwrap();
    assert_eq!(config.webhook_url.as_deref(), Some("https://example.com/max/webhook"));
    assert_eq!(config.api_token.as_deref(), Some("jwt"));
}

#[test]
fn config_minimal() {
    let json = serde_json::json!({"access_token": "token"});
    let config: MaxConfig = serde_json::from_value(json).unwrap();
    assert!(config.chat_id.is_none());
    assert!(config.webhook_url.is_none());
}

use flowlink_integrations_telegram::*;
use flowlink_integrations_core::*;

#[test]
fn parse_valid_config() {
    let json = serde_json::json!({
        "bot_token": "123456:ABC-DEF",
        "admin_chat_id": 12345,
        "dashboard_url": "https://dashboard.example.com"
    });
    let config: TelegramConfig = serde_json::from_value(json).unwrap();
    assert_eq!(config.bot_token, "123456:ABC-DEF");
    assert_eq!(config.admin_chat_id, Some(12345));
}

#[test]
fn reject_empty_token() {
    let integration = TelegramIntegration::new(None);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(integration.validate_config(&serde_json::json!({"bot_token": ""})));
    assert!(result.is_err());
}

#[test]
fn reject_invalid_token_format() {
    let integration = TelegramIntegration::new(None);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(integration.validate_config(&serde_json::json!({"bot_token": "invalid-no-colon"})));
    assert!(result.is_err());
}

#[test]
fn integration_kind() {
    let integration = TelegramIntegration::new(None);
    assert_eq!(integration.kind().0, "telegram");
}

#[test]
fn config_with_optional_fields() {
    let json = serde_json::json!({
        "bot_token": "123:ABC",
        "webhook_url": "https://example.com/webhook",
        "api_url": "http://localhost:3000",
        "api_token": "jwt-token"
    });
    let config: TelegramConfig = serde_json::from_value(json).unwrap();
    assert!(config.webhook_url.is_some());
    assert!(config.api_url.is_some());
    assert!(config.api_token.is_some());
}

#[test]
fn config_minimal() {
    let json = serde_json::json!({"bot_token": "123:ABC"});
    let config: TelegramConfig = serde_json::from_value(json).unwrap();
    assert!(config.admin_chat_id.is_none());
    assert!(config.webhook_url.is_none());
    assert!(config.dashboard_url.is_none());
}

use flowlink_relay::registry::Registry;
use flowlink_relay::auth::{AuthManager, Client};
use flowlink_relay::ratelimit::RateLimiter;

#[test]
fn test_registry_client_crud() {
    let dir = tempfile::tempdir().unwrap();
    let reg = Registry::new(dir.path()).unwrap();

    let client = reg.register_client("Test Corp".into(), "test@example.com".into()).unwrap();
    assert_eq!(client.name, "Test Corp");
    assert!(client.active);

    let found = reg.get_client(&client.id).unwrap();
    assert_eq!(found.name, "Test Corp");

    let by_token = reg.get_client_by_token(&client.api_token).unwrap();
    assert_eq!(by_token.id, client.id);

    assert_eq!(reg.list_clients().len(), 1);

    assert!(reg.deactivate_client(&client.id));
    assert!(!reg.get_client(&client.id).unwrap().active);
}

#[test]
fn test_registry_agent_crud() {
    let dir = tempfile::tempdir().unwrap();
    let reg = Registry::new(dir.path()).unwrap();

    let client = reg.register_client("Acme".into(), String::new()).unwrap();
    let agent = reg.register_agent(&client.id, "server-01".into(), "agent-token-123".into()).unwrap();
    assert_eq!(agent.name, "server-01");

    reg.update_agent_heartbeat(&agent.id);
    assert!(reg.get_agent(&agent.id).unwrap().last_seen.is_some());
}

#[test]
fn test_registry_persistence() {
    let dir = tempfile::tempdir().unwrap();
    {
        let reg = Registry::new(dir.path()).unwrap();
        let client = reg.register_client("Persist Test".into(), String::new()).unwrap();
        reg.register_agent(&client.id, "agent-p".into(), "token-p".into()).unwrap();
    }
    let reg2 = Registry::new(dir.path()).unwrap();
    assert_eq!(reg2.list_clients().len(), 1);
    assert_eq!(reg2.get_agent_by_token("token-p").unwrap().name, "agent-p");
}

#[test]
fn test_registry_agent_limit() {
    let dir = tempfile::tempdir().unwrap();
    let reg = Registry::new(dir.path()).unwrap();
    let client = reg.register_client("Limited".into(), String::new()).unwrap();
    for i in 0..10 {
        reg.register_agent(&client.id, format!("agent-{i}"), format!("token-{i}")).unwrap();
    }
    assert!(reg.register_agent(&client.id, "agent-11".into(), "token-11".into()).is_err());
}

#[test]
fn test_auth_manager() {
    let auth = AuthManager::new();
    auth.register_client(Client {
        client_id: "c1".into(),
        api_token: "secret123".into(),
        name: "Test Client".into(),
        active: true,
    });
    let found = auth.validate_token("secret123").unwrap();
    assert_eq!(found.client_id, "c1");
    assert!(auth.validate_token("wrong").is_none());
}

#[test]
fn test_rate_limiter() {
    let limiter = RateLimiter::new(3, 1);
    assert!(limiter.allow("key1"));
    assert!(limiter.allow("key1"));
    assert!(limiter.allow("key1"));
    assert!(!limiter.allow("key1"));
    assert!(limiter.allow("key2"));
}

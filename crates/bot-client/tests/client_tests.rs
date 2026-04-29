use flowlink_bot_client::*;

#[test]
fn create_clients() {
    let _local = FlowLinkClient::local("jwt");
    let _cloud = FlowLinkClient::cloud("jwt");
    let _custom = FlowLinkClient::new("http://custom:8080", AuthMethod::Jwt("j".into()));
    let _apikey = FlowLinkClient::new("http://localhost:3000", AuthMethod::ApiKey { key: "k".into(), secret: "s".into() });
    let _svc = FlowLinkClient::new("http://localhost:3000", AuthMethod::ServiceToken("svc".into()));
}

#[test]
fn health_response_serde() {
    let json = serde_json::json!({"status":"ok","version":"1.0","uptime_seconds":999,"agents_online":3,"db":"connected"});
    let resp: HealthResponse = serde_json::from_value(json).unwrap();
    assert_eq!(resp.status, "ok");
    assert_eq!(resp.version, "1.0");
    assert_eq!(resp.uptime_seconds, 999);
}

#[test]
fn agent_info_serde() {
    let agent = AgentInfo {
        agent_id: "a1".into(), hostname: "srv".into(), online: true,
        os: "linux".into(), last_seen: None, version: Some("1.0".into()),
    };
    let json = serde_json::to_string(&agent).unwrap();
    let back: AgentInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.agent_id, "a1");
    assert!(back.online);
    assert_eq!(back.version.unwrap(), "1.0");
}

#[test]
fn shield_alert_serde() {
    let alert = ShieldAlert {
        id: "al1".into(), agent_id: "a1".into(), risk: "critical".into(),
        command: "rm -rf /".into(), resolved: false, timestamp: "2025-01-01T00:00:00Z".into(),
    };
    let json = serde_json::to_string(&alert).unwrap();
    let back: ShieldAlert = serde_json::from_str(&json).unwrap();
    assert!(!back.resolved);
}

#[test]
fn approval_info_serde() {
    let a = ApprovalInfo {
        id: "ap1".into(), agent_id: "a1".into(), command: "cmd".into(),
        risk: "low".into(), created_at: "2025-01-01T00:00:00Z".into(),
    };
    let back: ApprovalInfo = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
    assert_eq!(back.id, "ap1");
}

#[test]
fn oauth_begin_response_serde() {
    let json = serde_json::json!({"authorize_url":"https://slack.com/oauth?state=x","integration_id":"inst-1"});
    let resp: OAuthBeginResponse = serde_json::from_value(json).unwrap();
    assert_eq!(resp.integration_id, "inst-1");
}

#[test]
fn integration_catalog_serde() {
    let json = serde_json::json!({"integrations":[{"kind":"telegram"}]});
    let cat: IntegrationCatalog = serde_json::from_value(json).unwrap();
    assert_eq!(cat.integrations.len(), 1);
}

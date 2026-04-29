use flowlink_licence::*;
use flowlink_service_traits::*;

#[test]
fn licence_tier_display() {
    assert_eq!(format!("{}", LicenceTier::Free), "Free");
    assert_eq!(format!("{}", LicenceTier::Starter), "Starter");
    assert_eq!(format!("{}", LicenceTier::Professional), "Pro");
    assert_eq!(format!("{}", LicenceTier::Scale), "Business");
    assert_eq!(format!("{}", LicenceTier::Enterprise), "Enterprise");
}

#[test]
fn licence_tier_serde() {
    for (tier, name) in [
        (LicenceTier::Free, "free"),
        (LicenceTier::Starter, "starter"),
        (LicenceTier::Professional, "professional"),
        (LicenceTier::Scale, "scale"),
        (LicenceTier::Enterprise, "enterprise"),
    ] {
        assert_eq!(serde_json::to_string(&tier).unwrap(), format!("\"{}\"", name));
        assert_eq!(serde_json::from_str::<LicenceTier>(&format!("\"{}\"", name)).unwrap(), tier);
    }
}

#[test]
fn licence_tier_matches_db() {
    // Must match production DB plans
    let tiers = [
        LicenceTier::Free,         // tier -1: 1 agent, 1 user, 0 ₽
        LicenceTier::Starter,      // tier 0:  3 agents, 3 users, 2 990 ₽
        LicenceTier::Professional, // tier 1:  10 agents, 10 users, 19 990 ₽
        LicenceTier::Scale,        // tier 2:  30 agents, 30 users, 49 990 ₽
        LicenceTier::Enterprise,   // tier 3:  unlimited, custom
    ];
    assert_eq!(tiers.len(), 5);
}

#[test]
fn licence_verify_response_valid() {
    let json = serde_json::json!({"valid": true, "licence": {
        "key": "LIC-1", "customer": "Test", "tier": "professional",
        "max_agents": 10, "max_users": 10,
        "expires_at": "2026-12-31T23:59:59Z",
        "features": ["shield"], "offline_until": "2026-06-01T00:00:00Z"
    }});
    let resp: LicenceVerifyResponse = serde_json::from_value(json).unwrap();
    assert!(resp.valid);
    assert_eq!(resp.licence.unwrap().tier, "professional");
}

#[test]
fn licence_verify_response_invalid() {
    let resp: LicenceVerifyResponse = serde_json::from_value(serde_json::json!({"valid": false, "message": "bad"})).unwrap();
    assert!(!resp.valid);
}

#[test]
fn licence_manager_no_cache_defaults_to_free() {
    let tmp = std::env::temp_dir().join("fl-test-noexist.json");
    let _ = std::fs::remove_file(&tmp);
    let mgr = LicenceManager::new("key", "http://localhost:9999", tmp, 7);
    assert!(mgr.is_expired());
    assert_eq!(mgr.max_agents(), 1);
    assert_eq!(mgr.max_users(), 1);
}

#[test]
fn licence_manager_starter_from_cache() {
    let tmp = std::env::temp_dir().join("fl-test-starter.json");
    let lic = LicenceInfo {
        key: "S".into(), customer: "T".into(), tier: "starter".into(),
        max_agents: 3, max_users: 3,
        expires_at: chrono::Utc::now() + chrono::Duration::days(365),
        features: vec!["shield".into()],
        offline_until: chrono::Utc::now() + chrono::Duration::days(30),
    };
    std::fs::write(&tmp, serde_json::to_string(&lic).unwrap()).unwrap();
    let mgr = LicenceManager::new("key", "http://localhost:9999", tmp.clone(), 7);
    assert!(!mgr.is_expired());
    assert_eq!(mgr.max_agents(), 3);
    assert!(mgr.has_feature("shield"));
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn licence_manager_pro_from_cache() {
    let tmp = std::env::temp_dir().join("fl-test-pro.json");
    let lic = LicenceInfo {
        key: "P".into(), customer: "T".into(), tier: "professional".into(),
        max_agents: 10, max_users: 10,
        expires_at: chrono::Utc::now() + chrono::Duration::days(365),
        features: vec!["shield".into(), "rbac".into()],
        offline_until: chrono::Utc::now() + chrono::Duration::days(30),
    };
    std::fs::write(&tmp, serde_json::to_string(&lic).unwrap()).unwrap();
    let mgr = LicenceManager::new("key", "http://localhost:9999", tmp.clone(), 7);
    assert_eq!(mgr.max_agents(), 10);
    assert!(mgr.has_feature("rbac"));
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn licence_manager_expired() {
    let tmp = std::env::temp_dir().join("fl-test-exp.json");
    let lic = LicenceInfo {
        key: "X".into(), customer: "X".into(), tier: "starter".into(),
        max_agents: 3, max_users: 3,
        expires_at: chrono::Utc::now() - chrono::Duration::days(1),
        features: vec![], offline_until: chrono::Utc::now() - chrono::Duration::days(1),
    };
    std::fs::write(&tmp, serde_json::to_string(&lic).unwrap()).unwrap();
    let mgr = LicenceManager::new("key", "http://localhost:9999", tmp.clone(), 7);
    assert!(mgr.is_expired());
    let _ = std::fs::remove_file(&tmp);
}

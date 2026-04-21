use crate::types::AuditEntry;
use anyhow::Result;
use flowlink_crypto::hmac_sha256_hex;

/// Compute HMAC-SHA256 for an audit entry
/// The HMAC is computed over the serialized entry fields (excluding the hmac field itself)
pub fn compute_hmac(entry: &AuditEntry, key: &[u8], prev_hmac: &str) -> Result<String> {
    let serialized = serialize_for_hmac(entry)?;
    let data = format!("{}||{}", prev_hmac, serialized);

    Ok(hmac_sha256_hex(key, data.as_bytes()))
}

/// Serialize entry fields for HMAC computation
/// Excludes the hmac field itself to avoid circular dependency
fn serialize_for_hmac(entry: &AuditEntry) -> Result<String> {
    // Create a JSON representation without the hmac field
    let mut map = serde_json::Map::new();

    map.insert("id".to_string(), serde_json::to_value(entry.id)?);
    map.insert(
        "timestamp".to_string(),
        serde_json::to_value(entry.timestamp)?,
    );
    map.insert(
        "agent_id".to_string(),
        serde_json::to_value(&entry.agent_id)?,
    );
    map.insert(
        "session_id".to_string(),
        serde_json::to_value(&entry.session_id)?,
    );
    map.insert("command".to_string(), serde_json::to_value(&entry.command)?);
    map.insert("args".to_string(), serde_json::to_value(&entry.args)?);
    map.insert("cwd".to_string(), serde_json::to_value(&entry.cwd)?);
    map.insert(
        "env_var_names".to_string(),
        serde_json::to_value(&entry.env_var_names)?,
    );
    map.insert(
        "risk_level".to_string(),
        serde_json::to_value(&entry.risk_level)?,
    );
    map.insert(
        "shield_verdict".to_string(),
        serde_json::to_value(&entry.shield_verdict)?,
    );
    map.insert(
        "shield_rules_matched".to_string(),
        serde_json::to_value(&entry.shield_rules_matched)?,
    );
    map.insert("tier".to_string(), serde_json::to_value(&entry.tier)?);
    map.insert(
        "original_command".to_string(),
        serde_json::to_value(&entry.original_command)?,
    );
    map.insert(
        "rewritten_command".to_string(),
        serde_json::to_value(&entry.rewritten_command)?,
    );
    map.insert(
        "rate_remaining".to_string(),
        serde_json::to_value(&entry.rate_remaining)?,
    );
    map.insert(
        "breaker_state".to_string(),
        serde_json::to_value(&entry.breaker_state)?,
    );
    map.insert(
        "exit_code".to_string(),
        serde_json::to_value(entry.exit_code)?,
    );
    map.insert(
        "stdout_hash".to_string(),
        serde_json::to_value(&entry.stdout_hash)?,
    );
    map.insert(
        "stderr_hash".to_string(),
        serde_json::to_value(&entry.stderr_hash)?,
    );
    map.insert(
        "duration_ms".to_string(),
        serde_json::to_value(entry.duration_ms)?,
    );
    map.insert(
        "files_modified".to_string(),
        serde_json::to_value(&entry.files_modified)?,
    );
    map.insert(
        "services_affected".to_string(),
        serde_json::to_value(&entry.services_affected)?,
    );
    map.insert(
        "containers_affected".to_string(),
        serde_json::to_value(&entry.containers_affected)?,
    );
    map.insert(
        "databases_affected".to_string(),
        serde_json::to_value(&entry.databases_affected)?,
    );
    map.insert(
        "git_commit".to_string(),
        serde_json::to_value(&entry.git_commit)?,
    );
    map.insert(
        "backup_id".to_string(),
        serde_json::to_value(&entry.backup_id)?,
    );
    map.insert(
        "rollback_available".to_string(),
        serde_json::to_value(entry.rollback_available)?,
    );
    map.insert(
        "health_check".to_string(),
        serde_json::to_value(&entry.health_check)?,
    );
    map.insert(
        "auto_restored".to_string(),
        serde_json::to_value(entry.auto_restored)?,
    );
    map.insert(
        "auto_restore_backup_id".to_string(),
        serde_json::to_value(&entry.auto_restore_backup_id)?,
    );
    map.insert(
        "policy_hash".to_string(),
        serde_json::to_value(&entry.policy_hash)?,
    );
    map.insert(
        "classification_rule".to_string(),
        serde_json::to_value(&entry.classification_rule)?,
    );

    let obj = serde_json::Value::Object(map);
    serde_json::to_string(&obj).map_err(|e| anyhow::anyhow!("Failed to serialize entry: {}", e))
}

/// Serialize an audit entry to JSON line format
pub fn to_jsonl(entry: &AuditEntry) -> Result<String> {
    serde_json::to_string(entry)
        .map_err(|e| anyhow::anyhow!("Failed to serialize entry to JSON: {}", e))
}

/// Deserialize an audit entry from JSON line format
pub fn from_jsonl(line: &str) -> Result<AuditEntry> {
    serde_json::from_str(line)
        .map_err(|e| anyhow::anyhow!("Failed to deserialize entry from JSON: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn create_test_entry() -> AuditEntry {
        AuditEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            agent_id: "test-agent".to_string(),
            session_id: "test-session".to_string(),
            command: "rm".to_string(),
            args: vec!["-rf".to_string(), "/tmp/test".to_string()],
            cwd: "/home/user".to_string(),
            env_var_names: vec!["PATH".to_string()],
            risk_level: RiskLevel::Medium,
            shield_verdict: ShieldVerdictType::Allow,
            shield_rules_matched: vec!["rule1".to_string()],
            tier: ActionTier::Destructive,
            original_command: None,
            rewritten_command: None,
            rate_remaining: None,
            breaker_state: None,
            exit_code: Some(0),
            stdout_hash: "abc123".to_string(),
            stderr_hash: "def456".to_string(),
            duration_ms: 150,
            files_modified: vec!["/tmp/test".to_string()],
            services_affected: vec![],
            containers_affected: vec![],
            databases_affected: vec![],
            git_commit: "commit123".to_string(),
            backup_id: Some("backup456".to_string()),
            rollback_available: true,
            health_check: None,
            auto_restored: false,
            auto_restore_backup_id: None,
            policy_hash: "policy789".to_string(),
            classification_rule: Some("test-rule".to_string()),
            hmac: String::new(),
        }
    }

    #[test]
    fn test_compute_hmac() {
        let entry = create_test_entry();
        let key = b"test-key";
        let prev_hmac = "0";

        let hmac = compute_hmac(&entry, key, prev_hmac).unwrap();
        assert!(!hmac.is_empty());
        assert_eq!(hmac.len(), 64); // SHA256 produces 32 bytes = 64 hex chars
    }

    #[test]
    fn test_hmac_chain_consistency() {
        let entry = create_test_entry();
        let key = b"test-key";
        let prev_hmac = "prev-hmac-value";

        let hmac1 = compute_hmac(&entry, key, prev_hmac).unwrap();
        let hmac2 = compute_hmac(&entry, key, prev_hmac).unwrap();

        assert_eq!(hmac1, hmac2);
    }

    #[test]
    fn test_jsonl_roundtrip() {
        let entry = create_test_entry();
        let jsonl = to_jsonl(&entry).unwrap();
        let deserialized = from_jsonl(&jsonl).unwrap();

        assert_eq!(entry.id, deserialized.id);
        assert_eq!(entry.command, deserialized.command);
        assert_eq!(entry.args, deserialized.args);
        assert_eq!(entry.tier, deserialized.tier);
    }
}

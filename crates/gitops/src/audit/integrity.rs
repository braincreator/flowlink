use crate::config::HmacKeySource;
use crate::types::{AuditEntry, IntegrityStatus};
use anyhow::{Context, Result};
use flowlink_crypto::{hmac_sha256_hex, sha256};
use tracing::{debug, warn};

pub struct IntegrityVerifier {
    key: Vec<u8>,
}

impl IntegrityVerifier {
    pub async fn new(source: &HmacKeySource) -> Result<Self> {
        let key = Self::derive_key(source)?;
        Ok(Self { key })
    }

    pub(crate) fn key(&self) -> &[u8] {
        &self.key
    }

    pub fn verify_chain(&self, entries: &[AuditEntry]) -> IntegrityStatus {
        let now = chrono::Utc::now();

        if entries.is_empty() {
            return IntegrityStatus {
                is_healthy: true,
                issues: vec![],
                warnings: vec!["No entries to verify".to_string()],
                last_checked: now,
            };
        }

        let mut issues = Vec::new();
        let warnings = Vec::new();

        for (i, entry) in entries.iter().enumerate() {
            let prev_hmac = if i == 0 {
                "0".to_string()
            } else {
                entries[i - 1].hmac.clone()
            };

            let expected_hmac = match compute_entry_hmac(entry, &self.key, &prev_hmac) {
                Ok(h) => h,
                Err(e) => {
                    warn!("Failed to compute HMAC for entry {}: {}", i, e);
                    issues.push(format!("Failed to compute HMAC for entry {}: {}", entry.id, e));
                    continue;
                }
            };

            if entry.hmac != expected_hmac {
                issues.push(format!(
                    "HMAC mismatch at entry {} (index {}): expected '{}', actual '{}'",
                    entry.id, i, expected_hmac, entry.hmac
                ));
            }
        }

        if issues.is_empty() {
            debug!("HMAC chain verification passed for {} entries", entries.len());
            IntegrityStatus {
                is_healthy: true,
                issues: vec![],
                warnings,
                last_checked: now,
            }
        } else {
            warn!("HMAC chain verification failed: {} issues found", issues.len());
            IntegrityStatus {
                is_healthy: false,
                issues,
                warnings,
                last_checked: now,
            }
        }
    }

    fn derive_key(source: &HmacKeySource) -> Result<Vec<u8>> {
        match source {
            HmacKeySource::MachineId => {
                let machine_id = Self::get_machine_id()?;
                Ok(sha256(machine_id.as_bytes()).to_vec())
            }
            HmacKeySource::ConfigKey { key } => {
                Ok(sha256(key.as_bytes()).to_vec())
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn get_machine_id() -> Result<String> {
        let content = std::fs::read_to_string("/etc/machine-id")
            .context("Failed to read /etc/machine-id")?;
        Ok(content.trim().to_string())
    }

    #[cfg(target_os = "macos")]
    fn get_machine_id() -> Result<String> {
        let output = std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
            .context("Failed to run ioreg")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains("IOPlatformUUID") {
                if let Some(eq_pos) = line.find('=') {
                    let uuid = line[eq_pos + 1..].trim().trim_matches('"');
                    if !uuid.is_empty() {
                        return Ok(uuid.to_string());
                    }
                }
            }
        }

        Err(anyhow::anyhow!("IOPlatformUUID not found in ioreg output"))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fn get_machine_id() -> Result<String> {
        Err(anyhow::anyhow!("Machine ID not available on this platform"))
    }
}

fn compute_entry_hmac(entry: &AuditEntry, key: &[u8], prev_hmac: &str) -> Result<String> {
    let serialized = serialize_entry_fields(entry)?;

    let data = format!("{}||{}", prev_hmac, serialized);

    Ok(hmac_sha256_hex(key, data.as_bytes()))
}

fn serialize_entry_fields(entry: &AuditEntry) -> Result<String> {
    let mut map = serde_json::Map::new();

    map.insert("id".to_string(), serde_json::to_value(&entry.id)?);
    map.insert("timestamp".to_string(), serde_json::to_value(&entry.timestamp)?);
    map.insert("agent_id".to_string(), serde_json::to_value(&entry.agent_id)?);
    map.insert("session_id".to_string(), serde_json::to_value(&entry.session_id)?);
    map.insert("command".to_string(), serde_json::to_value(&entry.command)?);
    map.insert("args".to_string(), serde_json::to_value(&entry.args)?);
    map.insert("cwd".to_string(), serde_json::to_value(&entry.cwd)?);
    map.insert("env_var_names".to_string(), serde_json::to_value(&entry.env_var_names)?);
    map.insert("risk_level".to_string(), serde_json::to_value(&entry.risk_level)?);
    map.insert("shield_verdict".to_string(), serde_json::to_value(&entry.shield_verdict)?);
    map.insert("shield_rules_matched".to_string(), serde_json::to_value(&entry.shield_rules_matched)?);
    map.insert("tier".to_string(), serde_json::to_value(&entry.tier)?);
    map.insert("original_command".to_string(), serde_json::to_value(&entry.original_command)?);
    map.insert("rewritten_command".to_string(), serde_json::to_value(&entry.rewritten_command)?);
    map.insert("rate_remaining".to_string(), serde_json::to_value(&entry.rate_remaining)?);
    map.insert("breaker_state".to_string(), serde_json::to_value(&entry.breaker_state)?);
    map.insert("exit_code".to_string(), serde_json::to_value(&entry.exit_code)?);
    map.insert("stdout_hash".to_string(), serde_json::to_value(&entry.stdout_hash)?);
    map.insert("stderr_hash".to_string(), serde_json::to_value(&entry.stderr_hash)?);
    map.insert("duration_ms".to_string(), serde_json::to_value(&entry.duration_ms)?);
    map.insert("files_modified".to_string(), serde_json::to_value(&entry.files_modified)?);
    map.insert("services_affected".to_string(), serde_json::to_value(&entry.services_affected)?);
    map.insert("containers_affected".to_string(), serde_json::to_value(&entry.containers_affected)?);
    map.insert("databases_affected".to_string(), serde_json::to_value(&entry.databases_affected)?);
    map.insert("git_commit".to_string(), serde_json::to_value(&entry.git_commit)?);
    map.insert("backup_id".to_string(), serde_json::to_value(&entry.backup_id)?);
    map.insert("rollback_available".to_string(), serde_json::to_value(&entry.rollback_available)?);
    map.insert("health_check".to_string(), serde_json::to_value(&entry.health_check)?);
    map.insert("auto_restored".to_string(), serde_json::to_value(&entry.auto_restored)?);
    map.insert("auto_restore_backup_id".to_string(), serde_json::to_value(&entry.auto_restore_backup_id)?);
    map.insert("policy_hash".to_string(), serde_json::to_value(&entry.policy_hash)?);
    map.insert("classification_rule".to_string(), serde_json::to_value(&entry.classification_rule)?);

    serde_json::to_string(&serde_json::Value::Object(map))
        .map_err(|e| anyhow::anyhow!("Failed to serialize entry: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HmacKeySource;
    use crate::types::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn create_test_entry(command: &str) -> AuditEntry {
        AuditEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            agent_id: "test-agent".to_string(),
            session_id: "test-session".to_string(),
            command: command.to_string(),
            args: vec![],
            cwd: "/tmp".to_string(),
            env_var_names: vec![],
            risk_level: RiskLevel::Low,
            shield_verdict: ShieldVerdictType::Allow,
            shield_rules_matched: vec![],
            tier: ActionTier::ReadOnly,
            original_command: None,
            rewritten_command: None,
            rate_remaining: None,
            breaker_state: None,
            exit_code: Some(0),
            stdout_hash: String::new(),
            stderr_hash: String::new(),
            duration_ms: 100,
            files_modified: vec![],
            services_affected: vec![],
            containers_affected: vec![],
            databases_affected: vec![],
            git_commit: String::new(),
            backup_id: None,
            rollback_available: false,
            health_check: None,
            auto_restored: false,
            auto_restore_backup_id: None,
            policy_hash: String::new(),
            classification_rule: None,
            hmac: String::new(),
        }
    }

    #[tokio::test]
    async fn test_derive_key_config_key() {
        let verifier = IntegrityVerifier::new(&HmacKeySource::ConfigKey {
            key: "test-key-123456".to_string(),
        })
        .await
        .unwrap();
        assert!(!verifier.key.is_empty());
    }

    #[tokio::test]
    async fn test_verify_chain_empty() {
        let verifier = IntegrityVerifier::new(&HmacKeySource::ConfigKey {
            key: "test-key".to_string(),
        })
        .await
        .unwrap();

        let entries: Vec<AuditEntry> = vec![];
        let status = verifier.verify_chain(&entries);
        assert!(status.is_healthy);
    }

    #[test]
    fn test_compute_entry_hmac_deterministic() {
        let key = b"test-key-123456";
        let entry = create_test_entry("ls");
        let hmac1 = compute_entry_hmac(&entry, key, "0").unwrap();
        let hmac2 = compute_entry_hmac(&entry, key, "0").unwrap();
        assert_eq!(hmac1, hmac2);
        assert_eq!(hmac1.len(), 64);
    }
}

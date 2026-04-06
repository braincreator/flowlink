// Audit log with HMAC integrity — port of internal/audit/hmac.go + internal/agent/audit.go
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, Write};
use std::path::PathBuf;

type HmacSha256 = Hmac<Sha256>;

const HMAC_FIELD: &str = "hmac";
const HMAC_SECRET_LEN: usize = 32;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: String,
    pub agent_id: String,
    pub action: String,
    pub command: Option<String>,
    pub path: Option<String>,
    pub risk_level: String,
    pub result: String,
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
    /// HMAC-SHA256 signature (computed from all other fields).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hmac: Option<String>,
}

pub struct AuditLog {
    log_dir: PathBuf,
    hmac_key: Vec<u8>,
}

impl AuditLog {
    pub fn new(log_dir: String, hmac_key: Vec<u8>) -> anyhow::Result<Self> {
        fs::create_dir_all(&log_dir)?;
        Ok(Self {
            log_dir: PathBuf::from(log_dir),
            hmac_key,
        })
    }

    /// Load or generate HMAC key from file.
    pub fn load_or_generate_key(key_path: Option<&str>) -> anyhow::Result<Vec<u8>> {
        let path = key_path
            .map(PathBuf::from)
            .unwrap_or_else(|| dirs_home().join(".flowlink").join("audit.key"));

        if let Ok(data) = fs::read(&path) {
            if data.len() >= HMAC_SECRET_LEN {
                return Ok(data[..HMAC_SECRET_LEN].to_vec());
            }
        }

        // Generate new key
        let key = Self::generate_key()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &key)?;
        Ok(key)
    }

    pub fn generate_key() -> anyhow::Result<Vec<u8>> {
        Ok(hmac::Hmac::<sha2::Sha256>::new_from_slice(b"flowlink-audit-key-gen")
            .expect("key")
            .finalize()
            .into_bytes()
            .to_vec())
    }

    /// Append an entry to today's JSONL log file with HMAC.
    pub fn log(&self, entry: &AuditEntry) -> anyhow::Result<()> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let filepath = self.log_dir.join(format!("audit-{}.jsonl", today));

        // Compute HMAC from all fields except "hmac"
        let signature = sign_entry(entry, &self.hmac_key);

        let mut entry_with_hmac = entry.clone();
        entry_with_hmac.hmac = Some(signature);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&filepath)?;
        let line = serde_json::to_string(&entry_with_hmac)?;
        writeln!(file, "{}", line)?;
        file.sync_all()?;

        Ok(())
    }

    /// Verify HMAC integrity of all entries in today's log.
    /// Returns Ok(true) if all entries are valid, Ok(false) if any are tampered.
    pub fn verify(&self) -> anyhow::Result<bool> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let filepath = self.log_dir.join(format!("audit-{}.jsonl", today));

        if !filepath.exists() {
            return Ok(true);
        }

        let file = File::open(&filepath)?;
        let reader = std::io::BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut entry: AuditEntry = serde_json::from_str(line)?;

            let stored_hmac = entry.hmac.take().unwrap_or_default();
            let expected = sign_entry(&entry, &self.hmac_key);

            if !hmac_constant_eq(&stored_hmac, &expected) {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

/// Compute HMAC-SHA256 of an entry's JSON representation (without the hmac field).
fn sign_entry(entry: &AuditEntry, secret: &[u8]) -> String {
    let mut map = serde_json::to_value(entry)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    map.remove(HMAC_FIELD);

    let json_bytes = serde_json::to_vec(&map).unwrap_or_default();
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC key error");
    mac.update(&json_bytes);
    hex_encode(mac.finalize().into_bytes().as_slice())
}

/// Constant-time HMAC comparison.
fn hmac_constant_eq(a: &str, b: &str) -> bool {
    use hmac::Mac;
    // Simple constant-time comparison via subtle or manual
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        result |= x ^ y;
    }
    result == 0
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

/// Verify a single raw JSON map entry (for generic use).
pub fn verify_entry(entry: &mut BTreeMap<String, serde_json::Value>, secret: &[u8]) -> bool {
    let stored = entry.remove(HMAC_FIELD).and_then(|v| v.as_str().map(String::from));
    match stored {
        Some(s) => {
            let json_bytes = serde_json::to_vec(entry).unwrap_or_default();
            let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC key error");
            mac.update(&json_bytes);
            let expected = hex_encode(mac.finalize().into_bytes().as_slice());
            hmac_constant_eq(&s, &expected)
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_log() -> AuditLog {
        let dir = tempfile::tempdir().unwrap();
        let key = vec![0u8; 32];
        AuditLog::new(dir.path().to_str().unwrap().into(), key).unwrap()
    }

    fn sample_entry() -> AuditEntry {
        AuditEntry {
            id: "e1".into(),
            timestamp: "2024-01-01T00:00:00Z".into(),
            agent_id: "agent-1".into(),
            action: "exec".into(),
            command: Some("echo hello".into()),
            path: None,
            risk_level: "none".into(),
            result: "ok".into(),
            duration_ms: Some(10),
            error: None,
            hmac: None,
        }
    }

    #[test]
    fn test_log_entry() {
        let log = test_log();
        let entry = sample_entry();
        log.log(&entry).unwrap();
    }

    #[test]
    fn test_verify_chain_integrity() {
        let log = test_log();
        for i in 0..5 {
            let mut entry = sample_entry();
            entry.id = format!("e{}", i);
            log.log(&entry).unwrap();
        }
        let valid = log.verify().unwrap();
        assert!(valid);
    }

    #[test]
    fn test_verify_tampered_entry() {
        let log = test_log();
        log.log(&sample_entry()).unwrap();

        // Tamper with the log file
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let filepath = log.log_dir.join(format!("audit-{}.jsonl", today));
        let content = std::fs::read_to_string(&filepath).unwrap();
        let tampered = content.replace("\"result\":\"ok\"", "\"result\":\"TAMPERED\"");
        std::fs::write(&filepath, tampered).unwrap();

        let valid = log.verify().unwrap();
        assert!(!valid);
    }

    #[test]
    fn test_verify_empty_log() {
        let log = test_log();
        assert!(log.verify().unwrap());
    }

    #[test]
    fn test_verify_entry_function() {
        let key = vec![0u8; 32];
        let mut entry = std::collections::BTreeMap::new();
        entry.insert("id".into(), serde_json::json!("test"));
        entry.insert("action".into(), serde_json::json!("exec"));
        // Sign it
        let json_bytes = serde_json::to_vec(&entry).unwrap();
        let mut mac = HmacSha256::new_from_slice(&key).unwrap();
        mac.update(&json_bytes);
        let sig = hex_encode(mac.finalize().into_bytes().as_slice());
        entry.insert("hmac".into(), serde_json::json!(sig));
        assert!(verify_entry(&mut entry, &key));
    }

    #[test]
    fn test_verify_entry_missing_hmac() {
        let key = vec![0u8; 32];
        let mut entry = std::collections::BTreeMap::new();
        entry.insert("id".into(), serde_json::json!("test"));
        assert!(!verify_entry(&mut entry, &key));
    }

    #[test]
    fn test_load_or_generate_key() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("audit.key");
        let key1 = AuditLog::load_or_generate_key(Some(key_path.to_str().unwrap())).unwrap();
        let key2 = AuditLog::load_or_generate_key(Some(key_path.to_str().unwrap())).unwrap();
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_generate_key_deterministic() {
        // The generate_key function uses a fixed seed, so it should be deterministic
        let k1 = AuditLog::generate_key().unwrap();
        let k2 = AuditLog::generate_key().unwrap();
        assert_eq!(k1, k2);
    }
}

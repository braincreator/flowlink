pub mod entry;
pub mod integrity;

use crate::config::AuditConfig;
use crate::types::{AuditEntry, IntegrityStatus};
use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tracing::{debug, info};

use entry::compute_hmac;
use integrity::IntegrityVerifier;

/// Audit trail with JSONL storage
pub struct AuditTrail {
    config: AuditConfig,
    storage_path: PathBuf,
}

impl AuditTrail {
    pub fn new(config: AuditConfig) -> Result<Self> {
        let storage_path = PathBuf::from(&config.storage_path);
        
        // Ensure audit directory exists
        std::fs::create_dir_all(&storage_path)
                .with_context(|| "Failed to create audit directory")?;
        
        Ok(Self {
            config,
            storage_path,
        })
    }
    
    /// Log an audit entry to the audit trail
    pub async fn log(&self, mut entry: AuditEntry) -> Result<()> {
        if !self.config.enabled {
            debug!("Audit logging is disabled");
            return Ok(());
        }
        
        // Get the date for file naming
        let date = entry.timestamp.date_naive();
        let file_path = self.get_log_file_path(date);
        
        // Get previous HMAC from the last entry of the day
        let prev_hmac = self.get_last_hmac(date).await?;
        
        // Derive HMAC key
        let verifier = IntegrityVerifier::new(&self.config.hmac_key_source).await?;
        let key = verifier.key();
        
        // Compute HMAC for this entry
        let hmac = compute_hmac(&entry, key, &prev_hmac)
            .with_context(|| format!("Failed to compute HMAC for audit entry: {}", entry.id))?;
        
        entry.hmac = hmac;
        
        // Append entry to JSONL file
        let json_line = serde_json::to_string(&entry)
            .with_context(|| format!("Failed to serialize audit entry: {}", entry.id))?;
            let mut line = json_line;
        line.push('\n');
        
        // Create parent directory if needed
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
            .with_context(|| "Failed to create audit directory")?;
        }
        
        // Append to file (create if doesn't exist)
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .await
            .with_context(|| format!("Failed to open audit file: {}", file_path.display()))?;
        
        file.write_all(line.as_bytes()).await
            .with_context(|| format!("Failed to write audit entry: {}", file_path.display()))?;
        
        info!("Logged audit entry {} to {}", entry.id, file_path.display());
        
        Ok(())
    }
    
    /// Get all audit entries for a specific date
    pub async fn get_log(&self, date: NaiveDate) -> Result<Vec<AuditEntry>> {
        let file_path = self.get_log_file_path(date);
        
        if !tokio::fs::try_exists(&file_path).await? {
            debug!("No audit log file for date: {}", date);
            return Ok(vec![]);
        }
        
        let content = tokio::fs::read_to_string(&file_path)
            .await
            .with_context(|| format!("Failed to read audit log file: {}", file_path.display()))?;
        
        let mut entries = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            
            let entry: AuditEntry = serde_json::from_str(line)
                .with_context(|| format!("Failed to parse audit entry: {}", line))?;
            entries.push(entry);
        }
        
        debug!("Retrieved {} audit entries for {}", entries.len(), date);
        Ok(entries)
    }
    
    /// Find a specific audit entry by ID
    pub async fn find_entry(&self, id: &str) -> Result<Option<AuditEntry>> {
        // Get today's date
        let today = Utc::now().date_naive();
        
        // Search today first
        if let Some(entry) = self.search_in_date(id, today).await? {
            return Ok(Some(entry));
        }
        
        // Search last 7 days (retention period)
        for days_back in 1..=7 {
            let date = today - chrono::Duration::days(days_back);
            if let Some(entry) = self.search_in_date(id, date).await? {
                return Ok(Some(entry));
            }
        }
        
        Ok(None)
    }
    
    /// Search for entry ID in a specific date
    async fn search_in_date(&self, id: &str, date: NaiveDate) -> Result<Option<AuditEntry>> {
        let entries = self.get_log(date).await?;
        Ok(entries.into_iter().find(|e| e.id.to_string() == id))
    }
    
    /// Verify the integrity of audit log for a specific date
    pub async fn verify_integrity(&self, date: NaiveDate) -> Result<IntegrityStatus> {
        let entries = self.get_log(date).await?;
        
        if entries.is_empty() {
            return Ok(IntegrityStatus {
                is_healthy: true,
                issues: vec![],
                warnings: vec!["No entries to verify".to_string()],
                last_checked: Utc::now(),
            });
        }
        
        let verifier = IntegrityVerifier::new(&self.config.hmac_key_source).await?;
        let status = verifier.verify_chain(&entries);
        
        info!("Integrity verification for {}: {} issues found", date, status.issues.len());
        
        Ok(status)
    }
    
    /// Search audit entries by text query in a date range
    pub async fn search(&self, query: &str, from: NaiveDate, to: NaiveDate) -> Result<Vec<AuditEntry>> {
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();
        
        let mut current_date = from;
        while current_date <= to {
            let entries = self.get_log(current_date).await?;
            
            for entry in entries {
                let mut matches = false;
                
                // Search in command
                if entry.command.to_lowercase().contains(&query_lower) {
                    matches = true;
                }
                
                // Search in args
                if !matches {
                    for arg in &entry.args {
                        if arg.to_lowercase().contains(&query_lower) {
                            matches = true;
                            break;
                        }
                    }
                }
                
                // Search in files modified
                if !matches {
                    for file in &entry.files_modified {
                        if file.to_lowercase().contains(&query_lower) {
                            matches = true;
                            break;
                        }
                    }
                }
                
                // Search in services affected
                if !matches {
                    for service in &entry.services_affected {
                        if service.to_lowercase().contains(&query_lower) {
                            matches = true;
                            break;
                        }
                    }
                }
                
                if matches {
                    results.push(entry);
                }
            }
            
            current_date = current_date + chrono::Duration::days(1);
        }
        
        debug!("Found {} audit entries matching query: {}", results.len(), query);
        Ok(results)
    }
    
    /// Get the log file path for a date
    fn get_log_file_path(&self, date: NaiveDate) -> PathBuf {
        self.storage_path.join(format!("audit/{}.jsonl", date.format("%Y-%m-%d")))
    }
    
    /// Get the HMAC of the last entry in a day's log
    async fn get_last_hmac(&self, date: NaiveDate) -> Result<String> {
        let entries = self.get_log(date).await?;
        
        if entries.is_empty() {
            return Ok("0".to_string());
        }
        
        Ok(entries.last().unwrap().hmac.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuditConfig, HmacKeySource};
    use crate::types::*;
    use tempfile::TempDir;
    use chrono::Utc;

    fn create_test_config(storage_path: &std::path::Path) -> AuditConfig {
        AuditConfig {
            enabled: true,
            storage_path: storage_path.to_string_lossy().to_string(),
            hmac_key_source: crate::config::HmacKeySource::ConfigKey { 
                key: "test-key-for-audit".to_string(),
            },
            retention_days: 30,
        }
    }

    fn create_test_entry(command: &str) -> AuditEntry {
        AuditEntry {
            id: uuid::Uuid::new_v4(),
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
            stdout_hash: "".to_string(),
            stderr_hash: "".to_string(),
            duration_ms: 100,
            files_modified: vec![],
            services_affected: vec![],
            containers_affected: vec![],
            databases_affected: vec![],
            git_commit: "".to_string(),
            backup_id: None,
            rollback_available: false,
            health_check: None,
            auto_restored: false,
            auto_restore_backup_id: None,
            policy_hash: "".to_string(),
            classification_rule: None,
            hmac: String::new(),
        }
    }

    #[tokio::test]
    async fn test_audit_log_and_retrieve() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(temp_dir.path());
        let mut trail = AuditTrail::new(config).unwrap();
        
        let entry = create_test_entry("ls -la");
        trail.log(entry.clone()).await.unwrap();
        
        let date = entry.timestamp.date_naive();
        let entries = trail.get_log(date).await.unwrap();
        
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].command, "ls -la");
    }

    #[tokio::test]
    async fn test_audit_find_entry() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(temp_dir.path());
        let mut trail = AuditTrail::new(config).unwrap();
        
        let entry = create_test_entry("rm test.txt");
        trail.log(entry.clone()).await.unwrap();
        
        let found = trail.find_entry(&entry.id.to_string()).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().command, "rm test.txt");
        
        let not_found = trail.find_entry("nonexistent-id").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_audit_verify_integrity() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(temp_dir.path());
        let mut trail = AuditTrail::new(config).unwrap();
        
        // Log multiple entries
        for i in 0..3 {
            let mut entry = create_test_entry(&format!("cmd{}", i));
            entry.timestamp = Utc::now();
            trail.log(entry).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        
        let date = Utc::now().date_naive();
        let status = trail.verify_integrity(date).await.unwrap();
        
        assert!(status.is_healthy);
    }

    #[tokio::test]
    async fn test_audit_search() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(temp_dir.path());
        let mut trail = AuditTrail::new(config).unwrap();
        
        let mut entry1 = create_test_entry("docker ps");
        entry1.timestamp = Utc::now();
        trail.log(entry1).await.unwrap();
        
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let mut entry2 = create_test_entry("kubectl get pods");
        entry2.timestamp = Utc::now();
        trail.log(entry2).await.unwrap();
        
        let today = Utc::now().date_naive();
        let results = trail.search("docker", today, today).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, "docker ps");
        
        let results = trail.search("kubectl", today, today).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command, "kubectl get pods");
    }
}

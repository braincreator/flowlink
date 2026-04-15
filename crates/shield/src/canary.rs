// FlowLink Shield — Canary Token Monitoring
// Honeypot file system: detect unauthorized access to decoy files
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use flowlink_core::channels::{AlertThreshold, CanaryToken};

// ═══════════════════════════════════════════════
// Canary Watcher
// ═══════════════════════════════════════════════

pub struct CanaryWatcher {
    tokens: Vec<CanaryToken>,
    watched_paths: Vec<PathBuf>,
}

impl CanaryWatcher {
    pub fn new(tokens: Vec<CanaryToken>) -> Self {
        let watched_paths = tokens.iter().map(|t| PathBuf::from(&t.path)).collect();
        Self {
            tokens,
            watched_paths,
        }
    }

    /// Create decoy honeypot files on disk
    pub fn create_decoy_files(&self) -> anyhow::Result<Vec<PathBuf>> {
        let mut created = Vec::new();
        for token in &self.tokens {
            let path = PathBuf::from(&token.path);
            if path.exists() {
                continue;
            }

            // Create parent dirs
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Create decoy content based on description
            let content = match token.description.as_str() {
                d if d.contains("shadow") => {
                    "root:x:0:0:root:/root:/bin/bash\ndaemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin\n".to_string()
                }
                d if d.contains("AWS") || d.contains("aws") => {
                    "[default]\naws_access_key_id = AKIAFAKEDEMOKEY123456\naws_secret_access_key = abcdefghijklmnopqrstuvwxyz0123456789ABCDEF\nregion = us-east-1\n".to_string()
                }
                d if d.contains("encryption") || d.contains("key") => {
                    "-----BEGIN ENCRYPTED KEY-----\ndGVzdC1mYWtlLWVuY3J5cHRpb24ta2V5LWRhdGE=\n-----END ENCRYPTED KEY-----\n".to_string()
                }
                _ => "# FlowLink Canary Token - DO NOT ACCESS\n".to_string(),
            };

            fs::write(&path, &content)?;
            // Set restrictive permissions (readable by root only)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = fs::Permissions::from_mode(0o600);
                fs::set_permissions(&path, perms)?;
            }
            created.push(path);
        }
        Ok(created)
    }

    /// Check if a file access should trigger an alert
    pub fn check_access(
        &self,
        path: &str,
        accessor_name: &str,
        accessor_uid: u32,
    ) -> Option<CanaryAlert> {
        let token = self.tokens.iter().find(|t| t.path == path)?;

        let should_alert = match &token.alert_threshold {
            AlertThreshold::Any => true,
            AlertThreshold::UnknownUser => {
                !token.expected_readers.contains(&accessor_name.to_string())
            }
            AlertThreshold::NonAdmin => {
                !is_admin(accessor_name, accessor_uid, &token.expected_readers)
            }
        };

        if !should_alert {
            return None;
        }

        let now = chrono::Utc::now();
        Some(CanaryAlert {
            token_path: path.to_string(),
            accessor: accessor_name.to_string(),
            accessor_uid,
            access_type: "read".to_string(),
            timestamp_nanos: now.timestamp_nanos_opt().unwrap_or(0) as u64,
            risk: if token.alert_threshold == AlertThreshold::Any {
                "high".to_string()
            } else {
                "medium".to_string()
            },
        })
    }

    pub fn list_tokens(&self) -> &[CanaryToken] {
        &self.tokens
    }

    pub fn watched_paths(&self) -> &[PathBuf] {
        &self.watched_paths
    }
}

fn is_admin(name: &str, _uid: u32, expected: &[String]) -> bool {
    expected.contains(&name.to_string()) || name == "root" || _uid == 0
}

// ═══════════════════════════════════════════════
// Canary Alert
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryAlert {
    pub token_path: String,
    pub accessor: String,
    pub accessor_uid: u32,
    pub access_type: String,
    pub timestamp_nanos: u64,
    pub risk: String,
}

// ═══════════════════════════════════════════════
// Config loading
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CanaryConfig {
    pub tokens: Vec<CanaryToken>,
}

impl CanaryConfig {
    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: CanaryConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}

// ═══════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tokens() -> Vec<CanaryToken> {
        vec![
            CanaryToken {
                path: "/tmp/canary-test/shadow.bak".into(),
                description: "Fake shadow file".into(),
                expected_readers: vec!["root".into()],
                alert_threshold: AlertThreshold::Any,
            },
            CanaryToken {
                path: "/tmp/canary-test/aws-creds".into(),
                description: "Fake AWS credentials".into(),
                expected_readers: vec!["deploy".into(), "admin".into()],
                alert_threshold: AlertThreshold::NonAdmin,
            },
            CanaryToken {
                path: "/tmp/canary-test/secret-key".into(),
                description: "Secret key".into(),
                expected_readers: vec!["root".into()],
                alert_threshold: AlertThreshold::UnknownUser,
            },
        ]
    }

    #[test]
    fn test_create_decoy_files() {
        let dir = tempfile::tempdir().unwrap();
        let tokens = vec![
            CanaryToken {
                path: dir.path().join("decoy1").to_str().unwrap().into(),
                description: "Fake shadow".into(),
                expected_readers: vec![],
                alert_threshold: AlertThreshold::Any,
            },
            CanaryToken {
                path: dir.path().join("decoy2").to_str().unwrap().into(),
                description: "Fake AWS credentials".into(),
                expected_readers: vec![],
                alert_threshold: AlertThreshold::Any,
            },
        ];
        let watcher = CanaryWatcher::new(tokens);
        let created = watcher.create_decoy_files().unwrap();
        assert_eq!(created.len(), 2);
        // Verify content
        let content = std::fs::read_to_string(dir.path().join("decoy1")).unwrap();
        assert!(content.contains("root:"));
        let content2 = std::fs::read_to_string(dir.path().join("decoy2")).unwrap();
        assert!(content2.contains("AKIAFAKE"));
    }

    #[test]
    fn test_check_access_any_threshold() {
        let watcher = CanaryWatcher::new(make_tokens());
        // Any access triggers alert
        let alert = watcher.check_access("/tmp/canary-test/shadow.bak", "root", 0);
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().risk, "high");
    }

    #[test]
    fn test_check_access_non_admin_allowed() {
        let watcher = CanaryWatcher::new(make_tokens());
        // deploy is in expected_readers
        let alert = watcher.check_access("/tmp/canary-test/aws-creds", "deploy", 1000);
        assert!(alert.is_none());
    }

    #[test]
    fn test_check_access_non_admin_triggered() {
        let watcher = CanaryWatcher::new(make_tokens());
        // random user is NOT in expected_readers
        let alert = watcher.check_access("/tmp/canary-test/aws-creds", "hacker", 1001);
        assert!(alert.is_some());
        assert_eq!(alert.as_ref().unwrap().accessor, "hacker");
    }

    #[test]
    fn test_check_access_unknown_user_allowed() {
        let watcher = CanaryWatcher::new(make_tokens());
        // root is in expected_readers
        let alert = watcher.check_access("/tmp/canary-test/secret-key", "root", 0);
        assert!(alert.is_none());
    }

    #[test]
    fn test_check_access_unknown_user_triggered() {
        let watcher = CanaryWatcher::new(make_tokens());
        let alert = watcher.check_access("/tmp/canary-test/secret-key", "stranger", 2000);
        assert!(alert.is_some());
    }

    #[test]
    fn test_check_access_non_watched_path() {
        let watcher = CanaryWatcher::new(make_tokens());
        let alert = watcher.check_access("/etc/passwd", "root", 0);
        assert!(alert.is_none());
    }

    #[test]
    fn test_list_tokens() {
        let watcher = CanaryWatcher::new(make_tokens());
        assert_eq!(watcher.list_tokens().len(), 3);
    }

    #[test]
    fn test_canary_alert_serialization() {
        let alert = CanaryAlert {
            token_path: "/etc/shadow.bak".into(),
            accessor: "hacker".into(),
            accessor_uid: 1001,
            access_type: "read".into(),
            timestamp_nanos: 1234567890,
            risk: "high".into(),
        };
        let json = serde_json::to_string(&alert).unwrap();
        let back: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(back["accessor"], "hacker");
        assert_eq!(back["risk"], "high");
    }

    #[test]
    fn test_config_loading() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("canary.yaml");
        std::fs::write(
            &config_path,
            r#"
tokens:
  - path: "/tmp/test1"
    description: "test"
    expected_readers: ["root"]
    alert_threshold: Any
"#,
        )
        .unwrap();
        let config = CanaryConfig::load_from_file(&config_path).unwrap();
        assert_eq!(config.tokens.len(), 1);
        assert_eq!(config.tokens[0].path, "/tmp/test1");
    }
}

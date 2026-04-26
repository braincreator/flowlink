// HashiCorp Vault Client — KV v2 secrets engine
// Writes approved discovery secrets to Vault with proper hierarchy
// Relay uses AppRole or Token auth — never stores Vault root token in DB

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Vault configuration (from relay.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    /// Vault server URL (e.g. "https://vault.flow-masters.ru:8200")
    pub address: String,
    /// Auth method: "token" or "approle"
    pub auth_method: String,
    /// Static token (for token auth)
    pub token: Option<String>,
    /// AppRole credentials
    pub role_id: Option<String>,
    pub secret_id: Option<String>,
    /// KV v2 mount path (default: "secret")
    pub mount_path: Option<String>,
    /// Namespace (for Vault Enterprise)
    pub namespace: Option<String>,
}

/// KV v2 secret entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSecret {
    /// Path within the mount, e.g. "{org_id}/{host}/{service}"
    pub path: String,
    /// Key-value pairs to store
    pub data: HashMap<String, String>,
    /// Metadata (version info, source, etc.)
    pub metadata: HashMap<String, String>,
}

/// Vault API response for KV v2 read
#[derive(Debug, Deserialize)]
struct KvV2ReadResponse {
    data: KvV2ReadData,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct KvV2ReadData {
    data: serde_json::Value,
    metadata: KvV2Metadata,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct KvV2Metadata {
    version: u64,
    created_time: String,
    destroyed: bool,
}

/// Vault API response for KV v2 write
#[derive(Debug, Deserialize)]
struct KvV2WriteResponse {
    data: KvV2WriteData,
}

#[derive(Debug, Deserialize)]
struct KvV2WriteData {
    version: u64,
}

/// Vault client
pub struct VaultClient {
    config: VaultConfig,
    http: reqwest::Client,
    cached_token: tokio::sync::RwLock<Option<String>>,
    mount_path: String,
}

impl VaultClient {
    pub fn new(config: VaultConfig) -> Result<Self> {
        let mount_path = config.mount_path.clone().unwrap_or_else(|| "secret".into());
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .danger_accept_invalid_certs(false)
            .build()
            .context("Failed to build HTTP client for Vault")?;

        Ok(Self {
            config,
            http,
            cached_token: tokio::sync::RwLock::new(None),
            mount_path,
        })
    }

    /// Get a valid token — either from config or via AppRole login
    async fn get_token(&self) -> Result<String> {
        // Check cache first
        {
            let cached = self.cached_token.read().await;
            if let Some(token) = cached.as_ref() {
                // TODO: check TTL, renew if needed
                return Ok(token.clone());
            }
        }

        let token = match self.config.auth_method.as_str() {
            "token" => {
                self.config.token.clone()
                    .context("Vault token not configured")?
            }
            "approle" => {
                self.login_approle().await?
            }
            _ => anyhow::bail!("Unsupported Vault auth method: {}", self.config.auth_method),
        };

        // Cache the token
        {
            let mut cached = self.cached_token.write().await;
            *cached = Some(token.clone());
        }

        Ok(token)
    }

    /// Login via AppRole
    async fn login_approle(&self) -> Result<String> {
        let role_id = self.config.role_id.as_ref()
            .context("Vault AppRole role_id not configured")?;
        let secret_id = self.config.secret_id.as_ref()
            .context("Vault AppRole secret_id not configured")?;

        let url = format!("{}/v1/auth/approle/login", self.config.address);
        let body = serde_json::json!({
            "role_id": role_id,
            "secret_id": secret_id,
        });

        let resp = self.http.post(&url)
            .json(&body)
            .send()
            .await
            .context("Vault AppRole login request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Vault AppRole login failed: {} {}", status, body);
        }

        #[derive(Deserialize)]
        struct LoginResponse {
            auth: AuthInfo,
        }
        #[derive(Deserialize)]
        struct AuthInfo {
            client_token: String,
        }

        let login: LoginResponse = resp.json().await
            .context("Failed to parse Vault login response")?;

        Ok(login.auth.client_token)
    }

    /// Build URL for KV v2 operation
    fn kv_url(&self, path: &str) -> String {
        format!("{}/v1/{}/data/{}", self.config.address, self.mount_path, path)
    }

    /// Build URL for KV v2 metadata
    fn kv_metadata_url(&self, path: &str) -> String {
        format!("{}/v1/{}/metadata/{}", self.config.address, self.mount_path, path)
    }

    /// Write a secret to Vault KV v2
    pub async fn write_secret(&self, secret: &VaultSecret) -> Result<u64> {
        let token = self.get_token().await?;
        let url = self.kv_url(&secret.path);

        let mut body = serde_json::json!({
            "data": secret.data,
            "options": {
                "cas": 0,  // Only write if key doesn't exist yet (create-only)
            }
        });

        // Add custom metadata if provided
        if !secret.metadata.is_empty() {
            body["options"]["custom_metadata"] = serde_json::to_value(&secret.metadata)?;
        }

        let mut req = self.http.post(&url)
            .header("X-Vault-Token", &token)
            .header("X-Vault-Request", "true")
            .json(&body);

        if let Some(ns) = &self.config.namespace {
            req = req.header("X-Vault-Namespace", ns);
        }

        let resp = req.send().await
            .context("Vault write request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            // If cas mismatch (secret already exists), that's OK — don't overwrite
            if body.contains("cas") || body.contains("check-and-set") {
                log::warn!("Vault secret {} already exists, skipping (cas=0)", secret.path);
                return Ok(0); // version 0 = not written (already existed)
            }
            anyhow::bail!("Vault write failed for {}: {} {}", secret.path, status, body);
        }

        let write_resp: KvV2WriteResponse = resp.json().await
            .context("Failed to parse Vault write response")?;

        log::info!("Vault write OK: {} v{}", secret.path, write_resp.data.version);
        Ok(write_resp.data.version)
    }

    /// Read a secret from Vault KV v2
    pub async fn read_secret(&self, path: &str) -> Result<Option<HashMap<String, String>>> {
        let token = self.get_token().await?;
        let url = self.kv_url(path);

        let mut req = self.http.get(&url)
            .header("X-Vault-Token", &token)
            .header("X-Vault-Request", "true");

        if let Some(ns) = &self.config.namespace {
            req = req.header("X-Vault-Namespace", ns);
        }

        let resp = req.send().await
            .context("Vault read request failed")?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Vault read failed for {}: {} {}", path, status, body);
        }

        let read_resp: KvV2ReadResponse = resp.json().await
            .context("Failed to parse Vault read response")?;

        let map: HashMap<String, String> = serde_json::from_value(read_resp.data.data)
            .unwrap_or_default();

        Ok(Some(map))
    }

    /// Delete a secret from Vault KV v2
    pub async fn delete_secret(&self, path: &str) -> Result<()> {
        let token = self.get_token().await?;
        let url = self.kv_metadata_url(path);

        let mut req = self.http.delete(&url)
            .header("X-Vault-Token", &token)
            .header("X-Vault-Request", "true");

        if let Some(ns) = &self.config.namespace {
            req = req.header("X-Vault-Namespace", ns);
        }

        let resp = req.send().await
            .context("Vault delete request failed")?;

        if !resp.status().is_success() && resp.status().as_u16() != 404 {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Vault delete failed for {}: {} {}", path, status, body);
        }

        Ok(())
    }

    /// List secrets at a path prefix
    pub async fn list_secrets(&self, path: &str) -> Result<Vec<String>> {
        let token = self.get_token().await?;
        // KV v2 list uses metadata endpoint
        let url = format!("{}/v1/{}/metadata/{}?list=true",
            self.config.address, self.mount_path, path);

        let mut req = self.http.get(&url)
            .header("X-Vault-Token", &token)
            .header("X-Vault-Request", "true");

        if let Some(ns) = &self.config.namespace {
            req = req.header("X-Vault-Namespace", ns);
        }

        let resp = req.send().await
            .context("Vault list request failed")?;

        if resp.status().as_u16() == 404 {
            return Ok(vec![]);
        }

        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        #[derive(Deserialize)]
        struct ListResponse {
            data: ListData,
        }
        #[derive(Deserialize)]
        struct ListData {
            keys: Vec<String>,
        }

        let list_resp: ListResponse = resp.json().await
            .context("Failed to parse Vault list response")?;

        Ok(list_resp.data.keys)
    }

    /// Health check — is Vault reachable and unsealed?
    pub async fn health(&self) -> Result<VaultHealth> {
        let url = format!("{}/v1/sys/health", self.config.address);
        let resp = self.http.get(&url).send().await;

        match resp {
            Ok(resp) => {
                let status = resp.status().as_u16();
                Ok(VaultHealth {
                    reachable: true,
                    initialized: status != 501,
                    sealed: status == 503,
                    standby: status == 429,
                })
            }
            Err(_e) => Ok(VaultHealth {
                reachable: false,
                initialized: false,
                sealed: true,
                standby: false,
            }),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct VaultHealth {
    pub reachable: bool,
    pub initialized: bool,
    pub sealed: bool,
    pub standby: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_config_deserialize() {
        let config: VaultConfig = serde_json::from_str(r#"{
            "address": "https://vault.example.com:8200",
            "auth_method": "token",
            "token": "s.12345",
            "mount_path": "secret"
        }"#).unwrap();
        assert_eq!(config.address, "https://vault.example.com:8200");
        assert_eq!(config.auth_method, "token");
        assert_eq!(config.mount_path, Some("secret".into()));
    }

    #[test]
    fn test_vault_config_approle() {
        let config: VaultConfig = serde_json::from_str(r#"{
            "address": "http://127.0.0.1:8200",
            "auth_method": "approle",
            "role_id": "abc",
            "secret_id": "def"
        }"#).unwrap();
        assert_eq!(config.auth_method, "approle");
        assert_eq!(config.role_id, Some("abc".into()));
    }

    #[test]
    fn test_vault_secret_serialize() {
        let secret = VaultSecret {
            path: "org-123/host-1/postgres".into(),
            data: {
                let mut m = HashMap::new();
                m.insert("DB_PASSWORD".into(), "secret123".into());
                m
            },
            metadata: {
                let mut m = HashMap::new();
                m.insert("source".into(), "discovery".into());
                m.insert("scan_id".into(), "scan-1".into());
                m
            },
        };
        let json = serde_json::to_string(&secret).unwrap();
        assert!(json.contains("org-123/host-1/postgres"));
        assert!(json.contains("DB_PASSWORD"));
        assert!(json.contains("discovery"));
    }

    #[test]
    fn test_kv_url_building() {
        let config = VaultConfig {
            address: "https://vault.example.com:8200".into(),
            auth_method: "token".into(),
            token: Some("test".into()),
            role_id: None,
            secret_id: None,
            mount_path: Some("secret".into()),
            namespace: None,
        };
        let client = VaultClient::new(config).unwrap();
        assert_eq!(
            client.kv_url("org-1/host-1/postgres"),
            "https://vault.example.com:8200/v1/secret/data/org-1/host-1/postgres"
        );
        assert_eq!(
            client.kv_metadata_url("org-1/host-1/postgres"),
            "https://vault.example.com:8200/v1/secret/metadata/org-1/host-1/postgres"
        );
    }

    #[test]
    fn test_vault_health_default() {
        let health = VaultHealth {
            reachable: false,
            initialized: false,
            sealed: true,
            standby: false,
        };
        assert!(!health.reachable);
        assert!(health.sealed);
    }
}

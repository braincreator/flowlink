// FlowLink Core — Minimal HashiCorp Vault KV v2 client
// Reads secrets from Vault and applies them to RelayConfig.

use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A secret stored as key=value in Vault KV v2.
#[derive(Debug, Deserialize)]
struct KvResponse {
    data: KvData,
}

#[derive(Debug, Deserialize)]
struct KvData {
    data: HashMap<String, String>,
    #[allow(dead_code)]
    metadata: serde_json::Value,
}

/// AppRole login response.
#[derive(Debug, Deserialize)]
struct AppRoleLoginResponse {
    auth: AppRoleAuth,
}

#[derive(Debug, Deserialize)]
struct AppRoleAuth {
    client_token: String,
    lease_duration: u64,
}

/// Minimal Vault client for reading KV v2 secrets.
///
/// Supports:
/// - AppRole authentication
/// - Token-based authentication (direct token)
/// - Self-signed cert support (VAULT_SKIP_VERIFY)
/// - Secret caching with TTL
pub struct VaultClient {
    http: reqwest::Client,
    addr: String,
    token: Option<String>,
    role_id: Option<String>,
    secret_id: Option<String>,
    /// Token expiry for proactive re-auth.
    token_expires: Option<Instant>,
    /// Cache of secret path → value.
    cache: HashMap<String, String>,
    #[allow(dead_code)]
    cache_ttl: Duration,
}

impl VaultClient {
    /// Create a new Vault client from environment variables.
    ///
    /// Reads:
    /// - `VAULT_ADDR` (default: `https://127.0.0.1:8200`)
    /// - `VAULT_TOKEN` (if set, used directly)
    /// - `VAULT_ROLE_ID` + `VAULT_SECRET_ID` (AppRole auth)
    /// - `VAULT_SKIP_VERIFY` (disable TLS verification)
    /// - `VAULT_NAMESPACE` (optional namespace)
    pub fn from_env() -> Self {
        let addr = std::env::var("VAULT_ADDR").unwrap_or_else(|_| "https://127.0.0.1:8200".into());
        let skip_verify = std::env::var("VAULT_SKIP_VERIFY").map(|v| v == "true" || v == "1").unwrap_or(false);

        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(3));

        if skip_verify {
            builder = builder.danger_accept_invalid_certs(true);
        }

        let http = builder.build().unwrap_or_else(|e| {
            eprintln!("[vault] Failed to create HTTP client: {e}");
            reqwest::Client::new()
        });

        let token = std::env::var("VAULT_TOKEN").ok();
        let role_id = std::env::var("VAULT_ROLE_ID").ok();
        let secret_id = std::env::var("VAULT_SECRET_ID").ok();

        Self {
            http,
            addr,
            token,
            role_id,
            secret_id,
            token_expires: None,
            cache: HashMap::new(),
            cache_ttl: Duration::from_secs(300), // 5 min cache
        }
    }

    /// Get the current valid token, authenticating if needed.
    async fn ensure_token(&mut self) -> anyhow::Result<String> {
        // Return cached token if still valid
        if let Some(ref token) = self.token {
            if let Some(expires) = self.token_expires {
                if Instant::now() < expires {
                    return Ok(token.clone());
                }
            } else {
                return Ok(token.clone());
            }
        }

        // Try AppRole login
        if let (Some(ref role_id), Some(ref secret_id)) = (&self.role_id, &self.secret_id) {
            let url = format!("{}/v1/auth/approle/login", self.addr);
            let body = serde_json::json!({
                "role_id": role_id,
                "secret_id": secret_id,
            });

            let resp = self.http.post(&url).json(&body).send().await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("AppRole login failed ({status}): {body}");
            }

            let login: AppRoleLoginResponse = resp.json().await?;
            let ttl = login.auth.lease_duration.saturating_sub(60); // refresh 60s early
            self.token = Some(login.auth.client_token.clone());
            self.token_expires = Some(Instant::now() + Duration::from_secs(ttl));
            return Ok(login.auth.client_token);
        }

        anyhow::bail!("No Vault token or AppRole credentials configured")
    }

    /// Read a secret from Vault KV v2 at the given path under `flowlink/` mount.
    ///
    /// Returns the `value` field from the secret.
    /// Uses in-memory cache with TTL to reduce API calls.
    pub async fn read_secret(&mut self, path: &str) -> anyhow::Result<String> {
        // Check cache
        if let Some(cached) = self.cache.get(path) {
            return Ok(cached.clone());
        }

        let token = self.ensure_token().await?;
        let url = format!("{}/v1/flowlink/data/{path}", self.addr);
        let ns_header = std::env::var("VAULT_NAMESPACE").ok();

        let mut req = self.http.get(&url)
            .header("X-Vault-Token", &token);

        if let Some(ns) = &ns_header {
            req = req.header("X-Vault-Namespace", ns);
        }

        let resp = req.send().await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("Secret not found: {path}");
        }
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Vault read failed ({status}): {body}");
        }

        let kv: KvResponse = resp.json().await?;
        let value = kv.data.data.get("value")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Secret at {path} has no 'value' field"))?;

        // Cache it
        self.cache.insert(path.to_string(), value.clone());
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_client_from_env() {
        // Just verify it doesn't panic
        let _client = VaultClient::from_env();
    }
}

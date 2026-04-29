//! FlowLink Bot API Client
//!
//! HTTP client for FlowLink REST API. Used by:
//! - Telegram/Slack/Discord integrations (bot commands)
//! - CLI tool (`flowlink status`, `flowlink agents`, etc.)
//! - Self-hosted relay for inter-service communication
//!
//! Works with both Cloud (https://api.flowlink.io) and
//! Self-hosted (http://localhost:3000) deployments.

use serde::{Deserialize, Serialize};
use std::time::Duration;

// ═══════════════════════════════════════════════
// Auth
// ═══════════════════════════════════════════════

/// Authentication method for API calls
#[derive(Clone)]
pub enum AuthMethod {
    /// JWT bearer token (user or admin)
    Jwt(String),
    /// API key pair
    ApiKey { key: String, secret: String },
    /// Internal service token (cloud inter-service)
    ServiceToken(String),
}

// ═══════════════════════════════════════════════
// Client
// ═══════════════════════════════════════════════

/// FlowLink API client
pub struct FlowLinkClient {
    base_url: String,
    auth: AuthMethod,
    client: reqwest::Client,
}

impl FlowLinkClient {
    pub fn new(base_url: &str, auth: AuthMethod) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            auth,
            client,
        }
    }

    /// Create client for local relay (self-hosted mode)
    pub fn local(jwt: &str) -> Self {
        Self::new("http://localhost:3000", AuthMethod::Jwt(jwt.to_string()))
    }

    /// Create client for cloud API
    pub fn cloud(jwt: &str) -> Self {
        Self::new("https://api.flowlink.io", AuthMethod::Jwt(jwt.to_string()))
    }

    fn auth_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse().unwrap());

        match &self.auth {
            AuthMethod::Jwt(token) => {
                headers.insert("Authorization", format!("Bearer {}", token).parse().unwrap());
            }
            AuthMethod::ApiKey { key, secret } => {
                headers.insert("X-API-Key", key.parse().unwrap());
                headers.insert("X-API-Secret", secret.parse().unwrap());
            }
            AuthMethod::ServiceToken(token) => {
                headers.insert("X-Service-Token", token.parse().unwrap());
            }
        }
        headers
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let resp = self.client
            .get(format!("{}{}", self.base_url, path))
            .headers(self.auth_headers())
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error {}: {}", status, body);
        }
        Ok(resp.json().await?)
    }

    async fn post<T: serde::de::DeserializeOwned>(&self, path: &str, body: &serde_json::Value) -> anyhow::Result<T> {
        let resp = self.client
            .post(format!("{}{}", self.base_url, path))
            .headers(self.auth_headers())
            .json(body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error {}: {}", status, body);
        }
        Ok(resp.json().await?)
    }

    async fn post_empty<T: serde::de::DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        self.post(path, &serde_json::json!({})).await
    }

    async fn delete<T: serde::de::DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let resp = self.client
            .delete(format!("{}{}", self.base_url, path))
            .headers(self.auth_headers())
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error {}: {}", status, body);
        }
        Ok(resp.json().await?)
    }

    // ═══════════════════════════════════════════════
    // System
    // ═══════════════════════════════════════════════

    pub async fn get_health(&self) -> anyhow::Result<HealthResponse> {
        self.get("/health").await
    }

    pub async fn get_system_info(&self) -> anyhow::Result<SystemInfo> {
        self.get("/api/system/info").await
    }

    // ═══════════════════════════════════════════════
    // Agents
    // ═══════════════════════════════════════════════

    pub async fn list_agents(&self) -> anyhow::Result<Vec<AgentInfo>> {
        self.get("/api/agents").await
    }

    pub async fn remove_agent(&self, id: &str) -> anyhow::Result<serde_json::Value> {
        self.delete(&format!("/api/agents/{}", id)).await
    }

    // ═══════════════════════════════════════════════
    // Shield
    // ═══════════════════════════════════════════════

    pub async fn get_alerts(&self) -> anyhow::Result<Vec<ShieldAlert>> {
        self.get("/api/shield/alerts").await
    }

    pub async fn get_shield_stats(&self) -> anyhow::Result<serde_json::Value> {
        self.get("/api/shield/stats").await
    }

    pub async fn approve(&self, pid: &str) -> anyhow::Result<serde_json::Value> {
        self.post_empty(&format!("/api/shield/approve/{}", pid)).await
    }

    pub async fn reject(&self, pid: &str) -> anyhow::Result<serde_json::Value> {
        self.post_empty(&format!("/api/shield/reject/{}", pid)).await
    }

    // ═══════════════════════════════════════════════
    // Approvals
    // ═══════════════════════════════════════════════

    pub async fn get_approvals(&self) -> anyhow::Result<Vec<ApprovalInfo>> {
        self.get("/api/approvals").await
    }

    pub async fn approve_request(&self, id: &str) -> anyhow::Result<serde_json::Value> {
        self.post_empty(&format!("/api/approvals/{}/approve", id)).await
    }

    pub async fn reject_request(&self, id: &str) -> anyhow::Result<serde_json::Value> {
        self.post_empty(&format!("/api/approvals/{}/reject", id)).await
    }

    // ═══════════════════════════════════════════════
    // Billing
    // ═══════════════════════════════════════════════

    pub async fn get_plans(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        self.get("/api/plans").await
    }

    pub async fn get_billing_info(&self) -> anyhow::Result<serde_json::Value> {
        self.get("/api/billing").await
    }

    pub async fn get_billing_plans(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        self.get("/api/billing/plans").await
    }

    pub async fn get_my_plan(&self) -> anyhow::Result<serde_json::Value> {
        self.get("/api/billing/my-plan").await
    }

    pub async fn change_plan(&self, plan_id: &str) -> anyhow::Result<serde_json::Value> {
        self.post("/api/billing/change-plan", &serde_json::json!({ "plan_id": plan_id })).await
    }

    pub async fn get_invoices(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        self.get("/api/billing/invoices").await
    }

    pub async fn get_usage(&self) -> anyhow::Result<serde_json::Value> {
        self.get("/api/billing/usage").await
    }

    pub async fn subscribe(&self, plan_id: &str) -> anyhow::Result<serde_json::Value> {
        self.post("/api/billing/subscribe", &serde_json::json!({ "plan_id": plan_id })).await
    }

    // ═══════════════════════════════════════════════
    // Audit
    // ═══════════════════════════════════════════════

    pub async fn get_audit(&self, limit: Option<u32>) -> anyhow::Result<Vec<serde_json::Value>> {
        let path = match limit {
            Some(n) => format!("/api/audit?limit={}", n),
            None => "/api/audit".to_string(),
        };
        self.get(&path).await
    }

    pub async fn get_audit_stats(&self) -> anyhow::Result<serde_json::Value> {
        self.get("/api/audit/stats").await
    }

    // ═══════════════════════════════════════════════
    // Config
    // ═══════════════════════════════════════════════

    pub async fn get_config(&self) -> anyhow::Result<serde_json::Value> {
        self.get("/api/config").await
    }

    pub async fn reload_config(&self) -> anyhow::Result<serde_json::Value> {
        self.post_empty("/api/config/reload").await
    }

    // ═══════════════════════════════════════════════
    // Sessions
    // ═══════════════════════════════════════════════

    pub async fn get_sessions(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        self.get("/api/sessions").await
    }

    // ═══════════════════════════════════════════════
    // Devices
    // ═══════════════════════════════════════════════

    pub async fn get_devices(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        self.get("/api/devices").await
    }

    // ═══════════════════════════════════════════════
    // Integrations Marketplace
    // ═══════════════════════════════════════════════

    pub async fn get_integration_catalog(&self) -> anyhow::Result<IntegrationCatalog> {
        self.get("/api/integrations/catalog").await
    }

    pub async fn list_integrations(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        self.get("/api/integrations").await
    }

    pub async fn install_integration(&self, kind: &str, config: serde_json::Value, events: Vec<String>) -> anyhow::Result<serde_json::Value> {
        self.post("/api/integrations", &serde_json::json!({
            "kind": kind,
            "config": config,
            "subscribed_events": events,
        })).await
    }

    pub async fn uninstall_integration(&self, id: &str) -> anyhow::Result<serde_json::Value> {
        self.delete(&format!("/api/integrations/{}", id)).await
    }

    pub async fn begin_oauth_integration(&self, kind: &str, events: Vec<String>) -> anyhow::Result<OAuthBeginResponse> {
        self.post("/api/integrations/oauth/begin", &serde_json::json!({
            "kind": kind,
            "subscribed_events": events,
        })).await
    }

    // ═══════════════════════════════════════════════
    // Auth
    // ═══════════════════════════════════════════════

    pub async fn get_auth_me(&self) -> anyhow::Result<serde_json::Value> {
        self.get("/api/auth/me").await
    }

    pub async fn get_account_info(&self) -> anyhow::Result<serde_json::Value> {
        self.get("/api/account/info").await
    }

    pub async fn get_auth_providers(&self) -> anyhow::Result<serde_json::Value> {
        self.get("/api/auth/providers").await
    }

    // ═══════════════════════════════════════════════
    // Backups
    // ═══════════════════════════════════════════════

    pub async fn get_backups(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        self.get("/api/backups").await
    }

    // ═══════════════════════════════════════════════
    // LLM
    // ═══════════════════════════════════════════════

    pub async fn get_llm_backends(&self) -> anyhow::Result<Vec<serde_json::Value>> {
        self.get("/api/llm/backends").await
    }
}

// ═══════════════════════════════════════════════
// Response Types
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub db: String,
    pub agents_online: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub version: String,
    pub uptime_seconds: u64,
    pub agents_online: usize,
    pub agents_total: usize,
    pub websocket_connections: usize,
    pub memory_usage_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub hostname: String,
    pub os: String,
    pub online: bool,
    pub last_seen: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldAlert {
    pub id: String,
    pub agent_id: String,
    pub risk: String,
    pub command: String,
    pub timestamp: String,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalInfo {
    pub id: String,
    pub agent_id: String,
    pub command: String,
    pub risk: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationCatalog {
    pub integrations: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthBeginResponse {
    pub authorize_url: String,
    pub integration_id: String,
}

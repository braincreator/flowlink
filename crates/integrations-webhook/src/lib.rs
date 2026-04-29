//! FlowLink Webhook Integration
//!
//! Forwards all subscribed events as HTTP POST requests to a configurable URL.
//! Supports HMAC-SHA256 signature verification for security.
//!
//! Payload format:
//! ```json
//! {
//!   "event": "agent_connected",
//!   "timestamp": "2025-01-01T00:00:00Z",
//!   "data": { ... },
//!   "signature": "sha256=<hex>"
//! }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use flowlink_integrations_core::*;

// ═══════════════════════════════════════════════
// Config
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Target URL to POST events to
    pub url: String,
    /// HMAC-SHA256 signing secret (optional but recommended)
    pub secret: Option<String>,
    /// Custom HTTP headers
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// HTTP method (default: POST)
    #[serde(default = "default_method")]
    pub method: String,
    /// Request timeout in seconds (default: 10)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// Retry count on failure (default: 3)
    #[serde(default = "default_retries")]
    pub retries: u32,
    /// Only forward events matching these types (empty = all)
    #[serde(default)]
    pub event_filter: Vec<String>,
}

fn default_method() -> String { "POST".into() }
fn default_timeout() -> u64 { 10 }
fn default_retries() -> u32 { 3 }

// ═══════════════════════════════════════════════
// Payload
// ═══════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
pub struct WebhookPayload {
    /// Event type name
    pub event: String,
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Account that owns this integration
    pub account_id: String,
    /// Integration instance ID
    pub integration_id: String,
    /// Event-specific data
    pub data: serde_json::Value,
    /// HMAC-SHA256 signature (if secret configured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

// ═══════════════════════════════════════════════
// Integration
// ═══════════════════════════════════════════════

pub struct WebhookIntegration;

impl WebhookIntegration {
    pub fn new() -> Self { Self }

    fn parse_config(config: &serde_json::Value) -> Result<WebhookConfig, IntegrationError> {
        serde_json::from_value(config.clone())
            .map_err(|e| IntegrationError::ConfigError(format!("Invalid webhook config: {}", e)))
    }

    fn event_type_name(event: &IntegrationEvent) -> &str {
        match event {
            IntegrationEvent::AgentConnected { .. } => "agent_connected",
            IntegrationEvent::AgentDisconnected { .. } => "agent_disconnected",
            IntegrationEvent::ShieldAlert { .. } => "shield_alert",
            IntegrationEvent::ApprovalRequested { .. } => "approval_requested",
            IntegrationEvent::ApprovalResolved { .. } => "approval_resolved",
            IntegrationEvent::PaymentReceived { .. } => "payment_received",
            IntegrationEvent::SubscriptionChanged { .. } => "subscription_changed",
            IntegrationEvent::PlanExpiring { .. } => "plan_expiring",
            IntegrationEvent::SystemAlert { .. } => "system_alert",
            IntegrationEvent::Custom { name, .. } => name.as_str(),
        }
    }

    fn event_data(event: &IntegrationEvent) -> serde_json::Value {
        match event {
            IntegrationEvent::AgentConnected { agent_id, hostname } => serde_json::json!({
                "agent_id": agent_id, "hostname": hostname
            }),
            IntegrationEvent::AgentDisconnected { agent_id, hostname } => serde_json::json!({
                "agent_id": agent_id, "hostname": hostname
            }),
            IntegrationEvent::ShieldAlert { agent_id, risk, command } => serde_json::json!({
                "agent_id": agent_id, "risk": risk, "command": command
            }),
            IntegrationEvent::ApprovalRequested { approval_id, agent_id, command, risk } => serde_json::json!({
                "approval_id": approval_id, "agent_id": agent_id, "command": command, "risk": risk
            }),
            IntegrationEvent::ApprovalResolved { approval_id, decision, resolved_by } => serde_json::json!({
                "approval_id": approval_id, "decision": decision, "resolved_by": resolved_by
            }),
            IntegrationEvent::PaymentReceived { account_id, amount_kopecks, description } => serde_json::json!({
                "account_id": account_id, "amount_kopecks": amount_kopecks, "description": description
            }),
            IntegrationEvent::SubscriptionChanged { account_id, plan } => serde_json::json!({
                "account_id": account_id, "plan": plan
            }),
            IntegrationEvent::PlanExpiring { account_id, days_left } => serde_json::json!({
                "account_id": account_id, "days_left": days_left
            }),
            IntegrationEvent::SystemAlert { level, message } => serde_json::json!({
                "level": level, "message": message
            }),
            IntegrationEvent::Custom { name, data } => serde_json::json!({
                "name": name, "data": data
            }),
        }
    }

    pub fn sign_payload(body: &[u8], secret: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(body);
        let result = mac.finalize();
        format!("sha256={}", hex::encode(result.into_bytes()))
    }
}

#[async_trait]
impl Integration for WebhookIntegration {
    fn kind(&self) -> IntegrationKind {
        IntegrationKind("webhook".into())
    }

    fn meta(&self) -> IntegrationMeta {
        builtin_catalog().into_iter()
            .find(|m| m.kind == IntegrationKind("webhook".into()))
            .expect("webhook meta must exist")
    }

    async fn validate_config(&self, config: &serde_json::Value) -> Result<(), IntegrationError> {
        let wc = Self::parse_config(config)?;
        if wc.url.is_empty() {
            return Err(IntegrationError::ConfigError("url is required".into()));
        }
        // Validate URL format
        if !wc.url.starts_with("http://") && !wc.url.starts_with("https://") {
            return Err(IntegrationError::ConfigError("url must start with http:// or https://".into()));
        }
        Ok(())
    }

    async fn start(
        &self,
        id: IntegrationId,
        config: IntegrationConfig,
        mut event_rx: tokio::sync::broadcast::Receiver<IntegrationEvent>,
    ) -> Result<(), IntegrationError> {
        let wc = Self::parse_config(&config.config)?;

        log::info!("🔗 Webhook integration starting: {} → {} (account {})",
            id, wc.url, config.account_id);

        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(wc.timeout_secs))
                .build()
                .unwrap_or_default();

            loop {
                let event = match event_rx.recv().await {
                    Ok(e) => e,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("Webhook {} lagged {} events", id, n);
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        log::info!("Webhook {} channel closed", id);
                        break;
                    }
                };

                let event_type = Self::event_type_name(&event).to_string();

                // Apply event filter
                if !wc.event_filter.is_empty() && !wc.event_filter.contains(&event_type) {
                    continue;
                }

                // Also check subscribed_events from config
                if !config.subscribed_events.is_empty()
                    && !config.subscribed_events.contains(&event_type)
                {
                    continue;
                }

                let data = Self::event_data(&event);
                let timestamp = chrono::Utc::now().to_rfc3339();

                let payload = WebhookPayload {
                    event: event_type.clone(),
                    timestamp,
                    account_id: config.account_id.clone(),
                    integration_id: id.clone(),
                    data: data.clone(),
                    signature: None,
                };

                let body = serde_json::to_vec(&payload).unwrap_or_default();
                let signature = wc.secret.as_ref()
                    .map(|s| Self::sign_payload(&body, s));

                // Build request — must be rebuilt on each retry
                let mut attempts = 0;
                let max_retries = wc.retries;
                loop {
                    attempts += 1;

                    let mut req = client
                        .post(&wc.url)
                        .header("Content-Type", "application/json")
                        .header("X-FlowLink-Event", &event_type)
                        .header("X-FlowLink-Delivery", &id);

                    if let Some(ref sig) = signature {
                        req = req.header("X-FlowLink-Signature", sig);
                    }
                    for (k, v) in &wc.headers {
                        req = req.header(k.as_str(), v.as_str());
                    }

                    match req
                        .body(body.clone())
                        .send()
                        .await
                    {
                        Ok(resp) => {
                            let status = resp.status();
                            if status.is_success() {
                                log::debug!("🔗 Webhook {}: sent {} → {}", id, event_type, status);
                            } else {
                                log::warn!("🔗 Webhook {}: {} returned {}", id, event_type, status);
                            }
                            break;
                        }
                        Err(e) => {
                            if attempts >= max_retries {
                                log::error!("🔗 Webhook {}: {} failed after {} attempts: {}", id, event_type, attempts, e);
                                break;
                            }
                            log::warn!("🔗 Webhook {}: {} attempt {} failed: {}, retrying...", id, event_type, attempts, e);
                            tokio::time::sleep(std::time::Duration::from_millis(500 * attempts as u64)).await;
                        }
                    }
                }
            }

            log::info!("🔗 Webhook integration {} stopped", id);
        });

        Ok(())
    }

    async fn stop(&self, id: &IntegrationId) -> Result<(), IntegrationError> {
        log::info!("🛑 Webhook integration stopped: {}", id);
        Ok(())
    }

    async fn handle_event(
        &self,
        event: &IntegrationEvent,
        _config: &IntegrationConfig,
    ) -> Vec<IntegrationAction> {
        // Webhook doesn't return actions — it sends HTTP requests in start()
        let _ = event;
        vec![IntegrationAction::Noop]
    }

    async fn handle_command(
        &self,
        command: &str,
        _args: &serde_json::Value,
        _config: &IntegrationConfig,
    ) -> Result<serde_json::Value, IntegrationError> {
        // Webhook doesn't support commands
        Err(IntegrationError::ConfigError(
            format!("Webhook integration does not support commands: {}", command)
        ))
    }

    async fn health_check(&self, _id: &IntegrationId) -> Result<bool, IntegrationError> {
        Ok(true)
    }
}

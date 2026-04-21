//! RuSIEM Integration
//!
//! Real-time event forwarding to RuSIEM via:
//! 1. Syslog (RFC 5424) over TCP — primary
//! 2. REST API webhook — fallback
//!
//! RuSIEM docs: https://www.rusiems.io

use axum::extract::State;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::RwLock;

use crate::server::AppState;

// ═══════════════════════════════════════════════════
// Config
// ═══════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RusiemConfig {
    /// RuSIEM collector host:port (syslog TCP)
    pub syslog_host: String,
    /// RuSIEM REST API URL for webhook events
    pub api_url: Option<String>,
    /// RuSIEM API token
    pub api_token: Option<String>,
    /// Facility code (default: 1 = user-level)
    #[serde(default = "default_facility")]
    pub facility: u8,
    /// Enable/disable
    #[serde(default)]
    pub enabled: bool,
}

fn default_facility() -> u8 { 1 }

impl Default for RusiemConfig {
    fn default() -> Self {
        Self {
            syslog_host: "localhost:514".to_string(),
            api_url: None,
            api_token: None,
            facility: 1,
            enabled: false,
        }
    }
}

// ═══════════════════════════════════════════════════
// Event types
// ═══════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RusiemEvent {
    pub timestamp: String,
    pub category: String,
    pub action: String,
    pub risk_score: u32,
    pub source: String,
    pub command: String,
    pub shield_level: u32,
    pub rule_name: String,
    pub hostname: String,
    pub username: String,
    pub pid: u32,
    pub org_id: Option<String>,
}

// ═══════════════════════════════════════════════════
// Syslog RFC 5424
// ═══════════════════════════════════════════════════

fn severity_for_action(action: &str) -> u8 {
    match action {
        "kill" => 2,
        "block" => 3,
        "warn" => 4,
        "allow" => 6,
        _ => 6,
    }
}

fn format_syslog_5424(event: &RusiemEvent, facility: u8, severity: u8) -> String {
    let pri = facility * 8 + severity;
    let sd = format!(
        "[flowlink@1 cat=\"{}\" action=\"{}\" risk=\"{}\" level=\"{}\" rule=\"{}\" agent=\"{}\" host=\"{}\" user=\"{}\" pid=\"{}\" org=\"{}\"]",
        event.category, event.action, event.risk_score, event.shield_level,
        event.rule_name, event.source, event.hostname, event.username, event.pid,
        event.org_id.as_deref().unwrap_or("-"),
    );
    let msg = serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string());
    format!("<{pri}>1 {} flowlink {} {} - {sd} {msg}\n",
        event.timestamp, event.hostname, event.pid)
}

// ═══════════════════════════════════════════════════
// Forwarder
// ═══════════════════════════════════════════════════

pub struct RusiemForwarder {
    config: Arc<RwLock<RusiemConfig>>,
    http: reqwest::Client,
}

impl RusiemForwarder {
    pub fn new(config: RusiemConfig) -> Self {
        Self { config: Arc::new(RwLock::new(config)), http: reqwest::Client::new() }
    }

    pub async fn forward(&self, event: &RusiemEvent) {
        let config = self.config.read().await;
        if !config.enabled { return; }

        let severity = severity_for_action(&event.action);
        let msg = format_syslog_5424(event, config.facility, severity);

        if let Err(e) = self.send_syslog(&msg, &config.syslog_host).await {
            log::warn!("RuSIEM syslog failed: {e}");
            if let Some(url) = &config.api_url {
                if let Err(e) = self.send_rest(url, &config.api_token, event).await {
                    log::error!("RuSIEM REST failed: {e}");
                }
            }
        }
    }

    async fn send_syslog(&self, msg: &str, host: &str) -> Result<(), String> {
        let addr = if host.contains(':') { host.to_string() } else { format!("{host}:514") };
        let mut stream = TcpStream::connect(&addr).await.map_err(|e| format!("TCP: {e}"))?;
        stream.write_all(msg.as_bytes()).await.map_err(|e| format!("Write: {e}"))?;
        Ok(())
    }

    async fn send_rest(&self, url: &str, token: &Option<String>, event: &RusiemEvent) -> Result<(), String> {
        let mut req = self.http.post(url).json(event);
        if let Some(t) = token { req = req.header("Authorization", format!("Bearer {t}")); }
        req.send().await.map_err(|e| format!("POST: {e}"))?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════
// Test endpoint
// ═══════════════════════════════════════════════════

#[derive(Serialize)]
pub struct TestResponse { pub ok: bool, pub message: String }

pub async fn test_connection(State(state): State<AppState>, axum::extract::Extension(claims): axum::extract::Extension<crate::auth::Claims>) -> axum::Json<TestResponse> {
    if !claims.is_admin { return axum::Json(TestResponse { ok: false, message: "Admin required".into() }); }
    let config = match &state.rusiem_config {
        Some(c) => c.read().await.clone(),
        None => return axum::Json(TestResponse { ok: false, message: "RuSIEM not configured".into() }),
    };
    if !config.enabled {
        return axum::Json(TestResponse { ok: false, message: "Disabled".into() });
    }
    axum::Json(TestResponse { ok: true, message: "Configured".into() })
}

// ═══════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn test_event() -> RusiemEvent {
        RusiemEvent {
            timestamp: "2026-04-21T00:00:00Z".into(),
            category: "command_intercepted".into(),
            action: "block".into(),
            risk_score: 95,
            source: "prod-1".into(),
            command: "rm -rf /".into(),
            shield_level: 3,
            rule_name: "rm_rf".into(),
            hostname: "prod-1.example.com".into(),
            username: "root".into(),
            pid: 12345,
            org_id: Some("org-123".into()),
        }
    }

    #[test]
    fn test_syslog_format() {
        let e = test_event();
        let msg = format_syslog_5424(&e, 1, 3);
        assert!(msg.starts_with("<11>1"));
        assert!(msg.contains("flowlink@1"));
        assert!(msg.contains("command_intercepted"));
    }

    #[test]
    fn test_severity() {
        assert_eq!(severity_for_action("kill"), 2);
        assert_eq!(severity_for_action("block"), 3);
        assert_eq!(severity_for_action("allow"), 6);
    }
}

// FlowLink Shield — Relay client for reporting interceptions and resolutions

use log::{info, warn};
use serde::Serialize;

/// HTTP client that reports shield events to the relay.
#[derive(Debug, Clone)]
pub struct RelayClient {
    relay_url: String,
    api_token: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct IngestRequest {
    alert_id: String,
    pid: u32,
    uid: u32,
    username: String,
    command: String,
    rule_name: String,
    action: String,
    snapshot: Option<String>,
    timestamp: i64,
}

#[derive(Serialize)]
struct ResolveRequest {
    pid: u32,
    approved: bool,
}

impl RelayClient {
    pub fn new(relay_url: String, api_token: String) -> Self {
        Self {
            relay_url: relay_url.trim_end_matches('/').to_string(),
            api_token,
            client: reqwest::Client::new(),
        }
    }

    pub fn relay_url(&self) -> &str {
        &self.relay_url
    }

    /// Report an interception to the relay's `/api/shield/ingest` endpoint.
    pub async fn report_interception(
        &self,
        alert_id: &str,
        pid: u32,
        uid: u32,
        username: &str,
        command: &str,
        rule_name: &str,
        action: &str,
        snapshot: Option<&str>,
    ) -> anyhow::Result<()> {
        let url = format!("{}/api/shield/ingest", self.relay_url);
        let body = IngestRequest {
            alert_id: alert_id.to_string(),
            pid,
            uid,
            username: username.to_string(),
            command: command.to_string(),
            rule_name: rule_name.to_string(),
            action: action.to_string(),
            snapshot: snapshot.map(|s| s.to_string()),
            timestamp: chrono::Utc::now().timestamp(),
        };

        match self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                info!("Reported interception to relay: alert={}, pid={}", alert_id, pid);
                Ok(())
            }
            Ok(resp) => {
                warn!("Relay returned {} for interception report", resp.status());
                Ok(())
            }
            Err(e) => {
                warn!("Failed to report interception to relay: {}", e);
                // Non-fatal — shield operates independently
                Ok(())
            }
        }
    }

    /// Report an approval resolution to the relay's `/api/shield/resolve` endpoint.
    pub async fn report_resolution(&self, pid: u32, approved: bool) -> anyhow::Result<()> {
        let url = format!("{}/api/shield/resolve", self.relay_url);
        let body = ResolveRequest { pid, approved };

        match self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                info!("Reported resolution to relay: pid={}, approved={}", pid, approved);
                Ok(())
            }
            Ok(resp) => {
                warn!("Relay returned {} for resolution report", resp.status());
                Ok(())
            }
            Err(e) => {
                warn!("Failed to report resolution to relay: {}", e);
                Ok(())
            }
        }
    }
}

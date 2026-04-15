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

        match self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                info!(
                    "Reported interception to relay: alert={}, pid={}",
                    alert_id, pid
                );
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

        match self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                info!(
                    "Reported resolution to relay: pid={}, approved={}",
                    pid, approved
                );
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

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════
    // RelayClient construction
    // ═══════════════════════════════════════════

    #[test]
    fn new_with_valid_url() {
        let client = RelayClient::new("http://localhost:8080".into(), "token123".into());
        assert_eq!(client.relay_url(), "http://localhost:8080");
    }

    #[test]
    fn new_trims_trailing_slash() {
        let client = RelayClient::new("http://localhost:8080/".into(), "tok".into());
        assert_eq!(client.relay_url(), "http://localhost:8080");
    }

    #[test]
    fn new_trims_multiple_trailing_slashes() {
        let client = RelayClient::new("http://localhost:8080///".into(), "tok".into());
        assert_eq!(client.relay_url(), "http://localhost:8080");
    }

    #[test]
    fn new_preserves_url_with_path() {
        let client = RelayClient::new("http://relay.example.com/api".into(), "tok".into());
        assert_eq!(client.relay_url(), "http://relay.example.com/api");
    }

    #[test]
    fn new_empty_url() {
        let client = RelayClient::new(String::new(), "tok".into());
        assert_eq!(client.relay_url(), "");
    }

    #[test]
    fn new_empty_token() {
        let client = RelayClient::new("http://localhost:8080".into(), String::new());
        assert_eq!(client.relay_url(), "http://localhost:8080");
    }

    #[test]
    fn clone_works() {
        let client = RelayClient::new("http://localhost:8080".into(), "tok".into());
        let cloned = client.clone();
        assert_eq!(cloned.relay_url(), client.relay_url());
    }

    #[test]
    fn debug_format() {
        let client = RelayClient::new("http://localhost:8080".into(), "secret".into());
        let debug = format!("{:?}", client);
        assert!(debug.contains("RelayClient"));
        assert!(debug.contains("localhost"));
    }

    // ═══════════════════════════════════════════
    // report_interception with mock server
    // ═══════════════════════════════════════════

    #[tokio::test]
    async fn report_interception_success_200() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            use tokio::io::AsyncReadExt;
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).await.unwrap();
            let body = String::from_utf8_lossy(&buf[..n]);
            // Verify it's a POST with JSON
            assert!(body.contains("POST"));
            assert!(body.contains("Bearer mytoken"));
            assert!(body.contains("alert-123"));
            assert!(body.contains("/api/shield/ingest"));

            // Send 200 response
            let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
            use tokio::io::AsyncWriteExt;
            stream.write_all(response).await.unwrap();
        });

        let client = RelayClient::new(format!("http://127.0.0.1:{}", port), "mytoken".into());
        let result = client
            .report_interception(
                "alert-123",
                1234,
                1000,
                "testuser",
                "rm -rf /",
                "rm_rf",
                "intercepted",
                Some("snap-1"),
            )
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn report_interception_server_error_500() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = vec![0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let response = b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n";
            stream.write_all(response).await.unwrap();
        });

        let client = RelayClient::new(format!("http://127.0.0.1:{}", port), "tok".into());
        // Non-fatal — should still return Ok(())
        let result = client
            .report_interception(
                "alert-456",
                9999,
                0,
                "root",
                "rm /",
                "rm",
                "intercepted",
                None,
            )
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn report_interception_connection_refused() {
        // Use a port that nothing is listening on
        let client = RelayClient::new("http://127.0.0.1:1".into(), "tok".into());
        // Non-fatal — should return Ok(())
        let result = client
            .report_interception("alert-789", 1, 0, "root", "ls", "ls", "intercepted", None)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn report_interception_invalid_url() {
        let client = RelayClient::new("not-a-valid-url".into(), "tok".into());
        // Should not panic — returns Ok(()) since errors are non-fatal
        let result = client
            .report_interception(
                "alert-invalid",
                1,
                0,
                "root",
                "ls",
                "ls",
                "intercepted",
                None,
            )
            .await;
        assert!(result.is_ok());
    }

    // ═══════════════════════════════════════════
    // report_resolution
    // ═══════════════════════════════════════════

    #[tokio::test]
    async fn report_resolution_success_200() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).await.unwrap();
            let body = String::from_utf8_lossy(&buf[..n]);
            assert!(body.contains("POST"));
            assert!(body.contains("/api/shield/resolve"));
            assert!(body.contains("\"approved\":true"));

            let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
            stream.write_all(response).await.unwrap();
        });

        let client = RelayClient::new(format!("http://127.0.0.1:{}", port), "mytoken".into());
        let result = client.report_resolution(1234, true).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn report_resolution_connection_refused() {
        let client = RelayClient::new("http://127.0.0.1:1".into(), "tok".into());
        let result = client.report_resolution(1234, false).await;
        assert!(result.is_ok());
    }

    // ═══════════════════════════════════════════
    // IngestRequest serialization
    // ═══════════════════════════════════════════

    #[test]
    fn ingest_request_serialization() {
        let req = IngestRequest {
            alert_id: "alert-1".into(),
            pid: 1234,
            uid: 1000,
            username: "alice".into(),
            command: "rm -rf /".into(),
            rule_name: "rm_rf".into(),
            action: "intercepted".into(),
            snapshot: Some("snap-1".into()),
            timestamp: 1700000000,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("alert-1"));
        assert!(json.contains("alice"));
        assert!(json.contains("snap-1"));
        assert!(json.contains("1700000000"));
    }

    #[test]
    fn ingest_request_no_snapshot() {
        let req = IngestRequest {
            alert_id: "alert-2".into(),
            pid: 1,
            uid: 0,
            username: "root".into(),
            command: "ls".into(),
            rule_name: "safe".into(),
            action: "allowed".into(),
            snapshot: None,
            timestamp: 0,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"snapshot\":null"));
    }

    // ═══════════════════════════════════════════
    // ResolveRequest serialization
    // ═══════════════════════════════════════════

    #[test]
    fn resolve_request_approved() {
        let req = ResolveRequest {
            pid: 1234,
            approved: true,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"approved\":true"));
    }

    #[test]
    fn resolve_request_denied() {
        let req = ResolveRequest {
            pid: 5678,
            approved: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"approved\":false"));
    }
}

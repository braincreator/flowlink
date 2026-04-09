// Remote LLM — forward requests through relay's LLM proxy
// Port of internal/agent/remote_llm.go

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LlmResponse {
    pub content: String,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub model: Option<String>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
}

/// Client that sends LLM requests through the relay proxy.
pub struct RemoteLlm {
    relay_url: String,
    api_token: String,
    #[allow(dead_code)]
    timeout: Duration,
    client: reqwest::Client,
}

impl RemoteLlm {
    pub fn new(relay_url: String, api_token: String, timeout_secs: u64) -> Self {
        // Trim trailing slash to prevent double-slash when constructing API paths.
        let relay_url = relay_url.trim_end_matches('/').to_string();
        Self {
            relay_url,
            api_token,
            timeout: Duration::from_secs(timeout_secs),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(timeout_secs))
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    /// Send a completion request to the relay's LLM proxy endpoint.
    pub async fn complete(
        &self,
        messages: Vec<LlmMessage>,
        model: Option<&str>,
    ) -> anyhow::Result<LlmResponse> {
        let url = format!("{}/api/v1/llm/complete", self.relay_url);

        let mut body = serde_json::json!({
            "messages": messages,
        });
        if let Some(m) = model {
            body["model"] = serde_json::json!(m);
        }

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM proxy error {}: {}", status, text);
        }

        let result: LlmResponse = resp.json().await?;
        if let Some(ref err) = result.error {
            anyhow::bail!("LLM error: {}", err);
        }
        Ok(result)
    }

    /// Simple chat with system + user messages.
    pub async fn chat(
        &self,
        system_prompt: &str,
        user_message: &str,
    ) -> anyhow::Result<String> {
        let messages = vec![
            LlmMessage {
                role: "system".into(),
                content: system_prompt.into(),
            },
            LlmMessage {
                role: "user".into(),
                content: user_message.into(),
            },
        ];
        let resp = self.complete(messages, None).await?;
        Ok(resp.content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construction() {
        let llm = RemoteLlm::new("https://relay.example.com".into(), "tok123".into(), 30);
        assert_eq!(llm.relay_url, "https://relay.example.com");
        assert_eq!(llm.api_token, "tok123");
        assert_eq!(llm.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_url_formation() {
        let llm = RemoteLlm::new("https://relay.example.com".into(), "tok".into(), 10);
        // We can't easily test without a server, but we can verify the client was built
        assert!(llm.timeout.as_secs() == 10);
    }

    #[test]
    fn test_messages_serialization() {
        let msgs = vec![
            LlmMessage { role: "system".into(), content: "You are helpful.".into() },
            LlmMessage { role: "user".into(), content: "Hello".into() },
        ];
        let json = serde_json::to_value(&msgs).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 2);
        assert_eq!(json[0]["role"], "system");
    }

    #[test]
    fn test_response_deserialization() {
        let json = r#"{"content":"Hi there","tokens_in":10,"tokens_out":5,"model":"gpt-4","duration_ms":200}"#;
        let resp: LlmResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.content, "Hi there");
        assert_eq!(resp.tokens_in, Some(10));
        assert_eq!(resp.tokens_out, Some(5));
        assert_eq!(resp.model, Some("gpt-4".into()));
    }

    #[test]
    fn test_response_with_error() {
        let json = r#"{"content":"","error":"rate limited"}"#;
        let resp: LlmResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.error, Some("rate limited".into()));
    }

    #[test]
    fn test_url_trailing_slash_handling() {
        let llm = RemoteLlm::new("https://relay.example.com/".into(), "tok".into(), 10);
        // Trailing slash should be trimmed so API path doesn't double-slash
        let expected = "https://relay.example.com/api/v1/llm/complete";
        let url = format!("{}/api/v1/llm/complete", llm.relay_url);
        assert_eq!(url, expected);
    }

    #[test]
    fn test_url_no_trailing_slash_unchanged() {
        let llm = RemoteLlm::new("https://relay.example.com".into(), "tok".into(), 10);
        let expected = "https://relay.example.com/api/v1/llm/complete";
        let url = format!("{}/api/v1/llm/complete", llm.relay_url);
        assert_eq!(url, expected);
    }

    #[test]
    fn test_url_multiple_trailing_slashes() {
        let llm = RemoteLlm::new("https://relay.example.com///".into(), "tok".into(), 10);
        let expected = "https://relay.example.com/api/v1/llm/complete";
        let url = format!("{}/api/v1/llm/complete", llm.relay_url);
        assert_eq!(url, expected);
    }
}

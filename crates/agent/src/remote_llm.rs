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
    timeout: Duration,
    client: reqwest::Client,
}

impl RemoteLlm {
    pub fn new(relay_url: String, api_token: String, timeout_secs: u64) -> Self {
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

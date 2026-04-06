// LLM Proxy — forwards LLM requests to configured backends
// Port of internal/relay/llm_proxy.go

use flowlink_core::config::LlmBackend;
use log::{info, warn, error};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system" | "user" | "assistant"
    pub content: String,
}

fn default_max_tokens() -> u32 { 1024 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

pub struct LlmProxy {
    backends: Vec<LlmBackend>,
    timeout: Duration,
    client: reqwest::Client,
}

impl LlmProxy {
    pub fn new(backends: Vec<LlmBackend>, timeout_secs: u32) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs as u64))
            .build()
            .unwrap_or_default();

        Self { backends, timeout: Duration::from_secs(timeout_secs as u64), client }
    }

    /// Send a chat completion request, trying backends in order.
    pub async fn complete(&self, mut request: LlmRequest) -> anyhow::Result<LlmResponse> {
        if self.backends.is_empty() {
            anyhow::bail!("LLM_ALL_BACKENDS_DOWN: no backends configured");
        }

        let mut last_error = None;

        for backend in &self.backends {
            request.model = backend.model.clone();

            let url = match &backend.base_url {
                Some(u) => format!("{}/chat/completions", u.trim_end_matches('/')),
                None => match backend.provider.as_str() {
                    "openai" => "https://api.openai.com/v1/chat/completions".into(),
                    "anthropic" => "https://api.anthropic.com/v1/messages".into(),
                    other => format!("https://api.{other}.com/v1/chat/completions"),
                },
            };

            let mut req = self.client.post(&url);
            if let Some(key) = &backend.api_key {
                req = req.bearer_auth(key);
            }

            match req.json(&request).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let body = resp.json::<LlmResponse>().await?;
                        info!("LLM response from {} (model: {})", backend.name, body.model);
                        return Ok(body);
                    } else {
                        let status = resp.status();
                        let text = resp.text().await.unwrap_or_default();
                        warn!("LLM backend {} returned {}: {}", backend.name, status, text);
                        last_error = Some(anyhow::anyhow!("{}: {}", status, text));
                    }
                }
                Err(e) => {
                    warn!("LLM backend {} request failed: {}", backend.name, e);
                    last_error = Some(e.into());
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("LLM_ALL_BACKENDS_DOWN")))
    }

    pub fn backend_names(&self) -> Vec<&str> {
        self.backends.iter().map(|b| b.name.as_str()).collect()
    }
}

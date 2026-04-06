// LLM Proxy — forwards LLM requests to configured backends with failover
// Port of internal/relay/llm_proxy.go

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures_util::Stream;
use log::{info, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use flowlink_core::config::LlmBackend;

// ── Request / Response types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    #[serde(default)]
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
    pub role: String,
    pub content: String,
}

fn default_max_tokens() -> u32 {
    4096
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub id: String,
    pub model: String,
    pub content: String,
    pub finish_reason: Option<String>,
    pub usage: LlmUsage,
    pub backend: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub provider: String,
    pub model: String,
    pub healthy: bool,
}

// ── Backend health tracking ──

#[derive(Clone)]
pub struct BackendEntry {
    pub name: String,
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub priority: i32,
    pub healthy: Arc<AtomicBool>,
    pub fail_count: Arc<AtomicU32>,
}

// ── LLM Proxy ──

pub struct LlmProxy {
    backends: Arc<RwLock<Vec<BackendEntry>>>,
    timeout: Duration,
    client: Client,
}

impl LlmProxy {
    pub fn new(backends: Vec<LlmBackend>, timeout_secs: u32) -> Self {
        let mut entries: Vec<BackendEntry> = backends
            .into_iter()
            .map(|b| BackendEntry {
                name: b.name,
                provider: b.provider,
                model: b.model,
                api_key: b.api_key,
                base_url: b.base_url,
                priority: b.priority.unwrap_or(1),
                healthy: Arc::new(AtomicBool::new(true)),
                fail_count: Arc::new(AtomicU32::new(0)),
            })
            .collect();

        // Sort by priority (lower = higher priority)
        entries.sort_by_key(|e| e.priority);

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs.max(10) as u64))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self {
            backends: Arc::new(RwLock::new(entries)),
            timeout: Duration::from_secs(timeout_secs.max(10) as u64),
            client,
        }
    }

    /// Send a chat completion request with failover across backends.
    pub async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        let backends = self.backends.read().await;

        if backends.is_empty() {
            anyhow::bail!("no LLM backends configured");
        }

        let mut last_error = None;
        let start = Instant::now();

        for backend in &backends {
            if !backend.healthy.load(Ordering::Relaxed) {
                continue;
            }

            let url = build_url(&backend.provider, &backend.base_url);
            let mut req = self.client.post(&url);

            if let Some(key) = &backend.api_key {
                match backend.provider.as_str() {
                    "anthropic" => {
                        req = req.header("x-api-key", key);
                        req = req.header("anthropic-version", "2023-06-01");
                    }
                    _ => {
                        req = req.bearer_auth(key);
                    }
                }
            }

            // Build provider-specific body
            let body = match backend.provider.as_str() {
                "anthropic" => build_anthropic_body(&request, &backend.model),
                _ => build_openai_body(&request, &backend.model),
            };

            match req.json(&body).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let text = resp.text().await.unwrap_or_default();
                    let parsed = parse_response(&backend.provider, &text);

                    backend.fail_count.store(0, Ordering::Relaxed);

                    let usage = parsed.usage.unwrap_or_default();
                    let response = LlmResponse {
                        id: parsed.id,
                        model: parsed.model,
                        content: parsed.content,
                        finish_reason: parsed.finish_reason,
                        usage,
                        backend: backend.name.clone(),
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                    info!(
                        "LLM response from {} (model: {}, tokens: {}/{}, {}ms)",
                        backend.name, response.model, usage.prompt_tokens, usage.completion_tokens, response.duration_ms
                    );
                    return Ok(response);
                }
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    warn!("LLM backend {} returned {}: {}", backend.name, status, &text[..text.len().min(300)]);
                    backend.fail_count.fetch_add(1, Ordering::Relaxed);
                    if backend.fail_count.load(Ordering::Relaxed) >= 3 {
                        backend.healthy.store(false, Ordering::Relaxed);
                        warn!("LLM backend {} marked unhealthy after 3 consecutive failures", backend.name);
                    }
                    last_error = Some(anyhow::anyhow!("{}: {}", status, &text[..text.len().min(200)]));
                }
                Err(e) => {
                    warn!("LLM backend {} request failed: {}", backend.name, e);
                    backend.fail_count.fetch_add(1, Ordering::Relaxed);
                    if backend.fail_count.load(Ordering::Relaxed) >= 3 {
                        backend.healthy.store(false, Ordering::Relaxed);
                    }
                    last_error = Some(e.into());
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("all LLM backends failed")))
    }

    /// Streaming completion — returns a stream of SSE chunks.
    pub async fn stream(
        &self,
        request: LlmRequest,
    ) -> Result<impl Stream<Item = Result<String>>> {
        let backends = self.backends.read().await;
        let backend = backends.iter().find(|b| b.healthy.load(Ordering::Relaxed));

        let backend = match backend {
            Some(b) => b.clone(),
            None => anyhow::bail!("no healthy LLM backend available"),
        };

        let url = build_url(&backend.provider, &backend.base_url);
        let mut req = self.client.post(&url);

        if let Some(key) = &backend.api_key {
            match backend.provider.as_str() {
                "anthropic" => {
                    req = req.header("x-api-key", key);
                    req = req.header("anthropic-version", "2023-06-01");
                }
                _ => {
                    req = req.bearer_auth(key);
                }
            }
        }

        let mut stream_req = request.clone();
        stream_req.stream = true;

        let body = match backend.provider.as_str() {
            "anthropic" => build_anthropic_body(&stream_req, &backend.model),
            _ => build_openai_body(&stream_req, &backend.model),
        };

        let resp = req.json(&body).send().await?.error_for_status()?;
        let byte_stream = resp.bytes_stream();

        Ok(futures_util::stream::unfold(byte_stream, move |mut stream| async move {
            use futures_util::StreamExt;
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        // Extract content from SSE data lines
                        for line in text.lines() {
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data.trim() == "[DONE]" {
                                    return None;
                                }
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                    if let Some(content) = json
                                        .pointer("/choices/0/delta/content")
                                        .or_else(|| json.pointer("/choices/0/message/content"))
                                        .and_then(|v| v.as_str())
                                    {
                                        return Some((Ok(content.to_string()), stream));
                                    }
                                    // Anthropic format
                                    if let Some(content) = json
                                        .get("content")
                                        .and_then(|v| v.as_array())
                                        .and_then(|arr| arr.first())
                                        .and_then(|v| v.get("text"))
                                        .and_then(|v| v.as_str())
                                    {
                                        return Some((Ok(content.to_string()), stream));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => return Some((Err(e.into()), stream)),
                }
            }
            None
        }))
    }

    /// Check health of all backends.
    pub async fn check_health(&self) -> Vec<(String, String)> {
        let backends = self.backends.read().await;
        let mut results = Vec::new();

        for backend in backends.iter() {
            let url = build_url(&backend.provider, &backend.base_url);
            // Just try a lightweight request
            let check_client = Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default();

            match check_client.get(&url.replace("/chat/completions", "/models")).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() || status.as_u16() == 405 {
                        backend.healthy.store(true, Ordering::Relaxed);
                        backend.fail_count.store(0, Ordering::Relaxed);
                        results.push((backend.name.clone(), "ok".into()));
                    } else {
                        results.push((backend.name.clone(), format!("http {}", status)));
                    }
                }
                Err(e) => {
                    results.push((backend.name.clone(), format!("unreachable: {}", e)));
                }
            }
        }

        results
    }

    /// List available models/backends.
    pub async fn list_models(&self) -> Vec<ModelInfo> {
        let backends = self.backends.read().await;
        backends
            .iter()
            .map(|b| ModelInfo {
                name: b.name.clone(),
                provider: b.provider.clone(),
                model: b.model.clone(),
                healthy: b.healthy.load(Ordering::Relaxed),
            })
            .collect()
    }

    /// Update backends at runtime.
    pub async fn set_backends(&self, backends: Vec<LlmBackend>) {
        let mut entries: Vec<BackendEntry> = backends
            .into_iter()
            .map(|b| BackendEntry {
                name: b.name,
                provider: b.provider,
                model: b.model,
                api_key: b.api_key,
                base_url: b.base_url,
                priority: b.priority.unwrap_or(1),
                healthy: Arc::new(AtomicBool::new(true)),
                fail_count: Arc::new(AtomicU32::new(0)),
            })
            .collect();
        entries.sort_by_key(|e| e.priority);
        let mut guard = self.backends.write().await;
        *guard = entries;
    }
}

// ── Helpers ──

fn build_url(provider: &str, base_url: &Option<String>) -> String {
    match base_url {
        Some(u) => {
            let u = u.trim_end_matches('/');
            if u.ends_with("/chat/completions") || u.ends_with("/messages") {
                u.to_string()
            } else if provider == "anthropic" {
                format!("{}/messages", u)
            } else {
                format!("{}/chat/completions", u)
            }
        }
        None => match provider {
            "openai" => "https://api.openai.com/v1/chat/completions".into(),
            "anthropic" => "https://api.anthropic.com/v1/messages".into(),
            "ollama" => "http://localhost:11434/api/chat".into(),
            other => format!("https://api.{other}.com/v1/chat/completions"),
        },
    }
}

fn build_openai_body(req: &LlmRequest, model: &str) -> serde_json::Value {
    serde_json::json!({
        "model": if req.model.is_empty() { model } else { &req.model },
        "messages": req.messages,
        "temperature": if req.temperature == 0.0 { 0.3 } else { req.temperature },
        "max_tokens": req.max_tokens,
        "stream": req.stream,
    })
}

fn build_anthropic_body(req: &LlmRequest, model: &str) -> serde_json::Value {
    // Split system message for Anthropic
    let mut system = String::new();
    let mut messages: Vec<serde_json::Value> = Vec::new();

    for msg in &req.messages {
        if msg.role == "system" {
            system = msg.content.clone();
        } else {
            messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content,
            }));
        }
    }

    let mut body = serde_json::json!({
        "model": if req.model.is_empty() { model } else { &req.model },
        "messages": messages,
        "max_tokens": req.max_tokens,
        "stream": req.stream,
    });

    if !system.is_empty() {
        body["system"] = serde_json::json!(system);
    }

    body
}

struct ParsedResponse {
    id: String,
    model: String,
    content: String,
    finish_reason: Option<String>,
    usage: Option<LlmUsage>,
}

fn parse_response(provider: &str, body: &str) -> ParsedResponse {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        match provider {
            "anthropic" => {
                let content = json
                    .get("content")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let usage = json.get("usage").map(|u| LlmUsage {
                    prompt_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                    completion_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                    total_tokens: 0,
                });

                ParsedResponse {
                    id: json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    model: json.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    content,
                    finish_reason: json.get("stop_reason").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    usage,
                }
            }
            _ => {
                // OpenAI-compatible
                let content = json
                    .pointer("/choices/0/message/content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let finish_reason = json
                    .pointer("/choices/0/finish_reason")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let usage = json.get("usage").map(|u| LlmUsage {
                    prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                    completion_tokens: u.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                    total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                });

                ParsedResponse {
                    id: json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    model: json.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    content,
                    finish_reason,
                    usage,
                }
            }
        }
    } else {
        ParsedResponse {
            id: String::new(),
            model: String::new(),
            content: body.to_string(),
            finish_reason: None,
            usage: None,
        }
    }
}

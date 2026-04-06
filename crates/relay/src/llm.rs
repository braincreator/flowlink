// LLM Proxy — forwards LLM requests to configured backends with failover
// Port of internal/relay/llm_proxy.go

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use futures_util::{Stream, StreamExt};
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

fn default_max_tokens() -> u32 { 4096 }

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmUsage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub provider: String,
    pub model: String,
    pub healthy: bool,
}

// ── Backend ──

#[derive(Clone)]
struct BackendEntry {
    name: String,
    provider: String,
    model: String,
    api_key: Option<String>,
    base_url: Option<String>,
    healthy: Arc<AtomicBool>,
    fail_count: Arc<AtomicU32>,
}

// ── LLM Proxy ──

pub struct LlmProxy {
    backends: Arc<RwLock<Vec<BackendEntry>>>,
    client: Client,
}

impl LlmProxy {
    pub fn new(backends: Vec<LlmBackend>, timeout_secs: u32) -> Self {
        let entries: Vec<BackendEntry> = backends
            .into_iter()
            .map(|b| BackendEntry {
                name: b.name,
                provider: b.provider,
                model: b.model,
                api_key: b.api_key,
                base_url: b.base_url,
                healthy: Arc::new(AtomicBool::new(true)),
                fail_count: Arc::new(AtomicU32::new(0)),
            })
            .collect();

        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs.max(10) as u64))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self {
            backends: Arc::new(RwLock::new(entries)),
            client,
        }
    }

    /// Send a chat completion request with failover.
    pub async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        let backends = self.backends.read().await;
        if backends.is_empty() {
            anyhow::bail!("no LLM backends configured");
        }

        let mut last_error = None;
        let start = Instant::now();

        for backend in backends.iter() {
            if !backend.healthy.load(Ordering::Relaxed) {
                continue;
            }

            let url = build_url(&backend.provider, &backend.base_url);
            let mut req = self.client.post(&url);

            if let Some(key) = &backend.api_key {
                match backend.provider.as_str() {
                    "anthropic" => {
                        req = req.header("x-api-key", key).header("anthropic-version", "2023-06-01");
                    }
                    _ => { req = req.bearer_auth(key); }
                }
            }

            let body = match backend.provider.as_str() {
                "anthropic" => build_anthropic_body(&request, &backend.model),
                _ => build_openai_body(&request, &backend.model),
            };

            match req.json(&body).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let text = resp.text().await.unwrap_or_default();
                    let parsed = parse_response(&backend.provider, &text);
                    backend.fail_count.store(0, Ordering::Relaxed);

                    let response = LlmResponse {
                        id: parsed.id,
                        model: parsed.model,
                        content: parsed.content,
                        finish_reason: parsed.finish_reason,
                        usage: parsed.usage,
                        backend: backend.name.clone(),
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                    info!("LLM response from {} (model: {}, {}ms)", backend.name, response.model, response.duration_ms);
                    return Ok(response);
                }
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    warn!("LLM backend {} returned {}: {}", backend.name, status, &text[..text.len().min(300)]);
                    backend.fail_count.fetch_add(1, Ordering::Relaxed);
                    if backend.fail_count.load(Ordering::Relaxed) >= 3 {
                        backend.healthy.store(false, Ordering::Relaxed);
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

    /// Streaming completion.
    pub async fn stream(&self, request: LlmRequest) -> Result<impl Stream<Item = Result<String, reqwest::Error>>> {
        let backends = self.backends.read().await;
        let backend = backends.iter().find(|b| b.healthy.load(Ordering::Relaxed))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no healthy LLM backend available"))?;

        let url = build_url(&backend.provider, &backend.base_url);
        let mut req = self.client.post(&url);

        if let Some(key) = &backend.api_key {
            match backend.provider.as_str() {
                "anthropic" => {
                    req = req.header("x-api-key", key).header("anthropic-version", "2023-06-01");
                }
                _ => { req = req.bearer_auth(key); }
            }
        }

        let mut stream_req = request.clone();
        stream_req.stream = true;
        let body = match backend.provider.as_str() {
            "anthropic" => build_anthropic_body(&stream_req, &backend.model),
            _ => build_openai_body(&stream_req, &backend.model),
        };

        let resp = req.json(&body).send().await?.error_for_status()?;
        Ok(resp.bytes_stream().map(|r| r.map(|bytes| String::from_utf8_lossy(&bytes).to_string())).boxed())
    }

    /// Check health of all backends.
    pub async fn check_health(&self) -> Vec<(String, String)> {
        let backends = self.backends.read().await;
        let mut results = Vec::new();

        for backend in backends.iter() {
            let url = build_url(&backend.provider, &backend.base_url);
            let check_client = Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default();

            let models_url = url.replace("/chat/completions", "/models").replace("/messages", "/models");
            match check_client.get(&models_url).send().await {
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

    /// List models/backends.
    pub async fn list_models(&self) -> Vec<ModelInfo> {
        let backends = self.backends.read().await;
        backends.iter().map(|b| ModelInfo {
            name: b.name.clone(),
            provider: b.provider.clone(),
            model: b.model.clone(),
            healthy: b.healthy.load(Ordering::Relaxed),
        }).collect()
    }

    /// Update backends at runtime.
    pub async fn set_backends(&self, backends: Vec<LlmBackend>) {
        let entries: Vec<BackendEntry> = backends.into_iter().map(|b| BackendEntry {
            name: b.name, provider: b.provider, model: b.model,
            api_key: b.api_key, base_url: b.base_url,
            healthy: Arc::new(AtomicBool::new(true)),
            fail_count: Arc::new(AtomicU32::new(0)),
        }).collect();
        *self.backends.write().await = entries;
    }
}

// ── Helpers ──

fn build_url(provider: &str, base_url: &Option<String>) -> String {
    match base_url {
        Some(u) => {
            let u = u.trim_end_matches('/');
            if provider == "anthropic" && !u.ends_with("/messages") {
                format!("{}/messages", u)
            } else if !u.ends_with("/chat/completions") {
                format!("{}/chat/completions", u)
            } else {
                u.to_string()
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
    let mut system = String::new();
    let mut messages: Vec<serde_json::Value> = Vec::new();
    for msg in &req.messages {
        if msg.role == "system" { system = msg.content.clone(); }
        else { messages.push(serde_json::json!({"role": msg.role, "content": msg.content})); }
    }
    let mut body = serde_json::json!({
        "model": if req.model.is_empty() { model } else { &req.model },
        "messages": messages,
        "max_tokens": req.max_tokens,
        "stream": req.stream,
    });
    if !system.is_empty() { body["system"] = serde_json::json!(system); }
    body
}

struct ParsedResponse {
    id: String, model: String, content: String,
    finish_reason: Option<String>, usage: LlmUsage,
}

fn parse_response(provider: &str, body: &str) -> ParsedResponse {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else {
        return ParsedResponse { id: String::new(), model: String::new(), content: body.to_string(), finish_reason: None, usage: LlmUsage::default() };
    };

    match provider {
        "anthropic" => {
            let content = json.pointer("/content/0/text").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let usage = LlmUsage {
                prompt_tokens: json.pointer("/usage/input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                completion_tokens: json.pointer("/usage/output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                total_tokens: 0,
            };
            ParsedResponse {
                id: json.get("id").and_then(|v| v.as_str()).unwrap_or("").into(),
                model: json.get("model").and_then(|v| v.as_str()).unwrap_or("").into(),
                content,
                finish_reason: json.get("stop_reason").and_then(|v| v.as_str()).map(|s| s.to_string()),
                usage,
            }
        }
        _ => {
            let content = json.pointer("/choices/0/message/content").and_then(|v| v.as_str()).unwrap_or("").into();
            let usage = LlmUsage {
                prompt_tokens: json.pointer("/usage/prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                completion_tokens: json.pointer("/usage/completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                total_tokens: json.pointer("/usage/total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            };
            ParsedResponse {
                id: json.get("id").and_then(|v| v.as_str()).unwrap_or("").into(),
                model: json.get("model").and_then(|v| v.as_str()).unwrap_or("").into(),
                content,
                finish_reason: json.pointer("/choices/0/finish_reason").and_then(|v| v.as_str()).map(|s| s.to_string()),
                usage,
            }
        }
    }
}

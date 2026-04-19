// Playground public scan endpoint — no auth required
// Uses real AnalysisEngine for live security scanning demo

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::{json, Value};
use serde::Deserialize;

use crate::AppState;
use flowlink_shield::{AnalysisEngine, Command, ThreatLevel};

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum PlaygroundRequest {
    #[serde(rename = "command")]
    Command { command: String },
    #[serde(rename = "script")]
    Script { script: String, #[serde(default)] language: String },
    #[serde(rename = "file")]
    File { path: String, #[serde(default = "default_write")] operation: String },
    #[serde(rename = "url")]
    Url { url: String, #[serde(default = "default_download")] purpose: String },
}

fn default_write() -> String { "write".into() }
fn default_download() -> String { "download".into() }

/// POST /api/playground/scan — public playground endpoint (no auth)
pub async fn playground_scan(
    State(_state): State<AppState>,
    Json(req): Json<PlaygroundRequest>,
) -> impl IntoResponse {
    let result = match req {
        PlaygroundRequest::Command { command } => scan_command(&command),
        PlaygroundRequest::Script { script, language } => scan_script(&script, &language),
        PlaygroundRequest::File { path, operation } => scan_file(&path, &operation),
        PlaygroundRequest::Url { url, purpose } => scan_url(&url, &purpose),
    };
    result
}

fn scan_command(command: &str) -> axum::response::Response {
    if command.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "command is empty"}))).into_response();
    }
    if command.len() > 1000 {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "command too long"}))).into_response();
    }
    let engine = AnalysisEngine { enable_ast: true, enable_interpreter: true };
    let cmd = Command { binary: extract_binary(command), args: shell_words(command), raw: command.to_string() };
    let report = engine.analyze(&cmd);
    let result = match report.threat {
        Some(t) => {
            let score = level_score(&t.level);
            json!({
                "type": "command",
                "command": command,
                "risk_level": level_str(&t.level),
                "threat_id": t.id,
                "threat_name": t.name,
                "explanation": t.description,
                "suggestion": t.suggestion,
                "score": score,
                "analysis_level": report.level_used,
            })
        },
        None => json!({
            "type": "command",
            "command": command,
            "risk_level": "safe",
            "score": 0,
            "analysis_level": 0,
        })
    };
    (StatusCode::OK, Json(result)).into_response()
}

fn scan_script(script: &str, language: &str) -> axum::response::Response {
    if script.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "script is empty"}))).into_response();
    }
    if script.len() > 5000 {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "script too long"}))).into_response();
    }
    let engine = AnalysisEngine { enable_ast: true, enable_interpreter: true };
    let lines: Vec<&str> = script.lines().collect();
    let mut line_results = Vec::new();
    let mut worst: u8 = 0;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            line_results.push(json!({"line": i + 1, "risk_level": "safe"}));
            continue;
        }
        let cmd = Command { binary: extract_binary(trimmed), args: shell_words(trimmed), raw: trimmed.to_string() };
        let report = engine.analyze(&cmd);
        match report.threat {
            Some(t) => {
                worst = worst.max(level_val(&t.level));
                line_results.push(json!({
                    "line": i + 1, "content": trimmed,
                    "risk_level": level_str(&t.level),
                    "threat": t.name, "explanation": t.description,
                }));
            }
            None => { line_results.push(json!({"line": i + 1, "content": trimmed, "risk_level": "safe"})); }
        }
    }
    let result = json!({
        "type": "script", "language": language,
        "overall_risk_level": match worst { 0 => "safe", 1..=2 => "warning", _ => "danger" },
        "lines": line_results,
    });
    (StatusCode::OK, Json(result)).into_response()
}

fn scan_file(path: &str, operation: &str) -> axum::response::Response {
    if path.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "path is empty"}))).into_response();
    }
    let mut risks: Vec<Value> = Vec::new();
    let mut score: u32 = 0;
    if path.contains("..") { risks.push(json!({"category": "path_traversal"})); score += 60; }
    for pp in &["/etc", "/var", "/usr", "/bin", "/sbin", "/boot"] {
        if path.starts_with(pp) { risks.push(json!({"category": "protected_path", "detail": pp})); score += 40; break; }
    }
    for cf in &["/etc/passwd", "/etc/shadow", "/etc/sudoers", "/etc/ssh/sshd_config"] {
        if path == *cf { risks.push(json!({"category": "critical_file", "detail": cf})); score = 100; }
    }
    let result = json!({
        "type": "file", "path": path, "operation": operation,
        "risk_level": risk_from_score(score), "score": score, "risks": risks,
    });
    (StatusCode::OK, Json(result)).into_response()
}

fn scan_url(url: &str, purpose: &str) -> axum::response::Response {
    if url.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "url is empty"}))).into_response();
    }
    let mut risks: Vec<Value> = Vec::new();
    let mut score: u32 = 0;
    for pp in &["127.0.0.1", "localhost", "169.254.", "10.", "192.168.", "::1"] {
        if url.contains(pp) { risks.push(json!({"category": "private_ip"})); score += 70; break; }
    }
    if url.starts_with("http://") { risks.push(json!({"category": "unencrypted"})); score += 15; }
    let result = json!({
        "type": "url", "url": url, "purpose": purpose,
        "risk_level": risk_from_score(score), "score": score, "risks": risks,
    });
    (StatusCode::OK, Json(result)).into_response()
}

// Helpers
fn extract_binary(cmd: &str) -> String { cmd.split_whitespace().next().unwrap_or("").to_string() }
fn shell_words(cmd: &str) -> Vec<String> { cmd.split_whitespace().map(|s| s.to_string()).collect() }
fn risk_from_score(s: u32) -> &'static str { if s >= 75 { "danger" } else if s >= 50 { "warning" } else if s >= 25 { "low" } else { "safe" } }
fn level_str(l: &ThreatLevel) -> &'static str {
    match l { ThreatLevel::Low => "warning", ThreatLevel::Medium => "warning", ThreatLevel::High => "danger", ThreatLevel::Critical => "critical" }
}
fn level_val(l: &ThreatLevel) -> u8 {
    match l { ThreatLevel::Low => 2, ThreatLevel::Medium => 3, ThreatLevel::High => 4, ThreatLevel::Critical => 5 }
}
fn level_score(l: &ThreatLevel) -> u32 {
    match l { ThreatLevel::Low => 35, ThreatLevel::Medium => 55, ThreatLevel::High => 75, ThreatLevel::Critical => 100 }
}

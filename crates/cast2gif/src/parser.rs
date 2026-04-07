use anyhow::{bail, Result};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CastEvent {
    pub timestamp: f64,
    pub data: String,
}

#[derive(Debug)]
pub struct CastFile {
    pub width: u32,
    pub height: u32,
    pub events: Vec<CastEvent>,
}

pub fn parse_cast(path: &Path) -> Result<CastFile> {
    let content = std::fs::read_to_string(path)?;
    let mut lines = content.lines();

    let header: serde_json::Value = serde_json::from_str(lines.next().unwrap_or(""))?;
    let version = header.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
    if version != 2 {
        bail!("Unsupported asciinema version: {}", version);
    }

    let width = header
        .get("width")
        .and_then(|v| v.as_u64())
        .unwrap_or(80) as u32;
    let height = header
        .get("height")
        .and_then(|v| v.as_u64())
        .unwrap_or(24) as u32;

    let mut events = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let arr: Vec<serde_json::Value> = match serde_json::from_str(line) {
            Ok(a) => a,
            Err(_) => continue,
        };
        if arr.len() < 3 {
            continue;
        }
        let timestamp = arr[0].as_f64().unwrap_or(0.0);
        let event_type = arr[1].as_str().unwrap_or("");
        let data = arr[2].as_str().unwrap_or("");

        if event_type == "o" && !data.is_empty() {
            events.push(CastEvent { timestamp, data: data.to_string() });
        }
    }
    // Sort by timestamp (files may not be sorted)
    events.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap());

    Ok(CastFile { width, height, events })
}

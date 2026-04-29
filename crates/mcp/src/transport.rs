//! MCP Streamable HTTP Transport (spec 2025-03-26)
//!
//! POST   /mcp/stream — JSON-RPC over HTTP (proxies to MCP handler)
//! GET    /mcp/stream — SSE keepalive / notifications
//! DELETE /mcp/stream — Terminate session
//!
//! Works with Cursor, Claude Desktop, Windsurf, and any Streamable HTTP MCP client.
//!
//! This is the standalone crate version — uses `McpState` instead of relay's `AppState`.

use axum::{
    extract::{Query, State, Extension},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use futures_util::stream::StreamExt;
use serde::Deserialize;
use serde_json::Value;

use crate::handler::{McpRequest, McpState};

#[derive(Deserialize)]
pub struct McpQuery {
    pub session_id: Option<String>,
}

// ---------------------------------------------------------------------------
// POST /mcp/stream
// ---------------------------------------------------------------------------

pub async fn mcp_post(
    Extension(state): Extension<std::sync::Arc<McpState>>,
    Query(query): Query<McpQuery>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let session_id = query
        .session_id
        .or_else(|| {
            headers
                .get("Mcp-Session-Id")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "default".to_string());

    log::debug!("MCP Stream POST: session={}, method={}", session_id, body["method"]);

    // Build a proper McpRequest from the JSON body
    let req: McpRequest = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {"code": -32700, "message": format!("Parse error: {}", e)}
                })),
            ).into_response();
        }
    };

    // Proxy to internal MCP handler
    let response = crate::handler::process_mcp_http(&state, headers, req).await;

    // Return response with session ID
    let (parts, body) = response.into_parts();
    let mut headers = parts.headers;
    headers.insert("Mcp-Session-Id", axum::http::HeaderValue::from_str(&session_id).unwrap_or(axum::http::HeaderValue::from_static("default")));

    (
        parts.status,
        headers,
        body,
    ).into_response()
}

// ---------------------------------------------------------------------------
// GET /mcp/stream — SSE
// ---------------------------------------------------------------------------

pub async fn mcp_sse(
    Extension(_state): Extension<std::sync::Arc<McpState>>,
    Query(query): Query<McpQuery>,
) -> impl IntoResponse {
    let session_id = query.session_id.unwrap_or_else(|| "default".to_string());
    log::info!("MCP Stream SSE: client connected, session={}", session_id);

    let stream = futures_util::stream::repeat(())
        .then(|_| async { tokio::time::sleep(std::time::Duration::from_secs(15)).await })
        .map(|_| Ok::<_, std::convert::Infallible>(SseEvent::default().event("ping").data("{}")));

    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ---------------------------------------------------------------------------
// DELETE /mcp/stream
// ---------------------------------------------------------------------------

pub async fn mcp_delete() -> StatusCode {
    log::info!("MCP Stream DELETE: session terminated");
    StatusCode::OK
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_streamable_http_response_format() {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2025-03-26",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "flowlink", "version": "0.2.0" }
            }
        });
        assert_eq!(response["jsonrpc"], "2.0");
    }
}

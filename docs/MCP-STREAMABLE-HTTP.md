# MCP Streamable HTTP Transport

FlowLink supports the **MCP Streamable HTTP** transport (spec `2025-03-26`), allowing direct integration with Cursor, Claude Desktop, Windsurf, and other MCP-compatible AI tools.

## Endpoints

| Method | URL | Description |
|--------|-----|-------------|
| `POST` | `/mcp/stream` | JSON-RPC request/response |
| `GET` | `/mcp/stream` | SSE stream (notifications) |
| `DELETE` | `/mcp/stream` | Terminate session |

Base URL: `https://flowlink.flow-masters.ru`

## Authentication

Include your FlowLink API key in every request:

```
Authorization: Bearer flk_your_api_key_here
```

Or: `x-api-key: flk_your_api_key_here`

Get an API key from the FlowLink Dashboard → API Keys.

## Cursor Configuration

Add to `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "flowlink": {
      "type": "streamable-http",
      "url": "https://flowlink.flow-masters.ru/mcp/stream",
      "headers": {
        "Authorization": "Bearer flk_your_api_key_here"
      }
    }
  }
}
```

## Claude Desktop Configuration

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "flowlink": {
      "type": "streamable-http",
      "url": "https://flowlink.flow-masters.ru/mcp/stream",
      "headers": {
        "Authorization": "Bearer flk_your_api_key_here"
      }
    }
  }
}
```

## Windsurf Configuration

Add to `~/.windsurf/mcp.json`:

```json
{
  "mcpServers": {
    "flowlink": {
      "serverUrl": "https://flowlink.flow-masters.ru/mcp/stream",
      "headers": {
        "Authorization": "Bearer flk_your_api_key_here"
      }
    }
  }
}
```

## Available Tools (20)

| Tool | Description |
|------|-------------|
| `scan_command` | Scan a command for security risks |
| `scan_script` | Scan a multi-line script |
| `scan_file` | Scan file content |
| `scan_url` | Scan a URL for risks |
| `get_policy` | Get current security policy |
| `explain_risk` | Get human-readable risk explanation |
| `audit_log` | Retrieve audit log entries |
| `policy_block_command` | Block a command pattern |
| `policy_unblock_command` | Unblock a command pattern |
| `policy_protect_path` | Protect a filesystem path |
| `policy_unprotect_path` | Unprotect a path |
| `policy_block_pid` | Block a process by PID |
| `policy_whitelist_pid` | Whitelist a process |
| `policy_status` | Get active policy status |
| `policy_reload` | Reload policy from config |
| `set_mode` | Set security mode (strict/moderate/permissive) |
| `set_threshold` | Set risk threshold (0-100) |
| `system_info` | Get system information |

## Protocol Flow

```
1. Client → POST /mcp/stream  {"method": "initialize", ...}
2. Server → 200 OK + capabilities + session ID

3. Client → POST /mcp/stream  {"method": "tools/list"}
4. Server → 200 OK + tool list

5. Client → POST /mcp/stream  {"method": "tools/call", "params": {"name": "scan_command", "arguments": {"command": "rm -rf /"}}}
6. Server → 200 OK + scan result (risk_level: "critical", score: 100)

7. Client → GET /mcp/stream   (optional SSE for notifications)
8. Server → SSE stream (keepalive pings every 15s)
```

## Also Available: Stdio Transport

For local CLI usage:

```bash
flowlink mcp --stdio
```

Add to MCP client config:

```json
{
  "mcpServers": {
    "flowlink-local": {
      "command": "flowlink",
      "args": ["mcp", "--stdio"]
    }
  }
}
```

## Rate Limits

- Public methods (`initialize`, `tools/list`): no auth required
- Tool calls: API key required, 100 requests/minute per key
- Playground (`/api/playground/scan`): 10 requests/minute

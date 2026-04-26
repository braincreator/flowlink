# FlowLink MCP Server

MCP (Model Context Protocol) server that exposes FlowLink Shield's security scanning to any AI agent — Claude Code, Cursor, Copilot, etc.

## Quick Start

### From source
```bash
cargo build --release --bin flowlink
```

### Add to your AI tool config

#### Claude Code (`.claude/mcp.json`)
```json
{
  "mcpServers": {
    "flowlink": {
      "command": "flowlink",
      "args": ["mcp"]
    }
  }
}
```

#### Cursor (`.cursor/mcp.json`)
```json
{
  "mcpServers": {
    "flowlink": {
      "command": "flowlink",
      "args": ["mcp"]
    }
  }
}
```

#### VS Code Copilot
```json
{
  "mcp": {
    "servers": {
      "flowlink": {
        "command": "flowlink",
        "args": ["mcp"]
      }
    }
  }
}
```

#### Windsurf / other MCP clients
Same pattern — `command: "flowlink"`, `args: ["mcp"]`. Uses stdio transport (stdin/stdout JSON-RPC).

## Tools (12)

### Agent Management

| Tool | Description |
|------|------------|
| `flowlink_agents` | List connected agents with status |
| `flowlink_deregister` | Disconnect an agent from relay |
| `flowlink_health` | Health check (relay + agents) |

### Execution

| Tool | Description |
|------|------------|
| `flowlink_exec` | Execute command on agent (streaming) |
| `flowlink_kill` | Kill process on agent |
| `flowlink_read` | Read file from agent |
| `flowlink_write` | Write file to agent |
| `flowlink_list` | List directory on agent |

### Security & Policy

| Tool | Description |
|------|------------|
| `flowlink_sysinfo` | Get system info from agent |
| `flowlink_approve` | Approve/reject pending command |
| `flowlink_policy` | Manage security policies (list, create, bind) |
| `flowlink_config_update` | Hot-reload agent configuration |

## How It Works

The MCP server exposes FlowLink's full security platform to AI agents:

- **Shield L1-L7**: KillSwitch → ReadOnly → Blacklist → Policy → Sandbox → Approval → Backup → Execute
- **Policy Engine**: Custom allow/deny rules per agent
- **Approval Workflow**: Block dangerous commands → human review via Telegram/Dashboard
- **All analysis runs locally** — no network calls, no LLM required. Typical scan time: < 1ms.

## Manual Testing

```bash
# Initialize
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | flowlink mcp

# List tools
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | flowlink mcp

# Scan a command
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"scan_command","arguments":{"command":"rm -rf /"}}}' | flowlink mcp
```

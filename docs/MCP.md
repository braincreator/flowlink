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

## Tools

### `scan_command`
Scans a single shell command for security risks.

```json
{
  "command": "rm -rf /var/log",
  "context": "cleaning up old logs"
}
```

Returns: `risk_level` (safe/warning/danger), `score` (0-100), `explanation`, `category`, `threat_id`.

### `scan_script`
Scans a multi-line script. Returns per-line analysis and overall risk assessment.

```json
{
  "script": "echo hello\nrm -rf /var\nls -la",
  "language": "bash"
}
```

### `get_policy`
Returns the current security policy configuration — enabled analysis levels, protected paths, and known dangerous operations.

### `explain_risk`
Detailed risk explanation with specific mitigations for a given command.

```json
{
  "command": "chmod 777 -R /var"
}
```

## How It Works

The MCP server uses FlowLink Shield's full 3-level analysis engine:

- **L1 — Pattern matching**: Structured argument parsing for known dangerous commands (rm, dd, mkfs, docker, git, etc.)
- **L2 — AST analysis**: tree-sitter bash parsing for commands hidden inside `bash -c`, eval, etc.
- **L3 — Interpreter heuristics**: Detects dangerous patterns in Python, Node, Perl, Ruby, PHP inline scripts

All analysis runs locally — no network calls, no LLM required. Typical scan time: < 1ms.

## Manual Testing

```bash
# Initialize
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | flowlink mcp

# List tools
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | flowlink mcp

# Scan a command
echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"scan_command","arguments":{"command":"rm -rf /"}}}' | flowlink mcp
```

#!/usr/bin/env bash
#
# FlowLink Self-Host Installer
# Устанавливает FlowLink agent + ServerGuard + shield config на Linux/macOS
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/braincreator/flowlink/main/scripts/install.sh | bash
#   ./install.sh [--relay URL] [--label NAME] [--shield-mode MODE] [--no-systemd] [--no-guard]
#
set -euo pipefail

# ── Colors ──
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; CYAN='\033[0;36m'; NC='\033[0m'

# ── Defaults ──
GITHUB_REPO="braincreator/flowlink"
INSTALL_DIR="/opt/flowlink"
BIN_DIR="$INSTALL_DIR/bin"
AGENT_DIR="$INSTALL_DIR/agent"
CONFIG_DIR="$INSTALL_DIR"
DATA_DIR="$INSTALL_DIR/data"
FLOWLINK_HOME="${FLOWLINK_HOME:-$HOME/.flowlink}"
RELAY_URL="${RELAY_URL:-wss://relay.flow-masters.ru:9093}"
RELAY_API="${RELAY_API:-http://127.0.0.1:9081}"
LABEL=""
SHIELD_MODE="${SHIELD_MODE:-moderate}"
NO_SYSTEMD=0
NO_GUARD=0

# ── Parse arguments ──
while [[ $# -gt 0 ]]; do
    case $1 in
        --relay)      RELAY_URL="$2"; shift 2 ;;
        --relay-api)  RELAY_API="$2"; shift 2 ;;
        --label)      LABEL="$2"; shift 2 ;;
        --shield-mode) SHIELD_MODE="$2"; shift 2 ;;
        --no-systemd) NO_SYSTEMD=1; shift ;;
        --no-guard)   NO_GUARD=1; shift ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "FlowLink Self-Host Installer — agent + ServerGuard + shield config"
            echo ""
            echo "Options:"
            echo "  --relay URL       WSS relay URL (default: wss://relay.flow-masters.ru:9093)"
            echo "  --relay-api URL   HTTP relay API (default: http://127.0.0.1:9081)"
            echo "  --label NAME      Agent label (default: hostname)"
            echo "  --shield-mode M   Shield mode: strict|moderate|permissive (default: moderate)"
            echo "  --no-systemd      Skip systemd service installation"
            echo "  --no-guard        Skip ServerGuard installation"
            echo ""
            echo "Shield modes:"
            echo "  strict      Block all suspicious commands (highest security)"
            echo "  moderate    Warn on medium/high risk, block critical (recommended)"
            echo "  permissive  Only block critical threats (development only)"
            exit 0 ;;
        *) echo -e "${RED}Unknown option: $1${NC}"; exit 1 ;;
    esac
done

# ── Logging ──
log_info()  { echo -e "${BLUE}[INFO]${NC} $1"; }
log_ok()    { echo -e "${GREEN}[OK]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# ── Detect OS/Arch ──
detect_os() {
    local os arch
    os="$(uname -s)"; arch="$(uname -m)"
    case "$os" in Linux)   OS_TYPE="linux" ;; Darwin) OS_TYPE="darwin" ;; *) log_error "Unsupported OS: $os"; exit 1 ;; esac
    case "$arch" in x86_64|amd64) ARCH_TYPE="amd64" ;; aarch64|arm64) ARCH_TYPE="arm64" ;; *) log_error "Unsupported arch: $arch"; exit 1 ;; esac
    log_info "Detected: $OS_TYPE/$ARCH_TYPE"
}

# ── Check prerequisites ──
check_prereqs() {
    command -v curl &>/dev/null || command -v wget &>/dev/null || { log_error "curl or wget required"; exit 1; }
    if [[ "$OS_TYPE" == "linux" ]] && ! command -v systemctl &>/dev/null; then
        log_warn "systemctl not found, skipping systemd"; NO_SYSTEMD=1
    fi
}

# ── Create directories ──
create_dirs() {
    log_info "Creating directories..."
    mkdir -p "$BIN_DIR" "$AGENT_DIR" "$CONFIG_DIR" "$DATA_DIR"/audit "$DATA_DIR"/forensics
    log_ok "Directories created"
}

# ── Write shield config ──
write_shield_config() {
    log_info "Configuring shield (mode: $SHIELD_MODE)..."
    local shield_file="$CONFIG_DIR/shield.json"

    # Validate mode
    case "$SHIELD_MODE" in strict|moderate|permissive) ;; *)
        log_warn "Invalid shield mode: $SHIELD_MODE, using moderate"
        SHIELD_MODE="moderate"
    esac

    # Set threshold based on mode
    local threshold
    case "$SHIELD_MODE" in
        strict)      threshold=25 ;;
        moderate)    threshold=50 ;;
        permissive)  threshold=75 ;;
    esac

    cat > "$shield_file" <<EOF
{
  "mode": "$SHIELD_MODE",
  "threshold": $threshold,
  "ast_enabled": true,
  "interpreter_enabled": true,
  "protected_paths": [
    "/etc/shadow", "/etc/passwd", "/etc/sudoers",
    "/root", "/var/log", "/boot",
    "/opt/flowlink/config",
    "$CONFIG_DIR"
  ],
  "blocked_commands": [
    "rm -rf /", "mkfs", "dd if=/dev/zero",
    "chmod -R 777 /", "chown -R",
    "> /dev/sda", ":(){ :|:& };:",
    "curl | sh", "wget | sh", "curl | bash", "wget | bash"
  ]
}
EOF
    chmod 600 "$shield_file"
    log_ok "Shield config: $shield_file (mode=$SHIELD_MODE, threshold=$threshold)"
}

# ── Write environment file ──
write_env_file() {
    local env_file="$CONFIG_DIR/.env"
    cat > "$env_file" <<EOF
# FlowLink Environment
# Edit these values to configure your agent
AGENT_ID=$AGENT_ID
RELAY_URL=$RELAY_URL
RELAY_API=$RELAY_API
SHIELD_MODE=$SHIELD_MODE
DATA_DIR=$DATA_DIR

# Logging
RUST_LOG=info

# Vault (optional — set VAULT_ADDR and VAULT_TOKEN for secret injection)
# VAULT_ADDR=https://vault.example.com:8200
# VAULT_TOKEN=

# SSO/SAML (optional — Enterprise plan required)
# SAML_IDP_METADATA_URL=
# SAML_SP_ENTITY_ID=
EOF
    chmod 600 "$env_file"
    log_ok "Environment file: $env_file"
}

# ── Signup via relay API ──
signup() {
    log_info "Registering agent with relay..."

    [[ -z "$LABEL" ]] && LABEL=$(hostname)

    local signup_url
    # If relay_api is localhost, use the HTTP URL directly
    signup_url="${RELAY_API}/api/v1/signup"

    local response
    response=$(curl -sf --max-time 10 -X POST "$signup_url" \
        -H "Content-Type: application/json" \
        -d "{\"agent_id\":\"$(hostname)-$(date +%s)\",\"os\":\"$OS_TYPE\",\"arch\":\"$ARCH_TYPE\"}" 2>/dev/null)

    if [[ -z "$response" ]]; then
        log_error "Signup failed — cannot reach relay at $signup_url"
        log_info "Make sure the relay is accessible. Use --relay-api to specify a different endpoint."
        exit 1
    fi

    AGENT_ID=$(echo "$response" | python3 -c "import sys,json; print(json.load(sys.stdin)['agent_id'])" 2>/dev/null || \
               echo "$response" | grep -o '"agent_id":"[^"]*"' | cut -d'"' -f4)
    TOKEN=$(echo "$response" | python3 -c "import sys,json; print(json.load(sys.stdin)['token'])" 2>/dev/null || \
            echo "$response" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)

    if [[ -z "$AGENT_ID" ]] || [[ -z "$TOKEN" ]]; then
        log_error "Signup returned invalid response: $response"
        exit 1
    fi

    log_ok "Agent registered: $AGENT_ID"
}

# ── Write config ──
write_config() {
    local config_file="$AGENT_DIR/${AGENT_ID}.json"
    cat > "$config_file" <<EOF
{
  "agent_id": "$AGENT_ID",
  "token": "$TOKEN",
  "relay_url": "$RELAY_URL",
  "label": "$LABEL"
}
EOF
    chmod 600 "$config_file"
    CONFIG_FILE="$config_file"
    log_ok "Config written: $config_file"
}

# ── Download binary ──
download_binary() {
    log_info "Downloading flowlink binary..."

    local url="https://flowlink.flow-masters.ru/downloads/flowlink-${OS_TYPE}-${ARCH_TYPE}.tar.gz"
    local tmp="/tmp/flowlink-$$.tmp"

    if command -v curl &>/dev/null; then
        curl -fsSL --max-time 60 "$url" -o "$tmp"
    else
        wget -q --timeout=60 "$url" -O "$tmp"
    fi

    if [[ ! -s "$tmp" ]]; then
        log_error "Download failed"
        rm -f "$tmp"
        exit 1
    fi

    local extract="/tmp/flowlink-extract-$$"
    mkdir -p "$extract"
    tar xzf "$tmp" -C "$extract" 2>/dev/null
    # Binary might be directly in archive or in a subdirectory
    local bin=$(find "$extract" -name "flowlink" -type f | head -1)
    if [[ -z "$bin" ]]; then
        # Maybe the archive IS the binary
        mv "$tmp" "$BIN_DIR/flowlink"
    else
        mv "$bin" "$BIN_DIR/flowlink"
        rm -rf "$extract"
    fi
    rm -f "$tmp"
    chmod 755 "$BIN_DIR/flowlink"
    log_ok "Binary installed: $BIN_DIR/flowlink"
}

# ── Install systemd service (template) ──
install_systemd() {
    [[ "$OS_TYPE" != "linux" ]] || [[ "$NO_SYSTEMD" -eq 1 ]] && return 0

    log_info "Installing systemd service..."

    # Install template unit file
    cat > /etc/systemd/system/flowlink-agent@.service <<EOF
[Unit]
Description=FlowLink Agent (%i)
After=network-online.target
Wants=network-online.target
StartLimitIntervalSec=300
StartLimitBurst=20

[Service]
Type=simple
WorkingDirectory=$AGENT_DIR
EnvironmentFile=$CONFIG_DIR/.env
ExecStart=$BIN_DIR/flowlink agent -c $AGENT_DIR/%i.json
Restart=always
RestartSec=2
TimeoutStartSec=30
TimeoutStopSec=10
KillMode=mixed
KillSignal=SIGTERM
FinalKillSignal=SIGKILL
SendSIGKILL=yes
StandardOutput=append:/var/log/flowlink-agent-%i.log
StandardError=append:/var/log/flowlink-agent-%i.log
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable "flowlink-agent@${AGENT_ID}"
    systemctl start "flowlink-agent@${AGENT_ID}"
    sleep 3

    if systemctl is-active --quiet "flowlink-agent@${AGENT_ID}"; then
        log_ok "Service started: flowlink-agent@${AGENT_ID}"
    else
        log_error "Service failed to start"
        journalctl -u "flowlink-agent@${AGENT_ID}" -n 20 --no-pager
        exit 1
    fi
}

# ── Install ServerGuard systemd service ──
install_guard() {
    [[ "$OS_TYPE" != "linux" ]] || [[ "$NO_SYSTEMD" -eq 1 ]] || [[ "$NO_GUARD" -eq 1 ]] && return 0

    log_info "Installing ServerGuard service..."

    cat > /etc/systemd/system/flowlink-guard@.service <<EOF
[Unit]
Description=FlowLink ServerGuard (%i)
After=network-online.target flowlink-agent@%i.service
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=$AGENT_DIR
ExecStart=$BIN_DIR/flowlink guard --relay ${RELAY_API} --agent ${AGENT_ID} --key ${TOKEN} start --foreground --docker --watch /etc,$AGENT_DIR
Restart=on-failure
RestartSec=5
TimeoutStartSec=30
TimeoutStopSec=10
Environment=RUST_LOG=info
StandardOutput=append:/var/log/flowlink-guard-%i.log
StandardError=append:/var/log/flowlink-guard-%i.log
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable "flowlink-guard@${AGENT_ID}"
    systemctl start "flowlink-guard@${AGENT_ID}"
    sleep 2

    if systemctl is-active --quiet "flowlink-guard@${AGENT_ID}"; then
        log_ok "ServerGuard started: flowlink-guard@${AGENT_ID}"
    else
        log_warn "ServerGuard failed to start (non-critical, agent works without it)"
        journalctl -u "flowlink-guard@${AGENT_ID}" -n 10 --no-pager
    fi
}

# ── Install LaunchAgent (macOS) ──
install_launchagent() {
    [[ "$OS_TYPE" != "darwin" ]] && return 0

    log_info "Installing LaunchAgent..."
    local plist="$HOME/Library/LaunchAgents/com.flowlink.agent.plist"
    mkdir -p "$(dirname "$plist")"
    cat > "$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>com.flowlink.agent</string>
    <key>ProgramArguments</key>
    <array>
        <string>$BIN_DIR/flowlink</string><string>agent</string><string>-c</string><string>$CONFIG_FILE</string>
    </array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
    <key>StandardOutPath</key><string>$FLOWLINK_HOME/flowlink.log</string>
    <key>StandardErrorPath</key><string>$FLOWLINK_HOME/flowlink.log</string>
</dict>
</plist>
EOF
    launchctl load "$plist" 2>/dev/null || true
    log_ok "LaunchAgent installed"
}

# ── Show status ──
show_status() {
    echo ""
    echo -e "${GREEN}═══════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  FlowLink Installed Successfully!${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════${NC}"
    echo ""
    echo -e "  Agent ID:     ${CYAN}$AGENT_ID${NC}"
    echo -e "  Config:       $CONFIG_DIR/${AGENT_ID}.json"
    echo -e "  Shield:       $CONFIG_DIR/shield.json (mode: $SHIELD_MODE)"
    echo -e "  Binary:       $BIN_DIR/flowlink"
    echo -e "  Relay:        $RELAY_URL"
    echo -e "  Label:        $LABEL"
    echo ""
    echo -e "${YELLOW}Next Steps:${NC}"
    echo ""
    echo "  1. Add to your AI agent config (e.g. ~/.claude/mcp.json):"
    echo ""
    echo -e "     ${CYAN}{${NC}"
    echo -e "     ${CYAN}  \"mcpServers\": {${NC}"
    echo -e "     ${CYAN}    \"flowlink\": {${NC}"
    echo -e "     ${CYAN}      \"url\": \"${RELAY_API}/mcp/stream\",${NC}"
    echo -e "     ${CYAN}      \"headers\": { \"Authorization\": \"Bearer $TOKEN\" }${NC}"
    echo -e "     ${CYAN}    }${NC}"
    echo -e "     ${CYAN}  }${NC}"
    echo -e "     ${CYAN}}${NC}"
    echo ""
    echo "  2. Verify connection:"
    echo "     $BIN_DIR/flowlink agent -c $CONFIG_DIR/${AGENT_ID}.json status"
    echo ""
    echo "  3. Configure features in relay dashboard:"
    echo "     https://flowlink.flow-masters.ru"
    echo ""
    echo -e "  ${YELLOW}Available features by plan:${NC}"
    echo -e "    Starter (free):    Shield, Policy Engine, Audit Log, E2EE"
    echo -e "    Professional:      + Approval, RBAC, ServerGuard, Forensics, AI Ops, Service Catalog"
    echo -e "    Business:          + Pattern Learning, SIEM Export, Webhooks, Change Management"
    echo -e "    Enterprise:       + SSO/SAML, On-Premise"
    echo ""

    if [[ "$OS_TYPE" == "linux" ]] && [[ "$NO_SYSTEMD" -eq 0 ]]; then
        echo -e "  ${YELLOW}Service management:${NC}"
        echo "    sudo systemctl status flowlink-agent@${AGENT_ID}"
        echo "    sudo systemctl restart flowlink-agent@${AGENT_ID}"
        echo "    sudo journalctl -u flowlink-agent@${AGENT_ID} -f"
        if [[ "$NO_GUARD" -eq 0 ]]; then
            echo "    sudo systemctl status flowlink-guard@${AGENT_ID}"
        fi
        echo ""
    elif [[ "$OS_TYPE" == "darwin" ]]; then
        echo -e "  ${YELLOW}Service management:${NC}"
        echo "    launchctl list | grep flowlink"
        echo "    tail -f $FLOWLINK_HOME/flowlink.log"
        echo ""
    fi

    echo -e "  ${YELLOW}Configuration files:${NC}"
    echo "    Agent config:  $CONFIG_DIR/${AGENT_ID}.json"
    echo "    Shield config: $CONFIG_DIR/shield.json"
    echo "    Environment:   $CONFIG_DIR/.env"
    echo "    Audit logs:    $DATA_DIR/audit/"
    echo "    Forensics:     $DATA_DIR/forensics/"
    echo ""
    echo -e "${GREEN}Agent will auto-reconnect on any network interruption.${NC}"
    if [[ "$OS_TYPE" == "linux" ]] && [[ "$NO_GUARD" -eq 0 ]]; then
        echo -e "${GREEN}ServerGuard monitors file/Docker changes on this host.${NC}"
    fi
    echo ""
}

# ── Main ──
main() {
    echo ""
    echo -e "${GREEN}═══════════════════════════════════════${NC}"
    echo -e "${GREEN}  FlowLink Agent Installer${NC}"
    echo -e "${GREEN}═══════════════════════════════════════${NC}"
    echo ""

    detect_os
    check_prereqs
    create_dirs
    signup
    write_config
    write_shield_config
    write_env_file
    download_binary
    install_systemd
    install_guard
    install_launchagent
    show_status
}

main

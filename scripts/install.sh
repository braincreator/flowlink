#!/usr/bin/env bash
#
# FlowLink Agent Installer
# Устанавливает FlowLink агент на Linux (x86_64/aarch64) и macOS (arm64)
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/braincreator/flowlink/main/scripts/install.sh | bash
#   ./install.sh [--relay URL] [--label NAME] [--no-systemd]
#
set -euo pipefail

# ── Colors ──
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'

# ── Defaults ──
GITHUB_REPO="braincreator/flowlink"
INSTALL_DIR="/opt/flowlink"
BIN_DIR="$INSTALL_DIR/bin"
AGENT_DIR="$INSTALL_DIR/agent"
CONFIG_DIR="$INSTALL_DIR"
FLOWLINK_HOME="${FLOWLINK_HOME:-$HOME/.flowlink}"
RELAY_URL="${RELAY_URL:-wss://relay.flow-masters.ru:9093}"
RELAY_API="${RELAY_API:-http://127.0.0.1:9081}"
LABEL=""
NO_SYSTEMD=0

# ── Parse arguments ──
while [[ $# -gt 0 ]]; do
    case $1 in
        --relay)    RELAY_URL="$2"; shift 2 ;;
        --relay-api) RELAY_API="$2"; shift 2 ;;
        --label)    LABEL="$2"; shift 2 ;;
        --no-systemd) NO_SYSTEMD=1; shift ;;
        --help|-h)
            echo "Usage: $0 [--relay URL] [--relay-api URL] [--label NAME] [--no-systemd]"
            echo ""
            echo "Options:"
            echo "  --relay URL      WSS relay URL (default: wss://relay.flow-masters.ru:9093)"
            echo "  --relay-api URL  HTTP relay API (default: http://127.0.0.1:9081)"
            echo "  --label NAME     Agent label (default: hostname)"
            echo "  --no-systemd     Skip systemd service installation"
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
    mkdir -p "$BIN_DIR" "$AGENT_DIR" "$CONFIG_DIR"
    log_ok "Directories created"
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
ExecStart=$BIN_DIR/flowlink agent -c $AGENT_DIR/%i.json
Restart=always
RestartSec=2
TimeoutStartSec=30
TimeoutStopSec=10
KillMode=mixed
KillSignal=SIGTERM
FinalKillSignal=SIGKILL
SendSIGKILL=yes
Environment=RUST_LOG=info
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
    echo -e "${GREEN}═══════════════════════════════════════${NC}"
    echo -e "${GREEN}  FlowLink Agent Installed!${NC}"
    echo -e "${GREEN}═══════════════════════════════════════${NC}"
    echo ""
    echo "  Agent ID:  $AGENT_ID"
    echo "  Config:    $CONFIG_FILE"
    echo "  Binary:    $BIN_DIR/flowlink"
    echo "  Relay:     $RELAY_URL"
    echo "  Label:     $LABEL"
    echo ""

    if [[ "$OS_TYPE" == "linux" ]] && [[ "$NO_SYSTEMD" -eq 0 ]]; then
        echo "  Commands:"
        echo "    sudo systemctl status flowlink-agent@${AGENT_ID}"
        echo "    sudo systemctl restart flowlink-agent@${AGENT_ID}"
        echo "    sudo journalctl -u flowlink-agent@${AGENT_ID} -f"
    elif [[ "$OS_TYPE" == "darwin" ]]; then
        echo "  Commands:"
        echo "    launchctl list | grep flowlink"
        echo "    tail -f $FLOWLINK_HOME/flowlink.log"
    fi
    echo ""
    echo -e "${GREEN}Agent will auto-reconnect on any network interruption.${NC}"
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
    download_binary
    install_systemd
    install_launchagent
    show_status
}

main

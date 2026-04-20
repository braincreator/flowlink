#!/usr/bin/env bash
#
# FlowLink Agent Installer
# Устанавливает FlowLink агент на Linux (x86_64/aarch64) и macOS (arm64)
#
# Usage: curl -fsSL https://raw.githubusercontent.com/braincreator/flowlink/main/scripts/install.sh | bash
#   или: ./install.sh [--token TOKEN] [--relay URL] [--label NAME]
#
set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Defaults
GITHUB_REPO="braincreator/flowlink"
BINARY_NAME="flowlink"
INSTALL_DIR="/usr/local/bin"
FLOWLINK_HOME="${FLOWLINK_HOME:-$HOME/.flowlink}"
RELAY_URL="${RELAY_URL:-wss://relay.flow-masters.ru:9093}"
TOKEN=""
LABEL=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --token)
            TOKEN="$2"
            shift 2
            ;;
        --relay)
            RELAY_URL="$2"
            shift 2
            ;;
        --label)
            LABEL="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [--token TOKEN] [--relay URL] [--label NAME]"
            echo ""
            echo "Options:"
            echo "  --token TOKEN   Предустановленный токен (если есть)"
            echo "  --relay URL     URL реле (default: wss://relay.flowmasters.ru/ws)"
            echo "  --label NAME    Имя агента (default: hostname)"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if command exists
command_exists() {
    command -v "$1" &> /dev/null
}

# Detect OS and architecture
detect_os() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"
    
    case "$OS" in
        Linux)
            OS_TYPE="linux"
            ;;
        Darwin)
            OS_TYPE="darwin"
            ;;
        *)
            log_error "Unsupported OS: $OS"
            exit 1
            ;;
    esac
    
    case "$ARCH" in
        x86_64|amd64)
            ARCH_TYPE="amd64"
            ;;
        aarch64|arm64)
            ARCH_TYPE="arm64"
            ;;
        *)
            log_error "Unsupported architecture: $ARCH"
            exit 1
            ;;
    esac
    
    log_info "Detected: $OS_TYPE/$ARCH_TYPE"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check curl or wget
    if ! command_exists curl && ! command_exists wget; then
        log_error "Neither curl nor wget found. Please install one of them."
        exit 1
    fi
    
    # Check for systemd on Linux
    if [[ "$OS_TYPE" == "linux" ]]; then
        if ! command_exists systemctl; then
            log_warn "systemctl not found. Will not install systemd service."
            NO_SYSTEMD=1
        else
            NO_SYSTEMD=0
        fi
    fi
    
    log_success "Prerequisites OK"
}

# Create flowlink directory
create_flowlink_dir() {
    log_info "Creating $FLOWLINK_HOME..."
    
    mkdir -p "$FLOWLINK_HOME"
    mkdir -p "$FLOWLINK_HOME/backups"
    chmod 700 "$FLOWLINK_HOME"
    
    log_success "Directory created"
}

# Generate agent_id (UUID without dashes)
generate_agent_id() {
    if command_exists uuidgen; then
        # macOS/BSD
        uuidgen | tr -d '-' | tr '[:upper:]' '[:lower:]'
    elif [[ -f /proc/sys/kernel/random/uuid ]]; then
        # Linux
        cat /proc/sys/kernel/random/uuid | tr -d '-'
    else
        # Fallback: use date + random
        echo "$(date +%s)$(shuf -i 100000-999999 -n 1)" | md5sum | head -c 32
    fi
}

# Generate token (64 hex chars = 32 bytes)
generate_token() {
    if command_exists openssl; then
        openssl rand -hex 32
    else
        # Fallback
        cat /dev/urandom | tr -dc 'a-f0-9' | fold -w 64 | head -n 1
    fi
}

# Create flowlink.json
create_config() {
    log_info "Creating configuration..."
    
    # Generate values if not provided
    if [[ -z "$TOKEN" ]]; then
        TOKEN=$(generate_token)
    fi
    
    AGENT_ID=$(generate_agent_id)
    
    if [[ -z "$LABEL" ]]; then
        LABEL=$(hostname)
    fi
    
    CONFIG_FILE="$FLOWLINK_HOME/flowlink.json"
    
    cat > "$CONFIG_FILE" <<EOF
{
  "agent_id": "$AGENT_ID",
  "token": "$TOKEN",
  "relay_url": "$RELAY_URL",
  "label": "$LABEL"
}
EOF
    
    chmod 600 "$CONFIG_FILE"
    
    log_success "Configuration created"
    echo ""
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo -e "${GREEN}  Agent Credentials${NC}"
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo -e "  Agent ID:  ${YELLOW}$AGENT_ID${NC}"
    echo -e "  Token:     ${YELLOW}$TOKEN${NC}"
    echo -e "  Relay:     $RELAY_URL"
    echo -e "  Label:     $LABEL"
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo ""
    echo -e "${YELLOW}⚠️  IMPORTANT: Save these credentials!${NC}"
    echo -e "${YELLOW}   Send Agent ID and Token to the relay operator.${NC}"
    echo ""
}

# Download binary
download_binary() {
    log_info "Downloading flowlink binary..."
    
    # Determine download URL
    LATEST_URL="https://flowlink.flow-masters.ru/downloads/flowlink-${OS_TYPE}-${ARCH_TYPE}.tar.gz"
    
    TEMP_FILE="/tmp/flowlink-$$.tmp"
    
    if command_exists curl; then
        curl -fsSL "$LATEST_URL" -o "$TEMP_FILE"
    else
        wget -q "$LATEST_URL" -O "$TEMP_FILE"
    fi
    
    if [[ ! -s "$TEMP_FILE" ]]; then
        log_error "Download failed or file is empty"
        rm -f "$TEMP_FILE"
        exit 1
    fi
    
    # Extract from tar.gz
    EXTRACT_DIR="/tmp/flowlink-extract-$$.tmp"
    mkdir -p "$EXTRACT_DIR"
    tar xzf "$TEMP_FILE" -C "$EXTRACT_DIR"
    cp "$EXTRACT_DIR/flowlink" "$TEMP_FILE"
    chmod +x "$TEMP_FILE"
    rm -rf "$EXTRACT_DIR"
    
    log_success "Binary downloaded"
}

# Install binary
install_binary() {
    log_info "Installing binary to $INSTALL_DIR..."
    
    # Check if we need sudo
    if [[ ! -w "$INSTALL_DIR" ]]; then
        SUDO="sudo"
        log_info "Using sudo for installation..."
    else
        SUDO=""
    fi
    
    $SUDO mv /tmp/flowlink-$$.tmp "$INSTALL_DIR/flowlink"
    $SUDO chmod 755 "$INSTALL_DIR/flowlink"
    
    log_success "Binary installed"
}

# Install systemd service (Linux only)
install_systemd_service() {
    if [[ "$OS_TYPE" != "linux" ]] || [[ "$NO_SYSTEMD" -eq 1 ]]; then
        return 0
    fi
    
    log_info "Installing systemd service..."
    
    # Determine user
    if [[ "$EUID" -eq 0 ]]; then
        SERVICE_USER="root"
        SERVICE_HOME="/root/.flowlink"
    else
        SERVICE_USER="$USER"
        SERVICE_HOME="$FLOWLINK_HOME"
    fi
    
    # Create service file
    SERVICE_FILE="/etc/systemd/system/flowlink.service"
    
    sudo tee "$SERVICE_FILE" > /dev/null <<EOF
[Unit]
Description=FlowLink Agent
Documentation=https://github.com/braincreator/flowlink
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$SERVICE_USER
WorkingDirectory=$SERVICE_HOME
ExecStart=$INSTALL_DIR/flowlink agent -c $SERVICE_HOME/flowlink.json
Restart=always
RestartSec=5
Environment=FLOWLINK_HOME=$SERVICE_HOME

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=$SERVICE_HOME
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF
    
    sudo systemctl daemon-reload
    sudo systemctl enable flowlink
    
    log_success "Systemd service installed"
}

# Install LaunchAgent (macOS only)
install_launchagent() {
    if [[ "$OS_TYPE" != "darwin" ]]; then
        return 0
    fi
    
    log_info "Installing LaunchAgent..."
    
    PLIST_FILE="$HOME/Library/LaunchAgents/com.flowlink.agent.plist"
    
    cat > "$PLIST_FILE" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.flowlink.agent</string>
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_DIR/flowlink</string>
        <string>agent</string>
        <string>-c</string>
        <string>$FLOWLINK_HOME/flowlink.json</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$FLOWLINK_HOME/flowlink.log</string>
    <key>StandardErrorPath</key>
    <string>$FLOWLINK_HOME/flowlink.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>FLOWLINK_HOME</key>
        <string>$FLOWLINK_HOME</string>
    </dict>
</dict>
</plist>
EOF
    
    launchctl load "$PLIST_FILE" 2>/dev/null || true
    
    log_success "LaunchAgent installed"
}

# Start service
start_service() {
    log_info "Starting FlowLink agent..."
    
    if [[ "$OS_TYPE" == "linux" ]] && [[ "$NO_SYSTEMD" -eq 0 ]]; then
        sudo systemctl start flowlink
        sleep 2
        if sudo systemctl is-active --quiet flowlink; then
            log_success "Service started"
        else
            log_error "Service failed to start"
            sudo journalctl -u flowlink -n 20 --no-pager
            exit 1
        fi
    elif [[ "$OS_TYPE" == "darwin" ]]; then
        launchctl start com.flowlink.agent 2>/dev/null || true
        sleep 2
        log_success "Service started"
    else
        log_info "Run manually: $INSTALL_DIR/flowlink agent start"
    fi
}

# Show status
show_status() {
    echo ""
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo -e "${GREEN}  FlowLink Installation Complete!${NC}"
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo ""
    echo "Configuration: $FLOWLINK_HOME/flowlink.json"
    echo "Binary:        $INSTALL_DIR/flowlink"
    echo ""
    
    if [[ "$OS_TYPE" == "linux" ]] && [[ "$NO_SYSTEMD" -eq 0 ]]; then
        echo "Service status:"
        sudo systemctl status flowlink --no-pager -l || true
        echo ""
        echo "Commands:"
        echo "  sudo systemctl status flowlink   # Check status"
        echo "  sudo systemctl stop flowlink     # Stop agent"
        echo "  sudo systemctl restart flowlink  # Restart agent"
        echo "  sudo journalctl -u flowlink -f   # View logs"
    elif [[ "$OS_TYPE" == "darwin" ]]; then
        echo "LaunchAgent status:"
        launchctl list | grep flowlink || echo "  (loading...)"
        echo ""
        echo "Commands:"
        echo "  launchctl list | grep flowlink  # Check status"
        echo "  launchctl stop com.flowlink.agent   # Stop agent"
        echo "  tail -f $FLOWLINK_HOME/flowlink.log  # View logs"
    fi
    echo ""
    echo -e "${YELLOW}Next steps:${NC}"
    echo "  1. Send your Agent ID and Token to the relay operator"
    echo "  2. Wait for operator to whitelist your agent"
    echo "  3. Agent will connect automatically"
    echo ""
}

# Main installation flow
main() {
    echo ""
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo -e "${GREEN}  FlowLink Agent Installer${NC}"
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo ""
    
    detect_os
    check_prerequisites
    create_flowlink_dir
    
    # Check if already installed
    if [[ -f "$FLOWLINK_HOME/flowlink.json" ]]; then
        log_warn "Configuration already exists at $FLOWLINK_HOME/flowlink.json"
        read -p "Overwrite? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            log_info "Keeping existing configuration"
        else
            create_config
        fi
    else
        create_config
    fi
    
    download_binary
    install_binary
    install_systemd_service
    install_launchagent
    start_service
    show_status
}

# Run main
main

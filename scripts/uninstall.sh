#!/usr/bin/env bash
#
# FlowLink Agent Uninstaller
# Completely removes FlowLink agent from the system
#
# Usage: ./uninstall.sh [--purge]
#
set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Defaults
INSTALL_DIR="/usr/local/bin"
FLOWLINK_HOME="${FLOWLINK_HOME:-$HOME/.flowlink}"
PURGE_CONFIG=false
NOTIFY_RELAY=true

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --purge)
            PURGE_CONFIG=true
            shift
            ;;
        --no-notify)
            NOTIFY_RELAY=false
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--purge] [--no-notify]"
            echo ""
            echo "Options:"
            echo "  --purge      Remove configuration and data (~/.flowlink)"
            echo "  --no-notify  Don't send disconnect notification to relay"
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

# Detect OS
detect_os() {
    OS="$(uname -s)"
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
}

# Stop systemd service (Linux)
stop_systemd_service() {
    if [[ "$OS_TYPE" != "linux" ]]; then
        return 0
    fi
    
    if ! command -v systemctl &> /dev/null; then
        return 0
    fi
    
    if [[ -f "/etc/systemd/system/flowlink.service" ]]; then
        log_info "Stopping systemd service..."
        
        sudo systemctl stop flowlink 2>/dev/null || true
        sudo systemctl disable flowlink 2>/dev/null || true
        
        log_success "Service stopped"
    fi
}

# Stop LaunchAgent (macOS)
stop_launchagent() {
    if [[ "$OS_TYPE" != "darwin" ]]; then
        return 0
    fi
    
    local plist_file="$HOME/Library/LaunchAgents/com.flowlink.agent.plist"
    
    if [[ -f "$plist_file" ]]; then
        log_info "Stopping LaunchAgent..."
        
        launchctl unload "$plist_file" 2>/dev/null || true
        launchctl stop com.flowlink.agent 2>/dev/null || true
        
        log_success "LaunchAgent stopped"
    fi
}

# Remove systemd service file (Linux)
remove_systemd_service() {
    if [[ "$OS_TYPE" != "linux" ]]; then
        return 0
    fi
    
    local service_file="/etc/systemd/system/flowlink.service"
    
    if [[ -f "$service_file" ]]; then
        log_info "Removing systemd service file..."
        
        sudo rm -f "$service_file"
        sudo systemctl daemon-reload
        
        log_success "Service file removed"
    fi
}

# Remove LaunchAgent plist (macOS)
remove_launchagent() {
    if [[ "$OS_TYPE" != "darwin" ]]; then
        return 0
    fi
    
    local plist_file="$HOME/Library/LaunchAgents/com.flowlink.agent.plist"
    
    if [[ -f "$plist_file" ]]; then
        log_info "Removing LaunchAgent plist..."
        
        rm -f "$plist_file"
        
        log_success "LaunchAgent plist removed"
    fi
}

# Notify relay about disconnect
notify_relay() {
    if [[ "$NOTIFY_RELAY" != "true" ]]; then
        return 0
    fi
    
    if [[ ! -f "$FLOWLINK_HOME/config.json" ]]; then
        return 0
    fi
    
    log_info "Notifying relay about disconnect..."
    
    # Extract relay URL and agent_id from config
    local relay_url=$(grep -o '"relay_url"[[:space:]]*:[[:space:]]*"[^"]*"' "$FLOWLINK_HOME/config.json" | cut -d'"' -f4)
    local agent_id=$(grep -o '"agent_id"[[:space:]]*:[[:space:]]*"[^"]*"' "$FLOWLINK_HOME/config.json" | cut -d'"' -f4)
    local token=$(grep -o '"token"[[:space:]]*:[[:space:]]*"[^"]*"' "$FLOWLINK_HOME/config.json" | cut -d'"' -f4)
    
    if [[ -z "$relay_url" ]] || [[ -z "$agent_id" ]]; then
        log_warn "Could not extract relay URL or agent_id from config"
        return 0
    fi
    
    # Convert WebSocket URL to HTTP URL
    local http_url="${relay_url/wss:/https:}"
    http_url="${http_url/ws:/http:}"
    http_url="${http_url%/ws}"
    
    # Send disconnect notification
    local api_url="${http_url}/api/v1/agents/${agent_id}/disconnect"
    
    if command -v curl &> /dev/null; then
        curl -s -X POST \
            -H "Authorization: Bearer $token" \
            -H "Content-Type: application/json" \
            -d '{"reason": "uninstall"}' \
            "$api_url" &>/dev/null || true
    fi
    
    log_success "Relay notified"
}

# Remove binary
remove_binary() {
    local binary_path="$INSTALL_DIR/flowlink"
    
    if [[ -f "$binary_path" ]]; then
        log_info "Removing binary..."
        
        if [[ -w "$INSTALL_DIR" ]]; then
            rm -f "$binary_path"
        else
            sudo rm -f "$binary_path"
        fi
        
        log_success "Binary removed"
    else
        log_info "Binary not found at $binary_path"
    fi
}

# Remove configuration and data
remove_config() {
    if [[ "$PURGE_CONFIG" != "true" ]]; then
        return 0
    fi
    
    if [[ -d "$FLOWLINK_HOME" ]]; then
        log_info "Removing configuration and data..."
        
        rm -rf "$FLOWLINK_HOME"
        
        log_success "Configuration removed"
    fi
}

# Ask for confirmation
ask_confirmation() {
    echo ""
    echo -e "${YELLOW}This will uninstall FlowLink Agent:${NC}"
    echo "  • Stop and remove service (systemd/LaunchAgent)"
    echo "  • Remove binary from $INSTALL_DIR"
    if [[ "$PURGE_CONFIG" == "true" ]]; then
        echo -e "  • ${RED}Remove all data in $FLOWLINK_HOME${NC}"
    else
        echo "  • Keep configuration in $FLOWLINK_HOME"
    fi
    echo ""
    
    read -p "Continue? (y/N): " -n 1 -r
    echo
    
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Uninstall cancelled"
        exit 0
    fi
}

# Main uninstall flow
main() {
    echo ""
    echo -e "${RED}════════════════════════════════════════${NC}"
    echo -e "${RED}  FlowLink Agent Uninstaller${NC}"
    echo -e "${RED}════════════════════════════════════════${NC}"
    echo ""
    
    detect_os
    ask_confirmation
    
    stop_systemd_service
    stop_launchagent
    remove_systemd_service
    remove_launchagent
    notify_relay
    remove_binary
    remove_config
    
    echo ""
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo -e "${GREEN}  FlowLink Agent Uninstalled${NC}"
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo ""
    
    if [[ "$PURGE_CONFIG" != "true" ]] && [[ -d "$FLOWLINK_HOME" ]]; then
        echo "Configuration preserved at: $FLOWLINK_HOME"
        echo "To remove completely, run: $0 --purge"
        echo ""
    fi
}

# Run main
main

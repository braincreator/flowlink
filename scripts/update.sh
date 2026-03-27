#!/usr/bin/env bash
#
# FlowLink Agent Updater
# Updates FlowLink agent to the latest version
#
# Usage: ./update.sh [--version VERSION]
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
TARGET_VERSION=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --version)
            TARGET_VERSION="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [--version VERSION]"
            echo ""
            echo "Options:"
            echo "  --version VERSION   Install specific version (e.g., v0.2.0)"
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
}

# Get current version
get_current_version() {
    if [[ -x "$INSTALL_DIR/flowlink" ]]; then
        CURRENT_VERSION=$("$INSTALL_DIR/flowlink" version 2>/dev/null | head -1 | awk '{print $2}' || echo "unknown")
    else
        CURRENT_VERSION="not installed"
    fi
    
    log_info "Current version: $CURRENT_VERSION"
}

# Get latest version from GitHub
get_latest_version() {
    local api_url="https://api.github.com/repos/$GITHUB_REPO/releases/latest"
    
    if command -v curl &> /dev/null; then
        LATEST_VERSION=$(curl -fsSL "$api_url" | grep '"tag_name"' | cut -d'"' -f4)
    else
        LATEST_VERSION=$(wget -qO- "$api_url" | grep '"tag_name"' | cut -d'"' -f4)
    fi
    
    if [[ -z "$LATEST_VERSION" ]]; then
        log_error "Could not determine latest version"
        exit 1
    fi
    
    log_info "Latest version: $LATEST_VERSION"
}

# Download new binary
download_binary() {
    log_info "Downloading flowlink $TARGET_VERSION..."
    
    local download_url
    
    if [[ -n "$TARGET_VERSION" ]]; then
        download_url="https://github.com/$GITHUB_REPO/releases/download/$TARGET_VERSION/flowlink-${OS_TYPE}-${ARCH_TYPE}"
    else
        download_url="https://github.com/$GITHUB_REPO/releases/latest/download/flowlink-${OS_TYPE}-${ARCH_TYPE}"
    fi
    
    TEMP_FILE="/tmp/flowlink-update-$$.tmp"
    
    if command -v curl &> /dev/null; then
        curl -fsSL "$download_url" -o "$TEMP_FILE"
    else
        wget -q "$download_url" -O "$TEMP_FILE"
    fi
    
    if [[ ! -s "$TEMP_FILE" ]]; then
        log_error "Download failed or file is empty"
        rm -f "$TEMP_FILE"
        exit 1
    fi
    
    chmod +x "$TEMP_FILE"
    
    # Verify it runs
    if ! "$TEMP_FILE" version &>/dev/null; then
        log_error "Downloaded binary is not executable"
        rm -f "$TEMP_FILE"
        exit 1
    fi
    
    log_success "Binary downloaded"
}

# Stop service before update
stop_service() {
    if [[ "$OS_TYPE" == "linux" ]] && command -v systemctl &> /dev/null; then
        if systemctl is-active --quiet flowlink 2>/dev/null; then
            log_info "Stopping service..."
            sudo systemctl stop flowlink
            SERVICE_WAS_RUNNING=true
        else
            SERVICE_WAS_RUNNING=false
        fi
    elif [[ "$OS_TYPE" == "darwin" ]]; then
        if launchctl list | grep -q com.flowlink.agent; then
            log_info "Stopping LaunchAgent..."
            launchctl stop com.flowlink.agent 2>/dev/null || true
            SERVICE_WAS_RUNNING=true
        else
            SERVICE_WAS_RUNNING=false
        fi
    else
        SERVICE_WAS_RUNNING=false
    fi
}

# Start service after update
start_service() {
    if [[ "$SERVICE_WAS_RUNNING" != "true" ]]; then
        return 0
    fi
    
    if [[ "$OS_TYPE" == "linux" ]] && command -v systemctl &> /dev/null; then
        log_info "Starting service..."
        sudo systemctl start flowlink
        sleep 2
        if sudo systemctl is-active --quiet flowlink; then
            log_success "Service started"
        else
            log_error "Service failed to start"
            exit 1
        fi
    elif [[ "$OS_TYPE" == "darwin" ]]; then
        log_info "Starting LaunchAgent..."
        launchctl start com.flowlink.agent 2>/dev/null || true
        sleep 2
        log_success "LaunchAgent started"
    fi
}

# Replace binary
replace_binary() {
    log_info "Replacing binary..."
    
    # Check if we need sudo
    if [[ -w "$INSTALL_DIR" ]]; then
        SUDO=""
    else
        SUDO="sudo"
    fi
    
    $SUDO mv "$TEMP_FILE" "$INSTALL_DIR/flowlink"
    $SUDO chmod 755 "$INSTALL_DIR/flowlink"
    
    log_success "Binary replaced"
}

# Main update flow
main() {
    echo ""
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo -e "${GREEN}  FlowLink Agent Updater${NC}"
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo ""
    
    detect_os
    get_current_version
    
    # Determine target version
    if [[ -z "$TARGET_VERSION" ]]; then
        get_latest_version
        TARGET_VERSION="$LATEST_VERSION"
    fi
    
    # Check if already up to date
    if [[ "$CURRENT_VERSION" == "$TARGET_VERSION" ]]; then
        log_success "Already at version $CURRENT_VERSION"
        exit 0
    fi
    
    stop_service
    download_binary
    replace_binary
    start_service
    
    # Verify
    NEW_VERSION=$("$INSTALL_DIR/flowlink" version | head -1 | awk '{print $2}')
    
    echo ""
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo -e "${GREEN}  Update Complete!${NC}"
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo ""
    echo "Previous version: $CURRENT_VERSION"
    echo "New version:      $NEW_VERSION"
    echo ""
}

# Run main
main

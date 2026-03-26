#!/bin/bash
# FlowLink Agent — One-liner Installer
# Использование: curl -sSL https://install.flowmasters.ru | bash
# Или локально:  bash scripts/install.sh

set -euo pipefail

# Цвета
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[INFO]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# Определяем ОС и архитектуру
detect_platform() {
    local os arch
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    case "$os" in
        linux*)  os="linux" ;;
        darwin*) os="darwin" ;;
        *)       error "Неподдерживаемая ОС: $os"; exit 1 ;;
    esac

    case "$arch" in
        x86_64|amd64) arch="amd64" ;;
        aarch64|arm64) arch="arm64" ;;
        *)             error "Неподдерживаемая архитектура: $arch"; exit 1 ;;
    esac

    echo "${os}_${arch}"
}

# Проверяем зависимости
check_deps() {
    if ! command -v curl &>/dev/null; then
        error "curl не найден. Установите curl."
        exit 1
    fi
}

# Главная установка
main() {
    echo ""
    echo "╔══════════════════════════════════════╗"
    echo "║       FlowLink Agent Installer       ║"
    echo "║  Удалённое AI-управление машиной     ║"
    echo "╚══════════════════════════════════════╝"
    echo ""

    check_deps

    local platform
    platform=$(detect_platform)
    info "Платформа: $platform"

    # Директория установки
    local install_dir="${HOME}/.flowlink/bin"
    mkdir -p "$install_dir"

    local binary_url="https://github.com/braincreator/flowlink/releases/latest/download/flowlink_${platform}"
    local binary_path="${install_dir}/flowlink"

    # Скачиваем бинарник
    info "Скачивание flowlink..."
    if ! curl -fSL -o "$binary_path" "$binary_url" 2>/dev/null; then
        warn "Релиз не найден, пробуем dev-версию..."
        # Fallback: если нет релиза, скачиваем из CI
        binary_url="https://releases.flowmasters.ru/flowlink_${platform}"
        if ! curl -fSL -o "$binary_path" "$binary_url" 2>/dev/null; then
            error "Не удалось скачать бинарник. Установите Go и соберите вручную:"
            error "  go install github.com/braincreator/flowlink/cmd/agent@latest"
            exit 1
        fi
    fi

    chmod +x "$binary_path"
    info "Бинарник установлен: $binary_path"

    # Добавляем в PATH
    local shell_rc=""
    if [ -n "${ZSH_VERSION:-}" ]; then
        shell_rc="${HOME}/.zshrc"
    elif [ -n "${BASH_VERSION:-}" ]; then
        shell_rc="${HOME}/.bashrc"
    fi

    if [ -n "$shell_rc" ] && ! grep -q '\.flowlink/bin' "$shell_rc" 2>/dev/null; then
        echo '' >> "$shell_rc"
        echo '# FlowLink Agent' >> "$shell_rc"
        echo 'export PATH="$HOME/.flowlink/bin:$PATH"' >> "$shell_rc"
        export PATH="${install_dir}:${PATH}"
        info "Добавлено в PATH ($shell_rc)"
        warn "Перезапустите терминал или выполните: source $shell_rc"
    fi

    echo ""
    info "Установка завершена! ✅"
    echo ""
    echo "Следующие шаги:"
    echo "  1. Перезапустите терминал (или source $shell_rc)"
    echo "  2. Инициализация:  flowlink --init"
    echo "  3. Запуск агента:  flowlink agent start"
    echo ""
    echo "Или всё за раз:"
    echo "  flowlink --init && flowlink agent start"
    echo ""
}

main "$@"

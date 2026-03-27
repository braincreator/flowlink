#Requires -Version 5.1
<#
.SYNOPSIS
    FlowLink Agent — установщик для Windows.
.DESCRIPTION
    Скачивает, устанавливает и настраивает FlowLink Agent как Windows Service.
.PARAMETER Token
    Предустановленный токен агента.
.PARAMETER RelayUrl
    URL реле (default: wss://relay.flowmasters.ru/ws).
.PARAMETER Label
    Имя агента (default: hostname).
.EXAMPLE
    .\install.ps1 -Token "abc123" -RelayUrl "wss://relay.flowmasters.ru/ws"
#>

param(
    [string]$Token = "",
    [string]$RelayUrl = "wss://relay.flowmasters.ru/ws",
    [string]$Label = $env:COMPUTERNAME
)

$ErrorActionPreference = "Stop"

# === Конфигурация ===
$GITHUB_REPO = "braincreator/flowlink"
$BINARY_NAME = "flowlink.exe"
$INSTALL_DIR = "$env:ProgramFiles\FlowLink"
$FLOWLINK_HOME = if ($env:FLOWLINK_HOME) { $env:FLOWLINK_HOME } else { "$env:LOCALAPPDATA\FlowLink" }
$SERVICE_NAME = "FlowLinkAgent"

# === Цвета ===
function Write-Info    { param([string]$Msg) Write-Host "[INFO] $Msg" -ForegroundColor Blue }
function Write-OK      { param([string]$Msg) Write-Host "[OK]   $Msg" -ForegroundColor Green }
function Write-Warn    { param([string]$Msg) Write-Host "[WARN] $Msg" -ForegroundColor Yellow }
function Write-Err     { param([string]$Msg) Write-Host "[ERR]  $Msg" -ForegroundColor Red }

# === Проверка прав администратора ===
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Err "Установщик требует прав администратора. Запустите PowerShell от имени администратора."
    exit 1
}

# === Определение архитектуры ===
if ([Environment]::Is64BitOperatingSystem) {
    $ARCH = "amd64"
} else {
    $ARCH = "386"
}
Write-Info "Архитектура: $ARCH"

# === Создание директорий ===
Write-Info "Создание директорий..."
New-Item -ItemType Directory -Force -Path $INSTALL_DIR | Out-Null
New-Item -ItemType Directory -Force -Path "$FLOWLINK_HOME\backups" | Out-Null
Write-OK "Директории созданы"

# === Генерация agent_id ===
$AGENT_ID = [Guid]::NewGuid().ToString("N")

# === Генерация токена ===
if (-not $Token) {
    $Token = -join ((1..64) | ForEach-Object { "{0:x}" -f (Get-Random -Max 16) })
}

# === Создание конфигурации ===
Write-Info "Создание конфигурации..."

$CONFIG_FILE = "$FLOWLINK_HOME\config.json"

$config = @{
    agent_id       = $AGENT_ID
    token          = $Token
    relay_url      = $RelayUrl
    heartbeat_sec  = 30
    label          = $Label
    work_dir       = ""
    sandbox        = @{
        allowed_dirs      = @()
        blocked_patterns  = @("rm -rf /*", "mkfs*", "dd if=*", "format ", "del /f /s /q C:\", "rd /s /q C:\")
        max_file_size     = 104857600
        max_exec_timeout  = 300
        allow_sudo        = $false
    }
    approval       = @{
        mode                  = "auto"
        soft_ask_notify       = $true
        hard_ask_timeout_sec  = 3600
        max_retries           = 3
    }
    backup         = @{
        max_snapshots   = 50
        max_total_size  = 5368709120
        retention_days  = 7
        backup_dir      = "$FLOWLINK_HOME\backups"
        enabled         = $true
    }
}

$config | ConvertTo-Json -Depth 10 | Set-Content -Path $CONFIG_FILE -Encoding UTF8
Write-OK "Конфигурация создана"

Write-Host ""
Write-Host "════════════════════════════════════════" -ForegroundColor Green
Write-Host "  Agent Credentials" -ForegroundColor Green
Write-Host "════════════════════════════════════════" -ForegroundColor Green
Write-Host "  Agent ID:  " -NoNewline; Write-Host $AGENT_ID -ForegroundColor Yellow
Write-Host "  Token:     " -NoNewline; Write-Host $Token -ForegroundColor Yellow
Write-Host "  Relay:     $RelayUrl"
Write-Host "  Label:     $Label"
Write-Host "════════════════════════════════════════" -ForegroundColor Green
Write-Host ""
Write-Warn "IMPORTANT: Сохраните эти учётные данные!"
Write-Warn "Отправьте Agent ID и Token оператору реле."
Write-Host ""

# === Скачивание бинарника ===
Write-Info "Скачивание flowlink..."

$RELEASE_URL = "https://github.com/$GITHUB_REPO/releases/latest/download/flowlink-windows-$ARCH.exe"
$TEMP_FILE = "$env:TEMP\flowlink-install-$PID.exe"

try {
    # Пробуем скачать из релиза
    Invoke-WebRequest -Uri $RELEASE_URL -OutFile $TEMP_FILE -UseBasicParsing -ErrorAction Stop
} catch {
    Write-Warn "Релиз не найден, пробуем main branch..."
    $TEMP_ZIP = "$env:TEMP\flowlink-install-$PID.zip"
    $ARCHIVE_URL = "https://github.com/$GITHUB_REPO/releases/download/latest/flowlink-windows-$ARCH.zip"
    try {
        Invoke-WebRequest -Uri $ARCHIVE_URL -OutFile $TEMP_ZIP -UseBasicParsing
        Expand-Archive -Path $TEMP_ZIP -DestinationPath "$env:TEMP\flowlink-extract" -Force
        Copy-Item "$env:TEMP\flowlink-extract\flowlink.exe" -Destination $TEMP_FILE -Force
        Remove-Item $TEMP_ZIP -Force
    } catch {
        Write-Err "Не удалось скачать бинарник. Скачайте вручную с: https://github.com/$GITHUB_REPO/releases"
        exit 1
    }
}

if (-not (Test-Path $TEMP_FILE)) {
    Write-Err "Скачанный файл не найден"
    exit 1
}

Write-OK "Бинарник скачан"

# === Установка бинарника ===
Write-Info "Установка бинарника в $INSTALL_DIR..."
Copy-Item $TEMP_FILE "$INSTALL_DIR\$BINARY_NAME" -Force
Remove-Item $TEMP_FILE -Force -ErrorAction SilentlyContinue
Write-OK "Бинарник установлен"

# === Установка как Windows Service ===
Write-Info "Установка Windows Service..."

# Проверяем, существует ли сервис
$existing = Get-Service -Name $SERVICE_NAME -ErrorAction SilentlyContinue
if ($existing) {
    Write-Warn "Сервис уже существует, обновляем..."
    Stop-Service -Name $SERVICE_NAME -Force -ErrorAction SilentlyContinue
    sc.exe delete $SERVICE_NAME | Out-Null
    Start-Sleep -Seconds 2
}

# Создаём сервис через sc.exe
$exePath = "$INSTALL_DIR\$BINARY_NAME"
sc.exe create $SERVICE_NAME binPath= "`"$exePath`" agent start --config `"$CONFIG_FILE`"" start= auto DisplayName= "FlowLink Agent" | Out-Null
sc.exe description $SERVICE_NAME "FlowLink Agent — удалённое управление и автоматизация" | Out-Null

# Устанавливаем переменную окружения для сервиса
[System.Environment]::SetEnvironmentVariable("FLOWLINK_HOME", $FLOWLINK_HOME, "Machine")

Write-OK "Windows Service установлен"

# === Настройка firewall ===
Write-Info "Настройка Windows Firewall..."
try {
    $rule = Get-NetFirewallRule -DisplayName "FlowLink Agent" -ErrorAction SilentlyContinue
    if (-not $rule) {
        New-NetFirewallRule -DisplayName "FlowLink Agent" -Direction Outbound -Action Allow -Program $exePath -Profile Any | Out-Null
        Write-OK "Firewall rule создано (outbound)"
    }
} catch {
    Write-Warn "Не удалось настроить firewall: $_"
}

# === Запуск сервиса ===
Write-Info "Запуск FlowLink Agent..."
Start-Service -Name $SERVICE_NAME
Start-Sleep -Seconds 3

$status = Get-Service -Name $SERVICE_NAME
if ($status.Status -eq "Running") {
    Write-OK "Сервис запущен!"
} else {
    Write-Warn "Сервис не запустился. Проверьте логи: $FLOWLINK_HOME\flowlink.log"
}

# === Итог ===
Write-Host ""
Write-Host "════════════════════════════════════════" -ForegroundColor Green
Write-Host "  Установка FlowLink завершена!" -ForegroundColor Green
Write-Host "════════════════════════════════════════" -ForegroundColor Green
Write-Host ""
Write-Host "Конфигурация: $CONFIG_FILE"
Write-Host "Бинарник:      $INSTALL_DIR\$BINARY_NAME"
Write-Host "Сервис:        $SERVICE_NAME"
Write-Host ""
Write-Host "Команды управления:" -ForegroundColor Cyan
Write-Host "  Start-Service $SERVICE_NAME          # Запуск"
Write-Host "  Stop-Service $SERVICE_NAME           # Остановка"
Write-Host "  Restart-Service $SERVICE_NAME        # Перезапуск"
Write-Host "  Get-Service $SERVICE_NAME            # Статус"
Write-Host "  sc.exe delete $SERVICE_NAME          # Удаление сервиса"
Write-Host ""
Write-Host "Следующие шаги:" -ForegroundColor Yellow
Write-Host "  1. Отправьте Agent ID и Token оператору реле"
Write-Host "  2. Дождитесь подтверждения регистрации"
Write-Host "  3. Агент подключится автоматически"
Write-Host ""

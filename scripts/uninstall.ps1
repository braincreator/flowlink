#Requires -Version 5.1
<#
.SYNOPSIS
    FlowLink Agent — удаление с Windows.
.DESCRIPTION
    Останавливает и удаляет Windows Service, бинарник и конфигурацию FlowLink Agent.
.EXAMPLE
    .\uninstall.ps1
#>

param(
    [switch]$RemoveConfig,
    [switch]$RemoveAll
)

$ErrorActionPreference = "Stop"

$SERVICE_NAME = "FlowLinkAgent"
$INSTALL_DIR = "$env:ProgramFiles\FlowLink"
$FLOWLINK_HOME = if ($env:FLOWLINK_HOME) { $env:FLOWLINK_HOME } else { "$env:LOCALAPPDATA\FlowLink" }

# === Цвета ===
function Write-Info { param([string]$Msg) Write-Host "[INFO] $Msg" -ForegroundColor Blue }
function Write-OK   { param([string]$Msg) Write-Host "[OK]   $Msg" -ForegroundColor Green }
function Write-Warn { param([string]$Msg) Write-Host "[WARN] $Msg" -ForegroundColor Yellow }

# === Проверка прав администратора ===
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Warn "Для полного удаления требуются права администратора."
}

Write-Host ""
Write-Host "════════════════════════════════════════" -ForegroundColor Red
Write-Host "  Удаление FlowLink Agent" -ForegroundColor Red
Write-Host "════════════════════════════════════════" -ForegroundColor Red
Write-Host ""

# === Остановка сервиса ===
$service = Get-Service -Name $SERVICE_NAME -ErrorAction SilentlyContinue
if ($service) {
    Write-Info "Остановка сервиса $SERVICE_NAME..."
    Stop-Service -Name $SERVICE_NAME -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2

    Write-Info "Удаление сервиса $SERVICE_NAME..."
    sc.exe delete $SERVICE_NAME | Out-Null
    Write-OK "Сервис удалён"
} else {
    Write-Info "Сервис $SERVICE_NAME не найден — пропуск"
}

# === Удаление firewall rule ===
try {
    $rule = Get-NetFirewallRule -DisplayName "FlowLink Agent" -ErrorAction SilentlyContinue
    if ($rule) {
        Remove-NetFirewallRule -DisplayName "FlowLink Agent" -ErrorAction SilentlyContinue
        Write-OK "Firewall rule удалено"
    }
} catch {
    # Firewall commands могут быть недоступны
}

# === Удаление переменной окружения ===
try {
    $current = [System.Environment]::GetEnvironmentVariable("FLOWLINK_HOME", "Machine")
    if ($current) {
        [System.Environment]::SetEnvironmentVariable("FLOWLINK_HOME", $null, "Machine")
        Write-OK "Переменная окружения FLOWLINK_HOME удалена"
    }
} catch {}

# === Удаление бинарника ===
if (Test-Path $INSTALL_DIR) {
    Write-Info "Удаление $INSTALL_DIR..."
    Remove-Item -Recurse -Force $INSTALL_DIR
    Write-OK "Бинарник удалён"
}

# === Удаление конфигурации ===
if ($RemoveConfig -or $RemoveAll) {
    if (Test-Path $FLOWLINK_HOME) {
        Write-Warn "Удаление $FLOWLINK_HOME (включая конфигурацию и бэкапы)..."
        Remove-Item -Recurse -Force $FLOWLINK_HOME
        Write-OK "Конфигурация удалена"
    }
} else {
    Write-Info "Конфигурация сохранена: $FLOWLINK_HOME"
    Write-Info "Для полного удаления запустите: .\uninstall.ps1 -RemoveConfig"
}

Write-Host ""
Write-OK "FlowLink Agent удалён!"
Write-Host ""

// Package config — загрузка и хранение конфигурации flowlink.
package config

import (
	"github.com/braincreator/flowlink/internal/protocol"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
)

// Config — конфигурация агента flowlink.
type Config struct {
	// Идентификация
	AgentID string `json:"agent_id"` // уникальный ID этой машины
	Token   string `json:"token"`    // pairwise токен для аутентификации

	// Подключение к реле
	RelayURL string `json:"relay_url"` // wss://relay.example.com/ws

	// Настройки агента
	HeartbeatSec int    `json:"heartbeat_sec"` // интервал пинга (default: 30)
	Label        string `json:"label"`         // человекочитаемое имя (default: hostname)
	WorkDir      string `json:"work_dir"`      // рабочая директория (default: cwd)

	// Sandbox — ограничения прав
	Sandbox SandboxConfig `json:"sandbox"`

	// ReadOnly — агент запускается в read-only режиме (default: true для безопасности)
	ReadOnly *bool `json:"read_only"`

	// Approval — настройки подтверждения (v2 с 3 режимами)
	Approval ApprovalConfigV2 `json:"approval"`

	// Backup — настройки резервного копирования
	Backup BackupConfig `json:"backup"`

	// KillSwitch — настройки защитного переключателя
	KillSwitch KillSwitchConfig `json:"kill_switch"`

	// E2EE — end-to-end encryption (always enabled)
	E2EE E2EEConfig `json:"e2ee"`
	
	// LLM — настройки LLM
	UseRelayLLM bool `json:"use_relay_llm"` // use relay LLM proxy (default: false — use own keys)

	// Payment — настройки платёжной системы
	Payment PaymentConfig `json:"payment"`

	// Autoscale — настройки автоскейлинга relay серверов
	Autoscale AutoscaleConfig `json:"autoscale"`
}

// AutoscaleConfig — настройки autoscaling для relay.
type AutoscaleConfig struct {
	Enabled         bool    `json:"enabled"`          // default: false
	Provider        string  `json:"provider"`         // "timeweb"
	TimewebToken    string  `json:"timeweb_token"`
	MinServers      int     `json:"min_servers"`      // 1
	MaxServers      int     `json:"max_servers"`      // 5
	ScaleUpAt       float64 `json:"scale_up_at"`      // clients per server (10)
	ScaleDownAt     float64 `json:"scale_down_at"`    // clients per server (3)
	CooldownMinutes int     `json:"cooldown_minutes"` // 10
	ServerCPU       int     `json:"server_cpu"`       // 1
	ServerRAM       int     `json:"server_ram"`       // 1024 (MB)
	ServerDisk      int     `json:"server_disk"`      // 10 (GB)
	ServerLocation  string  `json:"server_location"`  // "ru-1"
}

// PaymentConfig — настройки платёжной интеграции.
type PaymentConfig struct {
	Provider        string `json:"provider"`            // "tochka", "manual"
	TochkaClientID  string `json:"tochka_client_id"`
	TochkaSecret    string `json:"tochka_secret"`
	TochkaAccountID string `json:"tochka_account_id"`   // счёт/БИК
	WebhookURL      string `json:"webhook_url"`
	WebhookSecret   string `json:"webhook_secret"`
}

// SandboxConfig — ограничения для команд и файлов.
type SandboxConfig struct {
	// AllowedDirs — директории к которым есть доступ (пусто = весь диск)
	AllowedDirs []string `json:"allowed_dirs"`
	// BlockedPatterns — glob-паттерны заблокированных команд
	BlockedPatterns []string `json:"blocked_patterns"`
	// MaxFileSize — макс размер файла для записи/чтения в байтах (0 = unlimited)
	MaxFileSize int64 `json:"max_file_size"`
	// MaxExecTimeout — макс таймаут команды в секундах (0 = 300)
	MaxExecTimeout int `json:"max_exec_timeout"`
	// AllowSudo — разрешить sudo (default: false)
	AllowSudo bool `json:"allow_sudo"`
}

// ApprovalConfig — старая конфигурация (deprecated, используйте ApprovalConfigV2).
type ApprovalConfig struct {
	Mode                string   `json:"mode"`
	DangerousPatterns   []string `json:"dangerous_patterns"`
	AutoApprovePatterns []string `json:"auto_approve_patterns"`
}

// ApprovalConfigV2 — настройки подтверждения команд (3 режима).
type ApprovalConfigV2 struct {
	// Mode: "auto" | "soft_ask" | "hard_ask"
	// auto — безопасные команды выполняются сразу
	// soft_ask — средний риск → уведомление + выполнение
	// hard_ask — высокий риск → ждёт подтверждения
	Mode string `json:"mode"`
	// SoftAskNotify — отправлять уведомление при soft_ask (default: true)
	SoftAskNotify bool `json:"soft_ask_notify"`
	// HardAskTimeout — таймаут ожидания подтверждения в секундах (default: 3600)
	HardAskTimeout int `json:"hard_ask_timeout_sec"`
	// MaxRetries — максимум повторных запросов при hard_ask (default: 3)
	MaxRetries int `json:"max_retries"`
}

// KillSwitchConfig — настройки защитного переключателя (kill switch).
type KillSwitchConfig struct {
	DiskThreshold    float64 `json:"disk_threshold"`    // порог диска для readonly (default: 90%)
	CPUThreshold     float64 `json:"cpu_threshold"`     // порог CPU для паузы (default: 95%)
	CPUThresholdDur  int    `json:"cpu_threshold_sec"` // длительность превышения CPU (default: 300s)
}

// DefaultKillSwitchConfig — дефолтные настройки.
func DefaultKillSwitchConfig() KillSwitchConfig {
	return KillSwitchConfig{
		DiskThreshold:   90.0,
		CPUThreshold:    95.0,
		CPUThresholdDur: 300,
	}
}

// BackupConfig — настройки резервного копирования.
type BackupConfig struct {
	// MaxSnapshots — максимальное количество снапшотов (default: 50)
	MaxSnapshots int `json:"max_snapshots"`
	// MaxTotalSize — максимальный общий размер бэкапов в байтах (default: 5GB)
	MaxTotalSize int64 `json:"max_total_size"`
	// RetentionDays — срок хранения в днях (default: 7)
	RetentionDays int `json:"retention_days"`
	// BackupDir — директория для хранения бэкапов (default: ~/.flowlink/backups)
	BackupDir string `json:"backup_dir"`
	// Enabled — включено ли авто-резервное копирование (default: true)
	Enabled bool `json:"enabled"`
	// ScheduleInterval — интервал периодических бэкапов, e.g. "6h", "12h", "24h" (default: "" — disabled)
	ScheduleInterval string `json:"schedule_interval"`
}

// RelayConfig — конфигурация реле-сервера.
type RelayConfig struct {
	// Слушатели
	WSSAddr string `json:"wss_addr"` // ":8443" — WSS для агентов
	APIAddr string `json:"api_addr"` // ":8080" — HTTP API для OpenClaw

	// TLS
	TLSMode   string `json:"tls_mode"`   // "self-signed", "letsencrypt", "manual"
	TLSCert   string `json:"tls_cert"`   // путь к сертификату (для manual/self-signed)
	TLSKey    string `json:"tls_key"`    // путь к ключу (для manual/self-signed)
	TLSDomain string `json:"tls_domain"` // домен для Let's Encrypt
	TLSCache  string `json:"tls_cache"`  // кэш директория для Let's Encrypt (/var/lib/flowlink/tls-cache)

	// Auth
	APIToken string `json:"api_token"` // токен для HTTP API

	// Агенты
	AllowedTokens map[string]string `json:"allowed_tokens"` // token → agent_id whitelist

	// LLM Proxy — бэкенды для проксирования LLM запросов
	LLMBackends []LLMBackendConfig `json:"llm_backends"`

	// Настройки
	HeartbeatTimeout int `json:"heartbeat_timeout_sec"` // таймаут пинга (default: 90)
	MaxAgents        int `json:"max_agents"`           // макс кол-во агентов (0 = unlimited)

	// Telegram Bot — настройки бота для управления через Telegram
	TelegramBot *TelegramBotConfig `json:"telegram_bot,omitempty"`

	// Audit — настройки audit логов
	AuditHMACSecret string `json:"audit_hmac_secret"` // путь к файлу с HMAC ключом (default: ~/.flowlink/audit.key)

	// Rate Limit — лимиты запросов
	RateLimitPerMin  int `json:"rate_limit_per_min"`  // запросов в минуту (default: 30)
	RateLimitPerHour int `json:"rate_limit_per_hour"` // запросов в час (default: 200)

	// Integration Service — проксирование к Python интеграции (billing, S3, etc.)
	IntegrationURL   string `json:"integration_url,omitempty"`   // e.g. "http://localhost:9082"
	IntegrationToken string `json:"integration_token,omitempty"` // shared secret for relay→integration auth

	// CORS — allowed origins for cross-origin requests
	CORSOrigins []string `json:"cors_origins"` // default: ["*"]

	// Backup — настройки резервного копирования для dashboard
	Backup BackupConfig `json:"backup"`

	// E2EE is always enabled — no toggle needed in relay config.
}

// TelegramBotConfig — конфигурация Telegram-бота.
type TelegramBotConfig struct {
	Token      string   `json:"token"`       // Telegram Bot Token
	AllowedIDs []int64  `json:"allowed_ids"` // Telegram user IDs (ограничение доступа)
	NotifyOn   []string `json:"notify_on"`   // ["exec", "backup", "error", "approval"]
}

// LLMBackendConfig — конфигурация LLM backend для реле.
type LLMBackendConfig struct {
	Name     string `json:"name"`
	URL      string `json:"url"`
	APIKey   string `json:"api_key,omitempty"`
	Priority int    `json:"priority"` // 1 = высший
	Provider string `json:"provider"` // "openai_compatible", "groq", "ollama"
}

// DefaultConfig — конфигурация по умолчанию для агента.
func DefaultConfig() Config {
	home, _ := os.UserHomeDir()
	
	return Config{
		RelayURL:     "wss://relay.flowmasters.ru/ws",
		HeartbeatSec: 30,
		WorkDir:      "",
		Sandbox: SandboxConfig{
			MaxFileSize:    100 * 1024 * 1024, // 100MB
			MaxExecTimeout: 300,               // 5 минут
			AllowSudo:      false,
			BlockedPatterns: []string{
				"rm -rf /*",
				"mkfs*",
				"dd if=*",
				":(){ :|:& };:", // fork bomb
			},
		},
		Approval: ApprovalConfigV2{
			Mode:           "auto", // по умолчанию auto для удобства
			SoftAskNotify:  true,
			HardAskTimeout: 3600, // 1 час
			MaxRetries:     3,
		},
		Backup: BackupConfig{
			MaxSnapshots:  50,
			MaxTotalSize:  5 * 1024 * 1024 * 1024, // 5GB
			RetentionDays: 7,
			BackupDir:     filepath.Join(home, ".flowlink", "backups"),
			Enabled:       true,
		},
		ReadOnly: boolPtr(true), // безопасность: новый агент в read-only
	}
}

func boolPtr(v bool) *bool { return &v }

// DefaultRelayConfig — конфигурация по умолчанию для реле.
func DefaultRelayConfig() RelayConfig {
	home, _ := os.UserHomeDir()
	return RelayConfig{
		WSSAddr:          ":8443",
		APIAddr:          ":8080",
		HeartbeatTimeout: 90,
		MaxAgents:        100,
		CORSOrigins:      []string{"*"},
		Backup: BackupConfig{
			MaxSnapshots:  50,
			MaxTotalSize:  5 * 1024 * 1024 * 1024, // 5GB
			RetentionDays: 7,
			BackupDir:     filepath.Join(home, ".flowlink", "backups"),
			Enabled:       true,
		},
	}
}

// ConfigDir — директория конфигурации flowlink.
// Поддерживает переменную окружения FLOWLINK_CONFIG_DIR.
func ConfigDir() (string, error) {
	if dir := os.Getenv("FLOWLINK_CONFIG_DIR"); dir != "" {
		return dir, os.MkdirAll(dir, 0700)
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	p := filepath.Join(home, ".flowlink")
	return p, os.MkdirAll(p, 0700)
}

// ConfigPath — путь к файлу конфигурации.
func ConfigPath() (string, error) {
	dir, err := ConfigDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(dir, "config.json"), nil
}

// LoadConfig — загружает конфигурацию из файла.
func LoadConfig() (*Config, error) {
	path, err := ConfigPath()
	if err != nil {
		return nil, err
	}

	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			// Конфиг не существует — создаём дефолтный
			cfg := DefaultConfig()
			return &cfg, nil
		}
		return nil, protocol.ErrCause(protocol.CodeConfigLoadError, err)
	}

	var cfg Config
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, protocol.ErrCause(protocol.CodeConfigParseError, err)
	}

	// Fill defaults
	if cfg.HeartbeatSec == 0 {
		cfg.HeartbeatSec = 30
	}
	if cfg.Label == "" {
		cfg.Label, _ = os.Hostname()
	}

	return &cfg, nil
}

// SaveConfig — сохраняет конфигурацию в файл.
func SaveConfig(cfg *Config) error {
	path, err := ConfigPath()
	if err != nil {
		return err
	}

	data, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return protocol.ErrCause(protocol.CodeConfigSaveError, err)
	}

	return os.WriteFile(path, data, 0600)
}

// LoadRelayConfig — загружает конфигурацию реле.
func LoadRelayConfig(path string) (*RelayConfig, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, protocol.ErrCause(protocol.CodeConfigLoadError, err)
	}

	var cfg RelayConfig
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, protocol.ErrCause(protocol.CodeConfigParseError, err)
	}

	// Fill defaults
	if cfg.HeartbeatTimeout == 0 {
		cfg.HeartbeatTimeout = 90
	}

	return &cfg, nil
}

// E2EEConfig — настройки end-to-end шифрования.
type E2EEConfig struct {
	// Enabled — always true (E2EE cannot be disabled)
	Enabled    bool `json:"enabled"` // always true
	AutoRotate bool `json:"auto_rotate"`  // авто-ротация каждые 30 дней
}

// OSInfo — базовая информация об ОС.
func OSInfo() (osName, arch string) {
	osName = runtime.GOOS
	arch = runtime.GOARCH
	return
}

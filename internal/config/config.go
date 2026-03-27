// Package config — загрузка и хранение конфигурации flowlink.
package config

import (
	"encoding/json"
	"fmt"
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

	// LLM — настройки для автономного агента (L2)
	LLM LLMConfig `json:"llm"`

	// Task — настройки автономных задач
	Task TaskConfig `json:"task"`

	// Sandbox — ограничения прав
	Sandbox SandboxConfig `json:"sandbox"`

	// Approval — настройки подтверждения
	Approval ApprovalConfig `json:"approval"`
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

// ApprovalConfig — настройки апруваль (подтверждения) команд.
type ApprovalConfig struct {
	// Mode: "auto" | "ask" | "deny"
	// auto — всё выполняется автоматически
	// ask — спрашивать для опасных команд
	// deny — ничего не выполнять без явного разрешения
	Mode string `json:"mode"`
	// DangerousPatterns — команды, которые всегда требуют подтверждения
	DangerousPatterns []string `json:"dangerous_patterns"`
	// AutoApprovePatterns — команды, которые выполняются без подтверждения
	AutoApprovePatterns []string `json:"auto_approve_patterns"`
}

// RelayConfig — конфигурация реле-сервера.
type RelayConfig struct {
	// Слушатели
	WSSAddr string `json:"wss_addr"` // ":8443" — WSS для агентов
	APIAddr string `json:"api_addr"` // ":8080" — HTTP API для OpenClaw

	// TLS
	TLSCert string `json:"tls_cert"` // путь к сертификату (пусто = автогенерация)
	TLSKey  string `json:"tls_key"`  // путь к ключу

	// Auth
	APIToken string `json:"api_token"` // токен для HTTP API

	// Агенты
	AllowedTokens map[string]string `json:"allowed_tokens"` // token → agent_id whitelist

	// Настройки
	HeartbeatTimeout int `json:"heartbeat_timeout_sec"` // таймаут пинга (default: 90)
	MaxAgents        int `json:"max_agents"`           // макс кол-во агентов (0 = unlimited)
}

// DefaultConfig — конфигурация по умолчанию для агента.
func DefaultConfig() Config {
	return Config{
		RelayURL:     "wss://relay.flowmasters.ru/ws",
		HeartbeatSec: 30,
		WorkDir:      "",
		LLM:          DefaultLLMConfig(),
		Task:         DefaultTaskConfig(),
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
		Approval: ApprovalConfig{
			Mode: "ask",
			DangerousPatterns: []string{
				"rm *", "rm -r*", "rmdir*",
				"sudo*",
				"shutdown*", "reboot*", "halt*", "poweroff*",
				"chmod 777*", "chown*",
				"curl*|*sh", "wget*|*sh", // pipe to shell
				"mkfs*", "fdisk*", "parted*",
				"crontab*",
				"systemctl*",
				"iptables*", "ufw*",
			},
			AutoApprovePatterns: []string{
				"ls*", "cat*", "head*", "tail*", "wc*",
				"pwd", "whoami", "hostname", "uname*",
				"df*", "du*", "free*", "top*", "ps*",
				"docker ps*", "docker images*",
				"echo*", "date", "uptime",
				"git status*", "git log*",
			},
		},
	}
}

// DefaultRelayConfig — конфигурация по умолчанию для реле.
func DefaultRelayConfig() RelayConfig {
	return RelayConfig{
		WSSAddr:          ":8443",
		APIAddr:          ":8080",
		HeartbeatTimeout: 90,
		MaxAgents:        100,
	}
}

// ConfigDir — директория конфигурации flowlink.
func ConfigDir() (string, error) {
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
		return nil, fmt.Errorf("чтение конфига: %w", err)
	}

	var cfg Config
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, fmt.Errorf("парсинг конфига: %w", err)
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
		return fmt.Errorf("сериализация конфига: %w", err)
	}

	return os.WriteFile(path, data, 0600)
}

// LoadRelayConfig — загружает конфигурацию реле.
func LoadRelayConfig(path string) (*RelayConfig, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("чтение конфига реле: %w", err)
	}

	var cfg RelayConfig
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, fmt.Errorf("парсинг конфига реле: %w", err)
	}

	// Fill defaults
	if cfg.HeartbeatTimeout == 0 {
		cfg.HeartbeatTimeout = 90
	}

	return &cfg, nil
}

// OSInfo — базовая информация об ОС.
func OSInfo() (osName, arch string) {
	osName = runtime.GOOS
	arch = runtime.GOARCH
	return
}

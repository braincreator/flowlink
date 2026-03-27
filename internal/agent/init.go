// Package agent — FlowLink Remote Execution Agent
package agent

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"

	"github.com/braincreator/flowlink/internal/config"
)

// InitConfig — создаёт конфигурацию агента с генерацией credentials.
func InitConfig(relayURL, label, providedToken, approvalMode string) (*config.Config, error) {
	// Create config directory
	home, err := os.UserHomeDir()
	if err != nil {
		return nil, fmt.Errorf("getting home directory: %w", err)
	}

	flowlinkDir := filepath.Join(home, ".flowlink")
	if err := os.MkdirAll(flowlinkDir, 0700); err != nil {
		return nil, fmt.Errorf("creating config directory: %w", err)
	}

	// Create backups directory
	backupsDir := filepath.Join(flowlinkDir, "backups")
	if err := os.MkdirAll(backupsDir, 0700); err != nil {
		return nil, fmt.Errorf("creating backups directory: %w", err)
	}

	// Start with defaults
	cfg := config.DefaultConfig()

	// Generate agent_id (UUID v4-like, 32 hex chars)
	agentID, err := generateAgentID()
	if err != nil {
		return nil, fmt.Errorf("generating agent_id: %w", err)
	}
	cfg.AgentID = agentID

	// Generate or use provided token
	if providedToken != "" {
		cfg.Token = providedToken
	} else {
		token, err := generateToken()
		if err != nil {
			return nil, fmt.Errorf("generating token: %w", err)
		}
		cfg.Token = token
	}

	// Set user-provided values
	if relayURL != "" {
		cfg.RelayURL = relayURL
	}

	if label != "" {
		cfg.Label = label
	} else {
		// Default to hostname
		hostname, _ := os.Hostname()
		cfg.Label = hostname
	}

	if approvalMode != "" {
		cfg.Approval.Mode = approvalMode
	}

	// Set backup directory
	cfg.Backup.BackupDir = backupsDir

	// Save configuration
	if err := config.SaveConfig(&cfg); err != nil {
		return nil, fmt.Errorf("saving config: %w", err)
	}

	return &cfg, nil
}

// generateAgentID — создаёт уникальный ID агента (32 hex chars).
func generateAgentID() (string, error) {
	bytes := make([]byte, 16)
	if _, err := rand.Read(bytes); err != nil {
		return "", err
	}
	return hex.EncodeToString(bytes), nil
}

// generateToken — создаёт токен авторизации (64 hex chars = 32 bytes).
func generateToken() (string, error) {
	bytes := make([]byte, 32)
	if _, err := rand.Read(bytes); err != nil {
		return "", err
	}
	return hex.EncodeToString(bytes), nil
}

// IsInitialized — проверяет, инициализирован ли агент.
func IsInitialized() bool {
	home, err := os.UserHomeDir()
	if err != nil {
		return false
	}

	configPath := filepath.Join(home, ".flowlink", "config.json")
	if _, err := os.Stat(configPath); os.IsNotExist(err) {
		return false
	}

	cfg, err := config.LoadConfig()
	if err != nil {
		return false
	}

	return cfg.AgentID != "" && cfg.Token != ""
}

// GetConfigPath — возвращает путь к конфигурации.
func GetConfigPath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, ".flowlink", "config.json"), nil
}

// EnsureDirectories — создаёт все необходимые директории.
func EnsureDirectories() error {
	home, err := os.UserHomeDir()
	if err != nil {
		return err
	}

	dirs := []string{
		filepath.Join(home, ".flowlink"),
		filepath.Join(home, ".flowlink", "backups"),
		filepath.Join(home, ".flowlink", "logs"),
	}

	for _, dir := range dirs {
		if err := os.MkdirAll(dir, 0700); err != nil {
			return fmt.Errorf("creating directory %s: %w", dir, err)
		}
	}

	return nil
}

// WritePIDFile — записывает PID текущего процесса.
func WritePIDFile() error {
	home, err := os.UserHomeDir()
	if err != nil {
		return err
	}

	pidFile := filepath.Join(home, ".flowlink", "flowlink.pid")
	pid := os.Getpid()

	return os.WriteFile(pidFile, []byte(fmt.Sprintf("%d", pid)), 0644)
}

// RemovePIDFile — удаляет PID файл.
func RemovePIDFile() error {
	home, err := os.UserHomeDir()
	if err != nil {
		return err
	}

	pidFile := filepath.Join(home, ".flowlink", "flowlink.pid")
	return os.Remove(pidFile)
}

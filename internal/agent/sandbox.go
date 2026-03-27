package agent

import (
	"fmt"
	"log/slog"
	"strings"

	"github.com/braincreator/flowlink/internal/config"
)

// Sandbox — проверяет разрешения для команд и файлов.
type Sandbox struct {
	cfg *config.SandboxConfig
}

// NewSandbox — создаёт новый sandbox.
func NewSandbox(cfg *config.SandboxConfig) *Sandbox {
	return &Sandbox{cfg: cfg}
}

// AllowCommand — проверяет, разрешена ли команда.
func (s *Sandbox) AllowCommand(command string) bool {
	if command == "" {
		return false
	}

	// Проверка sudo
	if !s.cfg.AllowSudo && containsSudo(command) {
		slog.Warn("команда заблокирована: sudo не разрешён", "command", command)
		return false
	}

	// Проверка заблокированных паттернов
	for _, pattern := range s.cfg.BlockedPatterns {
		if matchGlob(command, pattern) {
			slog.Warn("команда заблокирована: совпадение с паттерном",
				"command", command, "pattern", pattern)
			return false
		}
	}

	return true
}

// AllowFilePath — проверяет, разрешён ли доступ к пути файла.
func (s *Sandbox) AllowFilePath(path string) bool {
	if len(s.cfg.AllowedDirs) == 0 {
		return true // нет ограничений
	}

	for _, dir := range s.cfg.AllowedDirs {
		if strings.HasPrefix(path, dir) {
			return true
		}
	}

	return false
}

// CheckFileSize — проверяет размер файла.
func (s *Sandbox) CheckFileSize(size int64) bool {
	if s.cfg.MaxFileSize == 0 {
		return true
	}
	return size <= s.cfg.MaxFileSize
}

// CheckTimeout — проверяет таймаут команды.
func (s *Sandbox) CheckTimeout(timeout int) int {
	if timeout == 0 {
		return s.cfg.MaxExecTimeout
	}
	if s.cfg.MaxExecTimeout > 0 && timeout > s.cfg.MaxExecTimeout {
		return s.cfg.MaxExecTimeout
	}
	return timeout
}

// containsSudo — проверяет наличие sudo в команде.
func containsSudo(command string) bool {
	trimmed := strings.TrimSpace(command)
	return strings.HasPrefix(trimmed, "sudo ") ||
		strings.HasPrefix(trimmed, "sudo\t") ||
		trimmed == "sudo"
}

// matchGlob — простой glob-матчинг для паттернов sandbox.
// Поддерживает * (any) в конце и начале паттерна.
func matchGlob(command, pattern string) bool {
	cmd := strings.TrimSpace(command)
	pat := strings.TrimSpace(pattern)

	if pat == "" {
		return false
	}

	// Паттерн начинается с *
	if strings.HasPrefix(pat, "*") {
		suffix := pat[1:]
		return strings.HasSuffix(cmd, suffix)
	}

	// Паттерн заканчивается на *
	if strings.HasSuffix(pat, "*") {
		prefix := pat[:len(pat)-1]
		return strings.HasPrefix(cmd, prefix)
	}

	// Паттерн содержит * в середине
	if idx := strings.Index(pat, "*"); idx >= 0 {
		prefix := pat[:idx]
		suffix := pat[idx+1:]
		return strings.HasPrefix(cmd, prefix) && strings.HasSuffix(cmd, suffix)
	}

	// Точное совпадение
	return cmd == pat
}

// ExecSafe — безопасное выполнение команды с интеграцией KillSwitch и Backup.
func (e *Executor) ExecSafe(cmd string, backup *BackupEngine, ks *KillSwitch) (output string, err error) {
	// Проверка Kill Switch
	if err := ks.CheckCommand(cmd); err != nil {
		return "", fmt.Errorf("kill switch: %w", err)
	}

	// Проверка на деструктивность и создание бэкапа
	if IsDestructive(cmd) {
		affectedPaths := DetectAffectedPaths(cmd)
		if len(affectedPaths) > 0 {
			snapshotID, backupErr := backup.CreateBefore(affectedPaths, cmd)
			if backupErr != nil {
				// Логируем ошибку, но продолжаем (backup не должен блокировать выполнение)
				slog.Warn("ошибка создания бэкапа", "err", backupErr, "paths", affectedPaths)
			} else {
				slog.Info("бэкап создан", "snapshot_id", snapshotID, "command", cmd)
			}
		}
	}

	// Существующая логика sandbox
	return e.Exec(cmd)
}

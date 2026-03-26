package agent

import (
	"fmt"
	"log/slog"
	"os"
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

// Approver — обрабатывает подтверждения (approval) для опасных команд.
type Approver struct {
	cfg *config.ApprovalConfig
}

// NewApprover — создаёт новый approver.
func NewApprover(cfg *config.ApprovalConfig) *Approver {
	return &Approver{cfg: cfg}
}

// NeedsApproval — определяет, нужна ли команда подтверждения.
func (a *Approver) NeedsApproval(command string) bool {
	switch a.cfg.Mode {
	case "auto":
		return false
	case "deny":
		return true
	case "ask":
		// Проверяем auto-approve паттерны
		for _, pattern := range a.cfg.AutoApprovePatterns {
			if matchGlob(command, pattern) {
				return false
			}
		}
		// Проверяем dangerous паттерны
		for _, pattern := range a.cfg.DangerousPatterns {
			if matchGlob(command, pattern) {
				return true
			}
		}
		// По умолчанию — не спрашиваем
		return false
	default:
		return false
	}
}

// AssessRisk — оценивает уровень риска команды.
func (a *Approver) AssessRisk(command string) string {
	highRiskPatterns := []string{
		"rm -rf", "sudo", "shutdown", "reboot", "mkfs",
		"chmod 777", "dd if=", "curl*|*sh", "wget*|*sh",
	}

	for _, pattern := range highRiskPatterns {
		if matchGlob(command, pattern) {
			return "high"
		}
	}

	mediumRiskPatterns := []string{
		"rm ", "rmdir", "chmod", "chown",
		"crontab", "systemctl", "iptables", "ufw",
	}

	for _, pattern := range mediumRiskPatterns {
		if matchGlob(command, pattern) {
			return "medium"
		}
	}

	return "low"
}

// AskTTY — спрашивает подтверждение в терминале.
// Возвращает true если пользователь разрешил.
func (a *Approver) AskTTY(command string) bool {
	fmt.Fprintf(os.Stderr, "\n⚠️  FlowLink: запрос на выполнение команды\n")
	fmt.Fprintf(os.Stderr, "   Команда: %s\n", command)
	fmt.Fprintf(os.Stderr, "   Риск: %s\n\n", a.AssessRisk(command))
	fmt.Fprintf(os.Stderr, "   Выполнить? [y/N]: ")

	var response string
	fmt.Scanln(&response)

	return strings.ToLower(strings.TrimSpace(response)) == "y"
}

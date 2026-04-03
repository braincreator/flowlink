// Package agent — Policy Layer для flowlink.
// Единая точка проверки всех команд перед выполнением.
// Команда проходит через цепочку проверок: KillSwitch → Read-only → Blacklist → Sandbox → Approval → Backup → Execute
package agent

import (
	"fmt"
	"log/slog"
	"strings"
	"time"

	"github.com/braincreator/flowlink/internal/config"
)

// PolicyResult — результат проверки команды через Policy Layer.
type PolicyResult struct {
	Allowed   bool   `json:"allowed"`
	Blocked   bool   `json:"blocked"`
	Reason    string `json:"reason"`
	RiskLevel string `json:"risk_level"` // "none", "low", "medium", "high"
	// Мета-информация
	SnapshotID   string `json:"snapshot_id,omitempty"`   // ID созданного бэкапа
	ApprovalID   string `json:"approval_id,omitempty"`   // ID запроса на подтверждение
	RequireApproval bool  `json:"require_approval"`       // Требуется подтверждение
}

// ExtendedBlacklist — расширенный чёрный список команд.
// Категории: system_destroy, data_destroy, service_disrupt, security_bypass
var ExtendedBlacklist = []struct {
	Pattern     string
	Category    string
	Description string
}{
	// === System Destroy ===
	{`rm -rf /`, "system_destroy", "Recursive removal of root filesystem"},
	{`rm -rf /*`, "system_destroy", "Recursive removal of root filesystem (wildcard)"},
	{`rm -rf .`, "system_destroy", "Recursive removal of current directory"},
	{`mkfs`, "system_destroy", "Format filesystem"},
	{`dd if=/dev/zero`, "system_destroy", "Zero out disk"},
	{`dd if=/dev/random`, "system_destroy", "Write random data to disk"},
	{`>:*>/dev/sd`, "system_destroy", "Write to block device"},
	{`chmod -R 777 /`, "system_destroy", "Make entire filesystem world-writable"},
	{`:(){ :|:& };:`, "system_destroy", "Fork bomb"},
	{`shutdown`, "system_destroy", "System shutdown"},
	{`reboot`, "system_destroy", "System reboot"},
	{`halt`, "system_destroy", "System halt"},
	{`poweroff`, "system_destroy", "Power off system"},
	{`init 0`, "system_destroy", "Switch to runlevel 0 (shutdown)"},
	{`init 6`, "system_destroy", "Switch to runlevel 6 (reboot)"},

	// === Data Destroy ===
	{`DROP DATABASE`, "data_destroy", "SQL: Drop entire database"},
	{`DROP TABLE`, "data_destroy", "SQL: Drop table"},
	{`TRUNCATE`, "data_destroy", "SQL: Truncate table"},
	{`DELETE FROM`, "data_destroy", "SQL: Delete all rows"},
	{`rm -rf /var`, "data_destroy", "Remove system data directory"},
	{`rm -rf /etc`, "data_destroy", "Remove system configuration"},
	{`rm -rf /home`, "data_destroy", "Remove user home directories"},
	{`rm -rf /usr`, "data_destroy", "Remove system programs"},
	{`rm -rf /opt`, "data_destroy", "Remove optional packages"},
	{`rm -rf /srv`, "data_destroy", "Remove service data"},
	{`git reset --hard`, "data_destroy", "Git: hard reset (data loss)"},
	{`git clean -fd`, "data_destroy", "Git: clean untracked files"},

	// === Service Disrupt ===
	{`systemctl stop`, "service_disrupt", "Stop systemd service"},
	{`systemctl disable`, "service_disrupt", "Disable systemd service"},
	{`systemctl mask`, "service_disrupt", "Mask systemd service"},
	{`iptables -F`, "service_disrupt", "Flush firewall rules"},
	{`iptables -X`, "service_disrupt", "Delete firewall chains"},
	{`ufw disable`, "service_disrupt", "Disable UFW firewall"},
	{`crontab -r`, "service_disrupt", "Remove all cron jobs"},
	{`docker rm`, "service_disrupt", "Remove Docker container"},
	{`docker rmi`, "service_disrupt", "Remove Docker image"},
	{`docker system prune`, "service_disrupt", "Prune Docker system"},
	{`docker volume rm`, "service_disrupt", "Remove Docker volume"},

	// === Security Bypass ===
	{`chmod 777`, "security_bypass", "Make files world-writable"},
	{`chown root`, "security_bypass", "Change ownership to root"},
	{`userdel`, "security_bypass", "Delete user account"},
	{`passwd `, "security_bypass", "Change user password"},
	{`visudo`, "security_bypass", "Edit sudoers file"},
	{`echo.*>>/etc/sudoers`, "security_bypass", "Modify sudoers file"},
	{`curl | bash`, "security_bypass", "Pipe remote script to bash"},
	{`curl | sh`, "security_bypass", "Pipe remote script to sh"},
	{`wget | bash`, "security_bypass", "Pipe remote script to bash"},
	{`wget | sh`, "security_bypass", "Pipe remote script to sh"},

	// === Package Management (medium risk) ===
	{`apt remove`, "service_disrupt", "Remove Debian/Ubuntu package"},
	{`apt-get remove`, "service_disrupt", "Remove Debian/Ubuntu package"},
	{`apt purge`, "service_disrupt", "Purge Debian/Ubuntu package"},
	{`apt-get purge`, "service_disrupt", "Purge Debian/Ubuntu package"},
	{`yum remove`, "service_disrupt", "Remove RHEL/CentOS package"},
	{`dnf remove`, "service_disrupt", "Remove Fedora/RHEL package"},
}

// WriteOnlyPatterns — паттерны, которые являются write-операциями.
var WriteOnlyPatterns = []string{
	"rm ", "rmdir", "unlink",
	"mv ", "cp ", "touch ",
	"mkdir", "mkfile",
	">", ">>",
	"tee ",
	"curl -o", "wget -O",
	"chmod ", "chown ",
	"git commit", "git push",
	"npm install", "pip install", "pip3 install",
	"cargo install", "go install",
	"docker run", "docker build",
	"systemctl start", "systemctl restart",
	"crontab",
}

// PolicyLayer — единая точка проверки команд.
type PolicyLayer struct {
	sandbox    *Sandbox
	approval   *ApproverV2
	backup     *BackupEngine
	killSwitch *KillSwitch
	cfg        *config.Config
	logger     *slog.Logger

	// Read-only mode (включается по умолчанию)
	readOnly bool
}

// NewPolicyLayer — создаёт новый Policy Layer.
func NewPolicyLayer(
	sandbox *Sandbox,
	approval *ApproverV2,
	backup *BackupEngine,
	killSwitch *KillSwitch,
	cfg *config.Config,
) *PolicyLayer {
	return &PolicyLayer{
		sandbox:    sandbox,
		approval:   approval,
		backup:     backup,
		killSwitch: killSwitch,
		cfg:        cfg,
		logger:     slog.Default(),
		readOnly:   true, // По умолчанию read-only
	}
}

// SetReadOnly — устанавливает режим read-only.
func (p *PolicyLayer) SetReadOnly(enabled bool) {
	p.readOnly = enabled
}

// IsReadOnly — возвращает текущий режим.
func (p *PolicyLayer) IsReadOnly() bool {
	return p.readOnly
}

// Check — полная проверка команды через все слои.
// Возвращает PolicyResult с решением.
func (p *PolicyLayer) Check(command string) *PolicyResult {
	result := &PolicyResult{
		Allowed:    true,
		Blocked:    false,
		RiskLevel:  "none",
		RequireApproval: false,
	}

	// ─── Layer 1: Kill Switch ───
	if err := p.killSwitch.CheckCommand(command); err != nil {
		result.Allowed = false
		result.Blocked = true
		result.Reason = fmt.Sprintf("kill switch: %s", err)
		return result
	}

	// ─── Layer 2: Read-only check ───
	if p.readOnly {
		if p.isWriteOperation(command) {
			result.Allowed = false
			result.Blocked = true
			result.Reason = "agent is in read-only mode; write operations are blocked"
			return result
		}
	}

	// ─── Layer 3: Extended Blacklist ───
	if category := p.checkBlacklist(command); category != "" {
		result.Allowed = false
		result.Blocked = true
		result.Reason = fmt.Sprintf("command blocked by policy (category: %s): %s", category, command)
		result.RiskLevel = "high"
		return result
	}

	// ─── Layer 4: Sandbox ───
	if !p.sandbox.AllowCommand(command) {
		result.Allowed = false
		result.Blocked = true
		result.Reason = "command blocked by sandbox policy"
		return result
	}

	// ─── Layer 5: Risk Classification ───
	risk := p.approval.ClassifyRisk(command)
	result.RiskLevel = risk

	// ─── Layer 6: Approval ───
	decision, approvalID, err := p.approval.CheckApproval(command)
	if err != nil {
		result.Allowed = false
		result.Blocked = true
		result.Reason = fmt.Sprintf("approval error: %v", err)
		return result
	}

	switch decision {
	case DecisionRejected:
		result.Allowed = false
		result.Blocked = true
		result.Reason = "command rejected by approval policy"
		return result
	case DecisionTimedOut:
		result.Allowed = false
		result.Blocked = true
		result.Reason = "approval timeout: no confirmation received"
		return result
	case DecisionPending:
		result.Allowed = false
		result.Blocked = false // Не заблокировано, но не выполнено
		result.Reason = "pending approval"
		result.RequireApproval = true
		result.ApprovalID = approvalID
		return result
	}

	// ─── Layer 7: Auto-backup for destructive commands ───
	if IsDestructive(command) && p.backup != nil {
		affectedPaths := DetectAffectedPaths(command)
		if len(affectedPaths) > 0 {
			snapshotID, err := p.backup.CreateBefore(affectedPaths, command)
			if err != nil {
				p.logger.Warn("policy: backup failed (continuing anyway)",
					"err", err, "command", command)
			} else {
				result.SnapshotID = snapshotID
				p.logger.Info("policy: auto-backup created",
					"snapshot_id", snapshotID,
					"command", command,
					"paths", len(affectedPaths),
				)
			}
		}
	}

	// Все проверки пройдены
	result.Allowed = true
	return result
}

// Approve — подтверждает pending команду.
func (p *PolicyLayer) Approve(approvalID string) {
	p.approval.Approve(approvalID)
}

// Reject — отклоняет pending команду.
func (p *PolicyLayer) Reject(approvalID string) {
	p.approval.Reject(approvalID)
}

// GetPendingApprovals — возвращает список ожидающих подтверждения.
func (p *PolicyLayer) GetPendingApprovals() []*ApprovalRequest {
	return p.approval.GetPending()
}

// checkBlacklist — проверяет команду против расширенного чёрного списка.
func (p *PolicyLayer) checkBlacklist(command string) string {
	cmd := strings.ToLower(strings.TrimSpace(command))

	for _, entry := range ExtendedBlacklist {
		pattern := strings.ToLower(entry.Pattern)
		// Use regex matching for patterns with special chars, simple contains otherwise
		if strings.ContainsAny(pattern, "*?") {
			if matchGlob(cmd, pattern) {
				p.logger.Warn("policy: command blocked by blacklist",
					"command", command,
					"pattern", entry.Pattern,
					"category", entry.Category,
				)
			return entry.Category
			}
		} else {
			if strings.Contains(cmd, pattern) {
				p.logger.Warn("policy: command blocked by blacklist",
					"command", command,
					"pattern", entry.Pattern,
					"category", entry.Category,
				)
				return entry.Category
			}
		}
	}

	return ""
}

// isWriteOperation — определяет, является ли команда write-операцией.
func (p *PolicyLayer) isWriteOperation(command string) bool {
	cmd := strings.ToLower(strings.TrimSpace(command))

	// Read-only команды (всегда разрешены в read-only режиме)
	readOnlyCommands := []string{
		"ls ", "ll ", "dir ",
		"cat ", "head ", "tail ", "less ", "more ",
		"grep ", "find ", "locate ", "which ",
		"ps ", "top ", "htop ",
		"df ", "du ", "free ",
		"uname ", "hostname ", "whoami ", "id ",
		"date ", "uptime ",
		"ip ", "ifconfig ", "ping ", "curl ", "wget ",
		"git status", "git log", "git diff", "git branch",
		"docker ps", "docker images",
		"systemctl status", "systemctl list",
		"env ", "printenv ",
	}

	for _, ro := range readOnlyCommands {
		if strings.HasPrefix(cmd, ro) {
			return false
		}
	}

	// Check write patterns
	for _, pattern := range WriteOnlyPatterns {
		if strings.Contains(cmd, pattern) {
			return true
		}
	}

	return false
}

// AuditCommand — логирует результат проверки команды для аудита.
func (p *PolicyLayer) AuditCommand(command string, result *PolicyResult) {
	// Формируем audit record
	record := map[string]any{
		"timestamp":    time.Now().Unix(),
		"command":      command,
		"allowed":      result.Allowed,
		"blocked":      result.Blocked,
		"reason":       result.Reason,
		"risk_level":   result.RiskLevel,
		"snapshot_id":  result.SnapshotID,
		"approval_id":  result.ApprovalID,
		"read_only":    p.readOnly,
	}

	p.logger.Info("policy audit",
		"command", command,
		"allowed", result.Allowed,
		"risk", result.RiskLevel,
		"reason", result.Reason,
	)

	// TODO: отправить audit record в relay для хранения
	_ = record
}

// GetStatus — возвращает текущий статус Policy Layer.
func (p *PolicyLayer) GetStatus() map[string]any {
	return map[string]any{
		"read_only":           p.readOnly,
		"kill_switch_mode":    p.killSwitch.Mode(),
		"approval_mode":       p.approval.Mode(),
		"pending_approvals":   len(p.GetPendingApprovals()),
		"blacklist_entries":   len(ExtendedBlacklist),
	}
}

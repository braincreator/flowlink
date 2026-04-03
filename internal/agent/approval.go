// Package agent — Approval Modes для flowlink.
// Три режима подтверждения команд: auto, soft_ask, hard_ask.
package agent

import (
	cryptorand "crypto/rand"
	"encoding/hex"
	"fmt"
	"log/slog"
	"os"
	"regexp"
	"strings"
	"sync"
	"time"

	"github.com/braincreator/flowlink/internal/config"
)

// ApprovalMode — режим подтверждения команд.
type ApprovalMode string

const (
	ApprovalAuto    ApprovalMode = "auto"     // безопасные команды выполняются сразу
	ApprovalSoftAsk ApprovalMode = "soft_ask" // средний риск → уведомление + выполнение
	ApprovalHardAsk ApprovalMode = "hard_ask" // высокий риск → ждёт подтверждения
)

// DefaultApprovalConfigV2 — конфигурация по умолчанию.
func DefaultApprovalConfigV2() config.ApprovalConfigV2 {
	return config.ApprovalConfigV2{
		Mode:           "auto",
		SoftAskNotify:  true,
		HardAskTimeout: 3600, // 1 час
		MaxRetries:     3,
	}
}

// ApprovalDecision — решение по команде.
type ApprovalDecision string

const (
	DecisionApproved ApprovalDecision = "approved"
	DecisionRejected ApprovalDecision = "rejected"
	DecisionPending  ApprovalDecision = "pending"
	DecisionTimedOut ApprovalDecision = "timed_out"
)

// ApprovalRequest — запрос на подтверждение.
type ApprovalRequest struct {
	ID          string           `json:"id"`
	Command     string           `json:"command"`
	Risk        string           `json:"risk"` // "low", "medium", "high"
	Mode        ApprovalMode     `json:"mode"`
	RequestedAt time.Time        `json:"requested_at"`
	Decision    ApprovalDecision `json:"decision"`
	DecidedAt   *time.Time       `json:"decided_at,omitempty"`
	Reason      string           `json:"reason,omitempty"`
}

// ApproverV2 — новый approver с 3 режимами.
type ApproverV2 struct {
	cfg          config.ApprovalConfigV2
	pending      map[string]*ApprovalRequest
	mu           sync.RWMutex
	logger       *slog.Logger
	notifyFn     func(req *ApprovalRequest)
	approveChan  chan string // канал для получения ответов approve
	rejectChan   chan string // канал для получения ответов reject
}

// NewApproverV2 — создаёт новый approver v2.
func NewApproverV2(cfg config.ApprovalConfigV2) *ApproverV2 {
	if cfg.HardAskTimeout == 0 {
		cfg = DefaultApprovalConfigV2()
	}

	return &ApproverV2{
		cfg:         cfg,
		pending:     make(map[string]*ApprovalRequest),
		logger:      slog.Default(),
		approveChan: make(chan string, 100),
		rejectChan:  make(chan string, 100),
	}
}

// ClassifyRisk — классифицирует риск команды.
func (a *ApproverV2) ClassifyRisk(command string) string {
	cmd := strings.ToLower(strings.TrimSpace(command))

	// Высокий риск — деструктивные команды
	highRiskPatterns := []*regexp.Regexp{
		regexp.MustCompile(`^rm\s+(-[rf]+\s+)*[^-]`), // rm file, rm -rf dir
		regexp.MustCompile(`^sudo\s+rm\s+`),
		regexp.MustCompile(`DROP\s+(DATABASE|TABLE)`),
		regexp.MustCompile(`TRUNCATE\s+`),
		regexp.MustCompile(`apt(-get)?\s+(remove|purge)`),
		regexp.MustCompile(`yum\s+remove`),
		regexp.MustCompile(`dnf\s+remove`),
		regexp.MustCompile(`docker\s+(rm|rmi)\s+`),
		regexp.MustCompile(`systemctl\s+(stop|disable|mask)\s+`),
		regexp.MustCompile(`iptables\s+-[FX]`),
		regexp.MustCompile(`chmod\s+777\s+`),
		regexp.MustCompile(`crontab\s+-r`),
		regexp.MustCompile(`userdel\s+`),
		regexp.MustCompile(`useradd\s+`),
		regexp.MustCompile(`shutdown|reboot|halt|poweroff`),
		regexp.MustCompile(`mkfs`),
		regexp.MustCompile(`dd\s+if=`),
		regexp.MustCompile(`>\s*/dev/sd`), // запись в блочное устройство
	}

	for _, pattern := range highRiskPatterns {
		if pattern.MatchString(cmd) {
			return "high"
		}
	}

	// Средний риск — потенциально опасные команды
	mediumRiskPatterns := []*regexp.Regexp{
		regexp.MustCompile(`^apt(-get)?\s+upgrade`),
		regexp.MustCompile(`^apt(-get)?\s+dist-upgrade`),
		regexp.MustCompile(`^docker\s+pull\s+`),
		regexp.MustCompile(`^docker\s+run\s+`),
		regexp.MustCompile(`^systemctl\s+restart\s+`),
		regexp.MustCompile(`^systemctl\s+start\s+`),
		regexp.MustCompile(`^npm\s+install`),
		regexp.MustCompile(`^npm\s+update`),
		regexp.MustCompile(`^pip\s+install`),
		regexp.MustCompile(`^pip3\s+install`),
		regexp.MustCompile(`^gem\s+install`),
		regexp.MustCompile(`^cargo\s+install`),
		regexp.MustCompile(`^go\s+install`),
		regexp.MustCompile(`^snap\s+install`),
		regexp.MustCompile(`^flatpak\s+install`),
		regexp.MustCompile(`^brew\s+install`),
		regexp.MustCompile(`^git\s+reset\s+--hard`),
		regexp.MustCompile(`^git\s+clean\s+-[fd]`),
		regexp.MustCompile(`^chmod\s+`),
		regexp.MustCompile(`^chown\s+`),
		regexp.MustCompile(`^mv\s+`),
		regexp.MustCompile(`^cp\s+-r\s+`),
	}

	for _, pattern := range mediumRiskPatterns {
		if pattern.MatchString(cmd) {
			return "medium"
		}
	}

	return "low"
}

// CheckApproval — проверяет, нужно ли подтверждение для команды.
// Возвращает (decision, requestID, error).
func (a *ApproverV2) CheckApproval(command string) (ApprovalDecision, string, error) {
	risk := a.ClassifyRisk(command)
	mode := ApprovalMode(a.cfg.Mode)

	// Low risk — всегда auto
	if risk == "low" {
		return DecisionApproved, "", nil
	}

	// High risk → hard_ask
	if risk == "high" {
		return a.processHardAsk(command, risk)
	}

	// Medium risk → soft_ask (notify + execute) или hard_ask
	if risk == "medium" {
		switch mode {
		case ApprovalAuto:
			// В auto режиме medium тоже auto
			return DecisionApproved, "", nil
		case ApprovalSoftAsk:
			// Уведомление + выполнение
			a.notifySoftAsk(command, risk)
			return DecisionApproved, "", nil
		case ApprovalHardAsk:
			// Medium в hard_ask режиме тоже требует подтверждения
			return a.processHardAsk(command, risk)
		}
	}

	return DecisionApproved, "", nil
}

// processHardAsk — обрабатывает hard_ask запрос.
func (a *ApproverV2) processHardAsk(command, risk string) (ApprovalDecision, string, error) {
	requestID := generateRequestID()

	req := &ApprovalRequest{
		ID:          requestID,
		Command:     command,
		Risk:        risk,
		Mode:        ApprovalHardAsk,
		RequestedAt: time.Now(),
		Decision:    DecisionPending,
	}

	// Сохраняем в pending
	a.mu.Lock()
	a.pending[requestID] = req
	a.mu.Unlock()

	// Отправляем уведомление через реле
	a.notifyHardAsk(req)

	// Ждём ответа с таймаутом
	decision := a.waitForDecision(requestID, a.cfg.HardAskTimeout)

	return decision, requestID, nil
}

// waitForDecision — ждёт решения по запросу с таймаутом.
func (a *ApproverV2) waitForDecision(requestID string, timeoutSec int) ApprovalDecision {
	timeout := time.Duration(timeoutSec) * time.Second
	deadline := time.Now().Add(timeout)

	// Retry loop с backoff
	retries := 0
	backoff := 1 * time.Second

	for time.Now().Before(deadline) && retries < a.cfg.MaxRetries {
		select {
		case approvedID := <-a.approveChan:
			if approvedID == requestID {
				a.updateDecision(requestID, DecisionApproved)
				return DecisionApproved
			}
		case rejectedID := <-a.rejectChan:
			if rejectedID == requestID {
				a.updateDecision(requestID, DecisionRejected)
				return DecisionRejected
			}
		default:
			// Ничего не пришло — ждём
		}

		time.Sleep(backoff)
		backoff = backoff * 2
		if backoff > 10*time.Second {
			backoff = 10 * time.Second
		}
		retries++
	}

	// Таймаут
	a.updateDecision(requestID, DecisionTimedOut)
	return DecisionTimedOut
}

// Approve — одобряет запрос.
func (a *ApproverV2) Approve(requestID string) {
	select {
	case a.approveChan <- requestID:
	default:
		a.logger.Warn("approve channel full", "request_id", requestID)
	}
}

// Reject — отклоняет запрос.
func (a *ApproverV2) Reject(requestID string) {
	select {
	case a.rejectChan <- requestID:
	default:
		a.logger.Warn("reject channel full", "request_id", requestID)
	}
}

// updateDecision — обновляет решение в pending запросе.
func (a *ApproverV2) updateDecision(requestID string, decision ApprovalDecision) {
	a.mu.Lock()
	defer a.mu.Unlock()

	if req, exists := a.pending[requestID]; exists {
		req.Decision = decision
		now := time.Now()
		req.DecidedAt = &now
	}
}

// notifySoftAsk — отправляет уведомление для soft_ask.
func (a *ApproverV2) notifySoftAsk(command, risk string) {
	if !a.cfg.SoftAskNotify {
		return
	}

	req := &ApprovalRequest{
		Command:     command,
		Risk:        risk,
		Mode:        ApprovalSoftAsk,
		RequestedAt: time.Now(),
		Decision:    DecisionApproved, // Уже одобрено, но уведомляем
	}

	if a.notifyFn != nil {
		go a.notifyFn(req)
	}

	a.logger.Info("soft_ask: команда выполнена с уведомлением",
		"command", command,
		"risk", risk,
	)
}

// notifyHardAsk — отправляет запрос на подтверждение для hard_ask.
func (a *ApproverV2) notifyHardAsk(req *ApprovalRequest) {
	if a.notifyFn != nil {
		go a.notifyFn(req)
	}

	a.logger.Info("hard_ask: ожидание подтверждения",
		"request_id", req.ID,
		"command", req.Command,
		"risk", req.Risk,
	)
}

// SetNotifyFn — устанавливает функцию уведомлений.
func (a *ApproverV2) SetNotifyFn(fn func(req *ApprovalRequest)) {
	a.notifyFn = fn
}

// Mode — возвращает текущий режим approval.
func (a *ApproverV2) Mode() string {
	return a.cfg.Mode
}

// GetPending — возвращает список pending запросов.
func (a *ApproverV2) GetPending() []*ApprovalRequest {
	a.mu.RLock()
	defer a.mu.RUnlock()

	var pending []*ApprovalRequest
	for _, req := range a.pending {
		if req.Decision == DecisionPending {
			pending = append(pending, req)
		}
	}
	return pending
}

// AskTTY — спрашивает подтверждение в терминале (fallback).
func (a *ApproverV2) AskTTY(command string) bool {
	risk := a.ClassifyRisk(command)

	fmt.Fprintf(os.Stderr, "\n⚠️  FlowLink: запрос на выполнение команды\n")
	fmt.Fprintf(os.Stderr, "   Команда: %s\n", command)
	fmt.Fprintf(os.Stderr, "   Риск: %s\n", risk)
	fmt.Fprintf(os.Stderr, "   Режим: %s\n\n", a.cfg.Mode)
	fmt.Fprintf(os.Stderr, "   Выполнить? [y/N]: ")

	var response string
	fmt.Scanln(&response)

	return strings.ToLower(strings.TrimSpace(response)) == "y"
}

// generateRequestID — генерирует уникальный ID запроса.
func generateRequestID() string {
	b := make([]byte, 4)
	cryptorand.Read(b)
	return fmt.Sprintf("req_%d_%s", time.Now().UnixNano(), hex.EncodeToString(b))
}

// Файл usage.go — учёт использования ресурсов клиентами.
// JSONL persistence, thread-safe.
package billing

import (
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// UsageRecord — запись об использовании ресурсов за месяц.
type UsageRecord struct {
	ID       string `json:"id"`
	ClientID string `json:"client_id"`
	Month    string `json:"month"`   // "2026-03"
	Commands int    `json:"commands"`
	Backups  int    `json:"backups"`
	Storage  int64  `json:"storage"` // текущий размер бэкапов
	Agents   int    `json:"agents"`
}

// Resource — тип ресурса для проверки лимита.
type Resource string

const (
	ResourceCommands Resource = "commands"
	ResourceBackups  Resource = "backups"
	ResourceStorage  Resource = "storage"
	ResourceAgents   Resource = "agents"
)

// LimitCheck — результат проверки лимита.
type LimitCheck struct {
	CanProceed bool   `json:"can_proceed"` // можно ли продолжать
	Remaining  int    `json:"remaining"`   // оставшееся количество (-1 = безлимит)
	Limit      int    `json:"limit"`       // текущий лимит
	Used       int    `json:"used"`        // текущее использование
	Message    string `json:"message"`     // описание
}

// UsageTracker — трекер использования (thread-safe).
type UsageTracker struct {
	mu       sync.Mutex
	records  map[string]*UsageRecord // "clientID:month" → record
	dataDir  string
	logger   *slog.Logger
	planStore *PlanStore
}

// NewUsageTracker — создаёт трекер, загружает данные из JSONL.
func NewUsageTracker(dataDir string, planStore *PlanStore, logger *slog.Logger) *UsageTracker {
	if logger == nil {
		logger = slog.Default()
	}
	os.MkdirAll(dataDir, 0700)

	ut := &UsageTracker{
		records:   make(map[string]*UsageRecord),
		dataDir:   dataDir,
		logger:    logger,
		planStore: planStore,
	}
	ut.load()
	return ut
}

// currentMonth — возвращает текущий месяц в формате "2026-03".
func currentMonth() string {
	return time.Now().Format("2006-01")
}

// recordKey — ключ для карты.
func recordKey(clientID, month string) string {
	return clientID + ":" + month
}

// getOrCreate — получает или создаёт запись использования.
func (ut *UsageTracker) getOrCreate(clientID, month string) *UsageRecord {
	key := recordKey(clientID, month)
	r, ok := ut.records[key]
	if !ok {
		r = &UsageRecord{
			ID:       clientID + ":" + month,
			ClientID: clientID,
			Month:    month,
		}
		ut.records[key] = r
	}
	return r
}

// RecordCommand — увеличивает счётчик команд на 1.
func (ut *UsageTracker) RecordCommand(clientID string) {
	ut.mu.Lock()
	defer ut.mu.Unlock()

	r := ut.getOrCreate(clientID, currentMonth())
	r.Commands++
	ut.persist(r)
}

// RecordAgent — устанавливает количество агентов.
func (ut *UsageTracker) RecordAgent(clientID string, count int) {
	ut.mu.Lock()
	defer ut.mu.Unlock()

	r := ut.getOrCreate(clientID, currentMonth())
	r.Agents = count
	ut.persist(r)
}

// UpdateStorage — обновляет размер хранилища бэкапов.
func (ut *UsageTracker) UpdateStorage(clientID string, size int64) {
	ut.mu.Lock()
	defer ut.mu.Unlock()

	r := ut.getOrCreate(clientID, currentMonth())
	r.Storage = size
	ut.persist(r)
}

// IncrementBackups — увеличивает счётчик бэкапов.
func (ut *UsageTracker) IncrementBackups(clientID string) {
	ut.mu.Lock()
	defer ut.mu.Unlock()

	r := ut.getOrCreate(clientID, currentMonth())
	r.Backups++
	ut.persist(r)
}

// GetUsage — возвращает текущее использование для клиента и месяца.
func (ut *UsageTracker) GetUsage(clientID, month string) *UsageRecord {
	ut.mu.Lock()
	defer ut.mu.Unlock()

	r, ok := ut.records[recordKey(clientID, month)]
	if !ok {
		return &UsageRecord{
			ID:       clientID + ":" + month,
			ClientID: clientID,
			Month:    month,
		}
	}
	// Возвращаем копию
	copy := *r
	return &copy
}

// CheckLimit — проверяет лимит по ресурсу для клиента.
func (ut *UsageTracker) CheckLimit(clientID string, resource Resource, planID string) LimitCheck {
	plan, ok := ut.planStore.GetPlan(planID)
	if !ok {
		return LimitCheck{
			CanProceed: false,
			Message:    fmt.Sprintf("план %s не найден", planID),
		}
	}

	usage := ut.GetUsage(clientID, currentMonth())
	var used, limit int

	switch resource {
	case ResourceCommands:
		used = usage.Commands
		limit = plan.MaxCommands
	case ResourceBackups:
		used = usage.Backups
		limit = plan.MaxBackups
	case ResourceAgents:
		used = usage.Agents
		limit = plan.MaxAgents
	case ResourceStorage:
		// Storage в байтах — используем int64
		if plan.MaxStorage == -1 {
			return LimitCheck{CanProceed: true, Remaining: -1, Limit: -1, Used: int(usage.Storage), Message: "безлимит"}
		}
		return LimitCheck{
			CanProceed: usage.Storage <= plan.MaxStorage,
			Remaining:  int(plan.MaxStorage - usage.Storage),
			Limit:      int(plan.MaxStorage),
			Used:       int(usage.Storage),
			Message:    "storage",
		}
	}

	// -1 = безлимит
	if limit == -1 {
		return LimitCheck{CanProceed: true, Remaining: -1, Limit: -1, Used: used, Message: "безлимит"}
	}

	remaining := limit - used
	canProceed := used < limit

	msg := fmt.Sprintf("%s: %d/%d", resource, used, limit)
	if !canProceed {
		msg = fmt.Sprintf("лимит превышен — %s: %d/%d", resource, used, limit)
	}

	return LimitCheck{
		CanProceed: canProceed,
		Remaining:  remaining,
		Limit:      limit,
		Used:       used,
		Message:    msg,
	}
}

// ResetMonthly — сбрасывает месячные счётчики команд и бэкапов для клиента.
func (ut *UsageTracker) ResetMonthly(clientID string) {
	ut.mu.Lock()
	defer ut.mu.Unlock()

	month := currentMonth()
	key := recordKey(clientID, month)
	r, ok := ut.records[key]
	if !ok {
		return
	}
	r.Commands = 0
	r.Backups = 0
	ut.persist(r)
	ut.logger.Info("месячные счётчики сброшены", "client", clientID, "month", month)
}

// === Persistence ===

// persist — дописывает запись в JSONL.
func (ut *UsageTracker) persist(r *UsageRecord) {
	path := filepath.Join(ut.dataDir, "usage.jsonl")
	f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0600)
	if err != nil {
		ut.logger.Error("ошибка сохранения usage", "err", err)
		return
	}
	defer f.Close()

	data, _ := jsonMarshal(r)
	f.Write(append(data, '\n'))
}

// load — загружает записи из JSONL.
func (ut *UsageTracker) load() {
	path := filepath.Join(ut.dataDir, "usage.jsonl")
	data, err := os.ReadFile(path)
	if err != nil {
		return // файла нет — нормально
	}

	for _, line := range splitLines(data) {
		if len(line) == 0 {
			continue
		}
		var r UsageRecord
		if jsonUnmarshal(line, &r) == nil && r.ClientID != "" {
			ut.records[recordKey(r.ClientID, r.Month)] = &r
		}
	}
}

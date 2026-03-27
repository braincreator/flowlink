// Package billing — тарифные планы, учёт использования, счета и оплата.
// Thread-safe, persistence в JSONL.
package billing

import (
	"sync"
)

// Размерности для удобства
const (
	KB = 1024
	MB = 1024 * KB
	GB = 1024 * MB
)

// Plan — тарифный план.
type Plan struct {
	ID           string   `json:"id"`
	Name         string   `json:"name"`
	MaxAgents    int      `json:"max_agents"`
	MaxCommands  int      `json:"max_commands"`   // в месяц (-1 = безлимит)
	MaxBackups   int      `json:"max_backups"`    // хранить одновременно (-1 = безлимит)
	MaxStorage   int64    `json:"max_storage"`    // байт для бэкапов (-1 = безлимит)
	PriceMonthly float64  `json:"price_monthly"`  // RUB
	Features     []string `json:"features"`       // ["telegram_bot", "audit", "mcp", "api"]
}

// HasFeature — проверяет наличие фичи в плане.
func (p Plan) HasFeature(feature string) bool {
	for _, f := range p.Features {
		if f == "all" || f == feature {
			return true
		}
	}
	return false
}

// предустановленные планы
var predefinedPlans = map[string]Plan{
	"free": {
		ID: "free", Name: "Бесплатный",
		MaxAgents: 1, MaxCommands: 100, MaxBackups: 3, MaxStorage: 100 * MB,
		PriceMonthly: 0,
		Features:     []string{"basic_exec"},
	},
	"starter": {
		ID: "starter", Name: "Стартовый",
		MaxAgents: 3, MaxCommands: 1000, MaxBackups: 10, MaxStorage: 1 * GB,
		PriceMonthly: 990,
		Features:     []string{"telegram_bot", "audit"},
	},
	"business": {
		ID: "business", Name: "Бизнес",
		MaxAgents: 25, MaxCommands: 10000, MaxBackups: 50, MaxStorage: 10 * GB,
		PriceMonthly: 4990,
		Features:     []string{"telegram_bot", "audit", "mcp", "api"},
	},
	"enterprise": {
		ID: "enterprise", Name: "Корпоративный",
		MaxAgents: 100, MaxCommands: -1, MaxBackups: -1, MaxStorage: 100 * GB,
		PriceMonthly: 19990,
		Features:     []string{"all"},
	},
}

// PlanStore — хранилище планов (thread-safe).
type PlanStore struct {
	mu    sync.RWMutex
	plans map[string]Plan // planID → Plan
}

// NewPlanStore — создаёт хранилище с предустановленными планами.
func NewPlanStore() *PlanStore {
	ps := &PlanStore{
		plans: make(map[string]Plan),
	}
	for id, p := range predefinedPlans {
		ps.plans[id] = p
	}
	return ps
}

// GetPlan — возвращает план по ID.
func (ps *PlanStore) GetPlan(id string) (Plan, bool) {
	ps.mu.RLock()
	defer ps.mu.RUnlock()
	p, ok := ps.plans[id]
	return p, ok
}

// ListPlans — возвращает все доступные планы.
func (ps *PlanStore) ListPlans() []Plan {
	ps.mu.RLock()
	defer ps.mu.RUnlock()
	result := make([]Plan, 0, len(ps.plans))
	for _, p := range ps.plans {
		result = append(result, p)
	}
	return result
}

// SetPlan — устанавливает/обновляет план (для кастомных планов).
func (ps *PlanStore) SetPlan(p Plan) {
	ps.mu.Lock()
	defer ps.mu.Unlock()
	ps.plans[p.ID] = p
}

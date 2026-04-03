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

// BillingPeriod — период подписки.
type BillingPeriod string

const (
	PeriodMonthly  BillingPeriod = "monthly"
	PeriodQuarterly BillingPeriod = "quarterly"
	PeriodYearly    BillingPeriod = "yearly"
)

// PeriodDiscount — скидки за длинные подписки (маркетинговые).
var PeriodDiscount = map[BillingPeriod]float64{
	PeriodMonthly:  1.0,   // 0%
	PeriodQuarterly: 0.85,  // 15% off
	PeriodYearly:    0.70,  // 30% off
}

// PeriodMonths — длительность в месяцах.
var PeriodMonths = map[BillingPeriod]int{
	PeriodMonthly:  1,
	PeriodQuarterly: 3,
	PeriodYearly:    12,
}

// PlanPrice — цена плана с учётом периода.
type PlanPrice struct {
	Period      BillingPeriod `json:"period"`
	MonthlyEquiv float64       `json:"monthly_equiv"` // цена в пересчёте на месяц
	Total       float64       `json:"total"`        // итоговая сумма за период
	Savings     float64       `json:"savings"`       // экономия vs monthly (USD)
	SavingsPct  string        `json:"savings_pct"`   // "15%", "30%"
}

// Plan — тарифный план.
type Plan struct {
	ID           string   `json:"id"`
	Name         string   `json:"name"`
	MaxAgents    int      `json:"max_agents"`
	MaxCommands  int      `json:"max_commands"`   // в месяц (-1 = безлимит)
	MaxBackups   int      `json:"max_backups"`    // хранить одновременно (-1 = безлимит)
	MaxStorage   int64    `json:"max_storage"`    // байт для бэкапов (-1 = безлимит)
	PriceMonthly float64  `json:"price_monthly"`  // USD (базовая цена за месяц)
	Features     []string `json:"features"`       // ["telegram_bot", "audit", "mcp", "api"]
}

// GetPrices — возвращает цены для всех периодов.
func (p Plan) GetPrices() []PlanPrice {
	prices := make([]PlanPrice, 0, 3)
	for _, period := range []BillingPeriod{PeriodMonthly, PeriodQuarterly, PeriodYearly} {
		discount := PeriodDiscount[period]
		months := PeriodMonths[period]
		monthlyEquiv := p.PriceMonthly * discount
		total := monthlyEquiv * float64(months)
		savings := p.PriceMonthly*float64(months) - total
		var savingsPct string
		switch period {
		case PeriodMonthly:
			savingsPct = ""
		case PeriodQuarterly:
			savingsPct = "15%"
		case PeriodYearly:
			savingsPct = "30%"
		}

		prices = append(prices, PlanPrice{
			Period:      period,
			MonthlyEquiv: monthlyEquiv,
			Total:       total,
			Savings:     savings,
			SavingsPct:  savingsPct,
		})
	}
	return prices
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
		ID: "free", Name: "Free",
		MaxAgents: 3, MaxCommands: 500, MaxBackups: 5, MaxStorage: 500 * MB,
		PriceMonthly: 0,
		Features:     []string{"basic_exec", "cli", "telegram_bot", "sandbox", "kill_switch"},
	},
	"starter": {
		ID: "starter", Name: "Cloud Starter",
		MaxAgents: 10, MaxCommands: 5000, MaxBackups: 20, MaxStorage: 5 * GB,
		PriceMonthly: 19,
		Features:     []string{"managed_relay", "auto_updates", "backups", "llm_proxy", "mcp", "email_support", "analytics"},
	},
	"pro": {
		ID: "pro", Name: "Cloud Pro",
		MaxAgents: 50, MaxCommands: -1, MaxBackups: -1, MaxStorage: 50 * GB,
		PriceMonthly: 49,
		Features:     []string{"priority_relay", "dedicated_ip", "sso", "team_management", "advanced_audit", "priority_support", "custom_integrations"},
	},
	"enterprise": {
		ID: "enterprise", Name: "Enterprise",
		MaxAgents: -1, MaxCommands: -1, MaxBackups: -1, MaxStorage: -1,
		PriceMonthly: 0, // custom pricing
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

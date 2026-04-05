// Package billing — подписки: создание, продление, отмена.
package billing

import (
	"github.com/braincreator/flowlink/internal/protocol"
	"encoding/json"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// SubscriptionStore — хранилище подписок (thread-safe).
type SubscriptionStore struct {
	mu            sync.Mutex
	subscriptions map[string]*Subscription     // subscriptionID → Subscription
	customerIdx   map[string][]string          // customerID → []subscriptionID
	dataDir       string
	logger        *slog.Logger
	planStore     *PlanStore
	gateway       PaymentGateway
}

// NewSubscriptionStore — создаёт хранилище подписок.
func NewSubscriptionStore(dataDir string, planStore *PlanStore, gateway PaymentGateway, logger *slog.Logger) *SubscriptionStore {
	if logger == nil {
		logger = slog.Default()
	}
	os.MkdirAll(dataDir, 0700)

	ss := &SubscriptionStore{
		subscriptions: make(map[string]*Subscription),
		customerIdx:   make(map[string][]string),
		dataDir:       dataDir,
		logger:        logger,
		planStore:     planStore,
		gateway:       gateway,
	}
	ss.load()
	return ss
}

// CreateSubscription — создаёт подписку после первого успешного платежа.
func (ss *SubscriptionStore) CreateSubscription(customerID, customerEmail, planID string, period BillingPeriod, paymentMethodID, firstPaymentID string) (*Subscription, error) {
	ss.mu.Lock()
	defer ss.mu.Unlock()

	// Проверяем план
	_, ok := ss.planStore.GetPlan(planID)
	if !ok {
		return nil, protocol.Err(protocol.CodeClientNotFound, planID)
	}

	// Проверяем, нет ли уже активной подписки
	for _, subID := range ss.customerIdx[customerID] {
		if sub, ok := ss.subscriptions[subID]; ok && sub.Status == SubscriptionStatusActive {
			return nil, protocol.Err(protocol.CodeSkillAlreadyExists, customerID, subID)
		}
	}

	now := time.Now()
	billingDay := now.Day()
	if billingDay > 28 {
		billingDay = 28 // Защита от месяцев с <31 днём
	}

	// Рассчитываем следующую дату списания
	nextBilling := ss.calculateNextBilling(now, period, billingDay)

	sub := &Subscription{
		ID:              fmt.Sprintf("sub_%s_%d", customerID, now.Unix()),
		CustomerID:      customerID,
		CustomerEmail:   customerEmail,
		PlanID:          planID,
		Period:          period,
		PaymentMethodID: paymentMethodID,
		Status:          SubscriptionStatusPending,
		NextBillingDate: nextBilling,
		StartedAt:       now,
		LastPaymentID:   firstPaymentID,
		BillingDay:      billingDay,
	}

	// Активируем если первый платёж уже прошёл
	if firstPaymentID != "" {
		sub.Status = SubscriptionStatusActive
	}

	ss.subscriptions[sub.ID] = sub
	ss.customerIdx[customerID] = appendIdx(ss.customerIdx[customerID], sub.ID)
	ss.persist(sub)

	ss.logger.Info("subscription created", "id", sub.ID, "customer", customerID, "plan", planID, "period", period, "next_billing", nextBilling.Format("2006-01-02"))

	return sub, nil
}

// RenewSubscription — продлевает подписку (recurring платёж).
func (ss *SubscriptionStore) RenewSubscription(subscriptionID string) (*Subscription, error) {
	ss.mu.Lock()
	defer ss.mu.Unlock()

	sub, ok := ss.subscriptions[subscriptionID]
	if !ok {
		return nil, protocol.Err(protocol.CodeClientNotFound, subscriptionID)
	}

	if sub.Status != SubscriptionStatusActive {
		return nil, protocol.Err(protocol.CodeClientDeactivated, subscriptionID)
	}

	// Получаем план
	plan, ok := ss.planStore.GetPlan(sub.PlanID)
	if !ok {
		return nil, fmt.Errorf("план %s не найден", sub.PlanID)
	}

	// Рассчитываем сумму для периода
	prices := plan.GetPrices()
	_ = prices // used for billing period lookup
	for _, p := range prices {
		if p.Period == sub.Period {
			_ = p.Total // amount for acquiring
			break
		}
	}

	// Создаём счёт
	_ = NewInvoiceStore(ss.dataDir, ss.planStore, ss.logger)
	_, err := NewInvoiceStore(ss.dataDir, ss.planStore, ss.logger).GenerateInvoice(sub.CustomerID, sub.PlanID)
	if err != nil {
		return nil, protocol.ErrCause(protocol.CodeConfigFailed, err)
	}

	// Создаём recurring платёж через gateway
	if tc, ok := ss.gateway.(*TochkaClient); ok {
		session, err := tc.CreateRecurringPayment(sub.CustomerEmail, sub.PaymentMethodID, sub.PlanID, sub.Period)
		if err != nil {
			return nil, protocol.ErrCause(protocol.CodeInternalError, err)
		}

		sub.LastPaymentID = session.PaymentID
		sub.NextBillingDate = ss.calculateNextBilling(time.Now(), sub.Period, sub.BillingDay)
		ss.persist(sub)

		ss.logger.Info("subscription renewed", "id", sub.ID, "payment", session.PaymentID, "next_billing", sub.NextBillingDate.Format("2006-01-02"))

		return sub, nil
	}

	return nil, fmt.Errorf("gateway не поддерживает recurring платежи")
}

// CancelSubscription — отменяет подписку (с возвратом если нужно).
func (ss *SubscriptionStore) CancelSubscription(subscriptionID string, refund bool) error {
	ss.mu.Lock()
	defer ss.mu.Unlock()

	sub, ok := ss.subscriptions[subscriptionID]
	if !ok {
		return protocol.Err(protocol.CodeClientNotFound, subscriptionID)
	}

	now := time.Now()
	sub.Status = SubscriptionStatusCancelled
	sub.CancelledAt = &now

	// Если нужен возврат последнего платежа
	if refund && sub.LastPaymentID != "" {
		plan, ok := ss.planStore.GetPlan(sub.PlanID)
		if ok {
			// Возвращаем сумму последнего периода
			prices := plan.GetPrices()
			var amount float64
			for _, p := range prices {
				if p.Period == sub.Period {
					amount = p.Total
					break
				}
			}
			if amount > 0 {
				rubAmount := USDtoRUB(amount)
				if err := ss.gateway.Refund(sub.LastPaymentID, rubAmount); err != nil {
					ss.logger.Error("refund error", "err", err, "payment", sub.LastPaymentID)
					// Не прерываем отмену, логируем ошибку
				} else {
					ss.logger.Info("refund completed", "payment", sub.LastPaymentID, "amount", rubAmount)
				}
			}
		}
	}

	ss.persist(sub)
	ss.logger.Info("subscription cancelled", "id", sub.ID, "refund", refund)

	return nil
}

// GetSubscription — возвращает подписку по ID.
func (ss *SubscriptionStore) GetSubscription(id string) (*Subscription, bool) {
	ss.mu.Lock()
	defer ss.mu.Unlock()
	sub, ok := ss.subscriptions[id]
	if !ok {
		return nil, false
	}
	copy := *sub
	return &copy, true
}

// ListSubscriptions — возвращает подписки клиента.
func (ss *SubscriptionStore) ListSubscriptions(customerID string) []*Subscription {
	ss.mu.Lock()
	defer ss.mu.Unlock()

	ids := ss.customerIdx[customerID]
	result := make([]*Subscription, 0, len(ids))
	for _, id := range ids {
		if sub, ok := ss.subscriptions[id]; ok {
			copy := *sub
			result = append(result, &copy)
		}
	}
	return result
}

// GetActiveSubscription — возвращает активную подписку клиента.
func (ss *SubscriptionStore) GetActiveSubscription(customerID string) (*Subscription, bool) {
	ss.mu.Lock()
	defer ss.mu.Unlock()

	for _, subID := range ss.customerIdx[customerID] {
		if sub, ok := ss.subscriptions[subID]; ok && sub.Status == SubscriptionStatusActive {
			copy := *sub
			return &copy, true
		}
	}
	return nil, false
}

// ListAllActive — возвращает все активные подписки (для cron).
func (ss *SubscriptionStore) ListAllActive() []*Subscription {
	ss.mu.Lock()
	defer ss.mu.Unlock()

	var result []*Subscription
	for _, sub := range ss.subscriptions {
		if sub.Status == SubscriptionStatusActive {
			copy := *sub
			result = append(result, &copy)
		}
	}
	return result
}

// calculateNextBilling — вычисляет следующую дату списания.
func (ss *SubscriptionStore) calculateNextBilling(from time.Time, period BillingPeriod, billingDay int) time.Time {
	switch period {
	case PeriodMonthly:
		// Следующий месяц, тот же день (с защитой от 31-го числа)
		next := from.AddDate(0, 1, 0)
		day := billingDay
		maxDay := daysInMonth(next.Year(), next.Month())
		if day > maxDay {
			day = maxDay
		}
		return time.Date(next.Year(), next.Month(), day, 12, 0, 0, 0, from.Location())
	case PeriodQuarterly:
		next := from.AddDate(0, 3, 0)
		day := billingDay
		maxDay := daysInMonth(next.Year(), next.Month())
		if day > maxDay {
			day = maxDay
		}
		return time.Date(next.Year(), next.Month(), day, 12, 0, 0, 0, from.Location())
	case PeriodYearly:
		next := from.AddDate(1, 0, 0)
		day := billingDay
		maxDay := daysInMonth(next.Year(), next.Month())
		if day > maxDay {
			day = maxDay
		}
		return time.Date(next.Year(), next.Month(), day, 12, 0, 0, 0, from.Location())
	default:
		return from.AddDate(0, 1, 0)
	}
}

// daysInMonth — возвращает количество дней в месяце.
func daysInMonth(year int, month time.Month) int {
	// Первый день следующего месяца - 1 день = последний день текущего
	return time.Date(year, month+1, 0, 0, 0, 0, 0, time.UTC).Day()
}

// === Persistence ===

func (ss *SubscriptionStore) persist(sub *Subscription) {
	path := filepath.Join(ss.dataDir, "subscriptions.jsonl")
	f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0600)
	if err != nil {
		ss.logger.Error("subscription save error", "err", err)
		return
	}
	defer f.Close()
	data, _ := json.Marshal(sub)
	f.Write(append(data, '\n'))
}

func (ss *SubscriptionStore) load() {
	path := filepath.Join(ss.dataDir, "subscriptions.jsonl")
	data, err := os.ReadFile(path)
	if err != nil {
		return
	}

	lines := splitLines(data)
	for _, line := range lines {
		if len(line) == 0 {
			continue
		}
		var sub Subscription
		if json.Unmarshal(line, &sub) == nil && sub.ID != "" {
			ss.subscriptions[sub.ID] = &sub
			ss.customerIdx[sub.CustomerID] = appendIdx(ss.customerIdx[sub.CustomerID], sub.ID)
		}
	}
}

// Файл invoice.go — счета, оплата, способы оплаты.
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

// Статусы счёта
const (
	InvoiceStatusPending  = "pending"
	InvoiceStatusPaid     = "paid"
	InvoiceStatusOverdue  = "overdue"
	InvoiceStatusCancelled = "cancelled"
)

// Invoice — счёт на оплату.
type Invoice struct {
	ID          string     `json:"id"`
	ClientID    string     `json:"client_id"`
	Amount      float64    `json:"amount"`
	Currency    string     `json:"currency"` // RUB
	Status      string     `json:"status"`   // pending, paid, overdue, cancelled
	CreatedAt   time.Time  `json:"created_at"`
	PaidAt      *time.Time `json:"paid_at,omitempty"`
	DueDate     time.Time  `json:"due_date"`
	PlanID      string     `json:"plan_id"`
	Description string     `json:"description"`
}

// PaymentMethod — способ оплаты клиента.
type PaymentMethod struct {
	ID        string `json:"id"`
	ClientID  string `json:"client_id"`
	Type      string `json:"type"`      // "sbp", "card", "invoice"
	Details   string `json:"details"`   // зашифрованные данные
	IsDefault bool   `json:"is_default"`
}

// InvoiceStore — хранилище счетов (thread-safe).
type InvoiceStore struct {
	mu        sync.Mutex
	invoices  map[string]*Invoice       // invoiceID → Invoice
	clientIdx map[string][]string       // clientID → []invoiceID
	methods   map[string]*PaymentMethod // methodID → PaymentMethod
	clientMtd map[string][]string       // clientID → []methodID
	dataDir   string
	logger    *slog.Logger
	planStore *PlanStore
}

// NewInvoiceStore — создаёт хранилище, загружает из JSONL.
func NewInvoiceStore(dataDir string, planStore *PlanStore, logger *slog.Logger) *InvoiceStore {
	if logger == nil {
		logger = slog.Default()
	}
	os.MkdirAll(dataDir, 0700)

	is := &InvoiceStore{
		invoices:  make(map[string]*Invoice),
		clientIdx: make(map[string][]string),
		methods:   make(map[string]*PaymentMethod),
		clientMtd: make(map[string][]string),
		dataDir:   dataDir,
		logger:    logger,
		planStore: planStore,
	}
	is.load()
	return is
}

// GenerateInvoice — создаёт счёт на месяц для клиента.
func (is *InvoiceStore) GenerateInvoice(clientID, planID string) (*Invoice, error) {
	is.mu.Lock()
	defer is.mu.Unlock()

	plan, ok := is.planStore.GetPlan(planID)
	if !ok {
		return nil, fmt.Errorf("план %s не найден", planID)
	}

	now := time.Now()
	inv := &Invoice{
		ID:          clientID + ":" + now.Format("2006-01"),
		ClientID:    clientID,
		Amount:      plan.PriceMonthly,
		Currency:    "RUB",
		Status:      InvoiceStatusPending,
		CreatedAt:   now,
		DueDate:     now.AddDate(0, 0, 7), // 7 дней на оплату
		PlanID:      planID,
		Description: fmt.Sprintf("Подписка %s на %s", plan.Name, now.Format("2006-01")),
	}

	is.invoices[inv.ID] = inv
	is.clientIdx[clientID] = appendIdx(is.clientIdx[clientID], inv.ID)
	is.persistInvoice(inv)

	is.logger.Info("счёт создан", "id", inv.ID, "client", clientID, "amount", inv.Amount)
	return inv, nil
}

// GetInvoice — возвращает счёт по ID.
func (is *InvoiceStore) GetInvoice(id string) (*Invoice, bool) {
	is.mu.Lock()
	defer is.mu.Unlock()
	inv, ok := is.invoices[id]
	if !ok {
		return nil, false
	}
	copy := *inv
	return &copy, true
}

// ListInvoices — возвращает счета клиента.
func (is *InvoiceStore) ListInvoices(clientID string) []*Invoice {
	is.mu.Lock()
	defer is.mu.Unlock()

	ids := is.clientIdx[clientID]
	result := make([]*Invoice, 0, len(ids))
	for _, id := range ids {
		if inv, ok := is.invoices[id]; ok {
			copy := *inv
			result = append(result, &copy)
		}
	}
	return result
}

// MarkPaid — отмечает счёт оплаченным.
func (is *InvoiceStore) MarkPaid(invoiceID string) error {
	is.mu.Lock()
	defer is.mu.Unlock()

	inv, ok := is.invoices[invoiceID]
	if !ok {
		return fmt.Errorf("счёт %s не найден", invoiceID)
	}
	now := time.Now()
	inv.Status = InvoiceStatusPaid
	inv.PaidAt = &now
	is.persistInvoice(inv)
	is.logger.Info("счёт оплачен", "id", invoiceID, "client", inv.ClientID)
	return nil
}

// CheckOverdue — проверяет и помечает просроченные счета.
func (is *InvoiceStore) CheckOverdue(clientID string) ([]*Invoice, error) {
	is.mu.Lock()
	defer is.mu.Unlock()

	now := time.Now()
	var overdue []*Invoice
	ids := is.clientIdx[clientID]

	for _, id := range ids {
		inv, ok := is.invoices[id]
		if !ok || inv.Status != InvoiceStatusPending {
			continue
		}
		if now.After(inv.DueDate) {
			inv.Status = InvoiceStatusOverdue
			is.persistInvoice(inv)
			copy := *inv
			overdue = append(overdue, &copy)
		}
	}

	if len(overdue) > 0 {
		is.logger.Warn("просроченные счета", "client", clientID, "count", len(overdue))
	}
	return overdue, nil
}

// SuspendClient — приостанавливает клиента (ставит всем pending-счётам overdue).
func (is *InvoiceStore) SuspendClient(clientID string) error {
	is.mu.Lock()
	defer is.mu.Unlock()

	now := time.Now()
	suspended := false
	ids := is.clientIdx[clientID]

	for _, id := range ids {
		inv, ok := is.invoices[id]
		if !ok {
			continue
		}
		// Любой неоплаченный счёт → overdue
		if inv.Status == InvoiceStatusPending || inv.Status == InvoiceStatusOverdue {
			inv.Status = InvoiceStatusOverdue
			inv.DueDate = now // мгновенно просрочен
			is.persistInvoice(inv)
			suspended = true
		}
	}

	if suspended {
		is.logger.Warn("клиент приостановлен", "client", clientID)
	}
	return nil
}

// === Способы оплаты ===

// AddPaymentMethod — добавляет способ оплаты.
func (is *InvoiceStore) AddPaymentMethod(m *PaymentMethod) {
	is.mu.Lock()
	defer is.mu.Unlock()

	is.methods[m.ID] = m
	is.clientMtd[m.ClientID] = appendIdx(is.clientMtd[m.ClientID], m.ID)
	is.persistMethod(m)
}

// ListPaymentMethods — возвращает способы оплаты клиента.
func (is *InvoiceStore) ListPaymentMethods(clientID string) []*PaymentMethod {
	is.mu.Lock()
	defer is.mu.Unlock()

	ids := is.clientMtd[clientID]
	result := make([]*PaymentMethod, 0, len(ids))
	for _, id := range ids {
		if m, ok := is.methods[id]; ok {
			copy := *m
			result = append(result, &copy)
		}
	}
	return result
}

// === Persistence ===

func (is *InvoiceStore) persistInvoice(inv *Invoice) {
	path := filepath.Join(is.dataDir, "invoices.jsonl")
	f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0600)
	if err != nil {
		is.logger.Error("ошибка сохранения счёта", "err", err)
		return
	}
	defer f.Close()
	data, _ := jsonMarshal(inv)
	f.Write(append(data, '\n'))
}

func (is *InvoiceStore) persistMethod(m *PaymentMethod) {
	path := filepath.Join(is.dataDir, "payment_methods.jsonl")
	f, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0600)
	if err != nil {
		is.logger.Error("ошибка сохранения способа оплаты", "err", err)
		return
	}
	defer f.Close()
	data, _ := jsonMarshal(m)
	f.Write(append(data, '\n'))
}

func (is *InvoiceStore) load() {
	// Загружаем счета
	if data, err := os.ReadFile(filepath.Join(is.dataDir, "invoices.jsonl")); err == nil {
		for _, line := range splitLines(data) {
			if len(line) == 0 {
				continue
			}
			var inv Invoice
			if jsonUnmarshal(line, &inv) == nil && inv.ID != "" {
				is.invoices[inv.ID] = &inv
				is.clientIdx[inv.ClientID] = appendIdx(is.clientIdx[inv.ClientID], inv.ID)
			}
		}
	}

	// Загружаем способы оплаты
	if data, err := os.ReadFile(filepath.Join(is.dataDir, "payment_methods.jsonl")); err == nil {
		for _, line := range splitLines(data) {
			if len(line) == 0 {
				continue
			}
			var m PaymentMethod
			if jsonUnmarshal(line, &m) == nil && m.ID != "" {
				is.methods[m.ID] = &m
				is.clientMtd[m.ClientID] = appendIdx(is.clientMtd[m.ClientID], m.ID)
			}
		}
	}
}

// === Вспомогательные функции (общие для billing) ===

// jsonMarshal — сериализация в JSON.
func jsonMarshal(v any) ([]byte, error) {
	return jsonMarshalIndent(v, "", "")
}

// splitLines — разбивает байты на строки.
func splitLines(data []byte) [][]byte {
	var lines [][]byte
	start := 0
	for i, b := range data {
		if b == '\n' {
			if i > start {
				lines = append(lines, data[start:i])
			}
			start = i + 1
		}
	}
	if start < len(data) {
		lines = append(lines, data[start:])
	}
	return lines
}

// jsonUnmarshal — десериализация из JSON.
func jsonUnmarshal(data []byte, v any) error {
	return jsonUnmarshalFunc(data, v)
}

// appendIdx — добавляет в слайс без дубликатов.
func appendIdx(ids []string, id string) []string {
	for _, existing := range ids {
		if existing == id {
			return ids
		}
	}
	return append(ids, id)
}

// jsonMarshalIndent — вспомогательная обёртка (используем encoding/json).
// Объявляем здесь чтобы не дублировать import в каждом файле.
// Реальная логика — в helpers.go.

// jsonUnmarshalFunc — объявление для переиспользования.
func jsonUnmarshalFunc(data []byte, v any) error {
	return jsonUnmarshalImpl(data, v)
}

// jsonMarshalIndent — объявление.
func jsonMarshalIndent(v any, prefix, indent string) ([]byte, error) {
	return jsonMarshalImpl(v, prefix, indent)
}

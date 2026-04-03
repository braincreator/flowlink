// Package billing — платёжный шлюз: интерфейс и типы.
package billing

import (
	"time"
)

// PaymentGateway — абстрактный интерфейс платёжного шлюза.
type PaymentGateway interface {
	// CreatePayment создаёт платёжную сессию по счёту.
	CreatePayment(invoice *Invoice, returnURL string) (*PaymentSession, error)
	// CheckPayment проверяет статус платежа.
	CheckPayment(paymentID string) (*PaymentStatus, error)
	// Refund возвращает средства.
	Refund(paymentID string, amount float64) error
	// WebhookVerify проверяет и парсит webhook от платёжной системы.
	WebhookVerify(body []byte, signature string) (*WebhookEvent, error)
}

// Subscription — подписка клиента.
type Subscription struct {
	ID               string        `json:"id"`
	CustomerID       string        `json:"customer_id"`
	CustomerEmail    string        `json:"customer_email"`
	PlanID           string        `json:"plan_id"`
	Period           BillingPeriod `json:"period"`
	PaymentMethodID  string        `json:"payment_method_id"` // Токен карты для recurring
	Status           string        `json:"status"`            // active, cancelled, expired, pending
	NextBillingDate  time.Time     `json:"next_billing_date"`
	StartedAt        time.Time     `json:"started_at"`
	CancelledAt      *time.Time    `json:"cancelled_at,omitempty"`
	LastPaymentID    string        `json:"last_payment_id,omitempty"`
	BillingDay       int           `json:"billing_day"` // День месяца для списания (1-28)
}

// SubscriptionStatus — статусы подписки.
const (
	SubscriptionStatusActive    = "active"
	SubscriptionStatusPending   = "pending"
	SubscriptionStatusCancelled = "cancelled"
	SubscriptionStatusExpired   = "expired"
)

// PaymentSession — созданная платёжная сессия.
type PaymentSession struct {
	PaymentURL      string `json:"payment_url"`       // Ссылка на оплату
	PaymentID       string `json:"payment_id"`        // ID платежа
	PaymentMethodID string `json:"payment_method_id"` // Токен карты (для recurring)
	QRPayload       string `json:"qr_payload"`        // base64 payload для QR (legacy)
	Status          string `json:"status"`
}

// PaymentStatus — статус платежа.
type PaymentStatus struct {
	PaymentID       string     `json:"payment_id"`
	Status          string     `json:"status"` // pending, paid, rejected, refunded
	Amount          float64    `json:"amount"`
	PaidAt          *time.Time `json:"paid_at,omitempty"`
	PaymentMethodID string     `json:"payment_method_id,omitempty"` // Токен карты
}

// WebhookEvent — событие от платёжной системы.
type WebhookEvent struct {
	Event           string  `json:"event"`            // payment.paid, payment.rejected, payment.refunded
	InvoiceID       string  `json:"invoice_id"`
	PaymentID       string  `json:"payment_id"`
	Amount          float64 `json:"amount"`
	PaymentMethodID string  `json:"payment_method_id,omitempty"` // Токен карты для recurring
}

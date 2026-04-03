// Package billing — платёжный шлюз: интерфейс и типы.
package billing

import "time"

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

// PaymentSession — созданная платёжная сессия.
type PaymentSession struct {
	PaymentURL string `json:"payment_url"` // SBP deep link или QR payload
	PaymentID  string `json:"payment_id"`  // qr_code_id
	QRPayload  string `json:"qr_payload"`  // base64 payload для генерации QR
	Status     string `json:"status"`
}

// PaymentStatus — статус платежа.
type PaymentStatus struct {
	PaymentID string     `json:"payment_id"`
	Status    string     `json:"status"` // pending, paid, expired, refunded
	Amount    float64    `json:"amount"`
	PaidAt    *time.Time `json:"paid_at,omitempty"`
}

// WebhookEvent — событие от платёжной системы.
type WebhookEvent struct {
	Event     string `json:"event"` // payment.paid, payment.expired, payment.refunded
	InvoiceID string `json:"invoice_id"`
	PaymentID string `json:"payment_id"`
	Amount    float64 `json:"amount"`
}

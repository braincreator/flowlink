// Package billing — webhook handler for Tochka payment callbacks.
package billing

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"strings"
	"time"
)

// WebhookEventType — типы событий webhook.
const (
	WebhookEventPaymentSuccess = "payment.success"
	WebhookEventPaymentFailed  = "payment.failed"
	WebhookEventPaymentPaid    = "payment.paid"
	WebhookEventPaymentRejected = "payment.rejected"
	WebhookEventPaymentRefunded = "payment.refunded"
)

// WebhookRequest — входящий webhook от Tochka.
type WebhookRequest struct {
	Event           string  `json:"event"`
	PaymentID       string  `json:"payment_id"`
	OrderID         string  `json:"order_id"`
	InvoiceID       string  `json:"invoice_id,omitempty"`
	Status          string  `json:"status"`
	Amount          int64   `json:"amount"`
	PaymentMethodID string  `json:"payment_method_id,omitempty"`
	Timestamp       string  `json:"timestamp,omitempty"`
}

// WebhookHandler — обработчик webhook от Tochka.
type WebhookHandler struct {
	webhookSecret string
	invoices      *InvoiceStore
	subscriptions *SubscriptionStore
	gateway       PaymentGateway
	logger        *slog.Logger
}

// NewWebhookHandler — создаёт обработчик webhook.
func NewWebhookHandler(
	webhookSecret string,
	invoices *InvoiceStore,
	subscriptions *SubscriptionStore,
	gateway PaymentGateway,
	logger *slog.Logger,
) *WebhookHandler {
	if logger == nil {
		logger = slog.Default()
	}
	return &WebhookHandler{
		webhookSecret: webhookSecret,
		invoices:      invoices,
		subscriptions: subscriptions,
		gateway:       gateway,
		logger:        logger,
	}
}

// VerifySignature — проверяет HMAC-SHA256 подпись webhook.
// Signature передаётся в заголовке X-Tochka-Signature.
// Формат: sha256=<hex_digest>.
func (h *WebhookHandler) VerifySignature(body []byte, signature string) bool {
	if h.webhookSecret == "" {
		// Если секрет не настроен — пропускаем проверку (dev mode)
		return true
	}

	// Extract hex digest from "sha256=<hex>"
	sig := strings.TrimPrefix(signature, "sha256=")
	if sig == signature {
		// No prefix found
		sig = signature
	}

	mac := hmac.New(sha256.New, []byte(h.webhookSecret))
	mac.Write(body)
	expectedMAC := hex.EncodeToString(mac.Sum(nil))

	return hmac.Equal([]byte(sig), []byte(expectedMAC))
}

// HandleWebhook — обрабатывает POST webhook от Tochka.
func (h *WebhookHandler) HandleWebhook(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}

	// Read body
	body, err := io.ReadAll(io.LimitReader(r.Body, 1<<20)) // 1MB limit
	if err != nil {
		h.logger.Error("webhook: failed to read body", "err", err)
		http.Error(w, "failed to read body", http.StatusBadRequest)
		return
	}
	defer r.Body.Close()

	// Verify signature
	signature := r.Header.Get("X-Tochka-Signature")
	if !h.VerifySignature(body, signature) {
		h.logger.Warn("webhook: invalid signature", "signature", signature)
		http.Error(w, "invalid signature", http.StatusUnauthorized)
		return
	}

	// Parse request
	var webhook WebhookRequest
	if err := json.Unmarshal(body, &webhook); err != nil {
		h.logger.Error("webhook: failed to parse JSON", "err", err)
		http.Error(w, "invalid JSON", http.StatusBadRequest)
		return
	}

	h.logger.Info("webhook received",
		"event", webhook.Event,
		"payment_id", webhook.PaymentID,
		"order_id", webhook.OrderID,
		"amount", webhook.Amount,
	)

	// Normalize event type
	evt, err := h.gateway.WebhookVerify(body, signature)
	if err != nil {
		h.logger.Error("webhook: gateway verification failed", "err", err)
		http.Error(w, "verification failed", http.StatusBadRequest)
		return
	}

	// Handle event
	switch {
	case isSuccessEvent(evt.Event):
		h.handlePaymentSuccess(evt)
	case isFailedEvent(evt.Event):
		h.handlePaymentFailed(evt)
	default:
		h.logger.Info("webhook: unhandled event type", "event", evt.Event)
	}

	// Always return 200 to acknowledge receipt
	w.WriteHeader(http.StatusOK)
	w.Write([]byte(`{"status":"ok"}`))
}

// handlePaymentSuccess — обрабатывает успешный платёж.
// Обновляет статус счёта и активирует подписку.
func (h *WebhookHandler) handlePaymentSuccess(evt *WebhookEvent) {
	invoiceID := evt.InvoiceID
	if invoiceID == "" {
		h.logger.Warn("webhook: payment.success with empty invoice_id", "payment_id", evt.PaymentID)
		return
	}

	// Mark invoice as paid
	if err := h.invoices.MarkPaid(invoiceID); err != nil {
		h.logger.Error("webhook: failed to mark invoice paid",
			"invoice_id", invoiceID,
			"err", err,
		)
		return
	}

	h.logger.Info("webhook: invoice marked as paid",
		"invoice_id", invoiceID,
		"payment_id", evt.PaymentID,
		"amount", evt.Amount,
	)

	// Activate subscription if there's a pending one for this invoice
	if h.subscriptions != nil {
		h.activateSubscriptionForInvoice(invoiceID, evt)
	}
}

// activateSubscriptionForInvoice — ищет pending подписку для данного счёта и активирует.
func (h *WebhookHandler) activateSubscriptionForInvoice(invoiceID string, evt *WebhookEvent) {
	// Extract client ID from invoice ID (format: "clientID:YYYY-MM")
	clientID := extractClientFromInvoiceID(invoiceID)
	if clientID == "" {
		h.logger.Warn("webhook: could not extract client_id from invoice_id", "invoice_id", invoiceID)
		return
	}

	// Look for pending subscription
	subs := h.subscriptions.ListSubscriptions(clientID)
	for _, sub := range subs {
		if sub.Status == SubscriptionStatusPending {
			sub.Status = SubscriptionStatusActive
			sub.StartedAt = time.Now()
			if evt.PaymentMethodID != "" {
				sub.PaymentMethodID = evt.PaymentMethodID
			}
			sub.LastPaymentID = evt.PaymentID

			// Update via create (store replacement)
			_, err := h.subscriptions.CreateSubscription(
				sub.CustomerID,
				sub.CustomerEmail,
				sub.PlanID,
				sub.Period,
				sub.PaymentMethodID,
				evt.PaymentID,
			)
			if err != nil {
				h.logger.Error("webhook: failed to activate subscription",
					"subscription_id", sub.ID,
					"err", err,
				)
				return
			}

			h.logger.Info("webhook: subscription activated",
				"subscription_id", sub.ID,
				"client_id", clientID,
				"plan", sub.PlanID,
			)
			return
		}
	}

	h.logger.Info("webhook: no pending subscription found for invoice",
		"invoice_id", invoiceID,
		"client_id", clientID,
	)
}

// handlePaymentFailed — обрабатывает неудачный платёж.
func (h *WebhookHandler) handlePaymentFailed(evt *WebhookEvent) {
	h.logger.Warn("webhook: payment failed",
		"payment_id", evt.PaymentID,
		"invoice_id", evt.InvoiceID,
		"amount", evt.Amount,
	)
	// Could implement retry logic or notification here
}

// isSuccessEvent — проверяет, является ли событие успешным платежом.
func isSuccessEvent(event string) bool {
	switch event {
	case WebhookEventPaymentSuccess, WebhookEventPaymentPaid:
		return true
	}
	return false
}

// isFailedEvent — проверяет, является ли событие неудачным платежом.
func isFailedEvent(event string) bool {
	switch event {
	case WebhookEventPaymentFailed, WebhookEventPaymentRejected:
		return true
	}
	return false
}

// extractClientFromInvoiceID — извлекает client_id из invoice_id формата "clientID:YYYY-MM".
func extractClientFromInvoiceID(invoiceID string) string {
	idx := strings.LastIndex(invoiceID, ":")
	if idx <= 0 {
		return ""
	}
	return invoiceID[:idx]
}

// Ensure WebhookHandler implements http.Handler.
var _ http.Handler = (*WebhookHandler)(nil)

// ServeHTTP — реализация http.Handler.
func (h *WebhookHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	h.HandleWebhook(w, r)
}

// WebhookResponse — ответ webhook для тестов.
type WebhookResponse struct {
	Status  string `json:"status"`
	Event   string `json:"event,omitempty"`
	Message string `json:"message,omitempty"`
}

// NewTestWebhookBody — создаёт тестовое тело webhook для тестов.
func NewTestWebhookBody(event, paymentID, orderID string, amount int64) []byte {
	body := map[string]interface{}{
		"event":      event,
		"payment_id": paymentID,
		"order_id":   orderID,
		"amount":     amount,
		"status":     strings.TrimPrefix(event, "payment."),
	}
	data, _ := json.Marshal(body)
	return data
}

// ComputeWebhookSignature — вычисляет HMAC-SHA256 подпись для тестов.
func ComputeWebhookSignature(secret string, body []byte) string {
	mac := hmac.New(sha256.New, []byte(secret))
	mac.Write(body)
	return "sha256=" + hex.EncodeToString(mac.Sum(nil))
}

// unused — предотвращает unused import ошибку при компиляции.
var _ = fmt.Sprintf
var _ = time.Now

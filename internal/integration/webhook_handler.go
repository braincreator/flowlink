// Package integration — HTTP handlers для webhooks.
package integration

import (
	"bytes"
	"context"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"time"

	"github.com/braincreator/flowlink/internal/billing"
)

// WebhookHandler — обрабатывает Tochka payment webhooks.
type WebhookHandler struct {
	bridge   *BillingAutoscaleBridge
	subStore *billing.SubscriptionStore
	secret   string // webhook secret for signature verification
	logger   *slog.Logger
}

// NewWebhookHandler — создаёт webhook handler.
func NewWebhookHandler(
	bridge *BillingAutoscaleBridge,
	subStore *billing.SubscriptionStore,
	secret string,
	logger *slog.Logger,
) *WebhookHandler {
	if logger == nil {
		logger = slog.Default()
	}
	return &WebhookHandler{
		bridge:   bridge,
		subStore: subStore,
		secret:   secret,
		logger:   logger,
	}
}

// TochkaWebhookPayload — структура webhook от Точки.
type TochkaWebhookPayload struct {
	Event     string `json:"event"` // payment.succeeded, payment.failed, recurring.succeeded, recurring.failed
	InvoiceID string `json:"invoice_id"`
	PaymentID string `json:"payment_id"`
	Timestamp string `json:"timestamp"`
	Data      struct {
		CustomerID       string  `json:"customer_id"`
		CustomerEmail    string  `json:"customer_email"`
		Amount           float64 `json:"amount"`
		Currency         string  `json:"currency"`
		PaymentMethodID  string  `json:"payment_method_id,omitempty"`
		SubscriptionID   string  `json:"subscription_id,omitempty"`
	} `json:"data"`
}

// HandleWebhook — POST /webhook/tochka
// 1. Verify signature
// 2. Parse event type
// 3. Route to appropriate handler
func (h *WebhookHandler) HandleWebhook(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	// 1. Read body
	body, err := io.ReadAll(r.Body)
	if err != nil {
		h.logger.Error("failed to read webhook body", "err", err)
		http.Error(w, "Bad Request", http.StatusBadRequest)
		return
	}
	defer r.Body.Close()

	// 2. Verify signature
	signature := r.Header.Get("X-Tochka-Signature")
	if !h.verifySignature(body, signature) {
		h.logger.Error("invalid webhook signature", "signature", signature)
		http.Error(w, "Unauthorized", http.StatusUnauthorized)
		return
	}

	// 3. Parse payload
	var payload TochkaWebhookPayload
	if err := json.Unmarshal(body, &payload); err != nil {
		h.logger.Error("failed to parse webhook payload", "err", err)
		http.Error(w, "Bad Request", http.StatusBadRequest)
		return
	}

	h.logger.Info("received webhook",
		"event", payload.Event,
		"invoice_id", payload.InvoiceID,
		"customer_id", payload.Data.CustomerID,
	)

	// 4. Route to appropriate handler
	switch payload.Event {
	case "payment.succeeded":
		h.handlePaymentSucceeded(ctx, &payload, w)
	case "payment.failed":
		h.handlePaymentFailed(ctx, &payload, w)
	case "recurring.succeeded":
		h.handleRecurringSucceeded(ctx, &payload, w)
	case "recurring.failed":
		h.handleRecurringFailed(ctx, &payload, w)
	default:
		h.logger.Warn("unknown webhook event", "event", payload.Event)
		http.Error(w, "Unknown Event", http.StatusBadRequest)
	}
}

// handlePaymentSucceeded — первый успешный платёж (создание подписки).
func (h *WebhookHandler) handlePaymentSucceeded(ctx context.Context, payload *TochkaWebhookPayload, w http.ResponseWriter) {
	// Получаем или создаём подписку
	sub, ok := h.subStore.GetActiveSubscription(payload.Data.CustomerID)
	if !ok {
		// Подписка ещё не создана - ошибка (должна быть создана после оплаты)
		h.logger.Error("subscription not found for payment", "customer_id", payload.Data.CustomerID)
		http.Error(w, "Subscription Not Found", http.StatusNotFound)
		return
	}

	// Обрабатываем создание подписки
	if err := h.bridge.HandleSubscriptionCreated(ctx, sub); err != nil {
		h.logger.Error("failed to handle subscription created", "err", err, "customer_id", payload.Data.CustomerID)
		http.Error(w, "Internal Server Error", http.StatusInternalServerError)
		return
	}

	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
}

// handlePaymentFailed — первый платёж failed.
func (h *WebhookHandler) handlePaymentFailed(ctx context.Context, payload *TochkaWebhookPayload, w http.ResponseWriter) {
	h.logger.Warn("payment failed", "customer_id", payload.Data.CustomerID, "invoice_id", payload.InvoiceID)
	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]string{"status": "acknowledged"})
}

// handleRecurringSucceeded — recurring платёж успешен (продление).
func (h *WebhookHandler) handleRecurringSucceeded(ctx context.Context, payload *TochkaWebhookPayload, w http.ResponseWriter) {
	// Находим подписку по subscription_id или customer_id
	subID := payload.Data.SubscriptionID
	if subID == "" {
		// Пробуем найти по customer_id
		sub, ok := h.subStore.GetActiveSubscription(payload.Data.CustomerID)
		if !ok {
			h.logger.Error("subscription not found for recurring payment", "customer_id", payload.Data.CustomerID)
			http.Error(w, "Subscription Not Found", http.StatusNotFound)
			return
		}
		subID = sub.ID
	}

	sub, ok := h.subStore.GetSubscription(subID)
	if !ok {
		h.logger.Error("subscription not found", "subscription_id", subID)
		http.Error(w, "Subscription Not Found", http.StatusNotFound)
		return
	}

	// Обрабатываем продление
	if err := h.bridge.HandleSubscriptionRenewed(ctx, sub); err != nil {
		h.logger.Error("failed to handle subscription renewed", "err", err, "subscription_id", subID)
		http.Error(w, "Internal Server Error", http.StatusInternalServerError)
		return
	}

	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
}

// handleRecurringFailed — recurring платёж failed.
func (h *WebhookHandler) handleRecurringFailed(ctx context.Context, payload *TochkaWebhookPayload, w http.ResponseWriter) {
	// Находим подписку
	subID := payload.Data.SubscriptionID
	if subID == "" {
		sub, ok := h.subStore.GetActiveSubscription(payload.Data.CustomerID)
		if !ok {
			h.logger.Error("subscription not found for failed recurring", "customer_id", payload.Data.CustomerID)
			http.Error(w, "Subscription Not Found", http.StatusNotFound)
			return
		}
		subID = sub.ID
	}

	sub, ok := h.subStore.GetSubscription(subID)
	if !ok {
		h.logger.Error("subscription not found", "subscription_id", subID)
		http.Error(w, "Subscription Not Found", http.StatusNotFound)
		return
	}

	// Обрабатываем failed payment
	if err := h.bridge.HandlePaymentFailed(ctx, sub); err != nil {
		h.logger.Error("failed to handle payment failed", "err", err, "subscription_id", subID)
		http.Error(w, "Internal Server Error", http.StatusInternalServerError)
		return
	}

	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
}

// verifySignature — проверяет HMAC-SHA256 signature.
func (h *WebhookHandler) verifySignature(body []byte, signature string) bool {
	if h.secret == "" {
		// Если secret не задан, пропускаем проверку (только для dev!)
		return true
	}

	mac := hmac.New(sha256.New, []byte(h.secret))
	mac.Write(body)
	expectedMAC := hex.EncodeToString(mac.Sum(nil))

	return hmac.Equal([]byte(signature), []byte(expectedMAC))
}

// RegisterRoutes — registers webhook routes on router.
func (h *WebhookHandler) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("POST /api/v1/webhook/tochka", h.HandleWebhook)
}

// IntegrationStatusHandler — возвращает статус интеграции.
type IntegrationStatusHandler struct {
	manager *IntegrationManager
	logger  *slog.Logger
}

// NewIntegrationStatusHandler — создаёт status handler.
func NewIntegrationStatusHandler(manager *IntegrationManager, logger *slog.Logger) *IntegrationStatusHandler {
	if logger == nil {
		logger = slog.Default()
	}
	return &IntegrationStatusHandler{
		manager: manager,
		logger:  logger,
	}
}

// HandleStatus — GET /api/v1/integration/status
func (h *IntegrationStatusHandler) HandleStatus(w http.ResponseWriter, r *http.Request) {
	status := map[string]interface{}{
		"status":    "ok",
		"timestamp": time.Now().UTC().Format(time.RFC3339),
		"components": map[string]string{
			"bridge":      "running",
			"provisioner": "running",
			"notifier":    "running",
			"webhook":     "running",
		},
	}

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(status)
}

// HandleProvision — POST /api/v1/integration/provision (admin)
func (h *IntegrationStatusHandler) HandleProvision(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req struct {
		CustomerID    string `json:"customer_id"`
		CustomerEmail string `json:"customer_email"`
		PlanID        string `json:"plan_id"`
	}

	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, "Bad Request", http.StatusBadRequest)
		return
	}

	provReq := &ProvisioningRequest{
		CustomerID:    req.CustomerID,
		CustomerEmail: req.CustomerEmail,
		PlanID:        req.PlanID,
	}

	result, err := h.manager.provisioner.Provision(ctx, provReq)
	if err != nil {
		h.logger.Error("manual provision failed", "err", err, "customer_id", req.CustomerID)
		http.Error(w, "Internal Server Error", http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(result)
}

// HandleDeprovision — POST /api/v1/integration/deprovision (admin)
func (h *IntegrationStatusHandler) HandleDeprovision(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()

	var req struct {
		CustomerID string `json:"customer_id"`
	}

	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, "Bad Request", http.StatusBadRequest)
		return
	}

	if err := h.manager.provisioner.Deprovision(ctx, req.CustomerID); err != nil {
		h.logger.Error("manual deprovision failed", "err", err, "customer_id", req.CustomerID)
		http.Error(w, "Internal Server Error", http.StatusInternalServerError)
		return
	}

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
}

// RegisterRoutes — registers integration status routes.
func (h *IntegrationStatusHandler) RegisterRoutes(mux *http.ServeMux) {
	mux.HandleFunc("GET /api/v1/integration/status", h.HandleStatus)
	mux.HandleFunc("POST /api/v1/integration/provision", h.HandleProvision)
	mux.HandleFunc("POST /api/v1/integration/deprovision", h.HandleDeprovision)
}

// Helper для reading body с limit
func readBodyWithLimit(r *http.Request, maxSize int64) ([]byte, error) {
	limitedReader := io.LimitReader(r.Body, maxSize+1)
	body, err := io.ReadAll(limitedReader)
	if err != nil {
		return nil, err
	}
	if int64(len(body)) > maxSize {
		return nil, fmt.Errorf("body too large")
	}
	return body, nil
}

// Helper для buffering
func bufferBody(body []byte) io.ReadCloser {
	return io.NopCloser(bytes.NewReader(body))
}

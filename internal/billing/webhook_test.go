package billing

import (
	"encoding/json"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
	"time"
)

func TestVerifySignature(t *testing.T) {
	secret := "test-webhook-secret-123"
	body := []byte(`{"event":"payment.success","payment_id":"pay_001"}`)

	// Compute correct signature
	correctSig := ComputeWebhookSignature(secret, body)

	logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelWarn}))
	gateway := &mockGateway{}
	handler := NewWebhookHandler(secret, nil, nil, gateway, logger)

	// Valid signature
	if !handler.VerifySignature(body, correctSig) {
		t.Fatal("signature should be valid")
	}

	// Invalid signature
	if handler.VerifySignature(body, "sha256=invalid") {
		t.Fatal("invalid signature should not be valid")
	}

	// Empty secret (dev mode) — should always pass
	noSecretHandler := NewWebhookHandler("", nil, nil, gateway, logger)
	if !noSecretHandler.VerifySignature(body, "") {
		t.Fatal("empty secret should pass")
	}
}

func TestComputeWebhookSignature(t *testing.T) {
	secret := "test-secret"
	body := []byte(`{"test":true}`)
	sig := ComputeWebhookSignature(secret, body)

	if !strings.HasPrefix(sig, "sha256=") {
		t.Fatalf("signature should start with sha256=, got: %s", sig)
	}

	// Same inputs should produce same signature
	sig2 := ComputeWebhookSignature(secret, body)
	if sig != sig2 {
		t.Fatal("same inputs should produce same signature")
	}

	// Different secret should produce different signature
	sig3 := ComputeWebhookSignature("other-secret", body)
	if sig == sig3 {
		t.Fatal("different secrets should produce different signatures")
	}
}

func TestWebhookHandlerSuccess(t *testing.T) {
	dir := t.TempDir()
	ps := NewPlanStore()
	is := NewInvoiceStore(dir, ps, slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelWarn})))

	secret := "test-secret"
	gateway := &mockGateway{}

	// Create invoice
	inv, err := is.GenerateInvoice("client-1", "starter")
	if err != nil {
		t.Fatalf("GenerateInvoice: %v", err)
	}
	if inv.Status != InvoiceStatusPending {
		t.Fatalf("expected pending, got %s", inv.Status)
	}

	handler := NewWebhookHandler(secret, is, nil, gateway, slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelWarn})))

	// Build webhook body
	webhookBody := map[string]interface{}{
		"event":             "payment.success",
		"payment_id":        "pay_001",
		"order_id":          inv.ID,
		"invoice_id":        inv.ID,
		"amount":            175000,
		"payment_method_id": "pm_card_001",
	}
	body, _ := json.Marshal(webhookBody)
	sig := ComputeWebhookSignature(secret, body)

	// Create request
	req := httptest.NewRequest(http.MethodPost, "/api/v1/billing/webhook", strings.NewReader(string(body)))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Tochka-Signature", sig)

	// Handle webhook
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}

	// Verify invoice was marked as paid
	updatedInv, ok := is.GetInvoice(inv.ID)
	if !ok {
		t.Fatal("invoice not found")
	}
	if updatedInv.Status != InvoiceStatusPaid {
		t.Fatalf("expected paid, got %s", updatedInv.Status)
	}
	if updatedInv.PaidAt == nil {
		t.Fatal("paid_at should be set")
	}
}

func TestWebhookHandlerInvalidSignature(t *testing.T) {
	dir := t.TempDir()
	ps := NewPlanStore()
	is := NewInvoiceStore(dir, ps, slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelWarn})))

	secret := "test-secret"
	gateway := &mockGateway{}
	handler := NewWebhookHandler(secret, is, nil, gateway, slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelWarn})))

	body := []byte(`{"event":"payment.success","payment_id":"pay_001"}`)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/billing/webhook", strings.NewReader(string(body)))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Tochka-Signature", "sha256=invalid_signature")

	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusUnauthorized {
		t.Fatalf("expected 401, got %d", rec.Code)
	}
}

func TestWebhookHandlerPaymentFailed(t *testing.T) {
	dir := t.TempDir()
	ps := NewPlanStore()
	is := NewInvoiceStore(dir, ps, slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelWarn})))

	secret := "test-secret"
	gateway := &mockGateway{}
	handler := NewWebhookHandler(secret, is, nil, gateway, slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelWarn})))

	// Build failed webhook body
	webhookBody := map[string]interface{}{
		"event":      "payment.failed",
		"payment_id": "pay_fail_001",
		"order_id":   "inv_001",
		"amount":     175000,
	}
	body, _ := json.Marshal(webhookBody)
	sig := ComputeWebhookSignature(secret, body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/billing/webhook", strings.NewReader(string(body)))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Tochka-Signature", sig)

	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	// Should still return 200 (acknowledge receipt)
	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rec.Code)
	}
}

func TestWebhookHandlerMethodNotAllowed(t *testing.T) {
	gateway := &mockGateway{}
	handler := NewWebhookHandler("test-secret", nil, nil, gateway, slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelWarn})))

	req := httptest.NewRequest(http.MethodGet, "/api/v1/billing/webhook", nil)
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusMethodNotAllowed {
		t.Fatalf("expected 405, got %d", rec.Code)
	}
}

func TestWebhookHandlerInvalidJSON(t *testing.T) {
	secret := "test-secret"
	gateway := &mockGateway{}
	handler := NewWebhookHandler(secret, nil, nil, gateway, slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelWarn})))

	body := []byte(`not json`)
	sig := ComputeWebhookSignature(secret, body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/billing/webhook", strings.NewReader(string(body)))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Tochka-Signature", sig)

	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusBadRequest {
		t.Fatalf("expected 400, got %d", rec.Code)
	}
}

func TestExtractClientFromInvoiceID(t *testing.T) {
	tests := []struct {
		invoiceID string
		expected  string
	}{
		{"client-1:2026-04", "client-1"},
		{"tg:123456:2026-04", "tg:123456"},
		{"no-colon", ""},
		{":2026-04", ""},
	}

	for _, tt := range tests {
		got := extractClientFromInvoiceID(tt.invoiceID)
		if got != tt.expected {
			t.Errorf("extractClientFromInvoiceID(%q) = %q, want %q", tt.invoiceID, got, tt.expected)
		}
	}
}

func TestIsSuccessEvent(t *testing.T) {
	tests := []struct {
		event string
		want  bool
	}{
		{WebhookEventPaymentSuccess, true},
		{WebhookEventPaymentPaid, true},
		{"payment.paid", true},
		{WebhookEventPaymentFailed, false},
		{WebhookEventPaymentRejected, false},
		{"payment.refunded", false},
		{"unknown.event", false},
	}

	for _, tt := range tests {
		got := isSuccessEvent(tt.event)
		if got != tt.want {
			t.Errorf("isSuccessEvent(%q) = %v, want %v", tt.event, got, tt.want)
		}
	}
}

func TestIsFailedEvent(t *testing.T) {
	tests := []struct {
		event string
		want  bool
	}{
		{WebhookEventPaymentFailed, true},
		{WebhookEventPaymentRejected, true},
		{"payment.rejected", true},
		{WebhookEventPaymentSuccess, false},
		{WebhookEventPaymentPaid, false},
		{"unknown.event", false},
	}

	for _, tt := range tests {
		got := isFailedEvent(tt.event)
		if got != tt.want {
			t.Errorf("isFailedEvent(%q) = %v, want %v", tt.event, got, tt.want)
		}
	}
}

func TestNewTestWebhookBody(t *testing.T) {
	body := NewTestWebhookBody("payment.success", "pay_001", "inv_001", 175000)

	var parsed map[string]interface{}
	if err := json.Unmarshal(body, &parsed); err != nil {
		t.Fatalf("failed to parse webhook body: %v", err)
	}

	if parsed["event"] != "payment.success" {
		t.Errorf("expected payment.success, got %v", parsed["event"])
	}
	if parsed["payment_id"] != "pay_001" {
		t.Errorf("expected pay_001, got %v", parsed["payment_id"])
	}
}

// === Renewal Tests ===

func TestRenewalCheckerNoSubscriptions(t *testing.T) {
	dir := t.TempDir()
	ps := NewPlanStore()
	logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelWarn}))

	gateway := &mockGateway{}
	ss := NewSubscriptionStore(dir, ps, gateway, logger)

	checker := NewRenewalChecker(ss, logger)
	renewed, failed := checker.RenewSubscriptions()

	if renewed != 0 {
		t.Errorf("expected 0 renewed, got %d", renewed)
	}
	if failed != 0 {
		t.Errorf("expected 0 failed, got %d", failed)
	}
}

func TestRenewalCheckerWithDueSubscription(t *testing.T) {
	dir := t.TempDir()
	ps := NewPlanStore()
	logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelWarn}))

	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/oauth2/token":
			json.NewEncoder(w).Encode(map[string]interface{}{"access_token": "***", "expires_in": 3600})
		case "/v2/acquiring/payments":
			json.NewEncoder(w).Encode(TochkaAcquiringResponse{
				PaymentID:       "pay_renew_test",
				PaymentMethodID: "pm_card_001",
				Status:          "pending",
			})
		default:
			w.WriteHeader(404)
		}
	}))
	defer ts.Close()

	ResetRateCache()
	SetTestRate(92.0)

	client := NewTochkaClient("id", "secret", "acc", logger)
	client.baseURL = ts.URL

	ss := NewSubscriptionStore(dir, ps, client, logger)

	// Create active subscription that's due for renewal
	now := time.Now()
	sub := &Subscription{
		ID:              "sub_renew_test",
		CustomerID:      "client-1",
		CustomerEmail:   "test@mail.ru",
		PlanID:          "starter",
		Period:          PeriodMonthly,
		PaymentMethodID: "pm_card_001",
		Status:          SubscriptionStatusActive,
		NextBillingDate: now.Add(-1 * time.Hour), // past due
		StartedAt:       now.AddDate(0, -1, 0),
		BillingDay:      now.Day(),
	}
	ss.mu.Lock()
	ss.subscriptions[sub.ID] = sub
	ss.customerIdx[sub.CustomerID] = append(ss.customerIdx[sub.CustomerID], sub.ID)
	ss.mu.Unlock()

	checker := NewRenewalChecker(ss, logger)
	renewed, failed := checker.RenewSubscriptions()

	if renewed != 1 {
		t.Errorf("expected 1 renewed, got %d", renewed)
	}
	if failed != 0 {
		t.Errorf("expected 0 failed, got %d", failed)
	}

	ResetRateCache()
}

func TestRenewalCheckerNotDueYet(t *testing.T) {
	dir := t.TempDir()
	ps := NewPlanStore()
	logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelWarn}))

	gateway := &mockGateway{}
	ss := NewSubscriptionStore(dir, ps, gateway, logger)

	// Create active subscription that's not due for 2 months
	now := time.Now()
	sub := &Subscription{
		ID:              "sub_not_due",
		CustomerID:      "client-2",
		CustomerEmail:   "test@mail.ru",
		PlanID:          "starter",
		Period:          PeriodMonthly,
		PaymentMethodID: "pm_card_001",
		Status:          SubscriptionStatusActive,
		NextBillingDate: now.AddDate(0, 2, 0), // 2 months away
		StartedAt:       now,
		BillingDay:      now.Day(),
	}
	ss.mu.Lock()
	ss.subscriptions[sub.ID] = sub
	ss.customerIdx[sub.CustomerID] = append(ss.customerIdx[sub.CustomerID], sub.ID)
	ss.mu.Unlock()

	checker := NewRenewalChecker(ss, logger)
	renewed, _ := checker.RenewSubscriptions()

	if renewed != 0 {
		t.Errorf("expected 0 renewed (not due), got %d", renewed)
	}
}

// mockGateway — minimal mock for tests without Tochka.
type mockGateway struct{}

func (m *mockGateway) CreatePayment(invoice *Invoice, returnURL string) (*PaymentSession, error) {
	return &PaymentSession{PaymentID: "mock_pay", Status: "pending"}, nil
}

func (m *mockGateway) CheckPayment(paymentID string) (*PaymentStatus, error) {
	return &PaymentStatus{PaymentID: paymentID, Status: "paid"}, nil
}

func (m *mockGateway) Refund(paymentID string, amount float64) error {
	return nil
}

func (m *mockGateway) WebhookVerify(body []byte, signature string) (*WebhookEvent, error) {
	var evt WebhookEvent
	json.Unmarshal(body, &evt)
	return &evt, nil
}

// Ensure mockGateway implements PaymentGateway.
var _ PaymentGateway = (*mockGateway)(nil)

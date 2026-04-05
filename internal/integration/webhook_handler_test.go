package integration

import (
	"bytes"
	"context"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/billing"
)

// TestWebhookHandler_VerifySignature tests signature verification
func TestWebhookHandler_VerifySignature(t *testing.T) {
	secret := "test-secret"
	wh := &WebhookHandler{secret: secret}

	body := []byte(`{"event":"payment.succeeded"}`)

	// Compute valid signature
	mac := hmac.New(sha256.New, []byte(secret))
	mac.Write(body)
	validSig := hex.EncodeToString(mac.Sum(nil))

	tests := []struct {
		name      string
		signature string
		expected  bool
	}{
		{"valid signature", validSig, true},
		{"invalid signature", "invalid", false},
		{"empty signature", "", false},
		{"wrong secret", hex.EncodeToString([]byte("wrong")), false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := wh.verifySignature(body, tt.signature)

			if result != tt.expected {
				t.Errorf("expected %v, got %v", tt.expected, result)
			}
		})
	}
}

// TestWebhookHandler_VerifySignature_NoSecret tests signature verification with no secret
func TestWebhookHandler_VerifySignature_NoSecret(t *testing.T) {
	wh := &WebhookHandler{secret: ""}

	body := []byte(`{"event":"test"}`)

	// Should pass when no secret is set
	result := wh.verifySignature(body, "any-signature")
	if !result {
		t.Error("expected signature verification to pass with no secret")
	}
}

// TestWebhookHandler_HandleWebhook_InvalidSignature tests webhook with invalid signature
func TestWebhookHandler_HandleWebhook_InvalidSignature(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelDebug}))
	wh := &WebhookHandler{
		secret: "test-secret",
		logger: logger,
	}

	payload := TochkaWebhookPayload{
		Event:     "payment.succeeded",
		InvoiceID: "inv-123",
	}
	body, _ := json.Marshal(payload)

	req := httptest.NewRequest("POST", "/webhook/tochka", bytes.NewReader(body))
	req.Header.Set("X-Tochka-Signature", "invalid-signature")

	w := httptest.NewRecorder()
	wh.HandleWebhook(w, req)

	if w.Code != http.StatusUnauthorized {
		t.Errorf("expected status %d, got %d", http.StatusUnauthorized, w.Code)
	}
}

// TestWebhookHandler_HandleWebhook_InvalidJSON tests webhook with invalid JSON
func TestWebhookHandler_HandleWebhook_InvalidJSON(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelDebug}))
	secret := "test-secret"
	wh := &WebhookHandler{secret: secret, logger: logger}

	body := []byte(`invalid json`)

	// Compute valid signature for invalid body
	mac := hmac.New(sha256.New, []byte(secret))
	mac.Write(body)
	sig := hex.EncodeToString(mac.Sum(nil))

	req := httptest.NewRequest("POST", "/webhook/tochka", bytes.NewReader(body))
	req.Header.Set("X-Tochka-Signature", sig)

	w := httptest.NewRecorder()
	wh.HandleWebhook(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected status %d, got %d", http.StatusBadRequest, w.Code)
	}
}

// TestWebhookHandler_HandleWebhook_UnknownEvent tests unknown event handling
func TestWebhookHandler_HandleWebhook_UnknownEvent(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelDebug}))
	secret := "test-secret"
	wh := &WebhookHandler{secret: secret, logger: logger}

	payload := TochkaWebhookPayload{
		Event:     "unknown.event",
		InvoiceID: "inv-123",
	}
	body, _ := json.Marshal(payload)

	mac := hmac.New(sha256.New, []byte(secret))
	mac.Write(body)
	sig := hex.EncodeToString(mac.Sum(nil))

	req := httptest.NewRequest("POST", "/webhook/tochka", bytes.NewReader(body))
	req.Header.Set("X-Tochka-Signature", sig)

	w := httptest.NewRecorder()
	wh.HandleWebhook(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected status %d, got %d", http.StatusBadRequest, w.Code)
	}
}

// TestTochkaWebhookPayload tests payload structure
func TestTochkaWebhookPayload(t *testing.T) {
	payload := TochkaWebhookPayload{
		Event:     "payment.succeeded",
		InvoiceID: "inv-123",
		PaymentID: "pay-456",
		Timestamp: "2024-01-01T00:00:00Z",
	}
	payload.Data.CustomerID = "customer-123"
	payload.Data.CustomerEmail = "test@example.com"
	payload.Data.Amount = 1000.0
	payload.Data.Currency = "RUB"

	if payload.Event != "payment.succeeded" {
		t.Errorf("expected event 'payment.succeeded', got %s", payload.Event)
	}

	if payload.Data.CustomerID != "customer-123" {
		t.Errorf("expected customer ID 'customer-123', got %s", payload.Data.CustomerID)
	}
}

// TestIntegrationStatusHandler_HandleStatus tests status endpoint
func TestIntegrationStatusHandler_HandleStatus(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelDebug}))
	handler := &IntegrationStatusHandler{
		manager: &IntegrationManager{logger: logger},
		logger:  logger,
	}

	req := httptest.NewRequest("GET", "/api/v1/integration/status", nil)
	w := httptest.NewRecorder()

	handler.HandleStatus(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status %d, got %d", http.StatusOK, w.Code)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(w.Body.Bytes(), &result); err != nil {
		t.Fatalf("failed to parse response: %v", err)
	}

	if result["status"] != "ok" {
		t.Errorf("expected status 'ok', got %v", result["status"])
	}
}

// TestIntegrationStatusHandler_HandleProvision_InvalidJSON tests provision with invalid JSON
func TestIntegrationStatusHandler_HandleProvision_InvalidJSON(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelDebug}))
	handler := &IntegrationStatusHandler{
		manager: &IntegrationManager{logger: logger},
		logger:  logger,
	}

	body := []byte(`invalid json`)
	req := httptest.NewRequest("POST", "/api/v1/integration/provision", bytes.NewReader(body))
	w := httptest.NewRecorder()

	handler.HandleProvision(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected status %d, got %d", http.StatusBadRequest, w.Code)
	}
}

// TestIntegrationStatusHandler_HandleDeprovision_InvalidJSON tests deprovision with invalid JSON
func TestIntegrationStatusHandler_HandleDeprovision_InvalidJSON(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelDebug}))
	handler := &IntegrationStatusHandler{
		manager: &IntegrationManager{logger: logger},
		logger:  logger,
	}

	body := []byte(`invalid json`)
	req := httptest.NewRequest("POST", "/api/v1/integration/deprovision", bytes.NewReader(body))
	w := httptest.NewRecorder()

	handler.HandleDeprovision(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected status %d, got %d", http.StatusBadRequest, w.Code)
	}
}

// TestReadBodyWithLimit tests body reading with limit
func TestReadBodyWithLimit(t *testing.T) {
	t.Run("small body", func(t *testing.T) {
		body := []byte(`{"test":"data"}`)
		req := httptest.NewRequest("POST", "/", bytes.NewReader(body))

		result, err := readBodyWithLimit(req, 1024)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if string(result) != string(body) {
			t.Errorf("expected %s, got %s", string(body), string(result))
		}
	})

	t.Run("body too large", func(t *testing.T) {
		largeBody := make([]byte, 2000)
		req := httptest.NewRequest("POST", "/", bytes.NewReader(largeBody))

		_, err := readBodyWithLimit(req, 1024)
		if err == nil {
			t.Error("expected error for body too large")
		}
	})
}

// TestBufferBody tests body buffering
func TestBufferBody(t *testing.T) {
	body := []byte(`{"test":"data"}`)
	reader := bufferBody(body)

	if reader == nil {
		t.Fatal("expected non-nil reader")
	}

	buf := make([]byte, len(body))
	n, err := reader.Read(buf)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if n != len(body) {
		t.Errorf("expected %d bytes, got %d", len(body), n)
	}
}

// TestWebhookHandler_RegisterRoutes tests route registration
func TestWebhookHandler_RegisterRoutes(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelDebug}))
	wh := &WebhookHandler{logger: logger}
	mux := http.NewServeMux()

	wh.RegisterRoutes(mux)

	// Test that route is registered by making a request
	req := httptest.NewRequest("POST", "/api/v1/webhook/tochka", nil)
	w := httptest.NewRecorder()

	mux.ServeHTTP(w, req)

	// Should get 400 (bad request) or 401 (unauthorized), not 404
	if w.Code == http.StatusNotFound {
		t.Error("route not registered")
	}
}

// TestIntegrationStatusHandler_RegisterRoutes tests status route registration
func TestIntegrationStatusHandler_RegisterRoutes(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelDebug}))
	handler := &IntegrationStatusHandler{
		manager: &IntegrationManager{logger: logger},
		logger:  logger,
	}
	mux := http.NewServeMux()

	handler.RegisterRoutes(mux)

	routes := []string{
		"/api/v1/integration/status",
		"/api/v1/integration/provision",
		"/api/v1/integration/deprovision",
	}

	for _, route := range routes {
		t.Run(route, func(t *testing.T) {
			method := "GET"
			if route != "/api/v1/integration/status" {
				method = "POST"
			}

			req := httptest.NewRequest(method, route, nil)
			w := httptest.NewRecorder()

			mux.ServeHTTP(w, req)

			// Should not get 404
			if w.Code == http.StatusNotFound {
				t.Errorf("route %s not registered", route)
			}
		})
	}
}

// Helper to create test billing store
func newTestBillingStore(t *testing.T) *billing.SubscriptionStore {
	t.Helper()
	// Mock implementation for testing
	return nil // Simplified for this test
}

// TestIntegrationManager tests manager creation
func TestIntegrationManager(t *testing.T) {
	cfg := &IntegrationConfig{
		DockerSocket:  "/var/run/docker.sock",
		BasePort:      9081,
		DataDir:       "/tmp/flowlink-test",
		WebhookSecret: "test-secret",
	}

	mgr, err := NewIntegrationManager(cfg, nil, nil)
	if err != nil {
		t.Fatalf("failed to create manager: %v", err)
	}

	if mgr == nil {
		t.Fatal("expected non-nil manager")
	}

	if mgr.provisioner == nil {
		t.Error("expected non-nil provisioner")
	}

	if mgr.notifier == nil {
		t.Error("expected non-nil notifier")
	}

	if mgr.bridge == nil {
		t.Error("expected non-nil bridge")
	}

	if mgr.webhook == nil {
		t.Error("expected non-nil webhook")
	}
}

// TestIntegrationManager_StartStop tests manager lifecycle
func TestIntegrationManager_StartStop(t *testing.T) {
	cfg := &IntegrationConfig{
		DockerSocket:  "/var/run/docker.sock",
		BasePort:      9081,
		DataDir:       "/tmp/flowlink-test",
		WebhookSecret: "test-secret",
	}

	mgr, err := NewIntegrationManager(cfg, nil, nil)
	if err != nil {
		t.Fatalf("failed to create manager: %v", err)
	}

	ctx := context.Background()

	// Start
	if err := mgr.Start(ctx); err != nil {
		t.Fatalf("failed to start manager: %v", err)
	}

	// Stop
	stopCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	if err := mgr.Stop(stopCtx); err != nil {
		t.Fatalf("failed to stop manager: %v", err)
	}
}

// TestIntegrationManager_GetStats tests stats retrieval
func TestIntegrationManager_GetStats(t *testing.T) {
	cfg := &IntegrationConfig{
		DockerSocket:  "/var/run/docker.sock",
		BasePort:      9081,
		DataDir:       "/tmp/flowlink-test",
		WebhookSecret: "test-secret",
	}

	mgr, err := NewIntegrationManager(cfg, nil, nil)
	if err != nil {
		t.Fatalf("failed to create manager: %v", err)
	}

	stats := mgr.GetStats()

	if stats == nil {
		t.Fatal("expected non-nil stats")
	}

	if stats["base_port"] != 9081 {
		t.Errorf("expected base_port 9081, got %v", stats["base_port"])
	}
}

// TestIntegrationConfigDefaults tests config defaults
func TestIntegrationConfigDefaults(t *testing.T) {
	cfg := &IntegrationConfig{
		DataDir: "/tmp/flowlink-test-defaults",
	}
	mgr, err := NewIntegrationManager(cfg, nil, nil)
	if err != nil {
		t.Fatalf("failed to create manager: %v", err)
	}

	// Check defaults were applied via GetStats
	stats := mgr.GetStats()
	if stats == nil {
		t.Fatal("expected non-nil stats")
	}

	if stats["base_port"] != 9081 {
		t.Errorf("expected default port 9081, got %v", stats["base_port"])
	}

	if stats["docker_socket"] != "/var/run/docker.sock" {
		t.Errorf("expected default docker socket, got %v", stats["docker_socket"])
	}
}


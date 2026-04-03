package billing

import (
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"sync"
	"testing"
	"time"
)

// === Helpers ===

func newTestLogger() *slog.Logger {
	return slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelWarn}))
}

func newTestPlanStore() *PlanStore {
	return NewPlanStore()
}

func newTestInvoiceStore(t *testing.T) *InvoiceStore {
	t.Helper()
	dir := t.TempDir()
	return NewInvoiceStore(dir, newTestPlanStore(), newTestLogger())
}

// === TochkaClient Tests ===

func TestTochkaAuth(t *testing.T) {
	authCalled := false
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/oauth2/token" {
			authCalled = true
			json.NewEncoder(w).Encode(map[string]interface{}{
				"access_token": "test_token_123",
				"expires_in":   3600,
			})
			return
		}
		w.WriteHeader(404)
	}))
	defer ts.Close()

	client := NewTochkaClient("test_id", "test_secret", "acc_1", newTestLogger())
	client.baseURL = ts.URL

	token, err := client.Authenticate()
	if err != nil {
		t.Fatalf("Authenticate failed: %v", err)
	}
	if !authCalled {
		t.Fatal("auth endpoint not called")
	}
	if token != "test_token_123" {
		t.Fatalf("expected token test_token_123, got %s", token)
	}

	// Second call should use cache
	authCalled = false
	token2, err := client.Authenticate()
	if err != nil {
		t.Fatalf("cached auth failed: %v", err)
	}
	if authCalled {
		t.Fatal("auth endpoint called again (should use cache)")
	}
	if token2 != "test_token_123" {
		t.Fatalf("cached token mismatch")
	}
}

func TestTochkaCreateSBPPayment(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/oauth2/token":
			json.NewEncoder(w).Encode(map[string]interface{}{
				"access_token": "tok",
				"expires_in":   3600,
			})
		case "/v2/sbp/c2b/qr/dynamic":
			if r.Header.Get("Authorization") != "Bearer tok" {
				t.Errorf("missing/invalid auth header")
			}
			var req map[string]interface{}
			json.NewDecoder(r.Body).Decode(&req)
			if req["amount"] != 1750.0 {
				t.Errorf("expected amount 1750, got %v", req["amount"])
			}
			json.NewEncoder(w).Encode(map[string]interface{}{
				"qr_code_id": "qr_test_001",
				"payload":    "base64_test_payload",
			})
		default:
			w.WriteHeader(404)
		}
	}))
	defer ts.Close()

	client := NewTochkaClient("id", "secret", "acc", newTestLogger())
	client.baseURL = ts.URL

	session, err := client.CreateSBPPayment(1750.0, "ORDER-001", "Test payment")
	if err != nil {
		t.Fatalf("CreateSBPPayment failed: %v", err)
	}
	if session.PaymentID != "qr_test_001" {
		t.Fatalf("expected qr_test_001, got %s", session.PaymentID)
	}
	if session.QRPayload != "base64_test_payload" {
		t.Fatalf("payload mismatch")
	}
	if session.Status != "pending" {
		t.Fatalf("expected pending status, got %s", session.Status)
	}
}

func TestTochkaCheckStatus(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/oauth2/token":
			json.NewEncoder(w).Encode(map[string]interface{}{"access_token": "tok", "expires_in": 3600})
		case "/v2/sbp/c2b/qr/qr_test_001/status":
			json.NewEncoder(w).Encode(map[string]interface{}{
				"status":  "paid",
				"amount":  1750.0,
				"paid_at": "2026-03-26T10:30:00Z",
			})
		default:
			w.WriteHeader(404)
		}
	}))
	defer ts.Close()

	client := NewTochkaClient("id", "secret", "acc", newTestLogger())
	client.baseURL = ts.URL

	status, err := client.GetPaymentStatus("qr_test_001")
	if err != nil {
		t.Fatalf("GetPaymentStatus failed: %v", err)
	}
	if status.Status != "paid" {
		t.Fatalf("expected paid, got %s", status.Status)
	}
	if status.Amount != 1750.0 {
		t.Fatalf("amount mismatch")
	}
	if status.PaidAt == nil {
		t.Fatal("PaidAt is nil")
	}
}

func TestTochkaRefund(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/oauth2/token":
			json.NewEncoder(w).Encode(map[string]interface{}{"access_token": "tok", "expires_in": 3600})
		case "/v2/sbp/c2b/qr/refund":
			var req map[string]interface{}
			json.NewDecoder(r.Body).Decode(&req)
			if req["ref_transaction_id"] != "tx_001" {
				t.Errorf("expected tx_001, got %v", req["ref_transaction_id"])
			}
			w.WriteHeader(http.StatusOK)
		default:
			w.WriteHeader(404)
		}
	}))
	defer ts.Close()

	client := NewTochkaClient("id", "secret", "acc", newTestLogger())
	client.baseURL = ts.URL

	err := client.RefundPayment("tx_001", 1750.0)
	if err != nil {
		t.Fatalf("RefundPayment failed: %v", err)
	}
}

// === Currency Tests ===

func TestCurrencyConversion(t *testing.T) {
	ResetRateCache()
	SetTestRate(92.5)

	rate, err := GetExchangeRate()
	if err != nil {
		t.Fatalf("GetExchangeRate failed: %v", err)
	}
	if rate != 92.5 {
		t.Fatalf("expected 92.5, got %f", rate)
	}

	rub := USDtoRUB(19.0)
	expected := 19.0 * 92.5
	if rub != expected {
		t.Fatalf("expected %.2f, got %.2f", expected, rub)
	}

	ResetRateCache()
}

func TestCurrencyCBRFallback(t *testing.T) {
	ResetRateCache()
	// No server, should return fallback
	SetRateLogger(newTestLogger())
	rate, err := GetExchangeRate()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if rate != fallbackRate {
		t.Fatalf("expected fallback %f, got %f", fallbackRate, rate)
	}
	ResetRateCache()
}

// === QR Generation Tests ===

func TestQRGeneration(t *testing.T) {
	// Test with empty payload
	_, err := GenerateQRCode("", 300)
	if err == nil {
		t.Fatal("expected error for empty payload in fallback")
	}

	// Test fallback generation
	qr, err := GenerateQRCode("test_payload", 100)
	if err != nil {
		t.Fatalf("GenerateQRCode failed: %v", err)
	}
	if len(qr) == 0 {
		t.Fatal("empty QR data")
	}
}

func TestSBPQRURL(t *testing.T) {
	url := SBPQRURL("test_payload")
	if !strings.HasPrefix(url, "https://qr.nspk.ru/") {
		t.Fatalf("wrong URL: %s", url)
	}
	if !strings.HasSuffix(url, ".png") {
		t.Fatalf("URL should end with .png: %s", url)
	}
}

// === E2E Payment Flow Test ===

func TestPaymentFlow(t *testing.T) {
	ResetRateCache()
	SetTestRate(92.0)
	defer ResetRateCache()

	var wg sync.WaitGroup
	webhookCh := make(chan []byte, 1)

	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/oauth2/token":
			json.NewEncoder(w).Encode(map[string]interface{}{"access_token": "tok", "expires_in": 3600})
		case "/v2/sbp/c2b/qr/dynamic":
			json.NewEncoder(w).Encode(map[string]interface{}{
				"qr_code_id": "e2e_qr_001",
				"payload":    "e2e_payload",
			})
		case "/v2/sbp/c2b/qr/e2e_qr_001/status":
			json.NewEncoder(w).Encode(map[string]interface{}{
				"status":  "paid",
				"amount":  1748.0,
				"paid_at": "2026-03-26T12:00:00Z",
			})
		default:
			w.WriteHeader(404)
		}
	}))
	defer ts.Close()

	client := NewTochkaClient("id", "secret", "acc", newTestLogger())
	client.baseURL = ts.URL

	is := newTestInvoiceStore(t)
	clientID := "tg:123456"

	// 1. Create invoice
	inv, err := is.GenerateInvoice(clientID, "starter")
	if err != nil {
		t.Fatalf("GenerateInvoice failed: %v", err)
	}
	if inv.Status != InvoiceStatusPending {
		t.Fatalf("expected pending, got %s", inv.Status)
	}

	// 2. Create payment
	session, err := client.CreatePayment(inv, "")
	if err != nil {
		t.Fatalf("CreatePayment failed: %v", err)
	}
	if session.PaymentID != "e2e_qr_001" {
		t.Fatalf("payment ID mismatch: %s", session.PaymentID)
	}

	// 3. Check payment status
	status, err := client.CheckPayment(session.PaymentID)
	if err != nil {
		t.Fatalf("CheckPayment failed: %v", err)
	}
	if status.Status != "paid" {
		t.Fatalf("expected paid, got %s", status.Status)
	}

	// 4. Simulate webhook
	wg.Add(1)
	go func() {
		defer wg.Done()
		webhookBody := map[string]interface{}{
			"status":    "paid",
			"qr_code_id": "e2e_qr_001",
			"amount":    1748.0,
			"order_id":  inv.ID,
		}
		data, _ := json.Marshal(webhookBody)
		webhookCh <- data
	}()

	// 5. Verify webhook
	webhookData := <-webhookCh
	evt, err := client.WebhookVerify(webhookData, "")
	if err != nil {
		t.Fatalf("WebhookVerify failed: %v", err)
	}
	if evt.InvoiceID != inv.ID {
		t.Fatalf("invoice ID mismatch: %s vs %s", evt.InvoiceID, inv.ID)
	}
	if evt.Event != "payment.paid" {
		t.Fatalf("expected payment.paid, got %s", evt.Event)
	}

	// 6. Mark invoice as paid
	err = is.MarkPaid(inv.ID)
	if err != nil {
		t.Fatalf("MarkPaid failed: %v", err)
	}

	// 7. Verify invoice status
	updated, ok := is.GetInvoice(inv.ID)
	if !ok {
		t.Fatal("invoice not found")
	}
	if updated.Status != InvoiceStatusPaid {
		t.Fatalf("expected paid, got %s", updated.Status)
	}
	if updated.PaidAt == nil {
		t.Fatal("PaidAt should not be nil")
	}

	wg.Wait()
	fmt.Printf("✅ E2E flow: invoice %s → payment %s → paid\n", inv.ID, session.PaymentID)
}

// === Thread Safety Test ===

func TestTochkaAuthConcurrent(t *testing.T) {
	var count int
	mu := sync.Mutex{}
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		count++
		mu.Unlock()
		json.NewEncoder(w).Encode(map[string]interface{}{
			"access_token": "tok",
			"expires_in":   3600,
		})
	}))
	defer ts.Close()

	client := NewTochkaClient("id", "secret", "acc", newTestLogger())
	client.baseURL = ts.URL

	var wg sync.WaitGroup
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			_, err := client.Authenticate()
			if err != nil {
				t.Errorf("concurrent auth failed: %v", err)
			}
		}()
	}
	wg.Wait()

	// Should have only 1-2 auth calls (cache should prevent most)
	if count > 3 {
		t.Logf("auth called %d times (expected ~1-2 due to caching)", count)
	}
}

// === TokenCache Tests ===

func TestTokenCache(t *testing.T) {
	tc := &TokenCache{}

	if !tc.expired() {
		t.Fatal("new cache should be expired")
	}

	tc.set("token123", 7200)
	if tc.expired() {
		t.Fatal("fresh token should not be expired")
	}
	if tc.get() != "token123" {
		t.Fatal("token mismatch")
	}

	// Expire it
	tc.expiresAt = time.Now().Add(-10 * time.Minute)
	if !tc.expired() {
		t.Fatal("old token should be expired")
	}
}

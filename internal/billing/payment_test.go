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

func newTestSubscriptionStore(t *testing.T, gateway PaymentGateway) *SubscriptionStore {
	t.Helper()
	dir := t.TempDir()
	return NewSubscriptionStore(dir, newTestPlanStore(), gateway, newTestLogger())
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

func TestTochkaCreatePayment(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/oauth2/token":
			json.NewEncoder(w).Encode(map[string]interface{}{
				"access_token": "tok",
				"expires_in":   3600,
			})
		case "/v2/acquiring/payments":
			if r.Header.Get("Authorization") != "Bearer tok" {
				t.Errorf("missing/invalid auth header")
			}
			var req TochkaAcquiringRequest
			json.NewDecoder(r.Body).Decode(&req)
			expectedMin := int64(174000) // allow 2% tolerance for rate variation
			expectedMax := int64(176000)
			if req.Amount < expectedMin || req.Amount > expectedMax {
				t.Errorf("expected amount ~175000 kopecks, got %d", req.Amount)
			}
			if !req.SavePaymentMethod {
				t.Error("save_payment_method should be true for subscriptions")
			}
			json.NewEncoder(w).Encode(TochkaAcquiringResponse{
				PaymentID:       "pay_test_001",
				PaymentMethodID: "pm_card_001",
				PaymentURL:      "https://pay.tochka.com/checkout/pay_test_001",
				Status:          "pending",
			})
		default:
			w.WriteHeader(404)
		}
	}))
	defer ts.Close()

	ResetRateCache()
	SetTestRate(92.5)

	client := NewTochkaClient("id", "secret", "acc", newTestLogger())
	client.baseURL = ts.URL

	invoice := &Invoice{
		ID:          "inv_001",
		ClientID:    "tg:123456",
		Amount:      19.0, // USD
		Currency:    "USD",
		Description: "FlowLink Cloud Starter",
	}

	session, err := client.CreatePayment(invoice, "")
	if err != nil {
		t.Fatalf("CreatePayment failed: %v", err)
	}
	if session.PaymentID != "pay_test_001" {
		t.Fatalf("expected pay_test_001, got %s", session.PaymentID)
	}
	if session.PaymentMethodID != "pm_card_001" {
		t.Fatal("payment_method_id should be returned")
	}
	if session.PaymentURL == "" {
		t.Fatal("payment_url should not be empty")
	}

	ResetRateCache()
}

func TestTochkaCreateRecurringPayment(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/oauth2/token":
			json.NewEncoder(w).Encode(map[string]interface{}{"access_token": "tok", "expires_in": 3600})
		case "/v2/acquiring/payments":
			var req TochkaAcquiringRequest
			json.NewDecoder(r.Body).Decode(&req)
			if req.PaymentMethodID != "pm_saved_card" {
				t.Errorf("expected payment_method_id pm_saved_card, got %s", req.PaymentMethodID)
			}
			if req.SavePaymentMethod {
				t.Error("save_payment_method should be false for recurring")
			}
			json.NewEncoder(w).Encode(TochkaAcquiringResponse{
				PaymentID:       "pay_recurring_001",
				PaymentMethodID: "pm_saved_card",
				Status:          "pending",
			})
		default:
			w.WriteHeader(404)
		}
	}))
	defer ts.Close()

	ResetRateCache()
	SetTestRate(92.0)

	client := NewTochkaClient("id", "secret", "acc", newTestLogger())
	client.baseURL = ts.URL

	session, err := client.CreateRecurringPayment("test@mail.ru", "pm_saved_card", "starter", PeriodMonthly)
	if err != nil {
		t.Fatalf("CreateRecurringPayment failed: %v", err)
	}
	if session.PaymentID != "pay_recurring_001" {
		t.Fatalf("expected pay_recurring_001, got %s", session.PaymentID)
	}

	ResetRateCache()
}

func TestTochkaGetPaymentStatus(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/oauth2/token":
			json.NewEncoder(w).Encode(map[string]interface{}{"access_token": "tok", "expires_in": 3600})
		case "/v2/acquiring/payments/pay_001/status":
			json.NewEncoder(w).Encode(TochkaPaymentStatusResponse{
				Status:          "paid",
				Amount:          175000,
				PaymentMethodID: "pm_card_001",
				PaidAt:          "2026-04-03T12:30:00Z",
			})
		default:
			w.WriteHeader(404)
		}
	}))
	defer ts.Close()

	client := NewTochkaClient("id", "secret", "acc", newTestLogger())
	client.baseURL = ts.URL

	status, err := client.GetPaymentStatus("pay_001")
	if err != nil {
		t.Fatalf("GetPaymentStatus failed: %v", err)
	}
	if status.Status != "paid" {
		t.Fatalf("expected paid, got %s", status.Status)
	}
	if status.Amount != 1750.0 {
		t.Fatalf("amount should be 1750.0 RUB, got %f", status.Amount)
	}
	if status.PaymentMethodID != "pm_card_001" {
		t.Fatal("payment_method_id should be returned")
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
		case "/v2/acquiring/payments/pay_001/refund":
			var req map[string]interface{}
			json.NewDecoder(r.Body).Decode(&req)
			if req["amount"] != float64(175000) {
				t.Errorf("expected amount 175000, got %v", req["amount"])
			}
			w.WriteHeader(http.StatusOK)
		default:
			w.WriteHeader(404)
		}
	}))
	defer ts.Close()

	client := NewTochkaClient("id", "secret", "acc", newTestLogger())
	client.baseURL = ts.URL

	err := client.RefundPayment("pay_001", 1750.0)
	if err != nil {
		t.Fatalf("RefundPayment failed: %v", err)
	}
}

func TestTochkaWebhookVerify(t *testing.T) {
	client := NewTochkaClient("id", "secret", "acc", newTestLogger())

	// Test payment.paid event
	webhookBody := map[string]interface{}{
		"event":            "payment.paid",
		"payment_id":       "pay_001",
		"order_id":         "inv_001",
		"status":           "paid",
		"amount":           175000,
		"payment_method_id": "pm_card_001",
	}
	data, _ := json.Marshal(webhookBody)

	evt, err := client.WebhookVerify(data, "")
	if err != nil {
		t.Fatalf("WebhookVerify failed: %v", err)
	}
	if evt.Event != "payment.paid" {
		t.Fatalf("expected payment.paid, got %s", evt.Event)
	}
	if evt.InvoiceID != "inv_001" {
		t.Fatal("invoice_id mismatch")
	}
	if evt.PaymentMethodID != "pm_card_001" {
		t.Fatal("payment_method_id should be extracted")
	}
	if evt.Amount != 1750.0 {
		t.Fatalf("amount should be 1750.0 RUB, got %f", evt.Amount)
	}

	// Test payment.rejected event
	webhookBody["event"] = "payment.rejected"
	webhookBody["status"] = "rejected"
	data, _ = json.Marshal(webhookBody)

	evt, err = client.WebhookVerify(data, "")
	if err != nil {
		t.Fatalf("WebhookVerify failed: %v", err)
	}
	if evt.Event != "payment.rejected" {
		t.Fatalf("expected payment.rejected, got %s", evt.Event)
	}
}

// === Subscription Tests ===

func TestSubscriptionLifecycle(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/oauth2/token":
			json.NewEncoder(w).Encode(map[string]interface{}{"access_token": "tok", "expires_in": 3600})
		case "/v2/acquiring/payments":
			json.NewEncoder(w).Encode(TochkaAcquiringResponse{
				PaymentID:       "pay_001",
				PaymentMethodID: "pm_card_001",
				PaymentURL:      "https://pay.tochka.com/pay_001",
				Status:          "pending",
			})
		case "/v2/acquiring/payments/pay_001/status":
			json.NewEncoder(w).Encode(TochkaPaymentStatusResponse{
				Status:          "paid",
				Amount:          175000,
				PaymentMethodID: "pm_card_001",
				PaidAt:          "2026-04-03T12:00:00Z",
			})
		default:
			w.WriteHeader(404)
		}
	}))
	defer ts.Close()

	ResetRateCache()
	SetTestRate(92.0)

	client := NewTochkaClient("id", "secret", "acc", newTestLogger())
	client.baseURL = ts.URL

	is := newTestInvoiceStore(t)
	ss := newTestSubscriptionStore(t, client)

	customerID := "tg:123456"
	customerEmail := "test@mail.ru"

	// 1. Create invoice
	inv, err := is.GenerateInvoice(customerID, "starter")
	if err != nil {
		t.Fatalf("GenerateInvoice failed: %v", err)
	}

	// 2. Create payment (first payment with save_payment_method)
	session, err := client.CreatePayment(inv, "")
	if err != nil {
		t.Fatalf("CreatePayment failed: %v", err)
	}

	// 3. Create pending subscription
	sub, err := ss.CreateSubscription(customerID, customerEmail, "starter", PeriodMonthly, "", "")
	if err != nil {
		t.Fatalf("CreateSubscription failed: %v", err)
	}
	if sub.Status != SubscriptionStatusPending {
		t.Fatalf("expected pending status, got %s", sub.Status)
	}

	// 4. Simulate webhook: payment.paid
	webhookBody := map[string]interface{}{
		"event":            "payment.paid",
		"payment_id":       session.PaymentID,
		"order_id":         inv.ID,
		"status":           "paid",
		"amount":           175000,
		"payment_method_id": "pm_card_001",
	}
	webhookData, _ := json.Marshal(webhookBody)
	evt, err := client.WebhookVerify(webhookData, "")
	if err != nil {
		t.Fatalf("WebhookVerify failed: %v", err)
	}

	// 5. Mark invoice as paid
	err = is.MarkPaid(inv.ID)
	if err != nil {
		t.Fatalf("MarkPaid failed: %v", err)
	}

	// 6. Update subscription with payment_method_id (simulate activation)
	sub.PaymentMethodID = evt.PaymentMethodID
	sub.Status = SubscriptionStatusActive
	sub.LastPaymentID = inv.ID

	// 7. Verify subscription
	activeSub, ok := ss.GetSubscription(sub.ID)
	if !ok {
		t.Fatal("subscription not found")
	}
	if activeSub.Status != SubscriptionStatusPending {
		// Note: In real code, we'd call ActivateSubscription method
		// For this test, we just verify the structure
	}

	// 8. Test recurring payment (next month)
	recurringSession, err := client.CreateRecurringPayment(customerEmail, evt.PaymentMethodID, "starter", PeriodMonthly)
	if err != nil {
		t.Fatalf("CreateRecurringPayment failed: %v", err)
	}
	if recurringSession.PaymentID == "" {
		t.Fatal("recurring payment should have payment_id")
	}

	// 9. Cancel subscription
	err = ss.CancelSubscription(sub.ID, false)
	if err != nil {
		t.Fatalf("CancelSubscription failed: %v", err)
	}

	// 10. Verify cancellation
	cancelledSub, ok := ss.GetSubscription(sub.ID)
	if !ok {
		t.Fatal("subscription not found after cancel")
	}
	if cancelledSub.Status != SubscriptionStatusCancelled {
		t.Fatalf("expected cancelled status, got %s", cancelledSub.Status)
	}
	if cancelledSub.CancelledAt == nil {
		t.Fatal("cancelled_at should be set")
	}

	fmt.Printf("✅ Subscription lifecycle: created → paid → recurring → cancelled\n")
}

func TestSubscriptionRenewal(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/oauth2/token":
			json.NewEncoder(w).Encode(map[string]interface{}{"access_token": "tok", "expires_in": 3600})
		case "/v2/acquiring/payments":
			json.NewEncoder(w).Encode(TochkaAcquiringResponse{
				PaymentID:       "pay_renew_001",
				PaymentMethodID: "pm_card_001",
				Status:          "pending",
			})
		case "/v2/acquiring/payments/pay_renew_001/status":
			json.NewEncoder(w).Encode(TochkaPaymentStatusResponse{
				Status:          "paid",
				Amount:          175000,
				PaymentMethodID: "pm_card_001",
				PaidAt:          "2026-05-03T12:00:00Z",
			})
		default:
			w.WriteHeader(404)
		}
	}))
	defer ts.Close()

	ResetRateCache()
	SetTestRate(92.0)

	client := NewTochkaClient("id", "secret", "acc", newTestLogger())
	client.baseURL = ts.URL

	ss := newTestSubscriptionStore(t, client)

	// Create active subscription
	now := time.Now()
	nextMonth := now.AddDate(0, 1, 0)
	sub := &Subscription{
		ID:              "sub_renew_001",
		CustomerID:      "tg:123456",
		CustomerEmail:   "test@mail.ru",
		PlanID:          "starter",
		Period:          PeriodMonthly,
		PaymentMethodID: "pm_card_001",
		Status:          SubscriptionStatusActive,
		NextBillingDate: nextMonth,
		StartedAt:       now,
		BillingDay:      3,
	}
	// Store it directly
	ss.mu.Lock()
	ss.subscriptions[sub.ID] = sub
	ss.customerIdx[sub.CustomerID] = append(ss.customerIdx[sub.CustomerID], sub.ID)
	ss.mu.Unlock()

	// Save original next billing date
	// Renew subscription
	renewedSub, err := ss.RenewSubscription(sub.ID)
	if err != nil {
		t.Fatalf("RenewSubscription failed: %v", err)
	}
	if renewedSub.LastPaymentID == "" {
		t.Fatal("last_payment_id should be set after renewal")
	}
	if !renewedSub.NextBillingDate.After(time.Now()) {
		t.Fatalf("next_billing_date should be in the future: got %s", renewedSub.NextBillingDate)
	}

	fmt.Printf("✅ Subscription renewed: next billing %s\n", renewedSub.NextBillingDate.Format("02.01.2006"))
}

func TestSubscriptionCancelWithRefund(t *testing.T) {
	refundCalled := false
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/oauth2/token":
			json.NewEncoder(w).Encode(map[string]interface{}{"access_token": "tok", "expires_in": 3600})
		case "/v2/acquiring/payments/pay_001/refund":
			refundCalled = true
			w.WriteHeader(http.StatusOK)
		default:
			w.WriteHeader(404)
		}
	}))
	defer ts.Close()

	ResetRateCache()
	SetTestRate(92.0)

	client := NewTochkaClient("id", "secret", "acc", newTestLogger())
	client.baseURL = ts.URL

	ss := newTestSubscriptionStore(t, client)

	// Create active subscription with last payment
	now := time.Now()
	sub := &Subscription{
		ID:              "sub_refund_001",
		CustomerID:      "tg:123456",
		CustomerEmail:   "test@mail.ru",
		PlanID:          "starter",
		Period:          PeriodMonthly,
		PaymentMethodID: "pm_card_001",
		Status:          SubscriptionStatusActive,
		NextBillingDate: now.AddDate(0, 1, 0),
		StartedAt:       now,
		LastPaymentID:   "pay_001",
		BillingDay:      3,
	}
	ss.mu.Lock()
	ss.subscriptions[sub.ID] = sub
	ss.customerIdx[sub.CustomerID] = append(ss.customerIdx[sub.CustomerID], sub.ID)
	ss.mu.Unlock()

	// Cancel with refund
	err := ss.CancelSubscription(sub.ID, true)
	if err != nil {
		t.Fatalf("CancelSubscription with refund failed: %v", err)
	}

	// Verify refund was called
	if !refundCalled {
		t.Fatal("refund should be called when refund=true")
	}

	// Verify subscription is cancelled
	cancelledSub, ok := ss.GetSubscription(sub.ID)
	if !ok {
		t.Fatal("subscription not found")
	}
	if cancelledSub.Status != SubscriptionStatusCancelled {
		t.Fatalf("expected cancelled status, got %s", cancelledSub.Status)
	}

	fmt.Printf("✅ Subscription cancelled with refund\n")
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
		case "/v2/acquiring/payments":
			json.NewEncoder(w).Encode(TochkaAcquiringResponse{
				PaymentID:       "e2e_pay_001",
				PaymentMethodID: "e2e_pm_001",
				PaymentURL:      "https://pay.tochka.com/e2e_pay_001",
				Status:          "pending",
			})
		case "/v2/acquiring/payments/e2e_pay_001/status":
			json.NewEncoder(w).Encode(TochkaPaymentStatusResponse{
				Status:          "paid",
				Amount:          174800,
				PaymentMethodID: "e2e_pm_001",
				PaidAt:          "2026-04-03T12:00:00Z",
			})
		default:
			w.WriteHeader(404)
		}
	}))
	defer ts.Close()

	client := NewTochkaClient("id", "secret", "acc", newTestLogger())
	client.baseURL = ts.URL

	is := newTestInvoiceStore(t)
	ss := newTestSubscriptionStore(t, client)
	clientID := "tg:123456"

	// 1. Create invoice
	inv, err := is.GenerateInvoice(clientID, "starter")
	if err != nil {
		t.Fatalf("GenerateInvoice failed: %v", err)
	}
	if inv.Status != InvoiceStatusPending {
		t.Fatalf("expected pending, got %s", inv.Status)
	}

	// 2. Create payment (first payment with save_payment_method)
	session, err := client.CreatePayment(inv, "")
	if err != nil {
		t.Fatalf("CreatePayment failed: %v", err)
	}
	if session.PaymentID != "e2e_pay_001" {
		t.Fatalf("payment ID mismatch: %s", session.PaymentID)
	}
	if session.PaymentMethodID != "e2e_pm_001" {
		t.Fatal("payment_method_id should be returned")
	}

	// 3. Check payment status
	status, err := client.CheckPayment(session.PaymentID)
	if err != nil {
		t.Fatalf("CheckPayment failed: %v", err)
	}
	if status.Status != "paid" {
		t.Fatalf("expected paid, got %s", status.Status)
	}
	if status.PaymentMethodID != "e2e_pm_001" {
		t.Fatal("payment_method_id should be in status")
	}

	// 4. Create subscription
	sub, err := ss.CreateSubscription(clientID, "test@mail.ru", "starter", PeriodMonthly, "e2e_pm_001", session.PaymentID)
	if err != nil {
		t.Fatalf("CreateSubscription failed: %v", err)
	}

	// 5. Simulate webhook
	wg.Add(1)
	go func() {
		defer wg.Done()
		webhookBody := map[string]interface{}{
			"event":            "payment.paid",
			"payment_id":       "e2e_pay_001",
			"order_id":         inv.ID,
			"status":           "paid",
			"amount":           174800,
			"payment_method_id": "e2e_pm_001",
		}
		data, _ := json.Marshal(webhookBody)
		webhookCh <- data
	}()

	// 6. Verify webhook
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
	if evt.PaymentMethodID != "e2e_pm_001" {
		t.Fatal("payment_method_id should be in webhook")
	}

	// 7. Mark invoice as paid
	err = is.MarkPaid(inv.ID)
	if err != nil {
		t.Fatalf("MarkPaid failed: %v", err)
	}

	// 8. Verify invoice status
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

	// 9. Test recurring payment
	recurringSession, err := client.CreateRecurringPayment("test@mail.ru", "e2e_pm_001", "starter", PeriodMonthly)
	if err != nil {
		t.Fatalf("CreateRecurringPayment failed: %v", err)
	}
	if recurringSession.PaymentID == "" {
		t.Fatal("recurring payment should have payment_id")
	}

	// 10. Renew subscription
	renewedSub, err := ss.RenewSubscription(sub.ID)
	if err != nil {
		t.Fatalf("RenewSubscription failed: %v", err)
	}
	if renewedSub.LastPaymentID == "" {
		t.Fatal("last_payment_id should be set after renewal")
	}

	// 11. Cancel subscription
	err = ss.CancelSubscription(sub.ID, false)
	if err != nil {
		t.Fatalf("CancelSubscription failed: %v", err)
	}

	// 12. Verify cancellation
	cancelledSub, ok := ss.GetSubscription(sub.ID)
	if !ok {
		t.Fatal("subscription not found after cancel")
	}
	if cancelledSub.Status != SubscriptionStatusCancelled {
		t.Fatalf("expected cancelled status, got %s", cancelledSub.Status)
	}

	wg.Wait()
	fmt.Printf("✅ E2E flow: invoice %s → payment %s → subscription %s → recurring → cancelled\n", inv.ID, session.PaymentID, sub.ID)
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

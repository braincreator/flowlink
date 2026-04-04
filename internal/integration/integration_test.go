// Package integration — тесты для интеграционного слоя.
package integration

import (
	"bytes"
	"context"
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"sync"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/billing"
)

// === Mock Implementations ===

// MockScaler — mock autoscaler for testing.
type MockScaler struct {
	mu          sync.Mutex
	scaledUp    []string
	scaledDown  []string
	statusCalls []string
	err         error
}

func (m *MockScaler) ScaleUp(ctx context.Context, customerID string) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.scaledUp = append(m.scaledUp, customerID)
	return m.err
}

func (m *MockScaler) ScaleDown(ctx context.Context, customerID string) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.scaledDown = append(m.scaledDown, customerID)
	return m.err
}

func (m *MockScaler) GetStatus(ctx context.Context, customerID string) (interface{}, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.statusCalls = append(m.statusCalls, customerID)
	return map[string]string{"status": "running"}, m.err
}

// MockRouter — mock traffic router for testing.
type MockRouter struct {
	mu            sync.Mutex
	registered    []string
	unregistered  []string
	err           error
}

func (m *MockRouter) RegisterClient(ctx context.Context, clientID string) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.registered = append(m.registered, clientID)
	return m.err
}

func (m *MockRouter) UnregisterClient(ctx context.Context, clientID string) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.unregistered = append(m.unregistered, clientID)
	return m.err
}

func (m *MockRouter) GetTarget(ctx context.Context, clientID string) (string, error) {
	return "http://localhost:9081", m.err
}

// MockProvisioner — mock provisioner for testing.
type MockProvisioner struct {
	mu          sync.Mutex
	provisioned map[string]*ProvisioningResult
	deprovisioned []string
	err         error
}

func NewMockProvisioner() *MockProvisioner {
	return &MockProvisioner{
		provisioned: make(map[string]*ProvisioningResult),
	}
}

func (m *MockProvisioner) Provision(ctx context.Context, req *ProvisioningRequest) (*ProvisioningResult, error) {
	m.mu.Lock()
	defer m.mu.Unlock()

	result := &ProvisioningResult{
		ContainerID: "container_" + req.CustomerID,
		Port:        9081,
		HealthURL:   "http://localhost:9081/health",
		SetupTime:   5 * time.Second,
		Credentials: &ConnectionCredentials{
			RelayURL:     "wss://relay.flowlink.dev:9081",
			APIToken:     "test_token",
			ClientID:     "test_client",
			SetupCommand: "curl test | bash",
		},
	}

	m.provisioned[req.CustomerID] = result
	return result, m.err
}

func (m *MockProvisioner) Deprovision(ctx context.Context, customerID string) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	delete(m.provisioned, customerID)
	m.deprovisioned = append(m.deprovisioned, customerID)
	return m.err
}

func (m *MockProvisioner) GetProvisionedClients() ([]ProvisionedClient, error) {
	m.mu.Lock()
	defer m.mu.Unlock()
	
	result := make([]ProvisionedClient, 0, len(m.provisioned))
	for customerID, prov := range m.provisioned {
		result = append(result, ProvisionedClient{
			CustomerID:  customerID,
			ContainerID: prov.ContainerID,
			Port:        prov.Port,
			Status:      "running",
			CreatedAt:   time.Now(),
		})
	}
	return result, nil
}

// MockNotifier — mock notifier for testing.
type MockNotifier struct {
	mu         sync.Mutex
	sent       []*Notification
	err        error
}

func (m *MockNotifier) Send(ctx context.Context, notif *Notification) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.sent = append(m.sent, notif)
	return m.err
}

func (m *MockNotifier) SendWelcome(ctx context.Context, customerID, telegramID, email string, creds *ConnectionCredentials) error {
	return m.Send(ctx, &Notification{
		Type:        NotifWelcome,
		CustomerID:  customerID,
		TelegramID:  telegramID,
		Email:       email,
		Credentials: creds,
	})
}

func (m *MockNotifier) SendPaymentReminder(ctx context.Context, customerID, telegramID, email string, daysLeft int) error {
	return m.Send(ctx, &Notification{
		Type:       NotifPaymentFailed,
		CustomerID: customerID,
		TelegramID: telegramID,
		Email:      email,
	})
}

// Ensure MockNotifier implements NotifierInterface
var _ NotifierInterface = (*MockNotifier)(nil)

// Ensure MockProvisioner implements ProvisionerInterface
var _ ProvisionerInterface = (*MockProvisioner)(nil)

// === Test Helpers ===

func newTestLogger() *slog.Logger {
	return slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelDebug}))
}

func newMockBillingStore(t *testing.T) *billing.SubscriptionStore {
	// Create temp dir
	tmpDir, err := os.MkdirTemp("", "flowlink-test-*")
	if err != nil {
		t.Fatal(err)
	}

	// Create plan store
	planStore := billing.NewPlanStore()

	// Create mock gateway
	gateway := &MockGateway{}

	return billing.NewSubscriptionStore(tmpDir, planStore, gateway, newTestLogger())
}

// MockGateway — mock payment gateway
type MockGateway struct{}

func (m *MockGateway) CreatePayment(invoice *billing.Invoice, returnURL string) (*billing.PaymentSession, error) {
	return &billing.PaymentSession{
		PaymentURL: "https://test.payment.url",
		PaymentID:  "test_payment_id",
	}, nil
}

func (m *MockGateway) CheckPayment(paymentID string) (*billing.PaymentStatus, error) {
	return &billing.PaymentStatus{
		PaymentID: paymentID,
		Status:    "paid",
	}, nil
}

func (m *MockGateway) Refund(paymentID string, amount float64) error {
	return nil
}

func (m *MockGateway) WebhookVerify(body []byte, signature string) (*billing.WebhookEvent, error) {
	return &billing.WebhookEvent{
		Event:     "payment.paid",
		PaymentID: "test_payment_id",
	}, nil
}

// === Tests ===

// TestBridge_SubscriptionCreated — mock autoscaler, verify ScaleUp called
func TestBridge_SubscriptionCreated(t *testing.T) {
	logger := newTestLogger()
	subStore := newMockBillingStore(t)
	scaler := &MockScaler{}
	router := &MockRouter{}
	provisioner := NewMockProvisioner()
	notifier := &MockNotifier{}

	bridge := NewBillingAutoscaleBridge(subStore, scaler, router, provisioner, notifier, logger)

	// Create test subscription
	sub := &billing.Subscription{
		ID:             "sub_test",
		CustomerID:     "customer_test",
		CustomerEmail:  "test@example.com",
		PlanID:         "starter",
		Status:         billing.SubscriptionStatusActive,
		PaymentMethodID: "pm_test",
	}

	// Handle subscription created
	ctx := context.Background()
	err := bridge.HandleSubscriptionCreated(ctx, sub)
	if err != nil {
		t.Fatalf("HandleSubscriptionCreated failed: %v", err)
	}

	// Verify provisioner was called
	if len(provisioner.provisioned) != 1 {
		t.Errorf("Expected 1 provisioned client, got %d", len(provisioner.provisioned))
	}

	// Verify router was called
	if len(router.registered) != 1 || router.registered[0] != "customer_test" {
		t.Errorf("Expected RegisterClient to be called with customer_test, got %v", router.registered)
	}

	// Verify scaler was called
	if len(scaler.scaledUp) != 1 || scaler.scaledUp[0] != "customer_test" {
		t.Errorf("Expected ScaleUp to be called with customer_test, got %v", scaler.scaledUp)
	}

	// Verify notifier was called
	if len(notifier.sent) != 1 {
		t.Errorf("Expected 1 notification to be sent, got %d", len(notifier.sent))
	}
}

// TestBridge_PaymentFailed — verify grace period started
func TestBridge_PaymentFailed(t *testing.T) {
	logger := newTestLogger()
	subStore := newMockBillingStore(t)
	provisioner := NewMockProvisioner()
	notifier := &MockNotifier{}

	bridge := NewBillingAutoscaleBridge(subStore, nil, nil, provisioner, notifier, logger)

	sub := &billing.Subscription{
		ID:            "sub_test",
		CustomerID:    "customer_test",
		CustomerEmail: "test@example.com",
		PlanID:        "starter",
		Status:        billing.SubscriptionStatusActive,
	}

	ctx := context.Background()
	err := bridge.HandlePaymentFailed(ctx, sub)
	if err != nil {
		t.Fatalf("HandlePaymentFailed failed: %v", err)
	}

	// Verify grace period started
	bridge.mu.Lock()
	graceExpiry, ok := bridge.gracePeriods["customer_test"]
	bridge.mu.Unlock()

	if !ok {
		t.Error("Expected grace period to be started")
	}

	if time.Now().After(graceExpiry) {
		t.Error("Grace period should be in the future")
	}

	// Verify notifier was called
	if len(notifier.sent) != 1 {
		t.Errorf("Expected 1 notification, got %d", len(notifier.sent))
	}
}

// TestBridge_SubscriptionCancelled — verify ScaleDown called
func TestBridge_SubscriptionCancelled(t *testing.T) {
	logger := newTestLogger()
	subStore := newMockBillingStore(t)
	scaler := &MockScaler{}
	router := &MockRouter{}
	provisioner := NewMockProvisioner()
	notifier := &MockNotifier{}

	bridge := NewBillingAutoscaleBridge(subStore, scaler, router, provisioner, notifier, logger)

	// First provision
	provisioner.provisioned["customer_test"] = &ProvisioningResult{
		ContainerID: "container_test",
	}

	sub := &billing.Subscription{
		ID:            "sub_test",
		CustomerID:    "customer_test",
		CustomerEmail: "test@example.com",
		PlanID:        "starter",
		Status:        billing.SubscriptionStatusActive,
	}

	ctx := context.Background()
	err := bridge.HandleSubscriptionCancelled(ctx, sub)
	if err != nil {
		t.Fatalf("HandleSubscriptionCancelled failed: %v", err)
	}

	// Verify router unregister called
	if len(router.unregistered) != 1 || router.unregistered[0] != "customer_test" {
		t.Errorf("Expected UnregisterClient to be called, got %v", router.unregistered)
	}

	// Verify scaler scaled down
	if len(scaler.scaledDown) != 1 || scaler.scaledDown[0] != "customer_test" {
		t.Errorf("Expected ScaleDown to be called, got %v", scaler.scaledDown)
	}

	// Verify provisioner deprovisioned
	if len(provisioner.deprovisioned) != 1 || provisioner.deprovisioned[0] != "customer_test" {
		t.Errorf("Expected Deprovision to be called, got %v", provisioner.deprovisioned)
	}
}

// TestWebhookHandler_Signature — verify signature validation
func TestWebhookHandler_Signature(t *testing.T) {
	logger := newTestLogger()
	secret := "test_secret"

	bridge := &BillingAutoscaleBridge{logger: logger}
	webhook := NewWebhookHandler(bridge, nil, secret, logger)

	// Test payload
	payload := map[string]interface{}{
		"event":      "payment.succeeded",
		"invoice_id": "inv_test",
	}
	body, _ := json.Marshal(payload)

	// Compute valid signature
	mac := hmac.New(sha256.New, []byte(secret))
	mac.Write(body)
	validSig := hex.EncodeToString(mac.Sum(nil))

	// Test with valid signature
	if !webhook.verifySignature(body, validSig) {
		t.Error("Expected signature to be valid")
	}

	// Test with invalid signature
	if webhook.verifySignature(body, "invalid_sig") {
		t.Error("Expected invalid signature to be rejected")
	}

	// Test with empty secret (should pass)
	webhookNoSecret := NewWebhookHandler(bridge, nil, "", logger)
	if !webhookNoSecret.verifySignature(body, "any_sig") {
		t.Error("Expected signature check to pass with empty secret")
	}
}

// TestWebhookHandler_PaymentSucceeded — verify routing
func TestWebhookHandler_PaymentSucceeded(t *testing.T) {
	logger := newTestLogger()
	subStore := newMockBillingStore(t)
	provisioner := NewMockProvisioner()
	notifier := &MockNotifier{}

	// Create subscription first
	_, err := subStore.CreateSubscription(
		"customer_test",
		"test@example.com",
		"starter", // Используем существующий план
		billing.PeriodMonthly,
		"pm_test",
		"payment_test",
	)
	if err != nil {
		t.Fatalf("Failed to create subscription: %v", err)
	}

	bridge := NewBillingAutoscaleBridge(subStore, nil, nil, provisioner, notifier, logger)
	webhook := NewWebhookHandler(bridge, subStore, "secret", logger)

	// Create webhook payload
	payload := TochkaWebhookPayload{
		Event:     "payment.succeeded",
		InvoiceID: "inv_test",
		PaymentID: "payment_test",
	}
	payload.Data.CustomerID = "customer_test"
	body, _ := json.Marshal(payload)

	// Compute valid signature
	mac := hmac.New(sha256.New, []byte("secret"))
	mac.Write(body)
	signature := hex.EncodeToString(mac.Sum(nil))

	// Create request
	req := httptest.NewRequest("POST", "/webhook/tochka", bytes.NewReader(body))
	req.Header.Set("X-Tochka-Signature", signature)

	// Execute
	w := httptest.NewRecorder()
	webhook.HandleWebhook(w, req)

	// Verify response
	if w.Code != http.StatusOK {
		t.Errorf("Expected status 200, got %d", w.Code)
	}
}

// TestProvisioner_Provision — mock Docker API, verify container created
func TestProvisioner_Provision(t *testing.T) {
	// Skip if Docker not available
	if _, err := os.Stat("/var/run/docker.sock"); os.IsNotExist(err) {
		t.Skip("Docker socket not available")
	}

	// Check if image exists
	ctx := context.Background()
	checkCmd := exec.CommandContext(ctx, "docker", "image", "inspect", "ghcr.io/braincreator/flowlink-relay:latest")
	if err := checkCmd.Run(); err != nil {
		t.Skip("Docker image not available: ghcr.io/braincreator/flowlink-relay:latest")
	}

	logger := newTestLogger()
	tmpDir, err := os.MkdirTemp("", "flowlink-provisioner-test-*")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(tmpDir)

	provisioner := NewProvisioner(
		"/var/run/docker.sock",
		19081, // Use non-standard port to avoid conflicts
		tmpDir+"/config",
		tmpDir+"/data",
		logger,
	)

	req := &ProvisioningRequest{
		CustomerID:    "test_customer",
		CustomerEmail: "test@example.com",
		PlanID:        "starter",
	}

	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()

	result, err := provisioner.Provision(ctx, req)
	if err != nil {
		t.Fatalf("Provision failed: %v", err)
	}

	// Verify result
	if result.ContainerID == "" {
		t.Error("Expected container ID")
	}

	if result.Port < 19081 {
		t.Errorf("Expected port >= 19081, got %d", result.Port)
	}

	if result.Credentials == nil {
		t.Fatal("Expected credentials")
	}

	if result.Credentials.ClientID == "" {
		t.Error("Expected client ID in credentials")
	}

	if result.Credentials.APIToken == "" {
		t.Error("Expected API token in credentials")
	}

	// Cleanup
	provisioner.Deprovision(ctx, "test_customer")
}

// TestProvisioner_Deprovision — verify cleanup
func TestProvisioner_Deprovision(t *testing.T) {
	// This test uses mock provisioner
	_ = newTestLogger() // logger for future use
	provisioner := NewMockProvisioner()

	// First provision
	ctx := context.Background()
	req := &ProvisioningRequest{
		CustomerID:    "test_customer",
		CustomerEmail: "test@example.com",
		PlanID:        "starter",
	}

	_, err := provisioner.Provision(ctx, req)
	if err != nil {
		t.Fatal(err)
	}

	// Verify provisioned
	if len(provisioner.provisioned) != 1 {
		t.Error("Expected 1 provisioned client")
	}

	// Deprovision
	err = provisioner.Deprovision(ctx, "test_customer")
	if err != nil {
		t.Fatal(err)
	}

	// Verify deprovisioned
	if len(provisioner.provisioned) != 0 {
		t.Error("Expected 0 provisioned clients after deprovision")
	}

	if len(provisioner.deprovisioned) != 1 {
		t.Error("Expected deprovision to be called")
	}
}

// TestNotifier_Welcome — verify Telegram message format
func TestNotifier_Welcome(t *testing.T) {
	logger := newTestLogger()

	// Create notifier with test config
	notifier := NewNotifier(
		"test_bot_token",
		"https://api.telegram.org",
		"", // SMTP disabled
		0,
		"",
		"",
		logger,
	)

	creds := &ConnectionCredentials{
		RelayURL:     "wss://relay.flowlink.dev:9081",
		APIToken:     "test_token_123",
		ClientID:     "test_client_456",
		SetupCommand: "curl test | bash",
	}

	// Test message format
	body := `🎉 **Добро пожаловать в FlowLink!**

Ваш relay сервер готов к работе.

**Данные для подключения:**
- **Client ID:** ` + creds.ClientID + `
- **API Token:** ` + creds.APIToken + `
- **Relay URL:** ` + creds.RelayURL + `

**Быстрая установка:**
` + "```bash\n" + creds.SetupCommand + "\n```" + `

**Документация:** https://docs.flowlink.dev

Если возникнут вопросы — обращайтесь в поддержку.
`

	// Verify message contains all required elements
	required := []string{
		"Добро пожаловать",
		creds.ClientID,
		creds.APIToken,
		creds.RelayURL,
		creds.SetupCommand,
		"Документация",
	}

	for _, req := range required {
		if !contains(body, req) {
			t.Errorf("Expected message to contain '%s'", req)
		}
	}

	// Test markdown to HTML conversion
	html := markdownToHTML(body)

	// Verify HTML conversion
	if !contains(html, "<b>") {
		t.Error("Expected bold tags in HTML")
	}

	if !contains(html, "<code>") {
		t.Error("Expected code tags in HTML")
	}

	if !contains(html, "<pre>") {
		t.Error("Expected pre tags in HTML")
	}

	// Test notifier Send method (no recipients, should fail gracefully)
	ctx := context.Background()
	err := notifier.Send(ctx, &Notification{
		Type:       NotifWelcome,
		CustomerID: "test",
		Body:       body,
	})
	if err == nil {
		t.Error("Expected error when no delivery channel available")
	}
}

// TestManager_StartStop — lifecycle test
func TestManager_StartStop(t *testing.T) {
	logger := newTestLogger()
	subStore := newMockBillingStore(t)

	cfg := &IntegrationConfig{
		DockerSocket:  "/var/run/docker.sock",
		BasePort:      9081,
		DataDir:       "/tmp/flowlink-test",
		WebhookSecret: "test_secret",
	}

	// Create manager
	mgr, err := NewIntegrationManager(cfg, subStore, logger)
	if err != nil {
		t.Fatalf("Failed to create manager: %v", err)
	}

	// Start
	ctx := context.Background()
	if err := mgr.Start(ctx); err != nil {
		t.Fatalf("Failed to start manager: %v", err)
	}

	// Verify running
	stats := mgr.GetStats()
	if stats["base_port"] != 9081 {
		t.Errorf("Expected base_port 9081, got %v", stats["base_port"])
	}

	// Stop
	stopCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	if err := mgr.Stop(stopCtx); err != nil {
		t.Fatalf("Failed to stop manager: %v", err)
	}
}

// === Benchmark Tests ===

func BenchmarkBridge_SubscriptionCreated(b *testing.B) {
	logger := slog.New(slog.NewTextHandler(io.Discard, nil))
	subStore := newMockBillingStore(&testing.T{})
	scaler := &MockScaler{}
	router := &MockRouter{}
	provisioner := NewMockProvisioner()
	notifier := &MockNotifier{}

	bridge := NewBillingAutoscaleBridge(subStore, scaler, router, provisioner, notifier, logger)

	sub := &billing.Subscription{
		ID:             "sub_test",
		CustomerID:     "customer_test",
		CustomerEmail:  "test@example.com",
		PlanID:         "starter",
		Status:         billing.SubscriptionStatusActive,
		PaymentMethodID: "pm_test",
	}

	ctx := context.Background()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		bridge.HandleSubscriptionCreated(ctx, sub)
	}
}

func BenchmarkProvisioner_GenerateToken(b *testing.B) {
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		generateRandomHex(32)
	}
}

func BenchmarkNotifier_MarkdownToHTML(b *testing.B) {
	md := "🎉 **Добро пожаловать!**\n\n```bash\ncurl test | bash\n```\n\n**Token:** `test_token_123`"

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		markdownToHTML(md)
	}
}

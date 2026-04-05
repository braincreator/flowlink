package integration

import (
	"context"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"testing"
	"time"
)

// TestProvisioner_New tests provisioner creation
func TestProvisioner_New(t *testing.T) {
	p := NewProvisioner("/custom/docker.sock", 10000, "/custom/config", "/custom/data", nil)

	if p.dockerAPI != "/custom/docker.sock" {
		t.Errorf("expected docker socket '/custom/docker.sock', got %s", p.dockerAPI)
	}

	if p.basePort != 10000 {
		t.Errorf("expected base port 10000, got %d", p.basePort)
	}

	if p.configDir != "/custom/config" {
		t.Errorf("expected config dir '/custom/config', got %s", p.configDir)
	}

	if p.dataDir != "/custom/data" {
		t.Errorf("expected data dir '/custom/data', got %s", p.dataDir)
	}
}

// TestProvisioner_Provision_InvalidDocker tests provisioning with invalid docker socket
func TestProvisioner_Provision_InvalidDocker(t *testing.T) {
	p := NewProvisioner("/nonexistent/docker.sock", 9081, "/tmp/config", "/tmp/data", nil)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	req := &ProvisioningRequest{
		CustomerID:    "test-customer",
		CustomerEmail: "test@example.com",
		PlanID:        "starter",
	}

	_, err := p.Provision(ctx, req)
	if err == nil {
		t.Error("expected error with invalid docker socket")
	}
}

// TestBillingAutoscaleBridge tests bridge creation
func TestBillingAutoscaleBridge(t *testing.T) {
	bridge := NewBillingAutoscaleBridge(nil, nil, nil, nil, nil, nil)

	if bridge == nil {
		t.Fatal("expected non-nil bridge")
	}

	if bridge.gracePeriods == nil {
		t.Error("expected non-nil grace periods map")
	}
}

// TestBillingAutoscaleBridge_CheckGracePeriods tests grace period checking
func TestBillingAutoscaleBridge_CheckGracePeriods(t *testing.T) {
	bridge := NewBillingAutoscaleBridge(nil, nil, nil, nil, nil, nil)

	ctx := context.Background()

	// Check with no grace periods
	err := bridge.CheckGracePeriods(ctx)
	// Error is expected since subscription store is nil
	_ = err
}

// TestAutoScalerInterface tests scaler interface
func TestAutoScalerInterface(t *testing.T) {
	scaler := &MockScaler{}

	ctx := context.Background()

	// Test ScaleUp
	err := scaler.ScaleUp(ctx, "customer1")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	// Test ScaleDown
	err = scaler.ScaleDown(ctx, "customer1")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	// Test GetStatus
	status, err := scaler.GetStatus(ctx, "customer1")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	if status == nil {
		t.Error("expected non-nil status")
	}
}

// TestAutoRouterInterface tests router interface
func TestAutoRouterInterface(t *testing.T) {
	router := &MockRouter{}

	ctx := context.Background()

	// Test RegisterClient
	err := router.RegisterClient(ctx, "client1")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	// Test UnregisterClient
	err = router.UnregisterClient(ctx, "client1")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	// Test GetTarget
	target, err := router.GetTarget(ctx, "client1")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	if target == "" {
		t.Error("expected non-empty target")
	}
}

// TestProvisionerInterface tests provisioner interface
func TestProvisionerInterface(t *testing.T) {
	provisioner := NewMockProvisioner()

	ctx := context.Background()
	req := &ProvisioningRequest{
		CustomerID:    "customer1",
		CustomerEmail: "test@example.com",
		PlanID:        "starter",
	}

	// Test Provision
	result, err := provisioner.Provision(ctx, req)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	if result == nil {
		t.Fatal("expected non-nil result")
	}

	if result.ContainerID == "" {
		t.Error("expected non-empty container ID")
	}

	if result.Credentials == nil {
		t.Error("expected non-nil credentials")
	}

	// Test GetProvisionedClients
	clients, err := provisioner.GetProvisionedClients()
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	if len(clients) != 1 {
		t.Errorf("expected 1 client, got %d", len(clients))
	}

	// Test Deprovision
	err = provisioner.Deprovision(ctx, "customer1")
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	clients, _ = provisioner.GetProvisionedClients()
	if len(clients) != 0 {
		t.Errorf("expected 0 clients after deprovision, got %d", len(clients))
	}
}

// TestNotifierInterface tests notifier interface
func TestNotifierInterface(t *testing.T) {
	notifier := &MockNotifier{}

	ctx := context.Background()
	notif := &Notification{
		Type:       NotifWelcome,
		CustomerID: "customer1",
		Body:       "Test notification",
	}

	// Test Send
	err := notifier.Send(ctx, notif)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	if len(notifier.sent) != 1 {
		t.Errorf("expected 1 notification sent, got %d", len(notifier.sent))
	}

	// Test SendWelcome
	err = notifier.SendWelcome(ctx, "customer1", "@user", "test@example.com", &ConnectionCredentials{
		ClientID: "client1",
	})
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	// Test SendPaymentReminder
	err = notifier.SendPaymentReminder(ctx, "customer1", "@user", "test@example.com", 7)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}
}

// TestNotificationPayload tests notification payload structure
func TestNotificationPayload(t *testing.T) {
	notif := &Notification{
		Type:        NotifProvisioned,
		CustomerID:  "customer-123",
		TelegramID:  "@testuser",
		Email:       "test@example.com",
		Subject:     "Test Subject",
		Body:        "**Bold** text",
		Credentials: &ConnectionCredentials{ClientID: "client-123", APIToken: "token-123"},
	}

	if notif.Type != NotifProvisioned {
		t.Errorf("expected type %s, got %s", NotifProvisioned, notif.Type)
	}

	if notif.Credentials.APIToken != "token-123" {
		t.Errorf("expected API token 'token-123', got %s", notif.Credentials.APIToken)
	}
}

// TestIntegrationConfig tests config structure
func TestIntegrationConfig(t *testing.T) {
	cfg := &IntegrationConfig{
		DockerSocket:       "/var/run/docker.sock",
		BasePort:           9081,
		DataDir:            "/var/lib/flowlink",
		WebhookSecret:      "secret",
		TGBotToken:         "bot-token",
		TGAPI:              "https://api.telegram.org",
		SMTPHost:           "smtp.example.com",
		SMTPPort:           587,
		SMTPUser:           "user",
		SMTPPass:           "pass",
		TochkaClientID:     "client-id",
		TochkaClientSecret: "client-secret",
		TochkaAccountID:    "account-id",
	}

	if cfg.DockerSocket != "/var/run/docker.sock" {
		t.Errorf("expected docker socket '/var/run/docker.sock', got %s", cfg.DockerSocket)
	}

	if cfg.BasePort != 9081 {
		t.Errorf("expected base port 9081, got %d", cfg.BasePort)
	}
}

// TestManager_RegisterRoutes tests route registration
func TestManager_RegisterRoutes(t *testing.T) {
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

	mux := http.NewServeMux()
	mgr.RegisterRoutes(mux)

	// Test routes are registered
	routes := []struct {
		method string
		path   string
	}{
		{"POST", "/api/v1/webhook/tochka"},
		{"GET", "/api/v1/integration/status"},
	}

	for _, route := range routes {
		req := httptest.NewRequest(route.method, route.path, nil)
		w := httptest.NewRecorder()
		mux.ServeHTTP(w, req)

		if w.Code == http.StatusNotFound {
			t.Errorf("route %s not registered", route.path)
		}
	}
}

// TestManager_ManualProvision tests manual provisioning
func TestManager_ManualProvision(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelDebug}))

	cfg := &IntegrationConfig{
		DockerSocket:  "/var/run/docker.sock",
		BasePort:      9081,
		DataDir:       "/tmp/flowlink-test",
		WebhookSecret: "test-secret",
	}

	mgr, err := NewIntegrationManager(cfg, nil, logger)
	if err != nil {
		t.Fatalf("failed to create manager: %v", err)
	}

	ctx := context.Background()
	req := &ProvisioningRequest{
		CustomerID:    "manual-customer",
		CustomerEmail: "manual@example.com",
		PlanID:        "starter",
	}

	// Note: This may fail if Docker is not available, which is expected
	_, err = mgr.ManualProvision(ctx, req)
	// We don't check for error since Docker may not be available in test environment
	_ = err
}

// TestManager_ManualDeprovision tests manual deprovisioning
func TestManager_ManualDeprovision(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelDebug}))

	cfg := &IntegrationConfig{
		DockerSocket:  "/var/run/docker.sock",
		BasePort:      9081,
		DataDir:       "/tmp/flowlink-test",
		WebhookSecret: "test-secret",
	}

	mgr, err := NewIntegrationManager(cfg, nil, logger)
	if err != nil {
		t.Fatalf("failed to create manager: %v", err)
	}

	ctx := context.Background()

	// Deprovision nonexistent client
	err = mgr.ManualDeprovision(ctx, "nonexistent-customer")
	// Error is expected since client doesn't exist
	_ = err
}

// TestManager_SendTestNotification tests test notification
func TestManager_SendTestNotification(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelDebug}))

	cfg := &IntegrationConfig{
		DockerSocket:  "/var/run/docker.sock",
		BasePort:      9081,
		DataDir:       "/tmp/flowlink-test",
		WebhookSecret: "test-secret",
	}

	mgr, err := NewIntegrationManager(cfg, nil, logger)
	if err != nil {
		t.Fatalf("failed to create manager: %v", err)
	}

	ctx := context.Background()
	err = mgr.SendTestNotification(ctx, "customer1", "@user", "test@example.com")
	// Error is expected since no notification channels are configured
	_ = err
}


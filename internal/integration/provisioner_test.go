package integration

import (
	"context"
	"testing"
	"time"
)

// TestGenerateRandomHex tests random hex generation
func TestGenerateRandomHex(t *testing.T) {
	tests := []struct {
		name   string
		length int
		want   int // expected string length
	}{
		{"16 bytes", 16, 32}, // 16 bytes = 32 hex chars
		{"32 bytes", 32, 64}, // 32 bytes = 64 hex chars
		{"1 byte", 1, 2},
		{"0 bytes", 0, 0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := generateRandomHex(tt.length)
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}

			if len(result) != tt.want {
				t.Errorf("expected length %d, got %d", tt.want, len(result))
			}

			// Generate another and verify they're different
			result2, err := generateRandomHex(tt.length)
			if err != nil {
				t.Fatalf("unexpected error on second generation: %v", err)
			}

			if result == result2 && tt.length > 0 {
				t.Error("expected different random values")
			}
		})
	}
}

// TestBuffer tests Buffer implementation
func TestBuffer(t *testing.T) {
	data := []byte("test data for buffer")
	buf := NewBuffer(data)

	// Test reading all data
	result := make([]byte, len(data)+10) // Extra space
	n, err := buf.Read(result)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if n != len(data) {
		t.Errorf("expected %d bytes read, got %d", len(data), n)
	}

	if string(result[:n]) != string(data) {
		t.Errorf("expected %s, got %s", string(data), string(result[:n]))
	}

	// Test reading after EOF
	n, err = buf.Read(result)
	if err == nil {
		t.Error("expected EOF error on second read")
	}
	if n != 0 {
		t.Errorf("expected 0 bytes on EOF, got %d", n)
	}
}

// TestBuffer_PartialRead tests partial reads
func TestBuffer_PartialRead(t *testing.T) {
	data := []byte("0123456789")
	buf := NewBuffer(data)

	// Read first 3 bytes
	part1 := make([]byte, 3)
	n, err := buf.Read(part1)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if n != 3 {
		t.Fatalf("expected 3 bytes, got %d", n)
	}
	if string(part1) != "012" {
		t.Errorf("expected '012', got %s", string(part1))
	}

	// Read next 4 bytes
	part2 := make([]byte, 4)
	n, err = buf.Read(part2)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if n != 4 {
		t.Fatalf("expected 4 bytes, got %d", n)
	}
	if string(part2) != "3456" {
		t.Errorf("expected '3456', got %s", string(part2))
	}

	// Read remaining
	part3 := make([]byte, 10)
	n, err = buf.Read(part3)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if n != 3 {
		t.Fatalf("expected 3 bytes, got %d", n)
	}
	if string(part3[:n]) != "789" {
		t.Errorf("expected '789', got %s", string(part3[:n]))
	}
}

// TestProvisioner_IsPortUsed tests port allocation
func TestProvisioner_IsPortUsed(t *testing.T) {
	p := &Provisioner{
		clients: make(map[string]*ProvisionedClient),
	}

	// No clients yet
	if p.isPortUsed(9081) {
		t.Error("port should not be used initially")
	}

	// Add a client
	p.clients["customer1"] = &ProvisionedClient{
		CustomerID: "customer1",
		Port:        9081,
	}

	// Now port should be used
	if !p.isPortUsed(9081) {
		t.Error("port 9081 should be used")
	}

	// Different port should not be used
	if p.isPortUsed(9082) {
		t.Error("port 9082 should not be used")
	}

	// Add another client
	p.clients["customer2"] = &ProvisionedClient{
		CustomerID: "customer2",
		Port:        9082,
	}

	if !p.isPortUsed(9082) {
		t.Error("port 9082 should be used")
	}
}

// TestProvisioner_GetProvisionedClients tests client listing
func TestProvisioner_GetProvisionedClients(t *testing.T) {
	p := &Provisioner{
		clients: make(map[string]*ProvisionedClient),
	}

	// Empty list
	clients, err := p.GetProvisionedClients()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(clients) != 0 {
		t.Errorf("expected 0 clients, got %d", len(clients))
	}

	// Add clients
	p.clients["customer1"] = &ProvisionedClient{
		CustomerID: "customer1",
		Port:        9081,
		Status:      "running",
		CreatedAt:   time.Now(),
	}

	p.clients["customer2"] = &ProvisionedClient{
		CustomerID: "customer2",
		Port:        9082,
		Status:      "stopped",
		CreatedAt:   time.Now(),
	}

	clients, err = p.GetProvisionedClients()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(clients) != 2 {
		t.Errorf("expected 2 clients, got %d", len(clients))
	}
}

// TestProvisioner_Deprovision_NonExistent tests deprovisioning non-existent client
func TestProvisioner_Deprovision_NonExistent(t *testing.T) {
	p := NewProvisioner("", 0, "", "", nil)

	ctx := context.Background()
	err := p.Deprovision(ctx, "nonexistent")

	if err == nil {
		t.Error("expected error for non-existent client")
	}
}

// TestProvisioningRequest tests request validation
func TestProvisioningRequest(t *testing.T) {
	req := &ProvisioningRequest{
		CustomerID:     "customer-123",
		CustomerEmail:  "test@example.com",
		PlanID:         "starter",
		SubscriptionID: "sub-123",
	}

	if req.CustomerID != "customer-123" {
		t.Errorf("expected customer ID 'customer-123', got %s", req.CustomerID)
	}

	if req.CustomerEmail != "test@example.com" {
		t.Errorf("expected email 'test@example.com', got %s", req.CustomerEmail)
	}
}

// TestProvisioningResult tests result structure
func TestProvisioningResult(t *testing.T) {
	result := &ProvisioningResult{
		ContainerID: "container-123",
		Port:        9081,
		HealthURL:   "http://localhost:9081/health",
		SetupTime:   5 * time.Second,
		Credentials: &ConnectionCredentials{
			RelayURL:     "wss://relay.flowlink.dev:9081",
			APIToken:     "test-token",
			ClientID:     "client-123",
			SetupCommand: "curl test | bash",
		},
	}

	if result.ContainerID != "container-123" {
		t.Errorf("expected container ID 'container-123', got %s", result.ContainerID)
	}

	if result.Port != 9081 {
		t.Errorf("expected port 9081, got %d", result.Port)
	}

	if result.Credentials == nil {
		t.Error("expected non-nil credentials")
	}

	if result.Credentials.ClientID != "client-123" {
		t.Errorf("expected client ID 'client-123', got %s", result.Credentials.ClientID)
	}
}

// TestConnectionCredentials tests credentials structure
func TestConnectionCredentials(t *testing.T) {
	creds := &ConnectionCredentials{
		RelayURL:     "wss://relay.flowlink.dev:9081",
		APIToken:     "test-api-token",
		ClientID:     "test-client-id",
		SetupCommand: "curl -sSL https://get.flowlink.dev/install.sh | bash",
	}

	if creds.RelayURL == "" {
		t.Error("relay URL should not be empty")
	}

	if creds.APIToken == "" {
		t.Error("API token should not be empty")
	}

	if creds.ClientID == "" {
		t.Error("client ID should not be empty")
	}

	if creds.SetupCommand == "" {
		t.Error("setup command should not be empty")
	}
}

// TestProvisionedClient tests client structure
func TestProvisionedClient(t *testing.T) {
	client := &ProvisionedClient{
		CustomerID:  "customer-123",
		ContainerID: "container-456",
		Port:        9081,
		Status:      "running",
		CreatedAt:   time.Now(),
		PlanID:      "starter",
	}

	if client.CustomerID != "customer-123" {
		t.Errorf("expected customer ID 'customer-123', got %s", client.CustomerID)
	}

	if client.Status != "running" {
		t.Errorf("expected status 'running', got %s", client.Status)
	}
}

// TestNewProvisionerDefaults tests provisioner defaults
func TestNewProvisionerDefaults(t *testing.T) {
	p := NewProvisioner("", 0, "", "", nil)

	if p.dockerAPI != "/var/run/docker.sock" {
		t.Errorf("expected default docker socket, got %s", p.dockerAPI)
	}

	if p.basePort != 9081 {
		t.Errorf("expected default port 9081, got %d", p.basePort)
	}

	if p.clients == nil {
		t.Error("expected non-nil clients map")
	}
}

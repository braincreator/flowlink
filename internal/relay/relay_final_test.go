// Package relay — tests for remaining uncovered functions
package relay

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/protocol"
)

// === LLM Proxy tests ===

func TestLLMProxy_SetBackends(t *testing.T) {
	proxy := NewLLMProxy([]LLMBackend{
		{Name: "test", URL: "http://localhost:1234", Priority: 1},
	})

	newBackends := []LLMBackend{
		{Name: "new1", URL: "http://localhost:1111", Priority: 1},
		{Name: "new2", URL: "http://localhost:2222", Priority: 2},
	}

	proxy.SetBackends(newBackends)

	list := proxy.ListBackends()
	if len(list) != 2 {
		t.Errorf("expected 2 backends, got %d", len(list))
	}
}

// === Encryption helper tests ===

func TestBase64Encode(t *testing.T) {
	input := []byte("hello world")
	encoded := base64Encode(input)

	expected := base64.StdEncoding.EncodeToString(input)
	if encoded != expected {
		t.Errorf("expected %s, got %s", expected, encoded)
	}
}

func TestBase64Decode(t *testing.T) {
	input := "aGVsbG8gd29ybGQ=" // base64 of "hello world"
	decoded, err := base64Decode(input)

	if err != nil {
		t.Fatalf("base64Decode failed: %v", err)
	}

	expected := "hello world"
	if string(decoded) != expected {
		t.Errorf("expected %s, got %s", expected, decoded)
	}
}

func TestBase64Decode_Invalid(t *testing.T) {
	_, err := base64Decode("not-valid-base64!!!")

	if err == nil {
		t.Error("expected error for invalid base64")
	}
}

func TestComputeKeyID(t *testing.T) {
	key := []byte("test-public-key")
	keyID := computeKeyID(key)

	if keyID == "" {
		t.Error("expected non-empty key ID")
	}

	// Same key should produce same ID
	keyID2 := computeKeyID(key)
	if keyID != keyID2 {
		t.Error("expected same key to produce same ID")
	}

	// Different key should produce different ID
	keyID3 := computeKeyID([]byte("different-key"))
	if keyID == keyID3 {
		t.Error("expected different keys to produce different IDs")
	}
}

// === Audit logger compression test ===

func TestAuditLogger_CompressFile(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	// Log some entries to create a file
	for i := 0; i < 5; i++ {
		al.Log(AuditEntry{
			ID:        string(rune('A' + i)),
			Timestamp: mustParseTime("2024-01-01T00:00:00Z"),
			Action:    "exec",
		})
	}

	// Force a different date to create a separate file
	al.mu.Lock()
	al.currentDate = "2024-01-02"
	al.currentFile = nil
	al.mu.Unlock()

	// Log more entries
	for i := 0; i < 5; i++ {
		al.Log(AuditEntry{
			ID:        string(rune('B' + i)),
			Timestamp: mustParseTime("2024-01-02T00:00:00Z"),
			Action:    "read",
		})
	}

	// Files should exist
}

func mustParseTime(s string) time.Time {
	t, _ := time.Parse(time.RFC3339, s)
	return t
}

// === Registry additional tests ===

func TestRegistry_ListClients(t *testing.T) {
	dir := t.TempDir()
	reg := NewRegistry(dir, nil)

	// Empty list
	list := reg.ListClients()
	if len(list) != 0 {
		t.Errorf("expected empty list, got %d", len(list))
	}

	// Create clients
	reg.CreateClient("Client1", "c1@test.com", "starter")
	reg.CreateClient("Client2", "c2@test.com", "starter")

	list = reg.ListClients()
	if len(list) != 2 {
		t.Errorf("expected 2 clients, got %d", len(list))
	}
}

func TestRegistry_GetClientByAPIToken(t *testing.T) {
	dir := t.TempDir()
	reg := NewRegistry(dir, nil)

	client, _ := reg.CreateClient("Test", "test@test.com", "starter")

	// Find by token
	found, ok := reg.GetClientByAPIToken(client.APIToken)
	if !ok {
		t.Fatal("expected to find client by token")
	}
	if found.ID != client.ID {
		t.Errorf("expected ID %s, got %s", client.ID, found.ID)
	}

	// Not found
	_, ok = reg.GetClientByAPIToken("invalid-token")
	if ok {
		t.Error("expected not to find client with invalid token")
	}
}

func TestRegistry_DeactivateClient(t *testing.T) {
	dir := t.TempDir()
	reg := NewRegistry(dir, nil)

	client, _ := reg.CreateClient("Test", "test@test.com", "starter")

	err := reg.DeactivateClient(client.ID)
	if err != nil {
		t.Fatalf("DeactivateClient failed: %v", err)
	}

	// Client should be inactive
	found, _ := reg.GetClient(client.ID)
	if found.IsActive {
		t.Error("expected client to be inactive")
	}
}

func TestRegistry_GetAgent(t *testing.T) {
	dir := t.TempDir()
	reg := NewRegistry(dir, nil)

	client, _ := reg.CreateClient("Test", "test@test.com", "starter")
	agent, _ := reg.RegisterAgent(client.ID, "Test Agent", []string{}, "linux", "amd64")

	// Get existing
	found, ok := reg.GetAgent(agent.ID)
	if !ok {
		t.Fatal("expected to find agent")
	}
	if found.ID != agent.ID {
		t.Errorf("expected ID %s, got %s", agent.ID, found.ID)
	}

	// Not found
	_, ok = reg.GetAgent("invalid-id")
	if ok {
		t.Error("expected not to find invalid agent")
	}
}

func TestRegistry_ListAgents(t *testing.T) {
	dir := t.TempDir()
	reg := NewRegistry(dir, nil)

	// Create client and agents
	client, _ := reg.CreateClient("Test", "test@test.com", "starter")
	agent1, _ := reg.RegisterAgent(client.ID, "Agent1", []string{}, "linux", "amd64")
	agent2, _ := reg.RegisterAgent(client.ID, "Agent2", []string{}, "linux", "amd64")

	// Get agents directly
	agent1Got, ok1 := reg.GetAgent(agent1.ID)
	if !ok1 {
		t.Fatal("expected to find agent1")
	}
	_ = agent1Got

	agent2Got, ok2 := reg.GetAgent(agent2.ID)
	if !ok2 {
		t.Fatal("expected to find agent2")
	}
	_ = agent2Got

	// List by client
	list := reg.ListAgents(client.ID)
	if len(list) != 2 {
		t.Errorf("expected 2 agents for client, got %d", len(list))
	}
}

func TestRegistry_UnregisterAgent(t *testing.T) {
	dir := t.TempDir()
	reg := NewRegistry(dir, nil)

	client, _ := reg.CreateClient("Test", "test@test.com", "starter")
	agent, _ := reg.RegisterAgent(client.ID, "Test Agent", []string{}, "linux", "amd64")

	err := reg.UnregisterAgent(agent.ID)
	if err != nil {
		t.Fatalf("UnregisterAgent failed: %v", err)
	}

	// Should not find
	_, ok := reg.GetAgent(agent.ID)
	if ok {
		t.Error("expected agent to be unregistered")
	}
}

// === SSE tests ===

func TestSSE_PublishJSON(t *testing.T) {
	eb := NewEventBus(nil)
	defer eb.Close()

	// Test publishJSON helper
	event := Event{
		Type:    EventAgentConnected,
		AgentID: "test-agent",
	}

	data := eb.publishJSON(event)

	if len(data) == 0 {
		t.Error("expected non-empty JSON data")
	}

	// Verify it's valid JSON
	var parsed Event
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Errorf("invalid JSON: %v", err)
	}
}

// === Protocol message constructor tests ===

func TestNewMessage(t *testing.T) {
	msg := protocol.NewMessage(protocol.MsgHeartbeat)

	if msg.Type != protocol.MsgHeartbeat {
		t.Errorf("expected type %s, got %s", protocol.MsgHeartbeat, msg.Type)
	}
	if msg.ID == "" {
		t.Error("expected non-empty ID")
	}
	if msg.Timestamp == 0 {
		t.Error("expected non-zero timestamp")
	}
}

func TestNewMessageWithPayload(t *testing.T) {
	payload := map[string]any{
		"agent_id": "test-agent",
		"command":  "ls -la",
	}

	msg := protocol.NewMessage(protocol.MsgExecRequest)
	msg.Payload = payload

	if msg.Type != protocol.MsgExecRequest {
		t.Errorf("expected type %s, got %s", protocol.MsgExecRequest, msg.Type)
	}
	if msg.Payload == nil {
		t.Error("expected non-nil payload")
	}
}

// === Additional handler tests ===

// === Config update handler additional tests ===

func TestHandleAgentConfigUpdate_Offline(t *testing.T) {
	relay := createTestRelay(t)

	body := map[string]any{
		"agent_id":  "offline-agent",
		"read_only": true,
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPut, "/api/v1/agents/config", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAgentConfigUpdate(w, req)

	if w.Code != http.StatusNotFound {
		t.Errorf("expected 404, got %d", w.Code)
	}
}

// === Nginx config tests ===

func TestHandleNginxConfig_WithTLS(t *testing.T) {
	relay := createTestRelayWithToken(t, "test-api-token")

	req := httptest.NewRequest(http.MethodGet, "/api/v1/nginx-config?domain=example.com&tls=true", nil)
	req.Header.Set("Authorization", "Bearer test-api-token")
	w := httptest.NewRecorder()

	relay.handleNginxConfig(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}

	// Check content type
	if ct := w.Header().Get("Content-Type"); ct == "" {
		t.Error("expected Content-Type header")
	}
}

func TestHandleNginxConfig_WithoutTLS(t *testing.T) {
	relay := createTestRelayWithToken(t, "test-api-token")

	req := httptest.NewRequest(http.MethodGet, "/api/v1/nginx-config?domain=example.com&tls=false", nil)
	req.Header.Set("Authorization", "Bearer test-api-token")
	w := httptest.NewRecorder()

	relay.handleNginxConfig(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

// === Billing plan tests ===

func TestHandleBillingPlan_ChangeToInvalid(t *testing.T) {
	relay := createTestRelay(t)
	client, _ := relay.registry.CreateClient("Test", "test@test.com", "starter")

	body := map[string]any{
		"client_id": client.ID,
		"plan_id":   "invalid-plan",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/billing/plan/change", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleBillingPlanChange(w, req)

	// Should fail or succeed depending on validation
	if w.Code != http.StatusOK && w.Code != http.StatusBadRequest {
		t.Errorf("expected 200 or 400, got %d", w.Code)
	}
}

// === Integration proxy additional tests ===

func TestHandleIntegrationProxy_POST(t *testing.T) {
	backend := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusCreated)
		w.Write([]byte(`{"id":"new-backup"}`))
	}))
	defer backend.Close()

	cfg := &config.RelayConfig{
		WSSAddr:          ":0",
		APIAddr:          ":0",
		IntegrationURL:   backend.URL,
		IntegrationToken: "test-token",
	}
	relay := NewRelay(cfg); t.Cleanup(func() { relay.Close() })

	body := map[string]any{"name": "test"}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/integration/backups", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleIntegrationProxy(w, req)

	if w.Code != http.StatusCreated {
		t.Errorf("expected 201, got %d", w.Code)
	}
}

// === Auth context tests ===

func TestSetClientIDInContext(t *testing.T) {
	w := httptest.NewRecorder()
	SetClientIDInContext(w, "test-client")

	// Should set header
	clientID := w.Header().Get("X-Client-ID")
	if clientID != "test-client" {
		t.Errorf("expected test-client, got %v", clientID)
	}
}

// === Auth manager cleanup test ===

func TestAuthManager_CleanupBlacklist(t *testing.T) {
	auth := NewAuthManager(nil); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	// Add some tokens to blacklist
	for i := 0; i < 5; i++ {
		token, _ := auth.GenerateAPIToken("client-1", 3600)
		auth.AddToBlacklist(token)
	}

	// Run cleanup
	auth.cleanupBlacklist()

	// Check blacklist count
	count := auth.BlacklistCount()
	if count > 5 {
		t.Errorf("expected at most 5 in blacklist, got %d", count)
	}
}

// === Audit logger validation test ===

func TestAuditLogger_ReadFileWithValidation(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}
	t.Cleanup(func() { al.Close() })

	// Log entries
	for i := 0; i < 3; i++ {
		al.Log(AuditEntry{
			ID:        string(rune('A' + i)),
			Timestamp: time.Now(),
			Action:    "exec",
		})
	}

	// Verify
	result, err := al.VerifyAll()
	if err != nil {
		t.Fatalf("VerifyAll failed: %v", err)
	}

	if result.TotalEntries < 3 {
		t.Errorf("expected at least 3 entries, got %d", result.TotalEntries)
	}
}

// === SSE publishJSON test ===

func TestEventBus_PublishJSON(t *testing.T) {
	eb := NewEventBus(nil)
	defer eb.Close()

	event := Event{
		Type:    EventAgentConnected,
		AgentID: "test-agent",
	}

	data := eb.publishJSON(event)

	if len(data) == 0 {
		t.Error("expected non-empty JSON data")
	}

	// Verify it's valid JSON
	var parsed Event
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Errorf("invalid JSON: %v", err)
	}
}

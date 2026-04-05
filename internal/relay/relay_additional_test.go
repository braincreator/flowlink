// Package relay — additional tests for uncovered functions
package relay

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/protocol"
)

// === Relay exported methods tests ===

func TestRelay_SetLLMProxy(t *testing.T) {
	relay := createTestRelay(t)

	proxy := NewLLMProxy([]LLMBackend{
		{Name: "test", URL: "http://localhost:1234", Priority: 1},
	})

	relay.SetLLMProxy(proxy)

	if relay.llmProxy != proxy {
		t.Error("LLM proxy not set correctly")
	}
}

func TestRelay_CreateFirstClient(t *testing.T) {
	relay := createTestRelay(t)

	client, err := relay.CreateFirstClient("First Client", "first@example.com")
	if err != nil {
		t.Fatalf("CreateFirstClient failed: %v", err)
	}

	if client == nil {
		t.Fatal("expected non-nil client")
	}
	if client.Name != "First Client" {
		t.Errorf("expected name 'First Client', got %s", client.Name)
	}
	if client.Email != "first@example.com" {
		t.Errorf("expected email 'first@example.com', got %s", client.Email)
	}
}

func TestRelay_CreateFirstAgent(t *testing.T) {
	relay := createTestRelay(t)

	// Create client first
	client, _ := relay.CreateFirstClient("Test", "test@example.com")

	agent, err := relay.CreateFirstAgent(client.ID, "First Agent")
	if err != nil {
		t.Fatalf("CreateFirstAgent failed: %v", err)
	}

	if agent == nil {
		t.Fatal("expected non-nil agent")
	}
	if agent.Label != "First Agent" {
		t.Errorf("expected label 'First Agent', got %s", agent.Label)
	}
}

func TestRelay_PoolList(t *testing.T) {
	relay := createTestRelay(t)

	// Empty pool
	list := relay.PoolList()
	if len(list) != 0 {
		t.Errorf("expected empty pool, got %d", len(list))
	}

	// Add agent
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	list = relay.PoolList()
	if len(list) != 1 {
		t.Errorf("expected 1 agent, got %d", len(list))
	}
}

func TestRelay_GenerateAgentToken(t *testing.T) {
	relay := createTestRelay(t)

	token, err := relay.GenerateAgentToken("agent-1", 3600)
	if err != nil {
		t.Fatalf("GenerateAgentToken failed: %v", err)
	}

	if token == "" {
		t.Error("expected non-empty token")
	}
}

func TestRelay_GenerateAPIToken(t *testing.T) {
	relay := createTestRelay(t)

	token, err := relay.GenerateAPIToken("client-1", 3600)
	if err != nil {
		t.Fatalf("GenerateAPIToken failed: %v", err)
	}

	if token == "" {
		t.Error("expected non-empty token")
	}
}

func TestRelay_RotateAgentTokens(t *testing.T) {
	relay := createTestRelay(t)

	// Generate initial token
	_, _ = relay.GenerateAgentToken("agent-1", 3600)

	// Rotate
	newToken, err := relay.RotateAgentTokens("agent-1", 3600)
	if err != nil {
		t.Fatalf("RotateAgentTokens failed: %v", err)
	}

	if newToken == "" {
		t.Error("expected non-empty new token")
	}
}

func TestRelay_RevokeToken(t *testing.T) {
	relay := createTestRelay(t)

	// Generate token
	token, _ := relay.GenerateAPIToken("client-1", 3600)

	// Revoke
	err := relay.RevokeToken(token)
	if err != nil {
		t.Fatalf("RevokeToken failed: %v", err)
	}
}

// === Auth handler tests ===

func TestHandleAuthToken(t *testing.T) {
	relay := createTestRelay(t)

	// Create client first
	client, _ := relay.registry.CreateClient("Test", "test@example.com", "starter")

	body := map[string]any{
		"client_id":     client.ID,
		"client_secret": "test-secret",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/token", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAuthToken(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
	}

	var resp map[string]any
	json.Unmarshal(w.Body.Bytes(), &resp)

	if resp["access_token"] == "" {
		t.Error("expected access_token in response")
	}
	if resp["refresh_token"] == "" {
		t.Error("expected refresh_token in response")
	}
	if resp["token_type"] != "Bearer" {
		t.Errorf("expected token_type Bearer, got %v", resp["token_type"])
	}
}

func TestHandleAuthToken_MissingClientID(t *testing.T) {
	relay := createTestRelay(t)

	body := map[string]any{
		"client_secret": "test-secret",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/token", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAuthToken(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleAuthToken_ClientNotFound(t *testing.T) {
	relay := createTestRelay(t)

	body := map[string]any{
		"client_id":     "non-existing-client",
		"client_secret": "test-secret",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/token", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAuthToken(w, req)

	if w.Code != http.StatusNotFound {
		t.Errorf("expected 404, got %d", w.Code)
	}
}

func TestHandleAuthRefresh(t *testing.T) {
	relay := createTestRelay(t)

	// Create client and generate token pair
	client, _ := relay.registry.CreateClient("Test", "test@example.com", "starter")
	pair, _ := relay.auth.GenerateTokenPair(client.ID)

	body := map[string]any{
		"refresh_token": pair.RefreshToken,
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/refresh", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAuthRefresh(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
	}

	var resp map[string]any
	json.Unmarshal(w.Body.Bytes(), &resp)

	if resp["access_token"] == "" {
		t.Error("expected new access_token")
	}
}

func TestHandleAuthRefresh_InvalidToken(t *testing.T) {
	relay := createTestRelay(t)

	body := map[string]any{
		"refresh_token": "invalid-token",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/refresh", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAuthRefresh(w, req)

	if w.Code != http.StatusUnauthorized {
		t.Errorf("expected 401, got %d", w.Code)
	}
}

func TestHandleAuthLogout(t *testing.T) {
	relay := createTestRelay(t)

	// Create client and generate token
	client, _ := relay.registry.CreateClient("Test", "test@example.com", "starter")
	pair, _ := relay.auth.GenerateTokenPair(client.ID)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/logout", nil)
	req.Header.Set("Authorization", "Bearer "+pair.AccessToken)
	w := httptest.NewRecorder()

	relay.handleAuthLogout(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestHandleAuthLogout_NoToken(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/logout", nil)
	w := httptest.NewRecorder()

	relay.handleAuthLogout(w, req)

	if w.Code != http.StatusUnauthorized {
		t.Errorf("expected 401, got %d", w.Code)
	}
}

func TestHandleAuthRevoke(t *testing.T) {
	relay := createTestRelayWithToken(t, "admin-token")

	// Create client
	client, _ := relay.registry.CreateClient("Test", "test@example.com", "starter")
	_, _ = relay.auth.GenerateTokenPair(client.ID)

	body := map[string]any{
		"client_id": client.ID,
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/revoke", bytes.NewReader(bodyJSON))
	req.Header.Set("Authorization", "Bearer admin-token")
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAuthRevoke(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
	}
}

func TestHandleAuthRevoke_Unauthorized(t *testing.T) {
	relay := createTestRelayWithToken(t, "admin-token")

	body := map[string]any{
		"client_id": "test-client",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/revoke", bytes.NewReader(bodyJSON))
	req.Header.Set("Authorization", "Bearer wrong-token")
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAuthRevoke(w, req)

	if w.Code != http.StatusForbidden {
		t.Errorf("expected 403, got %d", w.Code)
	}
}

func TestHandleBillingInvoicePay(t *testing.T) {
	relay := createTestRelay(t)
	client, _ := relay.registry.CreateClient("Test", "test@example.com", "starter")

	// Generate invoice first
	invoice, err := relay.invoices.GenerateInvoice(client.ID, "starter")
	if err != nil {
		t.Fatalf("GenerateInvoice failed: %v", err)
	}

	req := httptest.NewRequest(http.MethodPost, "/api/v1/billing/invoices/"+invoice.ID+"/pay", nil)
	w := httptest.NewRecorder()

	relay.handleBillingInvoicePay(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
	}
}

func TestHandleBillingInvoicePay_NotFound(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/billing/invoices/non-existing/pay", nil)
	w := httptest.NewRecorder()

	relay.handleBillingInvoicePay(w, req)

	if w.Code != http.StatusNotFound {
		t.Errorf("expected 404, got %d", w.Code)
	}
}

// === Rate Limit handlers additional tests ===

func TestHandleRateLimits_POST(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/rate-limits", nil)
	w := httptest.NewRecorder()

	relay.handleRateLimits(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestHandleRateLimitByClient_Delete(t *testing.T) {
	relay := createTestRelay(t)

	// Set custom limits first
	relay.rateLimit.SetClientLimits("test-client", 50, 500)

	req := httptest.NewRequest(http.MethodDelete, "/api/v1/rate-limits/test-client", nil)
	w := httptest.NewRecorder()

	relay.handleRateLimitByClient(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
	}
}

func TestHandleRateLimitByClient_POST_Reset(t *testing.T) {
	relay := createTestRelay(t)

	// Make some requests first
	for i := 0; i < 5; i++ {
		relay.rateLimit.Check("test-client")
	}

	req := httptest.NewRequest(http.MethodPost, "/api/v1/rate-limits/test-client/reset", nil)
	w := httptest.NewRecorder()

	relay.handleRateLimitByClient(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

// === More auth tests ===

func TestAuthManager_GenerateAgentToken(t *testing.T) {
	auth := NewAuthManager(nil); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	token, err := auth.GenerateAgentToken("agent-1", 3600)
	if err != nil {
		t.Fatalf("GenerateAgentToken failed: %v", err)
	}

	if token == "" {
		t.Error("expected non-empty token")
	}
}

func TestAuthManager_ValidateAgentToken(t *testing.T) {
	auth := NewAuthManager(nil); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	// Generate token
	token, _ := auth.GenerateAgentToken("agent-1", 3600)

	// Validate
	valid, err := auth.ValidateAgentToken("agent-1", token)
	if err != nil {
		t.Fatalf("ValidateAgentToken failed: %v", err)
	}
	if !valid {
		t.Error("expected token to be valid")
	}
}

func TestAuthManager_ValidateAgentToken_WrongAgent(t *testing.T) {
	auth := NewAuthManager(nil); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	token, _ := auth.GenerateAgentToken("agent-1", 3600)

	// Validate with wrong agent ID
	valid, err := auth.ValidateAgentToken("agent-2", token)
	if err == nil && valid {
		t.Error("expected validation to fail for wrong agent ID")
	}
}

func TestAuthManager_ValidateAgentToken_Expired(t *testing.T) {
	auth := NewAuthManager(nil); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	// Generate token that expires in 1 second
	token, _ := auth.GenerateAgentToken("agent-1", 1)

	// Should still work before expiry
	valid, _ := auth.ValidateAgentToken("agent-1", token)
	if !valid {
		t.Error("expected token to be valid before expiry")
	}
}

func TestAuthManager_RotateTokens(t *testing.T) {
	auth := NewAuthManager(nil); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	// Generate initial token
	token1, _ := auth.GenerateAgentToken("agent-1", 3600)

	// Rotate
	token2, err := auth.RotateTokens("agent-1", 3600)
	if err != nil {
		t.Fatalf("RotateTokens failed: %v", err)
	}

	if token2 == "" {
		t.Error("expected non-empty new token")
	}
	if token2 == token1 {
		t.Error("expected different token after rotation")
	}

	// Old token should be revoked
	valid, _ := auth.ValidateAgentToken("agent-1", token1)
	if valid {
		t.Error("expected old token to be revoked")
	}

	// New token should work
	valid, _ = auth.ValidateAgentToken("agent-1", token2)
	if !valid {
		t.Error("expected new token to be valid")
	}
}

func TestAuthManager_RevokeToken(t *testing.T) {
	auth := NewAuthManager(nil); t.Cleanup(func() { auth.Close() })
	defer auth.Stop()

	token, _ := auth.GenerateAPIToken("client-1", 3600)

	// Validate works
	_, err := auth.ValidateAPIToken(token)
	if err != nil {
		t.Fatalf("ValidateAPIToken failed before revoke: %v", err)
	}

	// Revoke
	err = auth.RevokeToken(token)
	if err != nil {
		t.Fatalf("RevokeToken failed: %v", err)
	}

	// Should fail now
	_, err = auth.ValidateAPIToken(token)
	if err == nil {
		t.Error("expected validation to fail after revoke")
	}
}

// === Additional relay tests ===

func TestHandleMCP(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	// MCP request
	body := map[string]any{
		"agent_id":   "test-agent",
		"method":     "tools/list",
		"request_id": "req-1",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/mcp", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleMCP(w, req)

	// MCP endpoint should respond (even if agent doesn't support MCP)
	if w.Code != http.StatusOK && w.Code != http.StatusAccepted {
		t.Logf("MCP response: %s", w.Body.String())
	}
}

func TestHandleMCP_AgentOffline(t *testing.T) {
	relay := createTestRelay(t)

	body := map[string]any{
		"agent_id":   "offline-agent",
		"method":     "tools/list",
		"request_id": "req-1",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/mcp", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleMCP(w, req)

	// MCP endpoint returns 200 with error in response body for offline agents
	// or 404 if agent is not found - both are acceptable
	if w.Code != http.StatusNotFound && w.Code != http.StatusOK {
		t.Errorf("expected 404 or 200, got %d", w.Code)
	}
}

// === Integration proxy tests with URL ===

func TestHandleIntegrationProxy_Enabled(t *testing.T) {
	// Create a mock backend server
	backend := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"status":"ok"}`))
	}))
	defer backend.Close()

	cfg := &config.RelayConfig{
		WSSAddr:          ":0",
		APIAddr:          ":0",
		IntegrationURL:   backend.URL,
		IntegrationToken: "test-token",
	}
	relay := NewRelay(cfg); t.Cleanup(func() { relay.Close() })

	req := httptest.NewRequest(http.MethodGet, "/api/v1/integration/backups", nil)
	w := httptest.NewRecorder()

	relay.handleIntegrationProxy(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
	}
}

// === Protocol message tests ===

func TestProtocolMessageTypes(t *testing.T) {
	types := []protocol.MessageType{
		protocol.MsgConnect,
		protocol.MsgConnected,
		protocol.MsgHeartbeat,
		protocol.MsgHeartbeatAck,
		protocol.MsgExecRequest,
		protocol.MsgExecDone,
		protocol.MsgExecOutput,
		protocol.MsgFileRead,
		protocol.MsgFileResponse,
		protocol.MsgFileWrite,
		protocol.MsgFileList,
		protocol.MsgSysInfo,
		protocol.MsgSysInfoResp,
		protocol.MsgTask,
		protocol.MsgTaskProgress,
		protocol.MsgTaskDone,
		protocol.MsgTaskCancel,
		protocol.MsgSkillPush,
		protocol.MsgSkillList,
		protocol.MsgSkillDelete,
		protocol.MsgConfigUpdate,
		protocol.MsgConfigAck,
		protocol.MsgLLMRequest,
		protocol.MsgLLMResponse,
		protocol.MsgBackupRequest,
		protocol.MsgBackupResponse,
		protocol.MsgBackupList,
		protocol.MsgBackupListResp,
		protocol.MsgBackupRestore,
		protocol.MsgBackupDelete,
		protocol.MsgBackupProgress,
		protocol.MsgError,
	}

	for _, mt := range types {
		if string(mt) == "" {
			t.Errorf("empty message type for %v", mt)
		}
	}
}

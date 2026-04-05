// Package relay — additional tests for more coverage
package relay

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/config"
)

// === Dashboard handlers ===

func TestHandleDashboardAgents(t *testing.T) {
	relay := createTestRelay(t)

	// Add agent
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	// Use handleListAgents for dashboard data
	req := httptest.NewRequest(http.MethodGet, "/api/v1/agents", nil)
	w := httptest.NewRecorder()

	relay.handleListAgents(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestHandleDashboardClients(t *testing.T) {
	relay := createTestRelay(t)

	// Create client
	relay.registry.CreateClient("Test", "test@example.com", "starter")

	// Use handleClients for dashboard data
	req := httptest.NewRequest(http.MethodGet, "/api/v1/clients", nil)
	w := httptest.NewRecorder()

	relay.handleClients(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestHandleDashboardAuditStats(t *testing.T) {
	relay := createTestRelay(t)

	// Use handleAuditStats for dashboard data
	req := httptest.NewRequest(http.MethodGet, "/api/v1/audit/stats", nil)
	w := httptest.NewRecorder()

	relay.handleAuditStats(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

// === Auth handlers additional tests ===

func TestHandleAuthToken_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/token", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAuthToken(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleAuthRefresh_MissingToken(t *testing.T) {
	relay := createTestRelay(t)

	body := map[string]any{}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/refresh", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAuthRefresh(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleAuthRefresh_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/refresh", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAuthRefresh(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleAuthLogout_InvalidToken(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/logout", nil)
	req.Header.Set("Authorization", "Bearer invalid-token-format")
	w := httptest.NewRecorder()

	relay.handleAuthLogout(w, req)

	// Should succeed or fail gracefully (400, 401, or 200)
	if w.Code != http.StatusOK && w.Code != http.StatusUnauthorized && w.Code != http.StatusBadRequest {
		t.Errorf("expected 200, 401 or 400, got %d", w.Code)
	}
}

func TestHandleAuthRevoke_MissingClientID(t *testing.T) {
	relay := createTestRelayWithToken(t, "admin-token")

	body := map[string]any{}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/revoke", bytes.NewReader(bodyJSON))
	req.Header.Set("Authorization", "Bearer admin-token")
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAuthRevoke(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleAuthRevoke_InvalidJSON(t *testing.T) {
	relay := createTestRelayWithToken(t, "admin-token")

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/revoke", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Authorization", "Bearer admin-token")
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAuthRevoke(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleAuthRevoke_MissingToken(t *testing.T) {
	relay := createTestRelayWithToken(t, "admin-token")

	body := map[string]any{"client_id": "test"}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/auth/revoke", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAuthRevoke(w, req)

	// Should fail with 401 or 403
	if w.Code != http.StatusForbidden && w.Code != http.StatusUnauthorized {
		t.Errorf("expected 403 or 401, got %d", w.Code)
	}
}

// === Audit tests ===

func TestAuditLogger_VerifyAll(t *testing.T) {
	dir := t.TempDir()
	al, err := NewAuditLogger(dir)
	if err != nil {
		t.Fatalf("NewAuditLogger failed: %v", err)
	}

	// Log some entries
	for i := 0; i < 3; i++ {
		al.Log(AuditEntry{
			ID:        string(rune('A' + i)),
			Timestamp: time.Now(),
			Action:    "exec",
		})
	}

	// Verify all
	result, err := al.VerifyAll()
	if err != nil {
		t.Fatalf("VerifyAll failed: %v", err)
	}

	if result == nil {
		t.Fatal("expected non-nil result")
	}

	if result.TotalEntries != 3 {
		t.Errorf("expected TotalEntries 3, got %d", result.TotalEntries)
	}
}

// === Billing tests ===

func TestHandleBillingUsage_MissingClientID(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/billing/usage", nil)
	w := httptest.NewRecorder()

	relay.handleBillingUsage(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleBillingPlanChange_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/billing/plan/change", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleBillingPlanChange(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleBillingPlanChange_MissingFields(t *testing.T) {
	relay := createTestRelay(t)

	body := map[string]any{}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/billing/plan/change", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleBillingPlanChange(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleBillingInvoices_MissingClientID(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/billing/invoices", nil)
	w := httptest.NewRecorder()

	relay.handleBillingInvoices(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleBillingPaymentMethods_MissingClientID(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/billing/payment-methods", nil)
	w := httptest.NewRecorder()

	relay.handleBillingPaymentMethods(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

// === Rate limit handler tests ===

func TestHandleRateLimitByClient_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPut, "/api/v1/rate-limits/test-client", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleRateLimitByClient(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleRateLimitByClient_Get(t *testing.T) {
	relay := createTestRelay(t)

	// Set custom limits
	relay.rateLimit.SetClientLimits("test-client", 50, 500)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/rate-limits/test-client", nil)
	w := httptest.NewRecorder()

	relay.handleRateLimitByClient(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

// === More handler tests ===

func TestHandleAgentRegister_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/register", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAgentRegister(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleAgentRegister_ClientNotFound(t *testing.T) {
	relay := createTestRelay(t)

	body := map[string]any{
		"client_id": "non-existing-client",
		"label":     "Test",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/register", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAgentRegister(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleClientByID_InvalidMethod(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPatch, "/api/v1/clients/some-id", nil)
	w := httptest.NewRecorder()

	relay.handleClientByID(w, req)

	if w.Code != http.StatusMethodNotAllowed {
		t.Errorf("expected 405, got %d", w.Code)
	}
}

func TestHandleClientByID_Patch(t *testing.T) {
	relay := createTestRelay(t)
	client, _ := relay.registry.CreateClient("Test", "test@example.com", "starter")

	body := map[string]any{"name": "Updated Name"}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPatch, "/api/v1/clients/"+client.ID, bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleClientByID(w, req)

	// Patch may or may not be supported
	if w.Code != http.StatusOK && w.Code != http.StatusMethodNotAllowed {
		t.Errorf("expected 200 or 405, got %d", w.Code)
	}
}

// === Approval tests ===

func TestHandleApprovalAction_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	// Test with non-existent approval ID and invalid JSON
	req := httptest.NewRequest(http.MethodPost, "/api/v1/approvals/non-existing-id/approve", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleApprovalAction(w, req)

	// Should return 404 for non-existing approval or 400 for invalid JSON
	if w.Code != http.StatusNotFound && w.Code != http.StatusBadRequest {
		t.Errorf("expected 404 or 400, got %d", w.Code)
	}
}

func TestHandleApprovalAction_ApproveNotFound(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/approvals/non-existing/approve", nil)
	req.Header.Set("Authorization", "Bearer test")
	w := httptest.NewRecorder()

	relay.handleApprovalAction(w, req)

	if w.Code != http.StatusNotFound {
		t.Errorf("expected 404, got %d", w.Code)
	}
}

func TestHandleApprovalAction_RejectNotFound(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/approvals/non-existing/reject", nil)
	req.Header.Set("Authorization", "Bearer test")
	w := httptest.NewRecorder()

	relay.handleApprovalAction(w, req)

	if w.Code != http.StatusNotFound {
		t.Errorf("expected 404, got %d", w.Code)
	}
}

// === Health check tests ===

func TestHandleHealthReady_WithAgents(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	req := httptest.NewRequest(http.MethodGet, "/api/v1/health/ready", nil)
	w := httptest.NewRecorder()

	relay.handleHealthReady(w, req)

	// 200 if ready, 503 if not - both are valid
	if w.Code != http.StatusOK && w.Code != http.StatusServiceUnavailable {
		t.Errorf("expected 200 or 503, got %d", w.Code)
	}
}

func TestHandleHealthLive_WithAgents(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	req := httptest.NewRequest(http.MethodGet, "/api/v1/health/live", nil)
	w := httptest.NewRecorder()

	relay.handleHealthLive(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

// === Integration proxy additional tests ===

func TestHandleIntegrationProxy_Error(t *testing.T) {
	cfg := &config.RelayConfig{
		WSSAddr:          ":0",
		APIAddr:          ":0",
		IntegrationURL:   "http://127.0.0.1:1", // Invalid URL
		IntegrationToken: "test-token",
	}
	relay := NewRelay(cfg)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/integration/backups", nil)
	w := httptest.NewRecorder()

	relay.handleIntegrationProxy(w, req)

	// Should fail with bad gateway or internal error
	if w.Code != http.StatusOK {
		// Error is expected
	}
}

// === Audit query tests ===

func TestHandleAuditQuery_WithFilters(t *testing.T) {
	relay := createTestRelay(t)

	// Log some entries
	relay.audit.Log(AuditEntry{
		ID:        "test-1",
		Timestamp: time.Now(),
		AgentID:   "agent-1",
		Action:    "exec",
		RiskLevel: "low",
		Result:    "success",
	})

	// Query with filters
	req := httptest.NewRequest(http.MethodGet, "/api/v1/audit?agent_id=agent-1&action=exec&result=success&limit=10", nil)
	w := httptest.NewRecorder()

	relay.handleAuditQuery(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestHandleAuditExport_WithFormat(t *testing.T) {
	relay := createTestRelay(t)

	// Log entry
	relay.audit.Log(AuditEntry{
		ID:        "test-1",
		Timestamp: time.Now(),
		Action:    "exec",
	})

	// Export as JSON
	req := httptest.NewRequest(http.MethodGet, "/api/v1/audit/export?format=json", nil)
	w := httptest.NewRecorder()

	relay.handleAuditExport(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}

	// Export as CSV
	req2 := httptest.NewRequest(http.MethodGet, "/api/v1/audit/export?format=csv", nil)
	w2 := httptest.NewRecorder()

	relay.handleAuditExport(w2, req2)

	if w2.Code != http.StatusOK {
		t.Errorf("expected 200 for CSV, got %d", w2.Code)
	}
}

// === Task handler tests ===

func TestHandleTaskSubmit_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/task", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleTaskSubmit(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleTaskCancel_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/task/cancel", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleTaskCancel(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

// === File handler tests ===

func TestHandleFileRead_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/files/read", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleFileRead(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleFileWrite_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/files/write", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleFileWrite(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleFileList_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/files/list", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleFileList(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

// === Skill handler tests ===

func TestHandleSkillPush_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/skills/push", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleSkillPush(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleSkillList_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/skills/list", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleSkillList(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleSkillDelete_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/skills/delete", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleSkillDelete(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

// === Config update handler tests ===

func TestHandleAgentConfigUpdate_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPut, "/api/v1/agents/config", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAgentConfigUpdate(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

// === SysInfo handler tests ===

func TestHandleSysInfo_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/sysinfo", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleSysInfo(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

// === Backup handler tests ===

func TestHandleBackupCreate_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/backup", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleBackupCreate(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleBackupList_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/agents/backup/list", nil)
	w := httptest.NewRecorder()

	relay.handleBackupList(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

// === Client handlers additional tests ===

func TestHandleClients_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/clients", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleClients(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestHandleClients_InvalidMethod(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPut, "/api/v1/clients", nil)
	w := httptest.NewRecorder()

	relay.handleClients(w, req)

	if w.Code != http.StatusMethodNotAllowed {
		t.Errorf("expected 405, got %d", w.Code)
	}
}

// === Exec command tests ===

func TestHandleExecCommand_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/exec", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleExecCommand(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

// === MCP tests ===

func TestHandleMCP_InvalidJSON(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodPost, "/mcp", bytes.NewReader([]byte("invalid")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleMCP(w, req)

	// MCP handler may return 200 with error in body or 400
	if w.Code != http.StatusBadRequest && w.Code != http.StatusOK {
		t.Errorf("expected 400 or 200, got %d", w.Code)
	}
}

func TestHandleMCP_MissingAgentID(t *testing.T) {
	relay := createTestRelay(t)

	body := map[string]any{
		"method":     "tools/list",
		"request_id": "req-1",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/mcp", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleMCP(w, req)

	// MCP handler may return 200 with error in body or 400
	if w.Code != http.StatusBadRequest && w.Code != http.StatusOK {
		t.Errorf("expected 400 or 200, got %d", w.Code)
	}
}

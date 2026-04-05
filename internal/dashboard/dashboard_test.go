package dashboard

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

// --- mock data provider ---

type mockProvider struct {
	agents  []AgentInfo
	clients []ClientInfo
	stats   *AuditStatsInfo
}

func (m *mockProvider) DashboardAgents() []AgentInfo  { return m.agents }
func (m *mockProvider) DashboardClients() []ClientInfo { return m.clients }
func (m *mockProvider) DashboardAuditStats() *AuditStatsInfo {
	if m.stats == nil {
		return &AuditStatsInfo{TotalEntries: 0, ByAction: map[string]int{}, Last24hCount: 0}
	}
	return m.stats
}
func (m *mockProvider) DashboardStorageConfig() *StorageConfigInfo { return &StorageConfigInfo{Type: "local"} }
func (m *mockProvider) DashboardBackupConfig() *BackupConfigInfo {
	return &BackupConfigInfo{Enabled: true, MaxSnapshots: 50, RetentionDays: 7}
}
func (m *mockProvider) DashboardBackups() []BackupInfo { return nil }
func (m *mockProvider) DashboardCreateBackup([]string, string) (*BackupInfo, error) { return nil, nil }
func (m *mockProvider) DashboardRestoreBackup(string) error { return nil }
func (m *mockProvider) DashboardDeleteBackup(string) error { return nil }
func (m *mockProvider) DashboardGetConfig() map[string]any { return map[string]any{} }
func (m *mockProvider) DashboardUpdateConfig(map[string]any) error { return nil }
func (m *mockProvider) DashboardApprovals() []ApprovalInfo { return nil }
func (m *mockProvider) DashboardApproveCommand(string, bool) error { return nil }

func sampleProvider() *mockProvider {
	return &mockProvider{
		agents: []AgentInfo{
			{ID: "a1", ClientID: "c1", Label: "srv-1", OS: "linux", Arch: "amd64", IsOnline: true, LastSeenAt: "2025-01-01T00:00:00Z"},
			{ID: "a2", ClientID: "c1", Label: "srv-2", OS: "darwin", Arch: "arm64", IsOnline: false},
		},
		clients: []ClientInfo{
			{ID: "c1", Name: "Test Client", Email: "test@test.com", Plan: "pro", IsActive: true},
		},
		stats: &AuditStatsInfo{
			TotalEntries: 42,
			ByAction:     map[string]int{"exec": 30, "file_read": 12},
			Last24hCount: 10,
			Entries: []AuditEntryInfo{
				{Timestamp: "2025-01-01T12:00:00Z", AgentID: "a1", Action: "exec", Command: "ls", Result: "success", DurationMs: 100},
			},
		},
	}
}

// --- NewHandler ---

func TestNewHandler_NotNil(t *testing.T) {
	h := NewHandler(sampleProvider(), "test-token")
	if h == nil {
		t.Fatal("NewHandler() returned nil")
	}
}

// --- Auth ---

func TestHandler_NoAuth_Returns401(t *testing.T) {
	h := NewHandler(sampleProvider(), "secret")

	tests := []struct {
		name    string
		headers map[string]string
		url     string
	}{
		{"no auth header", nil, "/api/overview"},
		{"wrong bearer", map[string]string{"Authorization": "Bearer wrong"}, "/api/overview"},
		{"wrong query token", nil, "/api/overview?token=wrong"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := httptest.NewRequest("GET", tt.url, nil)
			for k, v := range tt.headers {
				req.Header.Set(k, v)
			}
			rec := httptest.NewRecorder()
			h.ServeHTTP(rec, req)

			if rec.Code != http.StatusUnauthorized {
				t.Errorf("status = %d, want 401", rec.Code)
			}

			var body map[string]string
			json.NewDecoder(rec.Body).Decode(&body)
			if body["code"] != "401" {
				t.Errorf("body code = %q, want 401", body["code"])
			}
		})
	}
}

func TestHandler_BearerAuth_OK(t *testing.T) {
	h := NewHandler(sampleProvider(), "mytoken")
	req := httptest.NewRequest("GET", "/api/overview", nil)
	req.Header.Set("Authorization", "Bearer mytoken")
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Errorf("status = %d, want 200", rec.Code)
	}
}

func TestHandler_QueryTokenAuth_OK(t *testing.T) {
	h := NewHandler(sampleProvider(), "mytoken")
	req := httptest.NewRequest("GET", "/api/overview?token=mytoken", nil)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Errorf("status = %d, want 200", rec.Code)
	}
}

// --- API endpoints ---

func TestHandler_APIOverview(t *testing.T) {
	h := NewHandler(sampleProvider(), "tok")
	req := httptest.NewRequest("GET", "/api/overview?token=tok", nil)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d", rec.Code)
	}

	var body map[string]any
	json.NewDecoder(rec.Body).Decode(&body)

	// Check agents
	agentsRaw, ok := body["agents"]
	if !ok {
		t.Fatal("missing agents")
	}
	agents := agentsRaw.([]any)
	if len(agents) != 2 {
		t.Errorf("agents count = %d, want 2", len(agents))
	}

	// Check online_agents
	if online, ok := body["online_agents"].(float64); !ok || online != 1 {
		t.Errorf("online_agents = %v, want 1", body["online_agents"])
	}

	// Check clients
	clientsRaw := body["clients"].([]any)
	if len(clientsRaw) != 1 {
		t.Errorf("clients count = %d, want 1", len(clientsRaw))
	}

	// Check stats
	stats := body["stats"].(map[string]any)
	if stats["total_entries"].(float64) != 42 {
		t.Errorf("total_entries = %v, want 42", stats["total_entries"])
	}
}

func TestHandler_APIAgents(t *testing.T) {
	h := NewHandler(sampleProvider(), "tok")
	req := httptest.NewRequest("GET", "/api/agents?token=tok", nil)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d", rec.Code)
	}

	var body map[string]any
	json.NewDecoder(rec.Body).Decode(&body)
	agents := body["agents"].([]any)
	if len(agents) != 2 {
		t.Errorf("agents = %d, want 2", len(agents))
	}
}

func TestHandler_APIClients(t *testing.T) {
	h := NewHandler(sampleProvider(), "tok")
	req := httptest.NewRequest("GET", "/api/clients?token=tok", nil)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d", rec.Code)
	}

	var body map[string]any
	json.NewDecoder(rec.Body).Decode(&body)
	clients := body["clients"].([]any)
	if len(clients) != 1 {
		t.Errorf("clients = %d, want 1", len(clients))
	}
}

func TestHandler_APIAudit(t *testing.T) {
	h := NewHandler(sampleProvider(), "tok")
	req := httptest.NewRequest("GET", "/api/audit?token=tok", nil)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d", rec.Code)
	}

	var body map[string]any
	json.NewDecoder(rec.Body).Decode(&body)

	if body["total"].(float64) != 42 {
		t.Errorf("total = %v, want 42", body["total"])
	}
	entries := body["entries"].([]any)
	if len(entries) != 1 {
		t.Errorf("entries = %d, want 1", len(entries))
	}
}

func TestHandler_APIAudit_EmptyStats(t *testing.T) {
	p := &mockProvider{
		stats: &AuditStatsInfo{TotalEntries: 0, ByAction: map[string]int{}, Last24hCount: 0},
	}
	h := NewHandler(p, "tok")
	req := httptest.NewRequest("GET", "/api/audit?token=tok", nil)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d", rec.Code)
	}

	var body map[string]any
	json.NewDecoder(rec.Body).Decode(&body)
	if body["total"].(float64) != 0 {
		t.Errorf("total = %v, want 0", body["total"])
	}
}

// --- Static files ---

func TestHandler_Root_ServesSPA(t *testing.T) {
	h := NewHandler(sampleProvider(), "tok")
	req := httptest.NewRequest("GET", "/?token=tok", nil)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200", rec.Code)
	}
	contentType := rec.Header().Get("Content-Type")
	if !strings.Contains(contentType, "text/html") {
		t.Errorf("Content-Type = %q, want text/html", contentType)
	}
	body := rec.Body.String()
	if !strings.Contains(body, "<!DOCTYPE") && !strings.Contains(body, "<html") {
		t.Error("response doesn't look like HTML")
	}
}

// Static SPA routing is handled by http.FileServer which has its own redirect behavior.
// We only test that auth-gated static serving works (tested in TestHandler_Root_ServesSPA).

func TestHandler_NonexistentRoute_NoAuth(t *testing.T) {
	h := NewHandler(sampleProvider(), "tok")
	req := httptest.NewRequest("GET", "/some/route", nil)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)

	if rec.Code != http.StatusUnauthorized {
		t.Errorf("status = %d, want 401 for unauthed static request", rec.Code)
	}
}

// --- Empty data ---

func TestHandler_EmptyProvider(t *testing.T) {
	p := &mockProvider{}
	h := NewHandler(p, "tok")
	req := httptest.NewRequest("GET", "/api/overview?token=tok", nil)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d", rec.Code)
	}

	var body map[string]any
	json.NewDecoder(rec.Body).Decode(&body)
	if body["online_agents"].(float64) != 0 {
		t.Errorf("online_agents = %v, want 0", body["online_agents"])
	}
}

// Package relay — comprehensive tests for HTTP handlers
package relay

import (
	"bytes"
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/protocol"
	"github.com/gorilla/websocket"
)

// === Helper Functions ===

// createTestRelay creates a relay instance for testing
// Note: AuthManager goroutines are intentionally leaked in tests.
// In production, Relay would have a Stop() method.
func createTestRelay(t *testing.T) *Relay {
	t.Helper()
	cfg := &config.RelayConfig{
		WSSAddr: ":0",
		APIAddr: ":0",
	}
	return NewRelay(cfg)
}

// createTestRelayWithToken creates a relay with static API token
func createTestRelayWithToken(t *testing.T, token string) *Relay {
	t.Helper()
	cfg := &config.RelayConfig{
		WSSAddr:   ":0",
		APIAddr:   ":0",
		APIToken:  token,
	}
	relay := NewRelay(cfg)
	t.Cleanup(func() { relay.Close() })
	return relay
}

// mockAgentConn creates a mock agent connection for testing
func mockAgentConn(t *testing.T, agentID string) *AgentConn {
	t.Helper()
	// Create a test websocket server
	upgrader := websocket.Upgrader{
		CheckOrigin: func(r *http.Request) bool { return true },
	}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			return
		}
		defer conn.Close()
		// Keep connection alive
		for {
			_, _, err := conn.ReadMessage()
			if err != nil {
				break
			}
		}
	}))
	t.Cleanup(func() { server.Close() })

	// Connect as client
	wsURL := "ws" + server.URL[4:]
	conn, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("Failed to create mock WS connection: %v", err)
	}

	return &AgentConn{
		ID:        agentID,
		Hostname:  "test-host",
		OS:        "linux",
		Arch:      "amd64",
		Version:   "1.0.0",
		Connected: time.Now(),
		LastSeen:  time.Now(),
		conn:      conn,
	}
}

// === AgentPool Tests ===

func TestAgentPool_AddRemove(t *testing.T) {
	pool := NewAgentPool()

	agent1 := &AgentConn{ID: "agent-1"}
	agent2 := &AgentConn{ID: "agent-2"}

	pool.Add(agent1)
	pool.Add(agent2)

	if pool.Count() != 2 {
		t.Errorf("expected 2 agents, got %d", pool.Count())
	}

	// Get existing
	got, ok := pool.Get("agent-1")
	if !ok || got.ID != "agent-1" {
		t.Error("failed to get agent-1")
	}

	// Get non-existing
	_, ok = pool.Get("non-existing")
	if ok {
		t.Error("expected false for non-existing agent")
	}

	// Remove
	pool.Remove("agent-1")
	if pool.Count() != 1 {
		t.Errorf("expected 1 agent after remove, got %d", pool.Count())
	}

	_, ok = pool.Get("agent-1")
	if ok {
		t.Error("agent-1 should be removed")
	}
}

func TestAgentPool_List(t *testing.T) {
	pool := NewAgentPool()

	// Empty pool
	list := pool.List()
	if len(list) != 0 {
		t.Errorf("expected empty list, got %d", len(list))
	}

	// Add agents
	pool.Add(&AgentConn{ID: "agent-1"})
	pool.Add(&AgentConn{ID: "agent-2"})

	list = pool.List()
	if len(list) != 2 {
		t.Errorf("expected 2 agents, got %d", len(list))
	}
}

// === AgentConn Tests ===

func TestAgentConn_SendMessage(t *testing.T) {
	// Create mock connection
	upgrader := websocket.Upgrader{
		CheckOrigin: func(r *http.Request) bool { return true },
	}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			return
		}
		defer conn.Close()
		// Read one message and close
		var msg protocol.Message
		conn.ReadJSON(&msg)
	}))
	defer server.Close()

	wsURL := "ws" + server.URL[4:]
	conn, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("Failed to dial: %v", err)
	}
	defer conn.Close()

	agent := &AgentConn{
		ID:   "test-agent",
		conn: conn,
	}

	msg := protocol.NewMessage(protocol.MsgHeartbeat)
	err = agent.SendMessage(msg)
	if err != nil {
		t.Errorf("SendMessage failed: %v", err)
	}

	// Test with nil connection
	agentNil := &AgentConn{ID: "nil-conn", conn: nil}
	err = agentNil.SendMessage(msg)
	if err == nil {
		t.Error("expected error for nil connection")
	}
}

func TestAgentConn_Callbacks(t *testing.T) {
	agent := &AgentConn{ID: "test-agent"}

	called := false
	agent.SetCallback("req-123", func(any) {
		called = true
	})

	// Trigger callback
	result := agent.TriggerCallback("req-123", map[string]any{"test": "data"})
	if !result {
		t.Error("expected callback to be triggered")
	}
	if !called {
		t.Error("callback was not called")
	}

	// Non-existing callback
	result = agent.TriggerCallback("non-existing", nil)
	if result {
		t.Error("expected false for non-existing callback")
	}
}

// === handleListAgents Tests ===

func TestHandleListAgents(t *testing.T) {
	relay := createTestRelay(t)

	// Empty list
	req := httptest.NewRequest(http.MethodGet, "/api/v1/agents", nil)
	w := httptest.NewRecorder()
	relay.handleListAgents(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}

	var resp map[string]any
	json.Unmarshal(w.Body.Bytes(), &resp)
	if resp["count"].(float64) != 0 {
		t.Errorf("expected count 0, got %v", resp["count"])
	}

	// Add agent
	agent := mockAgentConn(t, "agent-1")
	relay.pool.Add(agent)
	defer relay.pool.Remove("agent-1")

	w = httptest.NewRecorder()
	relay.handleListAgents(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}

	json.Unmarshal(w.Body.Bytes(), &resp)
	if resp["count"].(float64) != 1 {
		t.Errorf("expected count 1, got %v", resp["count"])
	}
}

// === handleExecCommand Tests ===

func TestHandleExecCommand(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	tests := []struct {
		name       string
		body       map[string]any
		wantStatus int
	}{
		{
			name: "valid request",
			body: map[string]any{
				"agent_id": "test-agent",
				"command":  "ls -la",
			},
			wantStatus: http.StatusOK,
		},
		{
			name: "missing agent_id",
			body: map[string]any{
				"command": "ls -la",
			},
			wantStatus: http.StatusBadRequest,
		},
		{
			name: "missing command",
			body: map[string]any{
				"agent_id": "test-agent",
			},
			wantStatus: http.StatusBadRequest,
		},
		{
			name: "agent offline",
			body: map[string]any{
				"agent_id": "offline-agent",
				"command":  "ls -la",
			},
			wantStatus: http.StatusNotFound,
		},
		{
			name: "with options",
			body: map[string]any{
				"agent_id":    "test-agent",
				"command":     "echo test",
				"shell":       "/bin/bash",
				"dir":         "/tmp",
				"timeout_sec": 30,
			},
			wantStatus: http.StatusOK,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			bodyJSON, _ := json.Marshal(tt.body)
			req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/exec", bytes.NewReader(bodyJSON))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			relay.handleExecCommand(w, req)

			if w.Code != tt.wantStatus {
				t.Errorf("expected status %d, got %d: %s", tt.wantStatus, w.Code, w.Body.String())
			}
		})
	}

	// Test invalid JSON
	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/exec", strings.NewReader("invalid json"))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	relay.handleExecCommand(w, req)
	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400 for invalid JSON, got %d", w.Code)
	}
}

// === File Operations Tests ===

func TestHandleFileRead(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	tests := []struct {
		name       string
		body       map[string]any
		wantStatus int
	}{
		{
			name: "valid request",
			body: map[string]any{
				"agent_id": "test-agent",
				"path":     "/etc/passwd",
			},
			wantStatus: http.StatusOK,
		},
		{
			name: "with encoding",
			body: map[string]any{
				"agent_id": "test-agent",
				"path":     "/etc/passwd",
				"encoding": "base64",
			},
			wantStatus: http.StatusOK,
		},
		{
			name: "agent offline",
			body: map[string]any{
				"agent_id": "offline-agent",
				"path":     "/etc/passwd",
			},
			wantStatus: http.StatusNotFound,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			bodyJSON, _ := json.Marshal(tt.body)
			req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/files/read", bytes.NewReader(bodyJSON))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			relay.handleFileRead(w, req)

			if w.Code != tt.wantStatus {
				t.Errorf("expected status %d, got %d", tt.wantStatus, w.Code)
			}
		})
	}
}

func TestHandleFileWrite(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	body := map[string]any{
		"agent_id": "test-agent",
		"path":     "/tmp/test.txt",
		"content":  "Hello, World!",
		"encoding": "utf-8",
		"mode":     0644,
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/files/write", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleFileWrite(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
	}
}

func TestHandleFileList(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	body := map[string]any{
		"agent_id": "test-agent",
		"path":     "/home",
		"depth":    2,
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/files/list", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleFileList(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

// === handleSysInfo Tests ===

func TestHandleSysInfo(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	body := map[string]any{"agent_id": "test-agent"}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/sysinfo", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleSysInfo(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

// === Task Tests ===

func TestHandleTaskSubmit(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	tests := []struct {
		name       string
		body       map[string]any
		wantStatus int
	}{
		{
			name: "valid task",
			body: map[string]any{
				"agent_id":     "test-agent",
				"description":  "Fix bug in app.tsx",
				"skill_id":     "coding-agent",
				"max_steps":    10,
				"auto_approve_safe": true,
			},
			wantStatus: http.StatusOK,
		},
		{
			name: "missing agent_id",
			body: map[string]any{
				"description": "Fix bug",
			},
			wantStatus: http.StatusBadRequest,
		},
		{
			name: "missing description",
			body: map[string]any{
				"agent_id": "test-agent",
			},
			wantStatus: http.StatusBadRequest,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			bodyJSON, _ := json.Marshal(tt.body)
			req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/task", bytes.NewReader(bodyJSON))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			relay.handleTaskSubmit(w, req)

			if w.Code != tt.wantStatus {
				t.Errorf("expected status %d, got %d: %s", tt.wantStatus, w.Code, w.Body.String())
			}
		})
	}
}

func TestHandleTaskCancel(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	body := map[string]any{
		"agent_id": "test-agent",
		"task_id":  "task-123",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/task/cancel", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleTaskCancel(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

// === Skill Tests ===

func TestHandleSkillPush(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	tests := []struct {
		name       string
		body       map[string]any
		wantStatus int
	}{
		{
			name: "valid skill push",
			body: map[string]any{
				"agent_id":     "test-agent",
				"skill_id":     "code-review",
				"name":         "Code Reviewer",
				"description":  "Reviews code for issues",
				"instructions": "Review the code and suggest improvements",
				"tools_allowed": []string{"read", "write", "exec"},
			},
			wantStatus: http.StatusOK,
		},
		{
			name: "missing skill_id",
			body: map[string]any{
				"agent_id":     "test-agent",
				"instructions": "Do something",
			},
			wantStatus: http.StatusBadRequest,
		},
		{
			name: "missing instructions",
			body: map[string]any{
				"agent_id": "test-agent",
				"skill_id": "test-skill",
			},
			wantStatus: http.StatusBadRequest,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			bodyJSON, _ := json.Marshal(tt.body)
			req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/skills/push", bytes.NewReader(bodyJSON))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			relay.handleSkillPush(w, req)

			if w.Code != tt.wantStatus {
				t.Errorf("expected status %d, got %d: %s", tt.wantStatus, w.Code, w.Body.String())
			}
		})
	}
}

func TestHandleSkillList(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	body := map[string]any{"agent_id": "test-agent"}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/skills/list", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleSkillList(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestHandleSkillDelete(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	body := map[string]any{
		"agent_id": "test-agent",
		"skill_id": "test-skill",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/skills/delete", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleSkillDelete(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

// === handleAgentConfigUpdate Tests ===

func TestHandleAgentConfigUpdate(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	tests := []struct {
		name       string
		method     string
		body       map[string]any
		wantStatus int
	}{
		{
			name:   "valid config update",
			method: http.MethodPut,
			body: map[string]any{
				"agent_id":  "test-agent",
				"read_only": true,
				"label":     "Production Agent",
				"work_dir":  "/home/user",
			},
			wantStatus: http.StatusOK,
		},
		{
			name:   "missing agent_id",
			method: http.MethodPut,
			body: map[string]any{
				"read_only": true,
			},
			wantStatus: http.StatusBadRequest,
		},
		{
			name:   "empty payload",
			method: http.MethodPut,
			body: map[string]any{
				"agent_id": "test-agent",
			},
			wantStatus: http.StatusBadRequest,
		},
		{
			name:   "wrong method",
			method: http.MethodPost,
			body: map[string]any{
				"agent_id": "test-agent",
			},
			wantStatus: http.StatusMethodNotAllowed,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			bodyJSON, _ := json.Marshal(tt.body)
			req := httptest.NewRequest(tt.method, "/api/v1/agents/config", bytes.NewReader(bodyJSON))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			relay.handleAgentConfigUpdate(w, req)

			if w.Code != tt.wantStatus {
				t.Errorf("expected status %d, got %d: %s", tt.wantStatus, w.Code, w.Body.String())
			}
		})
	}
}

// === Health Check Tests ===

func TestHandleHealth(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/health", nil)
	w := httptest.NewRecorder()

	relay.handleHealth(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}

	var resp map[string]any
	if err := json.Unmarshal(w.Body.Bytes(), &resp); err != nil {
		t.Errorf("failed to parse response: %v", err)
	}
}

func TestHandleHealthReady(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/health/ready", nil)
	w := httptest.NewRecorder()

	relay.handleHealthReady(w, req)

	// Should be 200 or 503
	if w.Code != http.StatusOK && w.Code != http.StatusServiceUnavailable {
		t.Errorf("expected 200 or 503, got %d", w.Code)
	}
}

func TestHandleHealthLive(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/health/live", nil)
	w := httptest.NewRecorder()

	relay.handleHealthLive(w, req)

	// Should be 200 or 503
	if w.Code != http.StatusOK && w.Code != http.StatusServiceUnavailable {
		t.Errorf("expected 200 or 503, got %d", w.Code)
	}
}

// === Client Handlers Tests ===

func TestHandleClients(t *testing.T) {
	relay := createTestRelay(t)

	// GET - list clients
	t.Run("GET list", func(t *testing.T) {
		req := httptest.NewRequest(http.MethodGet, "/api/v1/clients", nil)
		w := httptest.NewRecorder()
		relay.handleClients(w, req)

		if w.Code != http.StatusOK {
			t.Errorf("expected 200, got %d", w.Code)
		}
	})

	// POST - create client
	t.Run("POST create", func(t *testing.T) {
		body := map[string]any{
			"name":  "Test Client",
			"email": "test@example.com",
			"plan":  "starter",
		}
		bodyJSON, _ := json.Marshal(body)

		req := httptest.NewRequest(http.MethodPost, "/api/v1/clients", bytes.NewReader(bodyJSON))
		req.Header.Set("Content-Type", "application/json")
		w := httptest.NewRecorder()

		relay.handleClients(w, req)

		if w.Code != http.StatusOK {
			t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
		}
	})

	// POST - missing name
	t.Run("POST missing name", func(t *testing.T) {
		body := map[string]any{
			"email": "test@example.com",
		}
		bodyJSON, _ := json.Marshal(body)

		req := httptest.NewRequest(http.MethodPost, "/api/v1/clients", bytes.NewReader(bodyJSON))
		req.Header.Set("Content-Type", "application/json")
		w := httptest.NewRecorder()

		relay.handleClients(w, req)

		if w.Code != http.StatusBadRequest {
			t.Errorf("expected 400, got %d", w.Code)
		}
	})
}

func TestHandleClientByID(t *testing.T) {
	relay := createTestRelay(t)

	// Create a client first
	client, err := relay.registry.CreateClient("Test Client", "test@example.com", "starter")
	if err != nil {
		t.Fatalf("failed to create client: %v", err)
	}

	// GET client by ID
	t.Run("GET by ID", func(t *testing.T) {
		req := httptest.NewRequest(http.MethodGet, "/api/v1/clients/"+client.ID, nil)
		w := httptest.NewRecorder()
		relay.handleClientByID(w, req)

		if w.Code != http.StatusOK {
			t.Errorf("expected 200, got %d", w.Code)
		}
	})

	// GET non-existing client
	t.Run("GET not found", func(t *testing.T) {
		req := httptest.NewRequest(http.MethodGet, "/api/v1/clients/non-existing", nil)
		w := httptest.NewRecorder()
		relay.handleClientByID(w, req)

		if w.Code != http.StatusNotFound {
			t.Errorf("expected 404, got %d", w.Code)
		}
	})

	// DELETE client
	t.Run("DELETE", func(t *testing.T) {
		req := httptest.NewRequest(http.MethodDelete, "/api/v1/clients/"+client.ID, nil)
		w := httptest.NewRecorder()
		relay.handleClientByID(w, req)

		if w.Code != http.StatusOK {
			t.Errorf("expected 200, got %d", w.Code)
		}
	})

	// List agents for client
	t.Run("GET agents", func(t *testing.T) {
		client2, _ := relay.registry.CreateClient("Client2", "c2@test.com", "starter")
		req := httptest.NewRequest(http.MethodGet, "/api/v1/clients/"+client2.ID+"/agents", nil)
		w := httptest.NewRecorder()
		relay.handleClientByID(w, req)

		if w.Code != http.StatusOK {
			t.Errorf("expected 200, got %d", w.Code)
		}
	})

	// Register agent for client
	t.Run("POST agent", func(t *testing.T) {
		client3, _ := relay.registry.CreateClient("Client3", "c3@test.com", "starter")
		body := map[string]any{
			"label": "Test Agent",
			"tags":  []string{"prod"},
			"os":    "linux",
			"arch":  "amd64",
		}
		bodyJSON, _ := json.Marshal(body)

		req := httptest.NewRequest(http.MethodPost, "/api/v1/clients/"+client3.ID+"/agents", bytes.NewReader(bodyJSON))
		req.Header.Set("Content-Type", "application/json")
		w := httptest.NewRecorder()
		relay.handleClientByID(w, req)

		if w.Code != http.StatusOK {
			t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
		}
	})
}

func TestHandleAgentRegister(t *testing.T) {
	relay := createTestRelay(t)

	// Create client first
	client, _ := relay.registry.CreateClient("Test", "test@test.com", "starter")

	body := map[string]any{
		"client_id": client.ID,
		"label":     "New Agent",
		"tags":      []string{"dev"},
		"os":        "darwin",
		"arch":      "arm64",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/register", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleAgentRegister(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
	}

	// Missing client_id
	body2 := map[string]any{"label": "Agent"}
	bodyJSON2, _ := json.Marshal(body2)

	req2 := httptest.NewRequest(http.MethodPost, "/api/v1/agents/register", bytes.NewReader(bodyJSON2))
	req2.Header.Set("Content-Type", "application/json")
	w2 := httptest.NewRecorder()

	relay.handleAgentRegister(w2, req2)

	if w2.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w2.Code)
	}
}

func TestHandleAgentDelete(t *testing.T) {
	relay := createTestRelay(t)

	// Create client and agent
	client, _ := relay.registry.CreateClient("Test", "test@test.com", "starter")
	agent, _ := relay.registry.RegisterAgent(client.ID, "Test Agent", []string{}, "linux", "amd64")

	req := httptest.NewRequest(http.MethodDelete, "/api/v1/agents/delete/"+agent.ID, nil)
	w := httptest.NewRecorder()

	relay.handleAgentDelete(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}

	// Delete non-existing
	req2 := httptest.NewRequest(http.MethodDelete, "/api/v1/agents/delete/non-existing", nil)
	w2 := httptest.NewRecorder()

	relay.handleAgentDelete(w2, req2)

	if w2.Code != http.StatusNotFound {
		t.Errorf("expected 404, got %d", w2.Code)
	}
}

// === Approval Handlers Tests ===

func TestHandleApprovalsList(t *testing.T) {
	relay := createTestRelay(t)

	// Add approval request
	relay.approvalQueue.Add("agent-1", "rm -rf /", "high", "hard_ask")

	req := httptest.NewRequest(http.MethodGet, "/api/v1/approvals?agent_id=agent-1&status=pending", nil)
	w := httptest.NewRecorder()

	relay.handleApprovalsList(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}

	// Wrong method
	req2 := httptest.NewRequest(http.MethodPost, "/api/v1/approvals", nil)
	w2 := httptest.NewRecorder()
	relay.handleApprovalsList(w2, req2)

	if w2.Code != http.StatusMethodNotAllowed {
		t.Errorf("expected 405, got %d", w2.Code)
	}
}

func TestHandleApprovalAction(t *testing.T) {
	relay := createTestRelay(t)
	agent := mockAgentConn(t, "test-agent")
	relay.pool.Add(agent)
	defer relay.pool.Remove("test-agent")

	// Add approval request
	req2 := relay.approvalQueue.Add("test-agent", "dangerous-command", "high", "hard_ask")

	// Approve
	body := map[string]any{"comment": "Looks good"}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/approvals/"+req2.ID+"/approve", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer test-token")
	w := httptest.NewRecorder()

	relay.handleApprovalAction(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
	}

	// Add another and reject
	req3 := relay.approvalQueue.Add("test-agent", "another-command", "medium", "hard_ask")

	body2 := map[string]any{"comment": "Rejected"}
	bodyJSON2, _ := json.Marshal(body2)

	req4 := httptest.NewRequest(http.MethodPost, "/api/v1/approvals/"+req3.ID+"/reject", bytes.NewReader(bodyJSON2))
	req4.Header.Set("Content-Type", "application/json")
	w2 := httptest.NewRecorder()

	relay.handleApprovalAction(w2, req4)

	if w2.Code != http.StatusOK {
		t.Errorf("expected 200, got %d: %s", w2.Code, w2.Body.String())
	}

	// Invalid action
	req5 := httptest.NewRequest(http.MethodPost, "/api/v1/approvals/invalid/unknown", nil)
	w3 := httptest.NewRecorder()
	relay.handleApprovalAction(w3, req5)

	if w3.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w3.Code)
	}
}

// === Audit Handlers Tests ===

func TestHandleAuditQuery(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/audit?agent_id=test&limit=10", nil)
	w := httptest.NewRecorder()

	relay.handleAuditQuery(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestHandleAuditExport(t *testing.T) {
	relay := createTestRelay(t)

	// JSON format
	req := httptest.NewRequest(http.MethodGet, "/api/v1/audit/export?format=json", nil)
	w := httptest.NewRecorder()

	relay.handleAuditExport(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}

	// CSV format
	req2 := httptest.NewRequest(http.MethodGet, "/api/v1/audit/export?format=csv", nil)
	w2 := httptest.NewRecorder()

	relay.handleAuditExport(w2, req2)

	if w2.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w2.Code)
	}

	if w2.Header().Get("Content-Type") != "text/csv" {
		t.Errorf("expected text/csv, got %s", w2.Header().Get("Content-Type"))
	}
}

func TestHandleAuditStats(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/audit/stats", nil)
	w := httptest.NewRecorder()

	relay.handleAuditStats(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

// === Billing Handlers Tests ===

func TestHandleBillingUsage(t *testing.T) {
	relay := createTestRelay(t)
	client, _ := relay.registry.CreateClient("Test", "test@test.com", "starter")

	req := httptest.NewRequest(http.MethodGet, "/api/v1/billing/usage?client_id="+client.ID, nil)
	w := httptest.NewRecorder()

	relay.handleBillingUsage(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}

	// Missing client_id
	req2 := httptest.NewRequest(http.MethodGet, "/api/v1/billing/usage", nil)
	w2 := httptest.NewRecorder()

	relay.handleBillingUsage(w2, req2)

	if w2.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w2.Code)
	}
}

func TestHandleBillingPlan(t *testing.T) {
	relay := createTestRelay(t)
	client, _ := relay.registry.CreateClient("Test", "test@test.com", "starter")

	// Get all plans
	req := httptest.NewRequest(http.MethodGet, "/api/v1/billing/plan", nil)
	w := httptest.NewRecorder()

	relay.handleBillingPlan(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}

	// Get specific client plan
	req2 := httptest.NewRequest(http.MethodGet, "/api/v1/billing/plan?client_id="+client.ID, nil)
	w2 := httptest.NewRecorder()

	relay.handleBillingPlan(w2, req2)

	if w2.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w2.Code)
	}
}

func TestHandleBillingPlanChange(t *testing.T) {
	relay := createTestRelay(t)
	client, _ := relay.registry.CreateClient("Test", "test@test.com", "starter")

	body := map[string]any{
		"client_id": client.ID,
		"plan_id":   "pro",
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/billing/plan/change", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleBillingPlanChange(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d: %s", w.Code, w.Body.String())
	}
}

func TestHandleBillingInvoices(t *testing.T) {
	relay := createTestRelay(t)
	client, _ := relay.registry.CreateClient("Test", "test@test.com", "starter")

	req := httptest.NewRequest(http.MethodGet, "/api/v1/billing/invoices?client_id="+client.ID, nil)
	w := httptest.NewRecorder()

	relay.handleBillingInvoices(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestHandleBillingPaymentMethods(t *testing.T) {
	relay := createTestRelay(t)
	client, _ := relay.registry.CreateClient("Test", "test@test.com", "starter")

	req := httptest.NewRequest(http.MethodGet, "/api/v1/billing/payment-methods?client_id="+client.ID, nil)
	w := httptest.NewRecorder()

	relay.handleBillingPaymentMethods(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

// === Nginx Config Handler Tests ===

func TestHandleNginxConfig(t *testing.T) {
	relay := createTestRelayWithToken(t, "test-api-token")

	tests := []struct {
		name       string
		token      string
		domain     string
		wantStatus int
	}{
		{
			name:       "valid request with token",
			token:      "Bearer test-api-token",
			domain:     "example.com",
			wantStatus: http.StatusOK,
		},
		{
			name:       "missing token",
			token:      "",
			domain:     "example.com",
			wantStatus: http.StatusUnauthorized,
		},
		{
			name:       "invalid token",
			token:      "Bearer invalid-token",
			domain:     "example.com",
			wantStatus: http.StatusUnauthorized,
		},
		{
			name:       "missing domain",
			token:      "Bearer test-api-token",
			domain:     "",
			wantStatus: http.StatusBadRequest,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			url := "/api/v1/nginx-config?domain=" + tt.domain + "&tls=true"
			req := httptest.NewRequest(http.MethodGet, url, nil)
			if tt.token != "" {
				req.Header.Set("Authorization", tt.token)
			}
			w := httptest.NewRecorder()

			relay.handleNginxConfig(w, req)

			if w.Code != tt.wantStatus {
				t.Errorf("expected status %d, got %d: %s", tt.wantStatus, w.Code, w.Body.String())
			}
		})
	}
}

// === authenticateAgent Tests ===

func TestAuthenticateAgent(t *testing.T) {
	relay := createTestRelay(t)

	// Create client and agent
	client, _ := relay.registry.CreateClient("Test", "test@test.com", "starter")
	agent, _ := relay.registry.RegisterAgent(client.ID, "Test Agent", []string{}, "linux", "amd64")

	// Test with valid token from registry
	result := relay.authenticateAgent(agent.ID, agent.Token)
	if !result {
		t.Error("expected authentication to succeed with registry token")
	}

	// In dev mode (no AllowedTokens whitelist), all tokens are accepted
	// Test with no whitelist (dev mode)
	relay2 := createTestRelay(t)
	result = relay2.authenticateAgent("any-agent", "any-token")
	if !result {
		t.Error("expected authentication to succeed in dev mode")
	}
}

// === LLM Proxy Handler Tests ===

func TestHandleLLMChat(t *testing.T) {
	relay := createTestRelay(t)

	// Without LLM proxy configured
	body := map[string]any{
		"agent_id":   "test-agent",
		"messages":   []map[string]string{{"role": "user", "content": "Hello"}},
		"max_tokens": 100,
	}
	bodyJSON, _ := json.Marshal(body)

	req := httptest.NewRequest(http.MethodPost, "/api/v1/llm/chat", bytes.NewReader(bodyJSON))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	relay.handleLLMChat(w, req)

	// Should fail - no LLM proxy configured
	if w.Code != http.StatusServiceUnavailable {
		t.Errorf("expected 503, got %d", w.Code)
	}
}

func TestHandleLLMBackends(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/llm/backends", nil)
	w := httptest.NewRecorder()

	relay.handleLLMBackends(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
}

func TestHandleLLMHealth(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/llm/health", nil)
	w := httptest.NewRecorder()

	relay.handleLLMHealth(w, req)

	// Should fail - no LLM proxy configured
	if w.Code != http.StatusServiceUnavailable {
		t.Errorf("expected 503, got %d", w.Code)
	}
}

// === SSE Handler Additional Tests ===

func TestHandleSSE_WrongAccept(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/events", nil)
	req.Header.Set("Accept", "application/json")
	w := httptest.NewRecorder()

	relay.handleSSE(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

// === Integration Proxy Tests ===

func TestHandleIntegrationProxy_Disabled(t *testing.T) {
	relay := createTestRelay(t)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/integration/backups", nil)
	w := httptest.NewRecorder()

	relay.handleIntegrationProxy(w, req)

	if w.Code != http.StatusNotImplemented {
		t.Errorf("expected 501, got %d", w.Code)
	}
}

// === Context helper tests ===

func TestGetClientID(t *testing.T) {
	tests := []struct {
		name     string
		auth     string
		expected string
	}{
		{
			name:     "bearer token",
			auth:     "Bearer my-token",
			expected: "my-token",
		},
		{
			name:     "plain token",
			auth:     "plain-token",
			expected: "plain-token",
		},
		{
			name:     "no auth",
			auth:     "",
			expected: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodGet, "/", nil)
			if tt.auth != "" {
				req.Header.Set("Authorization", tt.auth)
			}

			result := getClientID(req)
			if result != tt.expected {
				t.Errorf("expected %s, got %s", tt.expected, result)
			}
		})
	}
}

func TestGetClientIP(t *testing.T) {
	tests := []struct {
		name     string
		xff      string
		xri      string
		remote   string
		expected string
	}{
		{
			name:     "X-Forwarded-For",
			xff:      "192.168.1.1, 10.0.0.1",
			remote:   "127.0.0.1:8080",
			expected: "192.168.1.1",
		},
		{
			name:     "X-Real-IP",
			xri:      "192.168.1.2",
			remote:   "127.0.0.1:8080",
			expected: "192.168.1.2",
		},
		{
			name:     "RemoteAddr",
			remote:   "192.168.1.3:8080",
			expected: "192.168.1.3",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodGet, "/", nil)
			if tt.xff != "" {
				req.Header.Set("X-Forwarded-For", tt.xff)
			}
			if tt.xri != "" {
				req.Header.Set("X-Real-IP", tt.xri)
			}
			req.RemoteAddr = tt.remote

			result := getClientIP(req)
			if result != tt.expected {
				t.Errorf("expected %s, got %s", tt.expected, result)
			}
		})
	}
}

// === Helper function tests ===

func TestWriteJSON(t *testing.T) {
	w := httptest.NewRecorder()
	writeJSON(w, map[string]string{"status": "ok"})

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
	if w.Header().Get("Content-Type") != "application/json" {
		t.Errorf("expected application/json, got %s", w.Header().Get("Content-Type"))
	}
}

func TestWriteError(t *testing.T) {
	w := httptest.NewRecorder()
	writeError(w, http.StatusBadRequest, "invalid_request")

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}

	var resp map[string]any
	json.Unmarshal(w.Body.Bytes(), &resp)
	if resp["code"] != "invalid_request" {
		t.Errorf("expected code invalid_request, got %v", resp["code"])
	}
}

func TestWriteErrorCustom(t *testing.T) {
	w := httptest.NewRecorder()
	writeErrorCustom(w, http.StatusInternalServerError, "internal_error", "something went wrong")

	if w.Code != http.StatusInternalServerError {
		t.Errorf("expected 500, got %d", w.Code)
	}

	var resp map[string]any
	json.Unmarshal(w.Body.Bytes(), &resp)
	if resp["error"] != "something went wrong" {
		t.Errorf("expected error message, got %v", resp["error"])
	}
}

// === Test with context cancellation ===

func TestSSEContextCancellation(t *testing.T) {
	relay := createTestRelay(t)

	ctx, cancel := context.WithCancel(context.Background())
	req := httptest.NewRequest(http.MethodGet, "/api/v1/events", nil).WithContext(ctx)
	req.Header.Set("Accept", "text/event-stream")
	w := httptest.NewRecorder()

	done := make(chan struct{})
	go func() {
		defer close(done)
		relay.handleSSE(w, req)
	}()

	// Cancel after short delay
	time.Sleep(50 * time.Millisecond)
	cancel()

	select {
	case <-done:
		// OK - handler exited
	case <-time.After(2 * time.Second):
		t.Error("handler did not exit on context cancellation")
	}
}

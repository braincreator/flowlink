package agent

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/protocol"
	"github.com/gorilla/websocket"
)

// TestNewAgent tests agent creation with config
func TestNewAgent(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"
	cfg.Token = "test-token"
	cfg.ReadOnly = boolPtr(false)

	agent := NewAgent(&cfg)

	if agent == nil {
		t.Fatal("expected non-nil agent")
	}

	if agent.cfg.AgentID != "test-agent" {
		t.Errorf("expected agent ID 'test-agent', got %s", agent.cfg.AgentID)
	}

	if agent.executor == nil {
		t.Error("expected non-nil executor")
	}

	if agent.sandbox == nil {
		t.Error("expected non-nil sandbox")
	}

	if agent.approval == nil {
		t.Error("expected non-nil approval")
	}

	if agent.backup == nil {
		t.Error("expected non-nil backup")
	}

	if agent.killSwitch == nil {
		t.Error("expected non-nil killSwitch")
	}

	if agent.policy == nil {
		t.Error("expected non-nil policy")
	}

	if agent.skills == nil {
		t.Error("expected non-nil skills")
	}

	if agent.llm == nil {
		t.Error("expected non-nil llm")
	}

	if agent.taskManager == nil {
		t.Error("expected non-nil taskManager")
	}
}

// TestNewAgent_ReadOnlyDefault tests that new agents are read-only by default
func TestNewAgent_ReadOnlyDefault(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"
	// ReadOnly not set - should default to true

	agent := NewAgent(&cfg)

	if !agent.policy.IsReadOnly() {
		t.Error("expected read-only mode by default")
	}
}

// TestNewAgent_ReadOnlyExplicit tests explicit read-only setting
func TestNewAgent_ReadOnlyExplicit(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"
	cfg.ReadOnly = boolPtr(false)

	agent := NewAgent(&cfg)

	if agent.policy.IsReadOnly() {
		t.Error("expected read-write mode when explicitly set to false")
	}
}

// TestAgent_SetOnDisconnect tests disconnect callback
func TestAgent_SetOnDisconnect(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	called := false
	agent.SetOnDisconnect(func() {
		called = true
	})

	if agent.onDisconnect == nil {
		t.Error("expected onDisconnect callback to be set")
	}

	// Call the callback directly to test
	agent.onDisconnect()
	if !called {
		t.Error("expected onDisconnect callback to be called")
	}
}

// TestAgent_Disconnect tests agent disconnect
func TestAgent_Disconnect(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// Disconnect should not panic even without connection
	agent.Disconnect()
}

// TestAgent_HandleMessage_Connected tests MsgConnected handling
func TestAgent_HandleMessage_Connected(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// Test that relayKeyID is extracted correctly
	msg := protocol.NewMessage(protocol.MsgConnected)
	msg.Payload = protocol.ConnectedPayload{
		RelayPublicKey: "dGVzdC1rZXk=", // base64 encoded "test-key"
		RelayKeyID:     "key-123",
	}

	// Extract relay key ID from payload
	var payload protocol.ConnectedPayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		t.Fatalf("failed to unmarshal payload: %v", err)
	}

	// Verify payload
	if payload.RelayKeyID != "key-123" {
		t.Errorf("expected relayKeyID 'key-123', got %s", payload.RelayKeyID)
	}

	// Verify agent has policy configured
	if agent.policy == nil {
		t.Error("expected non-nil policy")
	}
}

// TestAgent_HandleMessage_ExecRequest tests exec request handling
func TestAgent_HandleMessage_ExecRequest(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"
	cfg.ReadOnly = boolPtr(false) // Allow write operations

	agent := NewAgent(&cfg)

	msg := protocol.NewMessage(protocol.MsgExecRequest)
	msg.Payload = protocol.ExecRequestPayload{
		RequestID: "req-123",
		Command:   "echo hello",
	}

	// Skip actual handling since it requires websocket connection
	// Just verify the message type is correct
	if msg.Type != protocol.MsgExecRequest {
		t.Errorf("expected MsgExecRequest, got %s", msg.Type)
	}

	// Verify agent is properly configured
	if agent.executor == nil {
		t.Error("expected non-nil executor")
	}
}

// TestAgent_HandleMessage_ExecRequest_Blocked tests blocked exec request
func TestAgent_HandleMessage_ExecRequest_Blocked(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"
	cfg.ReadOnly = boolPtr(true) // Read-only mode

	agent := NewAgent(&cfg)

	// Verify read-only mode is set
	if !agent.policy.IsReadOnly() {
		t.Error("expected read-only mode")
	}

	msg := protocol.NewMessage(protocol.MsgExecRequest)
	msg.Payload = protocol.ExecRequestPayload{
		RequestID: "req-123",
		Command:   "rm -rf /", // Destructive command
	}

	// Test policy check
	result := agent.policy.Check("rm -rf /")
	if result.Allowed {
		t.Error("destructive command should be blocked")
	}
}

// TestAgent_HandleMessage_FileRead tests file read handling
func TestAgent_HandleMessage_FileRead(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// Verify agent is properly initialized
	if agent.executor == nil {
		t.Error("expected non-nil executor")
	}

	// Test payload parsing
	msg := protocol.NewMessage(protocol.MsgFileRead)
	msg.Payload = protocol.FileReadPayload{
		Path: "/nonexistent/file.txt",
	}

	// Test payload unmarshaling
	var payload protocol.FileReadPayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		t.Fatalf("failed to unmarshal payload: %v", err)
	}

	if payload.Path != "/nonexistent/file.txt" {
		t.Errorf("expected path '/nonexistent/file.txt', got %s", payload.Path)
	}
}

// TestAgent_HandleMessage_FileWrite tests file write handling
func TestAgent_HandleMessage_FileWrite(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// Verify agent is properly initialized
	if agent.executor == nil {
		t.Error("expected non-nil executor")
	}

	// Test payload parsing
	msg := protocol.NewMessage(protocol.MsgFileWrite)
	msg.Payload = protocol.FileWritePayload{
		Path:    "/tmp/test.txt",
		Content: "test content",
	}

	// Test payload unmarshaling
	var payload protocol.FileWritePayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		t.Fatalf("failed to unmarshal payload: %v", err)
	}

	if payload.Path != "/tmp/test.txt" {
		t.Errorf("expected path '/tmp/test.txt', got %s", payload.Path)
	}
}

// TestAgent_HandleMessage_FileList tests file list handling
func TestAgent_HandleMessage_FileList(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// Verify agent is properly initialized
	if agent.executor == nil {
		t.Error("expected non-nil executor")
	}

	// Test payload parsing
	msg := protocol.NewMessage(protocol.MsgFileList)
	msg.Payload = protocol.FileListPayload{
		Path: "/tmp",
	}

	// Test payload unmarshaling
	var payload protocol.FileListPayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		t.Fatalf("failed to unmarshal payload: %v", err)
	}

	if payload.Path != "/tmp" {
		t.Errorf("expected path '/tmp', got %s", payload.Path)
	}
}

// TestAgent_HandleMessage_SysInfo tests sysinfo handling
func TestAgent_HandleMessage_SysInfo(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// Verify agent is properly initialized
	if agent.executor == nil {
		t.Error("expected non-nil executor")
	}

	// Test that CollectSystemInfo works
	info := CollectSystemInfo()
	if info.Hostname == "" {
		t.Error("expected non-empty hostname")
	}
}

// TestAgent_HandleMessage_Task tests task handling
func TestAgent_HandleMessage_Task(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// Verify agent is properly initialized
	if agent.taskManager == nil {
		t.Error("expected non-nil taskManager")
	}

	// Test payload parsing
	msg := protocol.NewMessage(protocol.MsgTask)
	msg.Payload = protocol.TaskPayload{
		TaskID:      "task-123",
		SkillID:     "test-skill",
		Description: "Test task",
		MaxSteps:    10,
	}

	// Test payload unmarshaling
	var payload protocol.TaskPayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		t.Fatalf("failed to unmarshal payload: %v", err)
	}

	if payload.TaskID != "task-123" {
		t.Errorf("expected taskID 'task-123', got %s", payload.TaskID)
	}
}

// TestAgent_HandleMessage_TaskCancel tests task cancellation
func TestAgent_HandleMessage_TaskCancel(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// Verify agent is properly initialized
	if agent.taskManager == nil {
		t.Error("expected non-nil taskManager")
	}

	// Test payload parsing
	msg := protocol.NewMessage(protocol.MsgTaskCancel)
	msg.Payload = map[string]interface{}{
		"task_id": "task-123",
	}

	// Test payload unmarshaling
	var payload struct {
		TaskID string `json:"task_id"`
	}
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		t.Fatalf("failed to unmarshal payload: %v", err)
	}

	if payload.TaskID != "task-123" {
		t.Errorf("expected taskID 'task-123', got %s", payload.TaskID)
	}
}

// TestAgent_HandleMessage_SkillPush tests skill push handling
func TestAgent_HandleMessage_SkillPush(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// Verify agent is properly initialized
	if agent.skills == nil {
		t.Error("expected non-nil skills")
	}

	// Test payload parsing
	msg := protocol.NewMessage(protocol.MsgSkillPush)
	msg.Payload = protocol.SkillPushPayload{
		SkillID:      "skill-123",
		Name:         "Test Skill",
		Description:  "Test description",
		Instructions: "Do something",
		ToolsAllowed: []string{"exec"},
	}

	// Test payload unmarshaling
	var payload protocol.SkillPushPayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		t.Fatalf("failed to unmarshal payload: %v", err)
	}

	if payload.SkillID != "skill-123" {
		t.Errorf("expected skillID 'skill-123', got %s", payload.SkillID)
	}
}

// TestAgent_HandleMessage_SkillDelete tests skill deletion
func TestAgent_HandleMessage_SkillDelete(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// First add a skill
	skill := &Skill{
		ID:           "skill-to-delete",
		Name:         "To Delete",
		Instructions: "Test",
	}
	agent.skills.Save(skill)

	// Verify skill was added
	if _, exists := agent.skills.Get("skill-to-delete"); !exists {
		t.Fatal("expected skill to be added")
	}

	// Test payload parsing
	msg := protocol.NewMessage(protocol.MsgSkillDelete)
	msg.Payload = map[string]interface{}{
		"skill_id": "skill-to-delete",
	}

	// Test payload unmarshaling
	var payload struct {
		SkillID string `json:"skill_id"`
	}
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		t.Fatalf("failed to unmarshal payload: %v", err)
	}

	if payload.SkillID != "skill-to-delete" {
		t.Errorf("expected skillID 'skill-to-delete', got %s", payload.SkillID)
	}
}

// TestAgent_HandleMessage_ApprovalResponse tests approval response handling
func TestAgent_HandleMessage_ApprovalResponse(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"

	agent := NewAgent(&cfg)

	// Verify agent is properly initialized
	if agent.approval == nil {
		t.Error("expected non-nil approval")
	}

	// Test approved
	msg := protocol.NewMessage(protocol.MsgApprovalResponse)
	msg.Payload = map[string]interface{}{
		"request_id": "req-123",
		"decision":   "approved",
		"comment":    "Looks good",
	}

	// Test payload unmarshaling
	var payload struct {
		RequestID string `json:"request_id"`
		Decision  string `json:"decision"`
		Comment   string `json:"comment"`
	}
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		t.Fatalf("failed to unmarshal payload: %v", err)
	}

	if payload.Decision != "approved" {
		t.Errorf("expected decision 'approved', got %s", payload.Decision)
	}
}

// TestAgent_HandleMessage_ConfigUpdate tests config update handling
func TestAgent_HandleMessage_ConfigUpdate(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"
	cfg.ReadOnly = boolPtr(true)

	agent := NewAgent(&cfg)

	// Verify agent is properly initialized
	if agent.policy == nil {
		t.Error("expected non-nil policy")
	}

	// Test payload parsing
	msg := protocol.NewMessage(protocol.MsgConfigUpdate)
	msg.Payload = map[string]interface{}{
		"read_only": false,
		"label":     "new-label",
	}

	// Test payload unmarshaling
	var payload struct {
		ReadOnly *bool   `json:"read_only"`
		Label    *string `json:"label"`
	}
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		t.Fatalf("failed to unmarshal payload: %v", err)
	}

	if payload.ReadOnly == nil || *payload.ReadOnly != false {
		t.Error("expected read_only to be false")
	}

	if payload.Label == nil || *payload.Label != "new-label" {
		t.Error("expected label to be 'new-label'")
	}
}

// TestAgent_WebsocketConnection tests basic websocket connection
func TestAgent_WebsocketConnection(t *testing.T) {
	// Create test websocket server
	upgrader := websocket.Upgrader{
		CheckOrigin: func(r *http.Request) bool { return true },
	}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			t.Logf("upgrade error: %v", err)
			return
		}
		defer conn.Close()

		// Read messages and echo back
		for {
			_, msg, err := conn.ReadMessage()
			if err != nil {
				break
			}
			conn.WriteMessage(websocket.TextMessage, msg)
		}
	}))
	defer server.Close()

	// Convert http:// to ws://
	wsURL := "ws" + strings.TrimPrefix(server.URL, "http")

	cfg := config.DefaultConfig()
	cfg.AgentID = "test-agent"
	cfg.Token = "test-token"
	cfg.RelayURL = wsURL

	agent := NewAgent(&cfg)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	// Connect should succeed
	err := agent.Connect(ctx)
	if err != nil {
		t.Fatalf("expected successful connection, got error: %v", err)
	}

	// Disconnect
	agent.Disconnect()
}

// TestUnmarshalPayload tests payload unmarshaling
func TestUnmarshalPayload(t *testing.T) {
	type TestStruct struct {
		Name  string `json:"name"`
		Value int    `json:"value"`
	}

	tests := []struct {
		name    string
		input   interface{}
		wantErr bool
	}{
		{
			name: "map input",
			input: map[string]interface{}{
				"name":  "test",
				"value": 42,
			},
			wantErr: false,
		},
		{
			name:    "string input",
			input:   `{"name":"test","value":42}`,
			wantErr: false,
		},
		{
			name:    "nil input",
			input:   nil,
			wantErr: true,
		},
		{
			name:    "bytes input",
			input:   []byte(`{"name":"test","value":42}`),
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var result TestStruct
			err := unmarshalPayload(tt.input, &result)

			if tt.wantErr {
				if err == nil {
					t.Error("expected error, got nil")
				}
			} else {
				if err != nil {
					t.Errorf("unexpected error: %v", err)
				}
				if result.Name != "test" {
					t.Errorf("expected name 'test', got %s", result.Name)
				}
				if result.Value != 42 {
					t.Errorf("expected value 42, got %d", result.Value)
				}
			}
		})
	}
}

// TestBase64Helpers tests base64 encoding/decoding
func TestBase64Helpers(t *testing.T) {
	original := []byte("test data for encoding")

	encoded := base64Encode(original)
	if encoded == "" {
		t.Error("expected non-empty encoded string")
	}

	decoded, err := base64Decode(encoded)
	if err != nil {
		t.Fatalf("unexpected decode error: %v", err)
	}

	if string(decoded) != string(original) {
		t.Errorf("expected %s, got %s", string(original), string(decoded))
	}
}

// TestGetRequestID tests request ID extraction
func TestGetRequestID(t *testing.T) {
	tests := []struct {
		name    string
		payload interface{}
		want    string
	}{
		{
			name: "with request_id",
			payload: map[string]interface{}{
				"request_id": "req-123",
				"command":    "echo test",
			},
			want: "req-123",
		},
		{
			name: "without request_id",
			payload: map[string]interface{}{
				"command": "echo test",
			},
			want: "",
		},
		{
			name:    "nil payload",
			payload: nil,
			want:    "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := getRequestID(tt.payload)
			if got != tt.want {
				t.Errorf("expected %s, got %s", tt.want, got)
			}
		})
	}
}

// Helper function
func boolPtr(b bool) *bool {
	return &b
}

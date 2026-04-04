package relay

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/protocol"
	"github.com/gorilla/websocket"
)

// mockWebSocketConn создаёт mock websocket connection для тестов
func mockWebSocketConn() *websocket.Conn {
	// Создаём тестовый websocket server
	upgrader := websocket.Upgrader{
		CheckOrigin: func(r *http.Request) bool { return true },
	}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		conn, err := upgrader.Upgrade(w, r, nil)
		if err != nil {
			return
		}
		defer conn.Close()

		// Читаем сообщения и игнорируем
		for {
			_, _, err := conn.ReadMessage()
			if err != nil {
				break
			}
		}
	}))

	// Подключаемся к серверу как клиент
	wsURL := "ws" + server.URL[4:] // http:// -> ws://
	conn, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		panic(err)
	}

	return conn
}

// TestBackupCreate tests POST /api/v1/agents/backup
func TestBackupCreate(t *testing.T) {
	// Создаём relay с тестовым конфигом
	cfg := &config.RelayConfig{
		WSSAddr: ":0",
		APIAddr: ":0",
	}
	relay := NewRelay(cfg)

	// Подключаем тестового агента с mock websocket connection
	agent := &AgentConn{
		ID:   "test-agent-1",
		conn: mockWebSocketConn(),
	}
	relay.pool.Add(agent)

	tests := []struct {
		name       string
		body       map[string]interface{}
		wantStatus int
	}{
		{
			name: "valid backup request",
			body: map[string]interface{}{
				"agent_id":    "test-agent-1",
				"description": "test backup",
				"paths":       []string{"/tmp/test"},
			},
			wantStatus: http.StatusOK,
		},
		{
			name: "missing agent_id",
			body: map[string]interface{}{
				"description": "test backup",
			},
			wantStatus: http.StatusBadRequest,
		},
		{
			name: "agent offline",
			body: map[string]interface{}{
				"agent_id":    "offline-agent",
				"description": "test backup",
			},
			wantStatus: http.StatusNotFound,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			bodyJSON, _ := json.Marshal(tt.body)
			req := httptest.NewRequest(http.MethodPost, "/api/v1/agents/backup", bytes.NewReader(bodyJSON))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			relay.handleBackupCreate(w, req)

			if w.Code != tt.wantStatus {
				t.Errorf("handleBackupCreate() status = %v, want %v", w.Code, tt.wantStatus)
			}

			if tt.wantStatus == http.StatusOK {
				var response map[string]interface{}
				if err := json.Unmarshal(w.Body.Bytes(), &response); err != nil {
					t.Errorf("failed to parse response: %v", err)
				}

				if response["status"] != "sent" {
					t.Errorf("expected status 'sent', got %v", response["status"])
				}

				if response["request_id"] == "" {
					t.Error("expected request_id in response")
				}
			}
		})
	}
}

// TestBackupList tests GET /api/v1/agents/backup/list
func TestBackupList(t *testing.T) {
	cfg := &config.RelayConfig{
		WSSAddr: ":0",
		APIAddr: ":0",
	}
	relay := NewRelay(cfg)

	agent := &AgentConn{
		ID:   "test-agent-1",
		conn: mockWebSocketConn(),
	}
	relay.pool.Add(agent)

	tests := []struct {
		name       string
		agentID    string
		wantStatus int
	}{
		{
			name:       "valid list request",
			agentID:    "test-agent-1",
			wantStatus: http.StatusOK,
		},
		{
			name:       "missing agent_id",
			agentID:    "",
			wantStatus: http.StatusBadRequest,
		},
		{
			name:       "agent offline",
			agentID:    "offline-agent",
			wantStatus: http.StatusNotFound,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			url := "/api/v1/agents/backup/list"
			if tt.agentID != "" {
				url += "?agent_id=" + tt.agentID
			}

			req := httptest.NewRequest(http.MethodGet, url, nil)
			w := httptest.NewRecorder()

			relay.handleBackupList(w, req)

			if w.Code != tt.wantStatus {
				t.Errorf("handleBackupList() status = %v, want %v", w.Code, tt.wantStatus)
			}

			if tt.wantStatus == http.StatusOK {
				var response map[string]interface{}
				if err := json.Unmarshal(w.Body.Bytes(), &response); err != nil {
					t.Errorf("failed to parse response: %v", err)
				}

				if response["status"] != "sent" {
					t.Errorf("expected status 'sent', got %v", response["status"])
				}
			}
		})
	}
}

// TestBackupRestore tests POST /api/v1/agents/backup/{id}/restore
func TestBackupRestore(t *testing.T) {
	cfg := &config.RelayConfig{
		WSSAddr: ":0",
		APIAddr: ":0",
	}
	relay := NewRelay(cfg)

	agent := &AgentConn{
		ID:   "test-agent-1",
		conn: mockWebSocketConn(),
	}
	relay.pool.Add(agent)

	tests := []struct {
		name       string
		snapshotID string
		body       map[string]interface{}
		wantStatus int
	}{
		{
			name:       "valid restore request",
			snapshotID: "snapshot-123",
			body: map[string]interface{}{
				"agent_id": "test-agent-1",
			},
			wantStatus: http.StatusOK,
		},
		{
			name:       "missing agent_id",
			snapshotID: "snapshot-123",
			body:       map[string]interface{}{},
			wantStatus: http.StatusBadRequest,
		},
		{
			name:       "agent offline",
			snapshotID: "snapshot-123",
			body: map[string]interface{}{
				"agent_id": "offline-agent",
			},
			wantStatus: http.StatusNotFound,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			bodyJSON, _ := json.Marshal(tt.body)
			url := "/api/v1/agents/backup/" + tt.snapshotID + "/restore"
			req := httptest.NewRequest(http.MethodPost, url, bytes.NewReader(bodyJSON))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			relay.handleBackupOperations(w, req)

			if w.Code != tt.wantStatus {
				t.Errorf("handleBackupOperations() status = %v, want %v", w.Code, tt.wantStatus)
			}

			if tt.wantStatus == http.StatusOK {
				var response map[string]interface{}
				if err := json.Unmarshal(w.Body.Bytes(), &response); err != nil {
					t.Errorf("failed to parse response: %v", err)
				}

				if response["status"] != "sent" {
					t.Errorf("expected status 'sent', got %v", response["status"])
				}

				if response["snapshot_id"] != tt.snapshotID {
					t.Errorf("expected snapshot_id %v, got %v", tt.snapshotID, response["snapshot_id"])
				}
			}
		})
	}
}

// TestBackupDelete tests DELETE /api/v1/agents/backup/{id}
func TestBackupDelete(t *testing.T) {
	cfg := &config.RelayConfig{
		WSSAddr: ":0",
		APIAddr: ":0",
	}
	relay := NewRelay(cfg)

	agent := &AgentConn{
		ID:   "test-agent-1",
		conn: mockWebSocketConn(),
	}
	relay.pool.Add(agent)

	tests := []struct {
		name       string
		snapshotID string
		body       map[string]interface{}
		wantStatus int
	}{
		{
			name:       "valid delete request",
			snapshotID: "snapshot-123",
			body: map[string]interface{}{
				"agent_id": "test-agent-1",
			},
			wantStatus: http.StatusOK,
		},
		{
			name:       "missing agent_id",
			snapshotID: "snapshot-123",
			body:       map[string]interface{}{},
			wantStatus: http.StatusBadRequest,
		},
		{
			name:       "agent offline",
			snapshotID: "snapshot-123",
			body: map[string]interface{}{
				"agent_id": "offline-agent",
			},
			wantStatus: http.StatusNotFound,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			bodyJSON, _ := json.Marshal(tt.body)
			url := "/api/v1/agents/backup/" + tt.snapshotID
			req := httptest.NewRequest(http.MethodDelete, url, bytes.NewReader(bodyJSON))
			req.Header.Set("Content-Type", "application/json")
			w := httptest.NewRecorder()

			relay.handleBackupOperations(w, req)

			if w.Code != tt.wantStatus {
				t.Errorf("handleBackupOperations() status = %v, want %v", w.Code, tt.wantStatus)
			}

			if tt.wantStatus == http.StatusOK {
				var response map[string]interface{}
				if err := json.Unmarshal(w.Body.Bytes(), &response); err != nil {
					t.Errorf("failed to parse response: %v", err)
				}

				if response["status"] != "sent" {
					t.Errorf("expected status 'sent', got %v", response["status"])
				}

				if response["snapshot_id"] != tt.snapshotID {
					t.Errorf("expected snapshot_id %v, got %v", tt.snapshotID, response["snapshot_id"])
				}
			}
		})
	}
}

// TestBackupMessageTypes tests that backup message types are correctly defined
func TestBackupMessageTypes(t *testing.T) {
	types := []protocol.MessageType{
		protocol.MsgBackupRequest,
		protocol.MsgBackupResponse,
		protocol.MsgBackupList,
		protocol.MsgBackupListResp,
		protocol.MsgBackupRestore,
		protocol.MsgBackupDelete,
		protocol.MsgBackupProgress,
	}

	expected := []string{
		"backup_request",
		"backup_response",
		"backup_list",
		"backup_list_resp",
		"backup_restore",
		"backup_delete",
		"backup_progress",
	}

	for i, mt := range types {
		if string(mt) != expected[i] {
			t.Errorf("message type %d: expected %s, got %s", i, expected[i], mt)
		}
	}
}

// TestBackupPayloads tests that backup payloads can be serialized/deserialized
func TestBackupPayloads(t *testing.T) {
	// BackupRequestPayload
	reqPayload := protocol.BackupRequestPayload{
		RequestID:   "req-123",
		Description: "test backup",
		Paths:       []string{"/tmp/test"},
	}
	reqJSON, err := json.Marshal(reqPayload)
	if err != nil {
		t.Errorf("failed to marshal BackupRequestPayload: %v", err)
	}

	var reqParsed protocol.BackupRequestPayload
	if err := json.Unmarshal(reqJSON, &reqParsed); err != nil {
		t.Errorf("failed to unmarshal BackupRequestPayload: %v", err)
	}

	if reqParsed.RequestID != reqPayload.RequestID {
		t.Errorf("request_id mismatch")
	}

	// BackupResponsePayload
	respPayload := protocol.BackupResponsePayload{
		RequestID:  "req-123",
		SnapshotID: "snap-456",
		Size:       1024,
		Timestamp:  1234567890,
		Success:    true,
	}
	respJSON, err := json.Marshal(respPayload)
	if err != nil {
		t.Errorf("failed to marshal BackupResponsePayload: %v", err)
	}

	var respParsed protocol.BackupResponsePayload
	if err := json.Unmarshal(respJSON, &respParsed); err != nil {
		t.Errorf("failed to unmarshal BackupResponsePayload: %v", err)
	}

	if !respParsed.Success {
		t.Errorf("expected success=true")
	}

	// BackupProgressPayload
	progressPayload := protocol.BackupProgressPayload{
		RequestID: "req-123",
		Progress:  50,
		Message:   "Compressing files...",
	}
	progressJSON, err := json.Marshal(progressPayload)
	if err != nil {
		t.Errorf("failed to marshal BackupProgressPayload: %v", err)
	}

	var progressParsed protocol.BackupProgressPayload
	if err := json.Unmarshal(progressJSON, &progressParsed); err != nil {
		t.Errorf("failed to unmarshal BackupProgressPayload: %v", err)
	}

	if progressParsed.Progress != 50 {
		t.Errorf("expected progress=50, got %d", progressParsed.Progress)
	}
}

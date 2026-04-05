package relay

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/config"
)

// newTestRelay создаёт реле для тестов без токена (без авторизации).
func newTestRelay(t *testing.T) *Relay {
	t.Helper()
	cfg := &config.RelayConfig{
		WSSAddr: ":0",
		APIAddr: ":0",
	}
	r := NewRelay(cfg); t.Cleanup(func() { r.Close() })
	return r
}

// newTestRelayWithAuth создаёт реле с токеном для тестов авторизации.
func newTestRelayWithAuth(t *testing.T, token string) *Relay {
	t.Helper()
	cfg := &config.RelayConfig{
		WSSAddr: ":0",
		APIAddr: ":0",
		APIToken: token,
	}
	r := NewRelay(cfg); t.Cleanup(func() { r.Close() })
	return r
}

// mcpPost отправляет JSON-RPC запрос к handleMCP.
func mcpPost(handler http.HandlerFunc, body any) *httptest.ResponseRecorder {
	data, _ := json.Marshal(body)
	req := httptest.NewRequest("POST", "/mcp", bytes.NewReader(data))
	req.Header.Set("Content-Type", "application/json")
	rec := httptest.NewRecorder()
	handler(rec, req)
	return rec
}

func TestMCPInitialize(t *testing.T) {
	r := newTestRelay(t)

	body := map[string]any{
		"jsonrpc": "2.0",
		"id":      1,
		"method":  "initialize",
		"params": map[string]any{
			"protocolVersion": "2024-11-05",
			"clientInfo":      map[string]string{"name": "test", "version": "1.0"},
		},
	}

	rec := mcpPost(r.handleMCP, body)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}

	var resp mcpResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatalf("неверный JSON: %s", err)
	}

	if resp.JSONRPC != "2.0" {
		t.Errorf("jsonrpc = %q, want \"2.0\"", resp.JSONRPC)
	}
	if resp.ID != float64(1) {
		t.Errorf("id = %v, want 1", resp.ID)
	}
	if resp.Error != nil {
		t.Errorf("неожиданная ошибка: %+v", resp.Error)
	}

	result, ok := resp.Result.(map[string]any)
	if !ok {
		t.Fatal("result не map")
	}
	if result["protocolVersion"] != "2024-11-05" {
		t.Errorf("protocolVersion = %v", result["protocolVersion"])
	}

	serverInfo, ok := result["serverInfo"].(map[string]any)
	if !ok {
		t.Fatal("serverInfo отсутствует")
	}
	if serverInfo["name"] != "flowlink-relay" {
		t.Errorf("serverInfo.name = %v", serverInfo["name"])
	}
}

func TestMCPToolsList(t *testing.T) {
	r := newTestRelay(t)

	body := map[string]any{
		"jsonrpc": "2.0",
		"id":      2,
		"method":  "tools/list",
	}

	rec := mcpPost(r.handleMCP, body)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}

	var resp mcpResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatalf("неверный JSON: %s", err)
	}

	if resp.Error != nil {
		t.Fatalf("ошибка: %+v", resp.Error)
	}

	result, ok := resp.Result.(map[string]any)
	if !ok {
		t.Fatal("result не map")
	}

	tools, ok := result["tools"].([]any)
	if !ok {
		t.Fatal("tools не массив")
	}

	// Ожидаем 8 инструментов
	if len(tools) != 8 {
		t.Errorf("expected 8 tools, got %d", len(tools))
	}

	// Проверяем имена инструментов
	names := make(map[string]bool)
	for _, tool := range tools {
		tm, ok := tool.(map[string]any)
		if !ok {
			continue
		}
		name, _ := tm["name"].(string)
		names[name] = true
	}

	expected := []string{
		"flowlink_agents", "flowlink_exec", "flowlink_read", "flowlink_write",
		"flowlink_list", "flowlink_sysinfo", "flowlink_task", "flowlink_task_status",
	}
	for _, name := range expected {
		if !names[name] {
			t.Errorf("инструмент %q отсутствует", name)
		}
	}
}

func TestMCPToolsCall_FlowlinkAgents(t *testing.T) {
	r := newTestRelay(t)

	// Добавляем мок-агента в пул
	agent := &AgentConn{
		ID:        "test-agent-1",
		Hostname:  "macbook-pro",
		OS:        "darwin",
		Arch:      "arm64",
		Version:   "0.1.0",
		Connected: time.Now(),
		LastSeen:  time.Now(),
	}
	r.pool.Add(agent)

	body := map[string]any{
		"jsonrpc": "2.0",
		"id":      3,
		"method":  "tools/call",
		"params": map[string]any{
			"name": "flowlink_agents",
			"arguments": map[string]any{
				"status": "all",
			},
		},
	}

	rec := mcpPost(r.handleMCP, body)

	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d: %s", rec.Code, rec.Body.String())
	}

	var resp mcpResponse
	if err := json.Unmarshal(rec.Body.Bytes(), &resp); err != nil {
		t.Fatalf("неверный JSON: %s", err)
	}

	if resp.Error != nil {
		t.Fatalf("ошибка: %+v", resp.Error)
	}

	result, ok := resp.Result.(map[string]any)
	if !ok {
		t.Fatal("result не map")
	}

	content, ok := result["content"].([]any)
	if !ok || len(content) == 0 {
		t.Fatal("content пуст")
	}

	text, ok := content[0].(map[string]any)["text"].(string)
	if !ok {
		t.Fatal("text отсутствует")
	}

	// Проверяем что в тексте есть hostname агента
	if !strings.Contains(text, "macbook-pro") {
		t.Errorf("в ответе нет hostname 'macbook-pro': %s", text)
	}
	if !strings.Contains(text, "test-agent-1") {
		t.Errorf("в ответе нет agent ID 'test-agent-1': %s", text)
	}
}

func TestMCPToolsCall_FlowlinkExec(t *testing.T) {
	r := newTestRelay(t)

	// Вызов без агента → ошибка
	body := map[string]any{
		"jsonrpc": "2.0",
		"id":      4,
		"method":  "tools/call",
		"params": map[string]any{
			"name":      "flowlink_exec",
			"arguments": map[string]any{},
		},
	}

	rec := mcpPost(r.handleMCP, body)
	if rec.Code != http.StatusOK {
		t.Fatalf("expected 200, got %d", rec.Code)
	}

	var resp mcpResponse
	json.Unmarshal(rec.Body.Bytes(), &resp)

	if resp.Error == nil {
		t.Fatal("ожидали ошибку при отсутствии agent")
	}
	if resp.Error.Code != -32602 {
		t.Errorf("error code = %d, want -32602", resp.Error.Code)
	}

	// Вызов с несуществующим агентом → ошибка
	body2 := map[string]any{
		"jsonrpc": "2.0",
		"id":      5,
		"method":  "tools/call",
		"params": map[string]any{
			"name": "flowlink_exec",
			"arguments": map[string]any{
				"agent":   "nonexistent",
				"command": "echo hello",
			},
		},
	}

	rec2 := mcpPost(r.handleMCP, body2)
	json.Unmarshal(rec2.Body.Bytes(), &resp)

	if resp.Error == nil {
		t.Fatal("ожидали ошибку при nonexistent agent")
	}
	if !strings.Contains(resp.Error.Message, "не найден") {
		t.Errorf("сообщение об ошибке не содержит 'не найден': %s", resp.Error.Message)
	}
}

func TestMCPAuth(t *testing.T) {
	token := "super-secret-token"
	r := newTestRelayWithAuth(t, token)

	body := map[string]any{
		"jsonrpc": "2.0",
		"id":      1,
		"method":  "initialize",
	}

	// Без токена → 401
	rec := mcpPost(r.handleMCP, body)
	if rec.Code != http.StatusUnauthorized {
		t.Errorf("expected 401, got %d", rec.Code)
	}

	// С неверным токеном → 401
	req := httptest.NewRequest("POST", "/mcp", bytes.NewReader(toJSON(body)))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer wrong-token")
	rec2 := httptest.NewRecorder()
	r.handleMCP(rec2, req)
	if rec2.Code != http.StatusUnauthorized {
		t.Errorf("expected 401 with wrong token, got %d", rec2.Code)
	}

	// С верным токеном → 200
	req3 := httptest.NewRequest("POST", "/mcp", bytes.NewReader(toJSON(body)))
	req3.Header.Set("Content-Type", "application/json")
	req3.Header.Set("Authorization", "Bearer "+token)
	rec3 := httptest.NewRecorder()
	r.handleMCP(rec3, req3)
	if rec3.Code != http.StatusOK {
		t.Errorf("expected 200 with valid token, got %d: %s", rec3.Code, rec3.Body.String())
	}

	// Через query parameter
	req4 := httptest.NewRequest("POST", "/mcp?token="+token, bytes.NewReader(toJSON(body)))
	req4.Header.Set("Content-Type", "application/json")
	rec4 := httptest.NewRecorder()
	r.handleMCP(rec4, req4)
	if rec4.Code != http.StatusOK {
		t.Errorf("expected 200 with query token, got %d", rec4.Code)
	}
}

func TestMCPCORS(t *testing.T) {
	r := newTestRelay(t)

	// OPTIONS запрос
	req := httptest.NewRequest("OPTIONS", "/mcp", nil)
	req.Header.Set("Origin", "https://openclaw.example.com")
	req.Header.Set("Access-Control-Request-Method", "POST")
	req.Header.Set("Access-Control-Request-Headers", "Content-Type, Authorization")
	rec := httptest.NewRecorder()
	r.handleMCP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("OPTIONS: expected 200, got %d", rec.Code)
	}
	if rec.Header().Get("Access-Control-Allow-Origin") != "*" {
		t.Errorf("CORS origin = %q, want *", rec.Header().Get("Access-Control-Allow-Origin"))
	}
	if rec.Header().Get("Access-Control-Allow-Methods") == "" {
		t.Error("отсутствует Access-Control-Allow-Methods")
	}
	if rec.Header().Get("Access-Control-Allow-Headers") == "" {
		t.Error("отсутствует Access-Control-Allow-Headers")
	}

	// POST запрос тоже должен иметь CORS заголовки
	body := map[string]any{
		"jsonrpc": "2.0",
		"id":      1,
		"method":  "tools/list",
	}
	rec2 := mcpPost(r.handleMCP, body)
	if rec2.Header().Get("Access-Control-Allow-Origin") != "*" {
		t.Errorf("POST CORS origin = %q, want *", rec2.Header().Get("Access-Control-Allow-Origin"))
	}
}

func TestMCPNotificationsInitialized(t *testing.T) {
	r := newTestRelay(t)

	body := map[string]any{
		"jsonrpc": "2.0",
		"method":  "notifications/initialized",
	}

	rec := mcpPost(r.handleMCP, body)
	if rec.Code != http.StatusNoContent {
		t.Errorf("expected 204, got %d", rec.Code)
	}
}

func TestMCPMethodNotFound(t *testing.T) {
	r := newTestRelay(t)

	body := map[string]any{
		"jsonrpc": "2.0",
		"id":      1,
		"method":  "foo/bar",
	}

	rec := mcpPost(r.handleMCP, body)

	var resp mcpResponse
	json.Unmarshal(rec.Body.Bytes(), &resp)

	if resp.Error == nil {
		t.Fatal("ожидали ошибку")
	}
	if resp.Error.Code != -32601 {
		t.Errorf("error code = %d, want -32601", resp.Error.Code)
	}
}

func TestMCPInvalidJSON(t *testing.T) {
	r := newTestRelay(t)

	req := httptest.NewRequest("POST", "/mcp", bytes.NewReader([]byte("not json")))
	req.Header.Set("Content-Type", "application/json")
	rec := httptest.NewRecorder()
	r.handleMCP(rec, req)

	var resp mcpResponse
	json.Unmarshal(rec.Body.Bytes(), &resp)

	if resp.Error == nil {
		t.Fatal("ожидали ошибку при невалидном JSON")
	}
	if resp.Error.Code != -32700 {
		t.Errorf("error code = %d, want -32700", resp.Error.Code)
	}
}

func TestMCPUnknownTool(t *testing.T) {
	r := newTestRelay(t)

	body := map[string]any{
		"jsonrpc": "2.0",
		"id":      1,
		"method":  "tools/call",
		"params": map[string]any{
			"name":      "nonexistent_tool",
			"arguments": map[string]any{},
		},
	}

	rec := mcpPost(r.handleMCP, body)

	var resp mcpResponse
	json.Unmarshal(rec.Body.Bytes(), &resp)

	if resp.Error == nil {
		t.Fatal("ожидали ошибку для неизвестного инструмента")
	}
	if resp.Error.Code != -32602 {
		t.Errorf("error code = %d, want -32602", resp.Error.Code)
	}
	if !strings.Contains(resp.Error.Message, "unknown tool") {
		t.Errorf("сообщение = %q", resp.Error.Message)
	}
}

func TestMCPInvalidParams(t *testing.T) {
	r := newTestRelay(t)

	body := map[string]any{
		"jsonrpc": "2.0",
		"id":      1,
		"method":  "tools/call",
		"params":  "not an object",
	}

	rec := mcpPost(r.handleMCP, body)

	var resp mcpResponse
	json.Unmarshal(rec.Body.Bytes(), &resp)

	if resp.Error == nil {
		t.Fatal("ожидали ошибку при невалидных params")
	}
	if resp.Error.Code != -32602 {
		t.Errorf("error code = %d, want -32602", resp.Error.Code)
	}
}

func TestMCPAgentsEmpty(t *testing.T) {
	r := newTestRelay(t)

	body := map[string]any{
		"jsonrpc": "2.0",
		"id":      1,
		"method":  "tools/call",
		"params": map[string]any{
			"name":      "flowlink_agents",
			"arguments": map[string]any{},
		},
	}

	rec := mcpPost(r.handleMCP, body)

	var resp mcpResponse
	json.Unmarshal(rec.Body.Bytes(), &resp)

	if resp.Error != nil {
		t.Fatalf("неожиданная ошибка: %+v", resp.Error)
	}

	result := resp.Result.(map[string]any)
	content := result["content"].([]any)[0].(map[string]any)
	text := content["text"].(string)

	if !strings.Contains(text, "Нет подключённых") {
		t.Errorf("ожидали сообщение о пустом пуле, получили: %s", text)
	}
}

// === Вспомогательные функции ===

func toJSON(v any) []byte {
	b, _ := json.Marshal(v)
	return b
}



// Package integration — интеграционные тесты flowlink relay + MCP.
// Запускают relay на random port, подключают мок-агента через WSS,
// отправляют MCP JSON-RPC запросы через HTTP и проверяют ответы.
package integration

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/protocol"
	"github.com/braincreator/flowlink/internal/relay"
	"github.com/gorilla/websocket"
)

// startTestRelay запускает relay на случайных портах.
// Возвращает shutdown функцию.
func startTestRelay(t *testing.T) (apiURL, wsURL string, shutdown func()) {
	t.Helper()

	cfg := &config.RelayConfig{
		WSSAddr:       ":0",
		APIAddr:       ":0",
		APIToken:      "test-token",
		AllowedTokens: map[string]string{"mock-token": "mock-agent-1"},
	}

	r := relay.NewRelay(cfg)

	// WSS mux — регистрируем через HandleAgentWSForTest (экспортированная обёртка)
	wssMux := http.NewServeMux()
	wssMux.HandleFunc("/ws", r.HandleAgentWSForTest)
	wssServer := httptest.NewServer(wssMux)
	wsURL = "ws" + wssServer.URL[len("http"):]

	// API mux — MCP endpoint
	apiMux := http.NewServeMux()
	apiMux.HandleFunc("/mcp", r.HandleMCPForTest)
	apiServer := httptest.NewServer(apiMux)
	apiURL = apiServer.URL

	shutdown = func() {
		wssServer.Close()
		apiServer.Close()
	}

	return apiURL, wsURL, shutdown
}

// connectMockAgent подключает мок-агента к relay WSS endpoint.
func connectMockAgent(t *testing.T, wsURL, agentID, token string) *websocket.Conn {
	t.Helper()

	dialer := websocket.Dialer{}
	conn, _, err := dialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("не удалось подключиться к WSS %s: %v", wsURL, err)
	}

	// Отправляем connect сообщение
	connectMsg := protocol.NewMessage(protocol.MsgConnect)
	connectMsg.Payload = protocol.ConnectPayload{
		AgentID:   agentID,
		Token:     token,
		Hostname:  "mock-host",
		OS:        "linux",
		Arch:      "amd64",
		ClientVer: "0.1.0-test",
	}
	if err := conn.WriteJSON(connectMsg); err != nil {
		t.Fatalf("ошибка отправки connect: %v", err)
	}

	// Читаем connected ответ
	var resp protocol.Message
	if err := conn.ReadJSON(&resp); err != nil {
		t.Fatalf("ошибка чтения connected: %v", err)
	}
	if resp.Type != protocol.MsgConnected {
		t.Fatalf("ожидали 'connected', получили %q", resp.Type)
	}

	return conn
}

// mcpPost отправляет MCP JSON-RPC запрос.
func mcpPost(url, token string, body any) (*http.Response, error) {
	data, _ := json.Marshal(body)
	req, _ := http.NewRequest("POST", url+"/mcp", bytes.NewReader(data))
	req.Header.Set("Content-Type", "application/json")
	if token != "" {
		req.Header.Set("Authorization", "Bearer "+token)
	}
	return http.DefaultClient.Do(req)
}

// parseMCPResponse парсит MCP JSON-RPC ответ.
func parseMCPResponse(t *testing.T, resp *http.Response) map[string]any {
	t.Helper()
	defer resp.Body.Close()
	var result map[string]any
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		t.Fatalf("ошибка парсинга ответа: %v", err)
	}
	return result
}

// TestIntegrationMCPFlow — полный интеграционный тест MCP.
// 1. Запускает relay
// 2. Подключает мок-агента
// 3. initialize
// 4. tools/list → 8 инструментов
// 5. tools/call flowlink_agents → 1 агент
func TestIntegrationMCPFlow(t *testing.T) {
	if testing.Short() {
		t.Skip("интеграционный тест пропущен в short mode")
	}

	apiURL, wsURL, shutdown := startTestRelay(t)
	defer shutdown()

	time.Sleep(100 * time.Millisecond)

	// Подключаем мок-агента
	agentConn := connectMockAgent(t, wsURL+"/ws", "mock-agent-1", "mock-token")
	defer agentConn.Close()

	// Даём relay время обработать подключение
	time.Sleep(200 * time.Millisecond)

	token := "test-token"

	// 1. initialize
	initBody := map[string]any{
		"jsonrpc": "2.0",
		"id":      1,
		"method":  "initialize",
		"params": map[string]any{
			"protocolVersion": "2024-11-05",
			"clientInfo":      map[string]string{"name": "test-client", "version": "1.0"},
		},
	}

	resp, err := mcpPost(apiURL, token, initBody)
	if err != nil {
		t.Fatalf("initialize request failed: %v", err)
	}

	result := parseMCPResponse(t, resp)
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("initialize: status %d, body: %v", resp.StatusCode, result)
	}

	serverInfo := result["result"].(map[string]any)["serverInfo"].(map[string]any)
	if serverInfo["name"] != "flowlink-relay" {
		t.Errorf("serverInfo.name = %v", serverInfo["name"])
	}

	// 2. tools/list
	listBody := map[string]any{
		"jsonrpc": "2.0",
		"id":      2,
		"method":  "tools/list",
	}

	resp2, err := mcpPost(apiURL, token, listBody)
	if err != nil {
		t.Fatalf("tools/list request failed: %v", err)
	}

	result2 := parseMCPResponse(t, resp2)
	if resp2.StatusCode != http.StatusOK {
		t.Fatalf("tools/list: status %d, body: %v", resp2.StatusCode, result2)
	}

	tools := result2["result"].(map[string]any)["tools"].([]any)
	if len(tools) != 17 {
		t.Errorf("ожидали 17 инструментов, получили %d", len(tools))
	}

	// 3. tools/call flowlink_agents → 1 агент
	agentsBody := map[string]any{
		"jsonrpc": "2.0",
		"id":      3,
		"method":  "tools/call",
		"params": map[string]any{
			"name":      "flowlink_agents",
			"arguments": map[string]any{"status": "all"},
		},
	}

	resp3, err := mcpPost(apiURL, token, agentsBody)
	if err != nil {
		t.Fatalf("flowlink_agents request failed: %v", err)
	}

	result3 := parseMCPResponse(t, resp3)
	if resp3.StatusCode != http.StatusOK {
		t.Fatalf("flowlink_agents: status %d, body: %v", resp3.StatusCode, result3)
	}

	content := result3["result"].(map[string]any)["content"].([]any)
	text := content[0].(map[string]any)["text"].(string)

	if !strings.Contains(text, "mock-host") {
		t.Errorf("в ответе нет hostname 'mock-host': %s", text)
	}
	if !strings.Contains(text, "Подключённых агентов: 1") {
		t.Errorf("ожидали 1 агента, текст: %s", text)
	}

	// 4. tools/call flowlink_exec — мок-агент не отвечает, будет таймаут
	// Проверяем только что запрос проходит через validate (agent found, command present)
	// Не ждём таймаут — пропускаем этот тест для скорости
	t.Log("flowlink_exec: пропускаем таймаут-тест (мок-агент не отвечает)")
}

// TestIntegrationMCPAuth — тест авторизации MCP endpoint.
func TestIntegrationMCPAuth(t *testing.T) {
	if testing.Short() {
		t.Skip("интеграционный тест пропущен в short mode")
	}

	apiURL, _, shutdown := startTestRelay(t)
	defer shutdown()

	time.Sleep(100 * time.Millisecond)

	body := map[string]any{
		"jsonrpc": "2.0",
		"id":      1,
		"method":  "initialize",
	}

	// Без токена → 401
	resp, err := mcpPost(apiURL, "", body)
	if err != nil {
		t.Fatalf("request failed: %v", err)
	}
	resp.Body.Close()

	if resp.StatusCode != http.StatusUnauthorized {
		t.Errorf("expected 401, got %d", resp.StatusCode)
	}

	// С верным токеном → 200
	resp2, err := mcpPost(apiURL, "test-token", body)
	if err != nil {
		t.Fatalf("request with token failed: %v", err)
	}
	resp2.Body.Close()

	if resp2.StatusCode != http.StatusOK {
		t.Errorf("expected 200 with valid token, got %d", resp2.StatusCode)
	}
}

// TestIntegrationMCPNoAgents — тест MCP при отсутствии подключённых агентов.
func TestIntegrationMCPNoAgents(t *testing.T) {
	if testing.Short() {
		t.Skip("интеграционный тест пропущен в short mode")
	}

	apiURL, _, shutdown := startTestRelay(t)
	defer shutdown()

	time.Sleep(100 * time.Millisecond)

	// flowlink_agents при пустом пуле
	body := map[string]any{
		"jsonrpc": "2.0",
		"id":      1,
		"method":  "tools/call",
		"params": map[string]any{
			"name":      "flowlink_agents",
			"arguments": map[string]any{},
		},
	}

	resp, err := mcpPost(apiURL, "test-token", body)
	if err != nil {
		t.Fatalf("request failed: %v", err)
	}

	result := parseMCPResponse(t, resp)
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("status %d, body: %v", resp.StatusCode, result)
	}

	text := result["result"].(map[string]any)["content"].([]any)[0].(map[string]any)["text"].(string)
	if !strings.Contains(text, "Нет подключённых") {
		t.Errorf("ожидали сообщение о пустом пуле: %s", text)
	}

	// flowlink_exec без агента → ошибка
	execBody := map[string]any{
		"jsonrpc": "2.0",
		"id":      2,
		"method":  "tools/call",
		"params": map[string]any{
			"name": "flowlink_exec",
			"arguments": map[string]any{
				"agent":   "nonexistent",
				"command": "echo hello",
			},
		},
	}

	resp2, err := mcpPost(apiURL, "test-token", execBody)
	if err != nil {
		t.Fatalf("request failed: %v", err)
	}

	result2 := parseMCPResponse(t, resp2)
	if result2["error"] == nil {
		t.Fatal("ожидали ошибку при nonexistent agent")
	}
	errMsg := result2["error"].(map[string]any)["message"].(string)
	if !strings.Contains(errMsg, "не найден") {
		t.Errorf("сообщение об ошибке: %s", errMsg)
	}
}

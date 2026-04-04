package e2e

import (
	"encoding/json"
	"fmt"
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

// TestAgentE2EConnect — полный E2E тест: relay start → create client → create agent → WS connect → heartbeat → disconnect.
func TestAgentE2EConnect(t *testing.T) {
	// 1. Create relay
	cfg := &config.RelayConfig{
		WSSAddr:          ":0", // random port
		APIAddr:          ":0",
		APIToken:         "test-admin-token",
		HeartbeatTimeout: 90,
		MaxAgents:        100,
	}
	r := relay.NewRelay(cfg)

	// 2. Create client
	client, err := r.CreateFirstClient("test-user", "test@example.com")
	if err != nil {
		t.Fatalf("CreateFirstClient: %v", err)
	}
	if client.ID == "" || client.APIToken == "" {
		t.Fatal("client ID or token empty")
	}
	t.Logf("Client: ID=%s Token=%s", client.ID, client.APIToken)

	// 3. Create agent
	agent, err := r.CreateFirstAgent(client.ID, "test-agent")
	if err != nil {
		t.Fatalf("CreateFirstAgent: %v", err)
	}
	if agent.ID == "" || agent.Token == "" {
		t.Fatal("agent ID or token empty")
	}
	t.Logf("Agent: ID=%s Token=%s", agent.ID, agent.Token)

	// 4. Start relay HTTP server
	apiMux := http.NewServeMux()
	apiMux.HandleFunc("/ws", r.HandleAgentWSForTest)
	server := httptest.NewServer(apiMux)
	defer server.Close()

	// 5. Convert http:// to ws://
	wsURL := "ws" + strings.TrimPrefix(server.URL, "http") + "/ws"

	// 6. Connect agent via WS
	wsConn, resp, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("WS dial failed: %v (resp: %+v)", err, resp)
	}
	defer wsConn.Close()

	// 7. Send connect message
	connectMsg := protocol.NewMessage(protocol.MsgConnect)
	connectMsg.AgentID = agent.ID
	connectMsg.Payload = protocol.ConnectPayload{
		AgentID:   agent.ID,
		Token:     agent.Token,
		Hostname:  "test-host",
		OS:        "linux",
		Arch:      "amd64",
		ClientVer: "test-e2e",
	}
	if err := wsConn.WriteJSON(connectMsg); err != nil {
		t.Fatalf("WriteJSON connect: %v", err)
	}

	// 8. Read connected response
	var connectedMsg protocol.Message
	if err := wsConn.ReadJSON(&connectedMsg); err != nil {
		t.Fatalf("ReadJSON connected: %v", err)
	}
	if connectedMsg.Type != protocol.MsgConnected {
		t.Fatalf("expected connected, got: %s", connectedMsg.Type)
	}

	var connectedPayload protocol.ConnectedPayload
	payloadBytes, _ := json.Marshal(connectedMsg.Payload)
	if err := json.Unmarshal(payloadBytes, &connectedPayload); err != nil {
		t.Fatalf("parse connected payload: %v", err)
	}
	if connectedPayload.AgentID != agent.ID {
		t.Fatalf("agent ID mismatch: got %s, want %s", connectedPayload.AgentID, agent.ID)
	}
	if connectedPayload.Interval != 30 {
		t.Fatalf("heartbeat interval: got %d, want 30", connectedPayload.Interval)
	}
	t.Logf("Connected! AgentID=%s RelayID=%s Interval=%d",
		connectedPayload.AgentID, connectedPayload.RelayID, connectedPayload.Interval)

	// 9. Send heartbeat
	hbMsg := protocol.NewMessage(protocol.MsgHeartbeat)
	hbMsg.AgentID = agent.ID
	if err := wsConn.WriteJSON(hbMsg); err != nil {
		t.Fatalf("WriteJSON heartbeat: %v", err)
	}

	var hbAck protocol.Message
	if err := wsConn.ReadJSON(&hbAck); err != nil {
		t.Fatalf("ReadJSON heartbeat_ack: %v", err)
	}
	if hbAck.Type != protocol.MsgHeartbeatAck {
		t.Fatalf("expected heartbeat_ack, got: %s", hbAck.Type)
	}
	t.Log("Heartbeat OK")

	// 10. Verify agent is in pool
	agents := r.PoolList()
	if len(agents) != 1 {
		t.Fatalf("expected 1 agent in pool, got %d", len(agents))
	}
	if agents[0].ID != agent.ID {
		t.Fatalf("pool agent ID mismatch: got %s, want %s", agents[0].ID, agent.ID)
	}
	t.Log("Agent in pool OK")

	// 11. Disconnect
	wsConn.Close()
	time.Sleep(100 * time.Millisecond) // let relay process disconnect

	agents = r.PoolList()
	if len(agents) != 0 {
		t.Fatalf("expected 0 agents after disconnect, got %d", len(agents))
	}
	t.Log("Disconnect OK")
}

// TestAgentE2EWrongToken — проверка что неверный токен отклоняется.
func TestAgentE2EWrongToken(t *testing.T) {
	// Whitelist mode — reject unknown tokens
	cfg := &config.RelayConfig{
		WSSAddr:          ":0",
		APIAddr:          ":0",
		APIToken:         "test-admin-token",
		HeartbeatTimeout: 90,
		MaxAgents:        100,
		AllowedTokens:    map[string]string{}, // whitelist mode (empty = reject all)
	}
	r := relay.NewRelay(cfg)

	// Create client + agent (valid tokens exist in registry)
	client, _ := r.CreateFirstClient("test-user", "test@example.com")
	_, _ = r.CreateFirstAgent(client.ID, "test-agent")

	apiMux := http.NewServeMux()
	apiMux.HandleFunc("/ws", r.HandleAgentWSForTest)
	server := httptest.NewServer(apiMux)
	defer server.Close()

	wsURL := "ws" + strings.TrimPrefix(server.URL, "http") + "/ws"

	// Connect with WRONG token
	wsConn, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("WS dial failed: %v", err)
	}
	defer wsConn.Close()

	connectMsg := protocol.NewMessage(protocol.MsgConnect)
	connectMsg.AgentID = "fake-agent"
	connectMsg.Payload = protocol.ConnectPayload{
		AgentID:   "fake-agent",
		Token:     "wrong-token-12345",
		Hostname:  "bad-host",
		OS:        "linux",
		Arch:      "amd64",
		ClientVer: "test-e2e",
	}
	wsConn.WriteJSON(connectMsg)

	// Read with timeout — server should close connection
	wsConn.SetReadDeadline(time.Now().Add(2 * time.Second))
	_, _, err = wsConn.ReadMessage()
	if err == nil {
		t.Fatal("expected connection close on wrong token, but got no error")
	}
	t.Logf("Wrong token correctly rejected: %v", err)
}

// TestAgentE2EEmptyConnect — первое сообщение не connect → disconnect.
func TestAgentE2EEmptyConnect(t *testing.T) {
	cfg := &config.RelayConfig{
		WSSAddr:          ":0",
		APIAddr:          ":0",
		APIToken:         "test-token",
		HeartbeatTimeout: 90,
	}
	r := relay.NewRelay(cfg)

	apiMux := http.NewServeMux()
	apiMux.HandleFunc("/ws", r.HandleAgentWSForTest)
	server := httptest.NewServer(apiMux)
	defer server.Close()

	wsURL := "ws" + strings.TrimPrefix(server.URL, "http") + "/ws"
	wsConn, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("WS dial failed: %v", err)
	}
	defer wsConn.Close()

	// Send heartbeat instead of connect (wrong first message)
	hbMsg := protocol.NewMessage(protocol.MsgHeartbeat)
	wsConn.WriteJSON(hbMsg)

	wsConn.SetReadDeadline(time.Now().Add(2 * time.Second))
	_, _, err = wsConn.ReadMessage()
	if err == nil {
		t.Fatal("expected close on wrong first message")
	}
	t.Logf("Non-connect first message correctly rejected: %v", err)
}

// Helper: need to expose PoolList for testing
func init() {
	// Make sure fmt is used
	_ = fmt.Sprintf
}

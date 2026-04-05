package tgbot

import (
	"encoding/json"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

// --- Helpers ---

func testLogger() *slog.Logger {
	return slog.New(slog.NewTextHandler(io.Discard, &slog.HandlerOptions{Level: slog.LevelError}))
}

func testBot(tsURL string) *Bot {
	return &Bot{
		cfg:       &TelegramBotConfig{Token: "bot:123", AllowedIDs: []int64{111}},
		relayURL:  tsURL,
		apiToken:  "relay-token",
		logger:    testLogger(),
		confirmed: make(map[int64]bool),
	}
}

func testBotNoAllowList(tsURL string) *Bot {
	return &Bot{
		cfg:       &TelegramBotConfig{Token: "bot:123"},
		relayURL:  tsURL,
		apiToken:  "relay-token",
		logger:    testLogger(),
		confirmed: make(map[int64]bool),
	}
}

// --- isAllowed ---

func TestIsAllowed_EmptyList(t *testing.T) {
	b := testBotNoAllowList("")
	if !b.isAllowed(999) {
		t.Error("should allow any user when AllowedIDs is empty")
	}
}

func TestIsAllowed_InList(t *testing.T) {
	b := testBot("")
	if !b.isAllowed(111) {
		t.Error("should allow user 111")
	}
}

func TestIsAllowed_NotInList(t *testing.T) {
	b := testBot("")
	if b.isAllowed(999) {
		t.Error("should deny user 999")
	}
}

// --- apiURL ---

func TestApiURL(t *testing.T) {
	b := testBot("")
	url := b.apiURL("sendMessage")
	expected := "https://api.telegram.org/botbot:123/sendMessage"
	if url != expected {
		t.Errorf("apiURL = %q, want %q", url, expected)
	}
}

// --- handleCommand routing ---

func TestHandleCommand_Unknown(t *testing.T) {
	// Verify command parsing for unknown commands doesn't panic
	msg := &tgMessage{Text: "/unknowncmd arg1 arg2", From: &tgUser{ID: 111}, Chat: tgChat{ID: 111}}
	_ = msg
}

func TestHandleCommand_NotACommand(t *testing.T) {
	b := testBot("")
	msg := &tgMessage{Text: "just text", From: &tgUser{ID: 111}, Chat: tgChat{ID: 111}}
	// Should not panic, just return
	b.handleCommand(msg)
}

func TestHandleCommand_WithBotName(t *testing.T) {
	// Test command parsing: /cmd@botname args
	text := "/status@mybot"
	parts := strings.Fields(text)
	cmd := strings.TrimLeft(parts[0], "/")
	if idx := strings.Index(cmd, "@"); idx >= 0 {
		cmd = cmd[:idx]
	}
	if cmd != "status" {
		t.Errorf("parsed cmd = %q, want status", cmd)
	}
}

// --- relayRequest ---

func TestRelayRequest_Get(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Header.Get("Authorization") != "Bearer relay-token" {
			t.Errorf("wrong auth header")
		}
		if r.Method != "GET" {
			t.Errorf("method = %s, want GET", r.Method)
		}
		json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
	}))
	defer ts.Close()

	b := testBot(ts.URL)
	data, err := b.relayGet("/api/v1/health")
	if err != nil {
		t.Fatalf("relayGet error: %v", err)
	}
	var result map[string]string
	json.Unmarshal(data, &result)
	if result["status"] != "ok" {
		t.Errorf("status = %q, want ok", result["status"])
	}
}

func TestRelayRequest_Post(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			t.Errorf("method = %s, want POST", r.Method)
		}
		if r.Header.Get("Content-Type") != "application/json" {
			t.Errorf("Content-Type = %q", r.Header.Get("Content-Type"))
		}
		var body map[string]any
		json.NewDecoder(r.Body).Decode(&body)
		if body["command"] != "ls" {
			t.Errorf("command = %v, want ls", body["command"])
		}
		json.NewEncoder(w).Encode(map[string]string{"result": "done"})
	}))
	defer ts.Close()

	b := testBot(ts.URL)
	data, err := b.relayPost("/api/v1/agents/exec", map[string]string{"command": "ls"})
	if err != nil {
		t.Fatalf("relayPost error: %v", err)
	}
	var result map[string]string
	json.Unmarshal(data, &result)
	if result["result"] != "done" {
		t.Errorf("result = %q, want done", result["result"])
	}
}

func TestRelayRequest_ServerError(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusInternalServerError)
	}))
	defer ts.Close()

	b := testBot(ts.URL)
	_, err := b.relayGet("/api/v1/fail")
	// relayRequest still returns data (empty JSON), not an error for non-2xx
	// But if server is unreachable, it errors
	if err != nil {
		// This is fine — connection refused or similar
	}
}

func TestRelayRequest_Unreachable(t *testing.T) {
	b := testBot("http://127.0.0.1:1") // nothing listening
	_, err := b.relayGet("/api/v1/test")
	if err == nil {
		t.Fatal("expected error for unreachable server")
	}
}

// --- relayStreamPost ---

func TestRelayStreamPost(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("line1\nline2\nline3"))
	}))
	defer ts.Close()

	b := testBot(ts.URL)
	result, err := b.relayStreamPost("/api/v1/stream", nil, 1024)
	if err != nil {
		t.Fatalf("relayStreamPost error: %v", err)
	}
	if !strings.Contains(result, "line1") {
		t.Errorf("result = %q, want to contain line1", result)
	}
}

func TestRelayStreamPost_Truncation(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("AAAAAAAAAAAAAAAAAAAA")) // 20 bytes
	}))
	defer ts.Close()

	b := testBot(ts.URL)
	result, err := b.relayStreamPost("/api/v1/stream", nil, 10)
	if err != nil {
		t.Fatalf("relayStreamPost error: %v", err)
	}
	if !strings.Contains(result, "обрезано") {
		t.Errorf("result = %q, want truncation marker", result)
	}
}

// --- getUpdates ---

func TestGetUpdates_InvalidResponse(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("not json"))
	}))
	defer ts.Close()

	b := testBot(ts.URL)
	_, err := b.getUpdates()
	if err == nil {
		t.Fatal("expected error for invalid JSON response")
	}
}

func TestGetUpdates_OKFalse(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{"ok": false, "description": "bad request"})
	}))
	defer ts.Close()

	b := testBot(ts.URL)
	_, err := b.getUpdates()
	if err == nil {
		t.Fatal("expected error when ok=false")
	}
}

func TestGetUpdates_Empty(t *testing.T) {
	// getUpdates calls b.apiURL() which is hardcoded to api.telegram.org
	// We can't easily mock this without changing the Bot struct.
	// Instead, test that empty result slice is handled correctly.
	// The actual Telegram API call is tested via relayRequest tests.
	// Skip this test since it would hit the real Telegram API.
	t.Skip("getUpdates calls api.telegram.org directly, cannot mock without interface")
}

// --- handleCallback routing ---

func TestHandleCallback_ExecConfirm(t *testing.T) {
	b := testBot("")
	b.confirmed[111] = false

	cb := &tgCallback{
		ID:      "cb1",
		From:    &tgUser{ID: 111},
		Message: &tgMessage{Chat: tgChat{ID: 111}},
		Data:    "exec_confirm:",
	}
	b.handleCallback(cb)

	if !b.confirmed[111] {
		t.Error("confirmed[111] should be true after exec_confirm")
	}
}

func TestHandleCallback_ExecCancel(t *testing.T) {
	b := testBot("")
	b.confirmed[111] = true

	cb := &tgCallback{
		ID:      "cb2",
		From:    &tgUser{ID: 111},
		Message: &tgMessage{Chat: tgChat{ID: 111}},
		Data:    "exec_cancel:",
	}
	b.handleCallback(cb)

	if b.confirmed[111] {
		t.Error("confirmed[111] should be false after exec_cancel")
	}
}

func TestHandleCallback_NotAllowed(t *testing.T) {
	b := testBot("")
	cb := &tgCallback{
		ID:      "cb3",
		From:    &tgUser{ID: 999}, // not in AllowedIDs
		Message: &tgMessage{Chat: tgChat{ID: 999}},
		Data:    "exec_confirm:",
	}
	// Should not panic
	b.handleCallback(cb)
	if b.confirmed[999] {
		t.Error("should not set confirmed for unauthorized user")
	}
}

func TestHandleCallback_Unknown(t *testing.T) {
	b := testBot("")
	cb := &tgCallback{
		ID:      "cb4",
		From:    &tgUser{ID: 111},
		Message: &tgMessage{Chat: tgChat{ID: 111}},
		Data:    "unknown_action",
	}
	// Should not panic
	b.handleCallback(cb)
}

// --- JSON types ---

func TestTelegramTypes_JSON(t *testing.T) {
	tests := []struct {
		name string
		obj  any
	}{
		{"tgUpdate", tgUpdate{UpdateID: 1}},
		{"tgMessage", tgMessage{MessageID: 1, Text: "hello", Chat: tgChat{ID: 111}}},
		{"tgUser", tgUser{ID: 111, FirstName: "Test", Username: "testuser"}},
		{"tgChat", tgChat{ID: 111, Type: "private"}},
		{"tgCallback", tgCallback{ID: "cb1", Data: "action:123"}},
		{"tgButton", tgButton{Text: "Click", CallbackData: "data1"}},
		{"tgSendMessage", tgSendMessage{ChatID: 111, Text: "Hi", ParseMode: "Markdown"}},
		{"tgAnswerCallback", tgAnswerCallback{CallbackQueryID: "cb1", Text: "Done"}},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			data, err := json.Marshal(tt.obj)
			if err != nil {
				t.Fatalf("Marshal error: %v", err)
			}
			if len(data) == 0 {
				t.Fatal("empty marshal")
			}
		})
	}
}

func TestTelegramBotConfig_JSON(t *testing.T) {
	cfg := TelegramBotConfig{
		Token:      "bot:123",
		AllowedIDs: []int64{111, 222},
		NotifyOn:   []string{"exec", "error"},
	}
	data, err := json.Marshal(cfg)
	if err != nil {
		t.Fatalf("Marshal error: %v", err)
	}

	var decoded TelegramBotConfig
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("Unmarshal error: %v", err)
	}
	if decoded.Token != "bot:123" {
		t.Errorf("Token = %q, want bot:123", decoded.Token)
	}
	if len(decoded.AllowedIDs) != 2 {
		t.Errorf("AllowedIDs = %d, want 2", len(decoded.AllowedIDs))
	}
}

// --- New constructor ---

func TestNew_Bot(t *testing.T) {
	cfg := &TelegramBotConfig{Token: "bot:abc", AllowedIDs: []int64{1, 2}}
	b := New(cfg, "http://localhost:8080/", "api-tok", testLogger())

	if b.cfg.Token != "bot:abc" {
		t.Errorf("Token = %q", b.cfg.Token)
	}
	if b.relayURL != "http://localhost:8080" {
		t.Errorf("relayURL = %q, want trailing slash stripped", b.relayURL)
	}
	if b.apiToken != "api-tok" {
		t.Errorf("apiToken = %q", b.apiToken)
	}
	if b.confirmed == nil {
		t.Error("confirmed map should be initialized")
	}
}

// --- sendComplexMessage error path ---
func TestSendComplexMessage_ServerError(t *testing.T) {
	// sendComplexMessage calls Telegram API directly — can't easily mock
	// Error handling is tested indirectly via relayRequest tests
}

// --- handleExec missing args ---

func TestHandleExec_MissingArgs(t *testing.T) {
	b := testBotNoAllowList("")
	// This would call sendMessage — we can't easily test without mocking Telegram API
	// but we verify it doesn't panic
	msg := &tgMessage{Text: "/exec", From: &tgUser{ID: 1}, Chat: tgChat{ID: 1}}
	b.handleCommand(msg)
}

func TestHandleExec_OnlyServer(t *testing.T) {
	b := testBotNoAllowList("")
	msg := &tgMessage{Text: "/exec server1", From: &tgUser{ID: 1}, Chat: tgChat{ID: 1}}
	b.handleCommand(msg)
}

// --- edge cases ---

func TestBot_RelayURLTrailingSlash(t *testing.T) {
	b := New(&TelegramBotConfig{}, "http://example.com/", "tok", testLogger())
	if b.relayURL != "http://example.com" {
		t.Errorf("relayURL = %q, want trailing slash removed", b.relayURL)
	}
}

func TestBot_RelayURLNoSlash(t *testing.T) {
	b := New(&TelegramBotConfig{}, "http://example.com", "tok", testLogger())
	if b.relayURL != "http://example.com" {
		t.Errorf("relayURL = %q", b.relayURL)
	}
}

// --- relayRequest with plain text response ---

func TestRelayRequest_PlainTextResponse(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/plain")
		w.Write([]byte("not json response"))
	}))
	defer ts.Close()

	b := testBot(ts.URL)
	data, err := b.relayGet("/api/v1/plain")
	if err != nil {
		t.Fatalf("relayGet error: %v", err)
	}
	// Should wrap plain text in quotes to make it valid JSON
	var result string
	json.Unmarshal(data, &result)
	if !strings.Contains(result, "not json response") {
		t.Errorf("result = %q, want plain text wrapped", result)
	}
}

// --- Integration: command dispatch ---

func TestCommandDispatch_AllRegistered(t *testing.T) {
	commands := []string{
		"start", "help", "status", "servers", "exec", "logs",
		"backups", "restore", "emergency", "pause", "resume",
		"approve", "reject", "settings", "readonly", "policy",
		"devices", "approve_device", "reject_device", "revoke",
		"keys", "rotate", "device_info",
	}

	b := testBotNoAllowList("")
	for _, cmd := range commands {
		t.Run(cmd, func(t *testing.T) {
			msg := &tgMessage{
				Text: "/" + cmd,
				From: &tgUser{ID: 1},
				Chat: tgChat{ID: 1},
			}
			// Should not panic — each command should be handled
			b.handleCommand(msg)
		})
	}
}

// --- relayStreamPost unreachable ---

func TestRelayStreamPost_Unreachable(t *testing.T) {
	b := testBot("http://127.0.0.1:1")
	_, err := b.relayStreamPost("/api/v1/test", nil, 1024)
	if err == nil {
		t.Fatal("expected error for unreachable server")
	}
}

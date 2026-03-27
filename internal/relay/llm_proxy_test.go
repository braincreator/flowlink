package relay

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestLLMProxy_BackendSorting(t *testing.T) {
	backends := []LLMBackend{
		{Name: "groq", URL: "https://api.groq.com", Priority: 2},
		{Name: "macbook", URL: "http://localhost:1234", Priority: 1},
		{Name: "ollama", URL: "http://localhost:11434", Priority: 3},
	}

	proxy := NewLLMProxy(backends)

	list := proxy.ListBackends()
	if len(list) != 3 {
		t.Fatalf("expected 3 backends, got %d", len(list))
	}

	// Проверяем сортировку по приоритету
	if list[0].Name != "macbook" {
		t.Errorf("expected first backend to be 'macbook', got %q", list[0].Name)
	}
	if list[1].Name != "groq" {
		t.Errorf("expected second backend to be 'groq', got %q", list[1].Name)
	}
	if list[2].Name != "ollama" {
		t.Errorf("expected third backend to be 'ollama', got %q", list[2].Name)
	}
}

func TestLLMProxy_ChatWithMock(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		resp := map[string]any{
			"id":      "test",
			"model":   "test-model",
			"choices": []map[string]any{
				{"message": map[string]string{"content": "hello world"}, "finish_reason": "stop"},
			},
			"usage": map[string]int{"prompt_tokens": 10, "completion_tokens": 5},
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	}))
	defer server.Close()

	proxy := NewLLMProxy([]LLMBackend{
		{Name: "mock", URL: server.URL, Priority: 1, Provider: "openai_compatible"},
	})

	result, err := proxy.Chat(
		[]any{map[string]string{"role": "user", "content": "hello"}},
		"test-model", 100, 0.3,
	)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Content != "hello world" {
		t.Errorf("expected 'hello world', got %q", result.Content)
	}
	if result.TokensIn != 10 {
		t.Errorf("expected 10 tokens_in, got %d", result.TokensIn)
	}
	if result.Backend != "mock" {
		t.Errorf("expected backend 'mock', got %q", result.Backend)
	}
}

func TestLLMProxy_Fallback(t *testing.T) {
	// Первый backend недоступен, второй отвечает
	server2 := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		resp := map[string]any{
			"choices": []map[string]any{
				{"message": map[string]string{"content": "fallback response"}, "finish_reason": "stop"},
			},
			"usage": map[string]int{"prompt_tokens": 5, "completion_tokens": 3},
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(resp)
	}))
	defer server2.Close()

	proxy := NewLLMProxy([]LLMBackend{
		{Name: "dead", URL: "http://127.0.0.1:1", Priority: 1},
		{Name: "alive", URL: server2.URL, Priority: 2},
	})

	result, err := proxy.Chat(
		[]any{map[string]string{"role": "user", "content": "test"}},
		"", 100, 0.3,
	)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Content != "fallback response" {
		t.Errorf("expected fallback, got %q", result.Content)
	}
	if result.Backend != "alive" {
		t.Errorf("expected backend 'alive', got %q", result.Backend)
	}
}

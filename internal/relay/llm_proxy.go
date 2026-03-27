package relay

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/braincreator/flowlink/internal/protocol"
)

// LLMBackend — конфигурация одного LLM backend.
type LLMBackend struct {
	Name     string `json:"name"`
	URL      string `json:"url"`
	APIKey   string `json:"api_key,omitempty"`
	Priority int    `json:"priority"` // 1 = высший
	Provider string `json:"provider"` // "openai_compatible", "groq", "ollama"
}

// LLMProxy — проксирует LLM запросы от агентов к бэкендам оператора.
type LLMProxy struct {
	backends []LLMBackend
	mu       sync.RWMutex
	logger   *slog.Logger
	client   *http.Client
}

// NewLLMProxy — создаёт LLM proxy.
func NewLLMProxy(backends []LLMBackend) *LLMProxy {
	// Сортируем по приоритету (1 = высший)
	sorted := make([]LLMBackend, len(backends))
	copy(sorted, backends)
	for i := 0; i < len(sorted); i++ {
		for j := i + 1; j < len(sorted); j++ {
			if sorted[j].Priority < sorted[i].Priority {
				sorted[i], sorted[j] = sorted[j], sorted[i]
			}
		}
	}

	return &LLMProxy{
		backends: sorted,
		logger:   slog.Default(),
		client:   &http.Client{Timeout: 120 * time.Second},
	}
}

// Chat — проксирует chat completion запрос к LLM backend.
func (p *LLMProxy) Chat(messages []any, model string, maxTokens int, temperature float64) (*LLMProxyResponse, error) {
	var lastErr error

	for _, backend := range p.backends {
		resp, err := p.chatBackend(backend, messages, model, maxTokens, temperature)
		if err != nil {
			p.logger.Warn("LLM backend недоступен",
				"backend", backend.Name,
				"url", backend.URL,
				"err", err,
			)
			lastErr = err
			continue // fallback на следующий
		}

		resp.Backend = backend.Name
		return resp, nil
	}

	return nil, fmt.Errorf("все LLM backends недоступны: %w", lastErr)
}

// chatBackend — отправляет запрос к конкретному backend.
func (p *LLMProxy) chatBackend(backend LLMBackend, messages []any, model string, maxTokens int, temperature float64) (*LLMProxyResponse, error) {
	url := backend.URL
	// Нормализуем URL
	if !strings.HasSuffix(url, "/chat/completions") {
		if !strings.HasSuffix(url, "/") {
			url += "/"
		}
		url += "chat/completions"
	}

	reqBody := map[string]any{
		"messages":    messages,
		"max_tokens":  maxTokens,
		"temperature": temperature,
	}
	if model != "" {
		reqBody["model"] = model
	} else if backend.Provider == "ollama" {
		reqBody["model"] = "llama3"
	} else {
		reqBody["model"] = "default"
	}

	bodyBytes, err := json.Marshal(reqBody)
	if err != nil {
		return nil, fmt.Errorf("сериализация: %w", err)
	}

	start := time.Now()

	req, err := http.NewRequest("POST", url, bytes.NewReader(bodyBytes))
	if err != nil {
		return nil, fmt.Errorf("создание запроса: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")
	if backend.APIKey != "" {
		req.Header.Set("Authorization", "Bearer "+backend.APIKey)
	}

	resp, err := p.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("HTTP: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("чтение ответа: %w", err)
	}

	if resp.StatusCode != 200 {
		return nil, fmt.Errorf("HTTP %d: %s", resp.StatusCode, truncateStr(string(respBody), 300))
	}

	// Парсим OpenAI-совместимый ответ
	var chatResp struct {
		ID      string `json:"id"`
		Model   string `json:"model"`
		Choices []struct {
			FinishReason string `json:"finish_reason"`
			Message      struct {
				Content string `json:"content"`
			} `json:"message"`
		} `json:"choices"`
		Usage struct {
			PromptTokens     int `json:"prompt_tokens"`
			CompletionTokens int `json:"completion_tokens"`
		} `json:"usage"`
	}

	if err := json.Unmarshal(respBody, &chatResp); err != nil {
		return nil, fmt.Errorf("парсинг ответа: %w", err)
	}

	if len(chatResp.Choices) == 0 {
		return nil, fmt.Errorf("пустой ответ от LLM")
	}

	choice := chatResp.Choices[0]

	return &LLMProxyResponse{
		Content:      choice.Message.Content,
		TokensIn:     chatResp.Usage.PromptTokens,
		TokensOut:    chatResp.Usage.CompletionTokens,
		Model:        chatResp.Model,
		Duration:     time.Since(start).Milliseconds(),
		FinishReason: choice.FinishReason,
	}, nil
}

// SetBackends — обновляет список бэкендов (runtime).
func (p *LLMProxy) SetBackends(backends []LLMBackend) {
	p.mu.Lock()
	defer p.mu.Unlock()

	// Сортируем по приоритету
	sorted := make([]LLMBackend, len(backends))
	copy(sorted, backends)
	for i := 0; i < len(sorted); i++ {
		for j := i + 1; j < len(sorted); j++ {
			if sorted[j].Priority < sorted[i].Priority {
				sorted[i], sorted[j] = sorted[j], sorted[i]
			}
		}
	}

	p.backends = sorted
}

// ListBackends — возвращает список бэкендов (без API ключей).
func (p *LLMProxy) ListBackends() []LLMBackendInfo {
	p.mu.RLock()
	defer p.mu.RUnlock()

	result := make([]LLMBackendInfo, len(p.backends))
	for i, b := range p.backends {
		result[i] = LLMBackendInfo{
			Name:     b.Name,
			URL:      b.URL,
			Priority: b.Priority,
			Provider: b.Provider,
		}
	}
	return result
}

// CheckHealth — проверяет доступность всех бэкендов.
func (p *LLMProxy) CheckHealth() map[string]string {
	p.mu.RLock()
	defer p.mu.RUnlock()

	results := make(map[string]string)
	for _, b := range p.backends {
		client := &http.Client{Timeout: 5 * time.Second}
		resp, err := client.Get(b.URL)
		if err != nil {
			results[b.Name] = "unreachable: " + err.Error()
		} else {
			resp.Body.Close()
			results[b.Name] = "ok"
		}
	}
	return results
}

// LLMProxyResponse — ответ от LLM proxy.
type LLMProxyResponse struct {
	Content      string `json:"content"`
	TokensIn     int    `json:"tokens_in"`
	TokensOut    int    `json:"tokens_out"`
	Model        string `json:"model"`
	Duration     int64  `json:"duration_ms"`
	FinishReason string `json:"finish_reason"`
	Backend      string `json:"backend,omitempty"`
}

// LLMBackendInfo — публичная информация о backend.
type LLMBackendInfo struct {
	Name     string `json:"name"`
	URL      string `json:"url"`
	Priority int    `json:"priority"`
	Provider string `json:"provider"`
}

// === Обработчики на реле ===

// handleLLMChat — проксирует LLM запрос от агента.
func (r *Relay) handleLLMChat(w http.ResponseWriter, req *http.Request) {
	var body struct {
		AgentID     string        `json:"agent_id"`
		Messages    []any         `json:"messages"`
		Model       string        `json:"model,omitempty"`
		MaxTokens   int           `json:"max_tokens,omitempty"`
		Temperature float64       `json:"temperature,omitempty"`
	}

	if err := json.NewDecoder(req.Body).Decode(&body); err != nil {
		writeError(w, http.StatusBadRequest, "неверный JSON")
		return
	}

	if r.llmProxy == nil {
		writeError(w, http.StatusServiceUnavailable, "LLM proxy не настроен")
		return
	}

	if body.MaxTokens == 0 {
		body.MaxTokens = 4096
	}
	if body.Temperature == 0 {
		body.Temperature = 0.3
	}

	resp, err := r.llmProxy.Chat(body.Messages, body.Model, body.MaxTokens, body.Temperature)
	if err != nil {
		writeError(w, http.StatusBadGateway, "LLM ошибка: "+err.Error())
		return
	}

	writeJSON(w, resp)
}

// handleLLMBackends — возвращает список LLM бэкендов.
func (r *Relay) handleLLMBackends(w http.ResponseWriter, req *http.Request) {
	if r.llmProxy == nil {
		writeJSON(w, map[string]any{"backends": []LLMBackendInfo{}})
		return
	}
	writeJSON(w, map[string]any{"backends": r.llmProxy.ListBackends()})
}

// handleLLMHealth — проверяет доступность LLM бэкендов.
func (r *Relay) handleLLMHealth(w http.ResponseWriter, req *http.Request) {
	if r.llmProxy == nil {
		writeError(w, http.StatusServiceUnavailable, "LLM proxy не настроен")
		return
	}
	writeJSON(w, map[string]any{"health": r.llmProxy.CheckHealth()})
}

// handleAgentLLMRequest — обрабатывает LLM запрос от агента через WSS.
// Агент прислал MsgLLMRequest → реле проксирует к LLM → MsgLLMResponse обратно.
func (r *Relay) handleAgentLLMRequest(agent *AgentConn, msg protocol.Message) {
	if r.llmProxy == nil {
		resp := protocol.NewMessage(protocol.MsgLLMResponse)
		resp.Payload = map[string]string{
			"error":      "LLM proxy не настроен на реле",
			"request_id": "",
		}
		agent.SendMessage(resp)
		return
	}

	// Извлекаем данные из payload
	var reqData struct {
		Messages    []any   `json:"messages"`
		Model       string  `json:"model"`
		MaxTokens   int     `json:"max_tokens"`
		Temperature float64 `json:"temperature"`
		RequestID   string  `json:"request_id"`
	}

	payloadBytes, _ := json.Marshal(msg.Payload)
	if err := json.Unmarshal(payloadBytes, &reqData); err != nil {
		resp := protocol.NewMessage(protocol.MsgLLMResponse)
		resp.Payload = map[string]string{
			"error":      fmt.Sprintf("неверный payload: %v", err),
			"request_id": "",
		}
		agent.SendMessage(resp)
		return
	}

	if reqData.MaxTokens == 0 {
		reqData.MaxTokens = 4096
	}
	if reqData.Temperature == 0 {
		reqData.Temperature = 0.3
	}

	// Проксируем к LLM
	resp, err := r.llmProxy.Chat(reqData.Messages, reqData.Model, reqData.MaxTokens, reqData.Temperature)
	if err != nil {
		errResp := protocol.NewMessage(protocol.MsgLLMResponse)
		errResp.Payload = map[string]string{
			"error":      err.Error(),
			"request_id": reqData.RequestID,
		}
		agent.SendMessage(errResp)
		return
	}

	// Отправляем ответ агенту
	llmResp := protocol.NewMessage(protocol.MsgLLMResponse)
	llmResp.Payload = map[string]any{
		"content":       resp.Content,
		"tokens_in":     resp.TokensIn,
		"tokens_out":    resp.TokensOut,
		"model":         resp.Model,
		"duration_ms":   resp.Duration,
		"finish_reason": resp.FinishReason,
		"backend":       resp.Backend,
		"request_id":    reqData.RequestID,
	}
	agent.SendMessage(llmResp)
}

func truncateStr(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}

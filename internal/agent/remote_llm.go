package agent

import (
	"encoding/json"
	"fmt"
	"log/slog"
	"time"

	"github.com/braincreator/flowlink/internal/protocol"
)

// RemoteLLM — LLM клиент, который отправляет запросы через реле к хосту оператора.
// Агент НЕ ходит к LLM напрямую — всё через WSS.
type RemoteLLM struct {
	agent  *Agent
	logger *slog.Logger
}

// NewRemoteLLM — создаёт remote LLM клиент.
func NewRemoteLLM(agent *Agent) *RemoteLLM {
	return &RemoteLLM{
		agent:  agent,
		logger: slog.Default(),
	}
}

// LLMRequest — запрос к LLM через реле.
type LLMRequest struct {
	// Сообщения для чата
	Messages []LLMMessage `json:"messages"`
	// Настройки (опционально, реле может переопределить)
	Model       string  `json:"model,omitempty"`
	MaxTokens   int     `json:"max_tokens,omitempty"`
	Temperature float64 `json:"temperature,omitempty"`
	// Request ID для трекинга
	RequestID string `json:"request_id"`
}

// RemoteLLMResponse — ответ от LLM через реле.
type RemoteLLMResponse struct {
	Content      string `json:"content"`
	TokensIn     int    `json:"tokens_in"`
	TokensOut    int    `json:"tokens_out"`
	Model        string `json:"model"`
	Duration     int64  `json:"duration_ms"`
	FinishReason string `json:"finish_reason"`
	Error        string `json:"error,omitempty"`
	Backend      string `json:"backend,omitempty"` // какой backend использовался (macbook/groq/ollama)
	RequestID    string `json:"request_id,omitempty"`
}

// Chat — отправляет сообщения через реле к LLM на хосте оператора.
func (r *RemoteLLM) Chat(messages []LLMMessage) (*RemoteLLMResponse, error) {
	reqID := protocol.RequestID()

	msg := protocol.NewMessage(protocol.MsgLLMRequest)
	msg.Payload = LLMRequest{
		Messages:    messages,
		MaxTokens:   4096,
		Temperature: 0.3,
		RequestID:   reqID,
	}

	r.logger.Debug("LLM запрос через реле",
		"request_id", reqID,
		"messages", len(messages),
	)

	// Отправляем запрос через WSS
	if err := r.agent.write(msg); err != nil {
		return nil, protocol.ErrCause(protocol.CodeLLMRequestError, err)
	}

	// Ждём ответа — будет обработан в readLoop и записан в pendingLLM
	resp, err := r.agent.waitForLLMResponse(reqID, 120*time.Second)
	if err != nil {
		return nil, err
	}

	if resp.Error != "" {
		return nil, fmt.Errorf("LLM ошибка: %s", resp.Error)
	}

	return resp, nil
}

// ChatSimple — простой вызов с system prompt и user message.
func (r *RemoteLLM) ChatSimple(systemPrompt, userMessage string) (string, error) {
	messages := []LLMMessage{
		{Role: "system", Content: systemPrompt},
		{Role: "user", Content: userMessage},
	}

	resp, err := r.Chat(messages)
	if err != nil {
		return "", err
	}

	return resp.Content, nil
}

// IsConfigured — всегда true, т.к. LLM через реле (оператор настраивает на своей стороне).
func (r *RemoteLLM) IsConfigured() bool {
	return true
}

// Provider — возвращает "relay" (LLM на стороне оператора).
func (r *RemoteLLM) Provider() string {
	return "relay"
}

// Model — unknown, определяется реле.
func (r *RemoteLLM) Model() string {
	return "relay-default"
}

// === Поддержка LLM в Agent ===

// pendingLLMResponse — хранит ожидаемые LLM-ответы.
type pendingLLMResponse struct {
	ch       chan *RemoteLLMResponse
	deadline time.Time
}

// registerPendingLLM — регистрирует ожидание LLM ответа.
func (a *Agent) registerPendingLLM(requestID string, timeout time.Duration) chan *RemoteLLMResponse {
	ch := make(chan *RemoteLLMResponse, 1)
	a.wsMu.Lock()
	if a.pendingLLM == nil {
		a.pendingLLM = make(map[string]*pendingLLMResponse)
	}
	a.pendingLLM[requestID] = &pendingLLMResponse{
		ch:       ch,
		deadline: time.Now().Add(timeout),
	}
	a.wsMu.Unlock()

	// Cleanup через timeout
	go func() {
		<-time.After(timeout)
		a.wsMu.Lock()
		delete(a.pendingLLM, requestID)
		a.wsMu.Unlock()
	}()

	return ch
}

// handleLLMResponse — обрабатывает ответ LLM от реле.
func (a *Agent) handleLLMResponse(msg protocol.Message) {
	payloadBytes, _ := json.Marshal(msg.Payload)

	var resp RemoteLLMResponse
	if err := json.Unmarshal(payloadBytes, &resp); err != nil {
		a.logger.Error("LLM response parse error", "err", err)
		return
	}

	a.wsMu.Lock()
	reqID := resp.RequestID
	pending, ok := a.pendingLLM[reqID]
	if ok {
		delete(a.pendingLLM, reqID)
		a.wsMu.Unlock()
		pending.ch <- &resp
	} else {
		a.wsMu.Unlock()
		a.logger.Warn("LLM ответ без ожидателя", "request_id", reqID)
	}
}

// waitForLLMResponse — ждёт LLM ответ по request ID.
func (a *Agent) waitForLLMResponse(requestID string, timeout time.Duration) (*RemoteLLMResponse, error) {
	ch := a.registerPendingLLM(requestID, timeout)

	select {
	case resp := <-ch:
		return resp, nil
	case <-time.After(timeout):
		return nil, protocol.Err(protocol.CodeMCPTimeout, timeout)
	case <-a.done:
		return nil, protocol.Err(protocol.CodeAgentNotConnected)
	}
}

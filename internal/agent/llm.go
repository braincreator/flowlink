package agent

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"strings"
	"time"

	"github.com/braincreator/flowlink/internal/config"
)

// LLMClient — клиент для вызова LLM API.
// Поддерживает OpenAI-совместимый формат (Groq, OpenAI, Zhipu, Ollama).
type LLMClient struct {
	cfg    config.LLMConfig
	logger *slog.Logger
}

// LLMMessage — сообщение в чате LLM.
type LLMMessage struct {
	Role    string `json:"role"`    // "system", "user", "assistant"
	Content string `json:"content"`
}

// LLMResponse — ответ от LLM.
type LLMResponse struct {
	Content      string
	TokensIn     int
	TokensOut    int
	Model        string
	Duration     time.Duration
	FinishReason string
}

// LLMToolCall — инструмент, который LLM хочет вызвать.
type LLMToolCall struct {
	ID       string `json:"id"`
	Name     string `json:"name"`
	Args     string `json:"arguments"` // JSON string
}

// NewLLMClient — создаёт LLM клиент.
func NewLLMClient(cfg config.LLMConfig) *LLMClient {
	return &LLMClient{
		cfg:    cfg,
		logger: slog.Default(),
	}
}

// Chat — отправляет сообщения в LLM и получает ответ.
func (c *LLMClient) Chat(messages []LLMMessage) (*LLMResponse, error) {
	baseURL := c.cfg.BaseURL
	switch c.cfg.Provider {
	case "groq":
		if baseURL == "" {
			baseURL = "https://api.groq.com/openai/v1"
		}
	case "openai":
		if baseURL == "" {
			baseURL = "https://api.openai.com/v1"
		}
	case "zhipu":
		if baseURL == "" {
			baseURL = "https://open.bigmodel.cn/api/paas/v4"
		}
	case "ollama":
		if baseURL == "" {
			baseURL = "http://localhost:11434"
		}
	default:
		if baseURL == "" {
			return nil, fmt.Errorf("неизвестный провайдер: %s", c.cfg.Provider)
		}
	}

	url := baseURL + "/chat/completions"

	// Формируем запрос (OpenAI-совместимый)
	reqBody := map[string]any{
		"model":       c.cfg.Model,
		"messages":    messages,
		"max_tokens":  c.cfg.MaxTokens,
		"temperature": c.cfg.Temperature,
	}

	bodyBytes, err := json.Marshal(reqBody)
	if err != nil {
		return nil, fmt.Errorf("сериализация запроса: %w", err)
	}

	c.logger.Debug("LLM запрос",
		"provider", c.cfg.Provider,
		"model", c.cfg.Model,
		"messages", len(messages),
	)

	start := time.Now()

	// HTTP запрос
	req, err := http.NewRequest("POST", url, bytes.NewReader(bodyBytes))
	if err != nil {
		return nil, fmt.Errorf("создание запроса: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")

	// Авторизация
	switch c.cfg.Provider {
	case "groq":
		req.Header.Set("Authorization", "Bearer "+c.cfg.APIKey)
	case "openai":
		req.Header.Set("Authorization", "Bearer "+c.cfg.APIKey)
	case "zhipu":
		// Zhipu использует JWT, но для простоты пробуем Bearer
		req.Header.Set("Authorization", "Bearer "+c.cfg.APIKey)
	case "ollama":
		// Ollama — без авторизации
	}

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("HTTP запрос к LLM: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("чтение ответа LLM: %w", err)
	}

	if resp.StatusCode != 200 {
		return nil, fmt.Errorf("LLM ошибка %d: %s", resp.StatusCode, truncate(string(respBody), 500))
	}

	// Парсим ответ
	var chatResp chatCompletionResponse
	if err := json.Unmarshal(respBody, &chatResp); err != nil {
		return nil, fmt.Errorf("парсинг ответа LLM: %w", err)
	}

	if len(chatResp.Choices) == 0 {
		return nil, fmt.Errorf("пустой ответ от LLM")
	}

	choice := chatResp.Choices[0]

	return &LLMResponse{
		Content:      choice.Message.Content,
		TokensIn:     chatResp.Usage.PromptTokens,
		TokensOut:    chatResp.Usage.CompletionTokens,
		Model:        chatResp.Model,
		Duration:     time.Since(start),
		FinishReason: choice.FinishReason,
	}, nil
}

// ChatSimple — простой вызов LLM с system prompt и user message.
func (c *LLMClient) ChatSimple(systemPrompt, userMessage string) (string, error) {
	messages := []LLMMessage{
		{Role: "system", Content: systemPrompt},
		{Role: "user", Content: userMessage},
	}

	resp, err := c.Chat(messages)
	if err != nil {
		return "", err
	}

	return resp.Content, nil
}

// SetAPIKey — устанавливает API ключ (для runtime-конфигурации).
func (c *LLMClient) SetAPIKey(key string) {
	c.cfg.APIKey = key
}

// Provider — возвращает текущего провайдера.
func (c *LLMClient) Provider() string {
	return c.cfg.Provider
}

// Model — возвращает текущую модель.
func (c *LLMClient) Model() string {
	return c.cfg.Model
}

// IsConfigured — проверяет, настроен ли LLM (есть API ключ).
func (c *LLMClient) IsConfigured() bool {
	if c.cfg.Provider == "ollama" {
		return true // Ollama не требует ключа
	}
	return c.cfg.APIKey != ""
}

// === JSON структуры для OpenAI-совместимого API ===

type chatCompletionResponse struct {
	ID      string `json:"id"`
	Model   string `json:"model"`
	Choices []struct {
		Index        int `json:"index"`
		FinishReason string `json:"finish_reason"`
		Message      struct {
			Role    string `json:"role"`
			Content string `json:"content"`
		} `json:"message"`
	} `json:"choices"`
	Usage struct {
		PromptTokens     int `json:"prompt_tokens"`
		CompletionTokens int `json:"completion_tokens"`
		TotalTokens      int `json:"total_tokens"`
	} `json:"usage"`
}

// truncate — обрезает строку до maxLen символов.
func truncate(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}

// BuildSystemPrompt — строит system prompt для автономного агента.
// Включает скилл, системную информацию и правила.
func BuildSystemPrompt(skill *Skill, sysInfo string) string {
	var sb strings.Builder

	sb.WriteString("Ты — автономный AI-агент FlowLink. Ты находишься на удалённой машине клиента.\n\n")
	sb.WriteString("## Твоя задача\n")
	sb.WriteString(skill.Instructions)
	sb.WriteString("\n\n")

	sb.WriteString("## Системная информация\n")
	sb.WriteString("```\n")
	sb.WriteString(sysInfo)
	sb.WriteString("```\n\n")

	sb.WriteString("## Правила\n")
	sb.WriteString("1. Выполняй команды пошагово, одну за одной\n")
	sb.WriteString("2. Перед опасной командой ставь ⚠️ WARNING\n")
	sb.WriteString("3. Если команда вернула ошибку — анализируй и пробуй исправить\n")
	sb.WriteString("4. Не удаляй файлы без явной необходимости\n")
	sb.WriteString("5. Не выполняй команды которые могут навредить системе\n")
	sb.WriteString("6. После завершения — отправь краткий отчёт что было сделано\n\n")

	sb.WriteString("## Доступные инструменты\n")
	sb.WriteString("- `exec(command)` — выполнить shell команду\n")
	sb.WriteString("- `read_file(path)` — прочитать файл\n")
	sb.WriteString("- `write_file(path, content)` — записать файл\n")
	sb.WriteString("- `list_files(path)` — список файлов в директории\n\n")

	sb.WriteString("## Формат ответа\n")
	sb.WriteString("Для вызова инструмента используй формат:\n")
	sb.WriteString("```tool_call\n")
	sb.WriteString("exec: ls -la /home\n")
	sb.WriteString("```\n\n")
	sb.WriteString("Для текстового ответа — просто пиши.\n")

	return sb.String()
}

// ParseToolCall — парсит tool_call из ответа LLM.
// Формат: ```tool_call\nexec: command\n``` или ```tool_call\nread_file: /path\n```
func ParseToolCall(content string) (tool string, args string, ok bool) {
	// Ищем блок tool_call
	content = strings.TrimSpace(content)

	// Простой формат: "exec: ls -la" или "read_file: /path/to/file"
	if idx := strings.Index(content, ":"); idx > 0 {
		tool = strings.TrimSpace(content[:idx])
		args = strings.TrimSpace(content[idx+1:])
		return tool, args, true
	}

	// Markdown-формат: ```tool_call\nexec: command\n```
	if strings.Contains(content, "```tool_call") {
		// Извлекаем содержимое блока
		lines := strings.Split(content, "\n")
		inBlock := false
		for _, line := range lines {
			if strings.Contains(line, "```tool_call") {
				inBlock = true
				continue
			}
			if inBlock && strings.HasPrefix(line, "```") {
				break
			}
			if inBlock {
				if colonIdx := strings.Index(line, ":"); colonIdx > 0 {
					tool = strings.TrimSpace(line[:colonIdx])
					args = strings.TrimSpace(line[colonIdx+1:])
					return tool, args, true
				}
			}
		}
	}

	return "", "", false
}

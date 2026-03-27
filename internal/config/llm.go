package config

// LLMConfig — настройки LLM провайдера для автономного агента (L2).
type LLMConfig struct {
	// Провайдер: "groq", "openai", "anthropic", "zhipu"
	Provider string `json:"provider"`
	// Модель
	Model string `json:"model"`
	// API ключ (хранится локально, не передаётся по сети)
	APIKey string `json:"api_key"`
	// Base URL (для кастомных endpoints)
	BaseURL string `json:"base_url,omitempty"`
	// Макс токенов на ответ
	MaxTokens int `json:"max_tokens"`
	// Температура (0-1)
	Temperature float64 `json:"temperature"`
}

// DefaultLLMConfig — дефолтные настройки LLM.
func DefaultLLMConfig() LLMConfig {
	return LLMConfig{
		Provider:   "groq",
		Model:      "llama-3.3-70b-versatile",
		MaxTokens:  4096,
		Temperature: 0.3,
	}
}

// Preset-модели для провайдеров
var LLMPresets = map[string]LLMConfig{
	"groq-fast": {
		Provider: "groq", Model: "llama-3.1-8b-instant",
		MaxTokens: 2048, Temperature: 0.3,
	},
	"groq-smart": {
		Provider: "groq", Model: "llama-3.3-70b-versatile",
		MaxTokens: 4096, Temperature: 0.3,
	},
	"openai": {
		Provider: "openai", Model: "gpt-4o-mini",
		MaxTokens: 4096, Temperature: 0.3,
	},
	"zhipu": {
		Provider: "zhipu", Model: "glm-4-flash",
		BaseURL: "https://open.bigmodel.cn/api/paas/v4",
		MaxTokens: 4096, Temperature: 0.3,
	},
}

// TaskConfig — настройки автономных задач.
type TaskConfig struct {
	// Макс кол-во шагов (итераций LLM) за одну задачу
	MaxSteps int `json:"max_steps"`
	// Макс время выполнения задачи в секундах (0 = unlimited)
	MaxDuration int `json:"max_duration_sec"`
	// Таймаут на один шаг LLM (сек)
	StepTimeout int `json:"step_timeout_sec"`
	// Автоматически принимать safe-команды (без апруваля)
	AutoApproveSafe bool `json:"auto_approve_safe"`
}

// DefaultTaskConfig — дефолтные настройки задач.
func DefaultTaskConfig() TaskConfig {
	return TaskConfig{
		MaxSteps:       20,
		MaxDuration:    1800, // 30 минут
		StepTimeout:    120,  // 2 минуты на шаг
		AutoApproveSafe: true,
	}
}

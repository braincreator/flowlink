package config

// TaskConfig — конфигурация автономной задачи (от оператора через реле).
type TaskConfig struct {
	SkillID      string `json:"skill_id"`
	MaxSteps     int    `json:"max_steps"`
	ApprovalMode string `json:"approval_mode"` // "auto", "manual", "ask"
}

// DefaultTaskConfig — значения по умолчанию.
func DefaultTaskConfig() TaskConfig {
	return TaskConfig{
		MaxSteps:     20,
		ApprovalMode: "auto",
	}
}

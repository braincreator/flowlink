package agent

import (
	"context"
	"fmt"
	"log/slog"
	"strings"
	"sync"
	"time"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/protocol"
)

// Task — автономная задача, которую агент выполняет с помощью LLM.
type Task struct {
	ID          string    `json:"id"`
	SkillID     string    `json:"skill_id"`
	Description string    `json:"description"`
	LLMConfig   config.LLMConfig `json:"llm_config,omitempty"`
	TaskConfig  config.TaskConfig  `json:"task_config,omitempty"`
	CreatedAt   time.Time `json:"created_at"`
	StartedAt   time.Time `json:"started_at,omitempty"`
	CompletedAt time.Time `json:"completed_at,omitempty"`
	Status      string    `json:"status"` // "pending", "running", "done", "error", "cancelled"
	Steps       []TaskStep `json:"steps,omitempty"`
	Error       string    `json:"error,omitempty"`
}

// TaskStep — один шаг выполнения задачи.
type TaskStep struct {
	Number    int       `json:"number"`
	Tool      string    `json:"tool"`
	Args      string    `json:"args"`
	Output    string    `json:"output,omitempty"`
	Error     string    `json:"error,omitempty"`
	ExitCode  int       `json:"exit_code,omitempty"`
	Duration  int64     `json:"duration_ms"`
	Timestamp time.Time `json:"timestamp"`
}

// TaskProgress — прогресс задачи для отправки на реле.
type TaskProgress struct {
	TaskID    string `json:"task_id"`
	StepNum   int    `json:"step_num"`
	TotalSteps int  `json:"total_steps,omitempty"`
	Tool      string `json:"tool"`
	Status    string `json:"status"` // "step_start", "step_done", "task_done", "task_error"
	Output    string `json:"output,omitempty"`
	Error     string `json:"error,omitempty"`
}

// TaskManager — управляет автономными задачами.
type TaskManager struct {
	mu      sync.Mutex
	tasks   map[string]*Task
	agent   *Agent
	llm     *LLMClient
	skills  *SkillStore
	logger  *slog.Logger
}

// NewTaskManager — создаёт менеджер задач.
func NewTaskManager(agent *Agent, llm *LLMClient, skills *SkillStore) *TaskManager {
	return &TaskManager{
		tasks:  make(map[string]*Task),
		agent:  agent,
		llm:    llm,
		skills: skills,
		logger: slog.Default(),
	}
}

// SubmitTask — принимает задачу от реле и запускает выполнение.
func (tm *TaskManager) SubmitTask(task *Task) error {
	tm.mu.Lock()
	defer tm.mu.Unlock()

	if _, exists := tm.tasks[task.ID]; exists {
		return fmt.Errorf("задача %s уже существует", task.ID)
	}

	// Проверяем скилл
	if task.SkillID != "" {
		if _, ok := tm.skills.Get(task.SkillID); !ok {
			return fmt.Errorf("скилл %s не найден", task.SkillID)
		}
	}

	// Дефолтные настройки
	if task.TaskConfig.MaxSteps == 0 {
		task.TaskConfig = config.DefaultTaskConfig()
	}
	if task.LLMConfig.Provider != "" && task.LLMConfig.APIKey != "" {
		tm.llm = NewLLMClient(task.LLMConfig)
	}

	task.Status = "pending"
	task.CreatedAt = time.Now()
	tm.tasks[task.ID] = task

	// Запускаем асинхронно
	go tm.runTask(task)

	return nil
}

// runTask — основной цикл выполнения задачи.
func (tm *TaskManager) runTask(task *Task) {
	tm.mu.Lock()
	task.Status = "running"
	task.StartedAt = time.Now()
	tm.mu.Unlock()

	tm.logger.Info("запуск автономной задачи",
		"task_id", task.ID,
		"skill", task.SkillID,
		"description", task.Description,
	)

	ctx, cancel := context.WithTimeout(context.Background(),
		time.Duration(task.TaskConfig.MaxDuration)*time.Second)
	defer cancel()

	// Загружаем скилл
	var systemPrompt string
	if task.SkillID != "" {
		if skill, ok := tm.skills.Get(task.SkillID); ok {
			sysInfo := getOSInfo()
			systemPrompt = BuildSystemPrompt(skill, sysInfo)
		}
	}

	// Если скилл не указан — простой system prompt
	if systemPrompt == "" {
		systemPrompt = fmt.Sprintf(
			"Ты — автономный AI-агент FlowLink на машине %s (%s).\n"+
				"Выполняй задачи клиента пошагово.\n"+
				"Доступные инструменты: exec, read_file, write_file, list_files\n"+
				"Формат вызова: `exec: command` или `read_file: /path`",
			getHostname(), getOSInfo(),
		)
	}

	// История чата
	messages := []LLMMessage{
		{Role: "system", Content: systemPrompt},
		{Role: "user", Content: task.Description},
	}

	// Цикл шагов
	for step := 1; step <= task.TaskConfig.MaxSteps; step++ {
		select {
		case <-ctx.Done():
			tm.finishTask(task, "timeout", fmt.Sprintf("таймаут задачи (%d сек)", task.TaskConfig.MaxDuration))
			return
		default:
		}

		// Отправляем прогресс
		tm.sendProgress(task.ID, TaskProgress{
			StepNum: step,
			Tool:    "llm_thinking",
			Status:  "step_start",
		})

		// Вызываем LLM
		resp, err := tm.llm.Chat(messages)
		if err != nil {
			tm.logger.Error("ошибка LLM", "step", step, "err", err)
			// Добавляем ошибку в контекст и пробуем ещё раз
			messages = append(messages, LLMMessage{
				Role:    "assistant",
				Content: fmt.Sprintf("Ошибка вызова LLM: %v. Попробуй ещё раз.", err),
			})
			continue
		}

		llmContent := strings.TrimSpace(resp.Content)
		tm.logger.Debug("LLM ответ", "step", step, "tokens_in", resp.TokensIn, "tokens_out", resp.TokensOut)

		// Проверяем — это tool_call или текстовый ответ?
		tool, args, ok := ParseToolCall(llmContent)
		if !ok {
			// Текстовый ответ — проверяем на завершение
			if isTaskComplete(llmContent) {
				messages = append(messages, LLMMessage{Role: "assistant", Content: llmContent})
				tm.finishTask(task, "done", "")
				return
			}

			// Добавляем в контекст и продолжаем
			messages = append(messages, LLMMessage{Role: "assistant", Content: llmContent})
			continue
		}

		// Выполняем инструмент
		stepResult := tm.executeTool(ctx, tool, args, task.TaskConfig.StepTimeout)
		taskStep := TaskStep{
			Number:    step,
			Tool:      tool,
			Args:      args,
			Output:    truncate(stepResult.output, 2000),
			Error:     stepResult.err,
			ExitCode:  stepResult.exitCode,
			Duration:  stepResult.duration,
			Timestamp: time.Now(),
		}

		tm.mu.Lock()
		task.Steps = append(task.Steps, taskStep)
		tm.mu.Unlock()

		// Отправляем прогресс
		tm.sendProgress(task.ID, TaskProgress{
			StepNum: step,
			Tool:    tool,
			Status:  "step_done",
			Output:  truncate(stepResult.output, 1000),
			Error:   stepResult.err,
		})

		// Формируем результат для LLM
		var resultText string
		if stepResult.err != "" {
			resultText = fmt.Sprintf("Ошибка: %s", stepResult.err)
		} else {
			resultText = stepResult.output
		}

		// Добавляем в контекст: что мы попросили, что получили
		messages = append(messages, LLMMessage{
			Role:    "assistant",
			Content: llmContent,
		})
		messages = append(messages, LLMMessage{
			Role:    "user",
			Content: fmt.Sprintf("Результат выполнения %s:\n```\n%s\n```", tool, resultText),
		})
	}

	// Превысили лимит шагов
	tm.finishTask(task, "error", fmt.Sprintf("превышен лимит шагов (%d)", task.TaskConfig.MaxSteps))
}

// toolResult — результат выполнения инструмента.
type toolResult struct {
	output   string
	err      string
	exitCode int
	duration int64
}

// executeTool — выполняет инструмент (exec, file ops).
func (tm *TaskManager) executeTool(ctx context.Context, tool, args string, timeout int) toolResult {
	start := time.Now()

	// Контекст с таймаутом шага
	if timeout == 0 {
		timeout = 120
	}
	stepCtx, cancel := context.WithTimeout(ctx, time.Duration(timeout)*time.Second)
	_ = stepCtx
	defer cancel()

	switch tool {
	case "exec":
		stdout, stderr, exitCode := tm.agent.executor.ExecSync(args, "", timeout)
		output := stdout
		if stderr != "" {
			if output != "" {
				output += "\n"
			}
			output += "[stderr] " + stderr
		}
		errMsg := ""
		if exitCode != 0 {
			errMsg = fmt.Sprintf("exit code %d", exitCode)
		}
		return toolResult{
			output:   output,
			err:      errMsg,
			exitCode: exitCode,
			duration: time.Since(start).Milliseconds(),
		}

	case "read_file":
		resp := ReadFile(protocol.FileReadPayload{Path: args})
		if resp.Error != "" {
			return toolResult{err: resp.Error, duration: time.Since(start).Milliseconds()}
		}
		return toolResult{output: resp.Content, duration: time.Since(start).Milliseconds()}

	case "write_file":
		// Формат: write_file: path\ncontent
		parts := strings.SplitN(args, "\n", 2)
		if len(parts) < 2 {
			return toolResult{err: "формат: write_file: path\\ncontent", duration: time.Since(start).Milliseconds()}
		}
		resp := WriteFile(protocol.FileWritePayload{
			Path:     strings.TrimSpace(parts[0]),
			Content:  parts[1],
			Encoding: "utf8",
		})
		if resp.Error != "" {
			return toolResult{err: resp.Error, duration: time.Since(start).Milliseconds()}
		}
		return toolResult{output: "файл записан", duration: time.Since(start).Milliseconds()}

	case "list_files":
		resp := ListFiles(protocol.FileListPayload{Path: args})
		if resp.Error != "" {
			return toolResult{err: resp.Error, duration: time.Since(start).Milliseconds()}
		}
		var sb strings.Builder
		for _, e := range resp.Entries {
			if e.IsDir {
				sb.WriteString(fmt.Sprintf("📁 %s/\n", e.Name))
			} else {
				sb.WriteString(fmt.Sprintf("📄 %s (%d bytes)\n", e.Name, e.Size))
			}
		}
		return toolResult{output: sb.String(), duration: time.Since(start).Milliseconds()}

	default:
		return toolResult{
			err:      fmt.Sprintf("неизвестный инструмент: %s", tool),
			duration: time.Since(start).Milliseconds(),
		}
	}
}

// finishTask — завершает задачу.
func (tm *TaskManager) finishTask(task *Task, status, errMsg string) {
	tm.mu.Lock()
	defer tm.mu.Unlock()

	task.Status = status
	task.CompletedAt = time.Now()
	task.Error = errMsg

	tm.logger.Info("задача завершена",
		"task_id", task.ID,
		"status", status,
		"steps", len(task.Steps),
		"error", errMsg,
	)

	// Отправляем финальный прогресс
	finalStatus := "task_done"
	if status == "error" {
		finalStatus = "task_error"
	}
	tm.sendProgress(task.ID, TaskProgress{
		Status: finalStatus,
		Error:  errMsg,
	})
}

// sendProgress — отправляет прогресс на реле.
func (tm *TaskManager) sendProgress(taskID string, progress TaskProgress) {
	progress.TaskID = taskID
	msg := protocol.NewMessage(protocol.MsgTaskProgress)
	msg.Payload = progress
	if err := tm.agent.write(msg); err != nil {
		tm.logger.Error("ошибка отправки прогресса", "err", err)
	}
}

// GetTask — возвращает задачу по ID.
func (tm *TaskManager) GetTask(taskID string) (*Task, bool) {
	tm.mu.Lock()
	defer tm.mu.Unlock()
	task, ok := tm.tasks[taskID]
	return task, ok
}

// ListTasks — возвращает список задач.
func (tm *TaskManager) ListTasks() []*Task {
	tm.mu.Lock()
	defer tm.mu.Unlock()
	result := make([]*Task, 0, len(tm.tasks))
	for _, t := range tm.tasks {
		result = append(result, t)
	}
	return result
}

// CancelTask — отменяет задачу.
func (tm *TaskManager) CancelTask(taskID string) error {
	tm.mu.Lock()
	defer tm.mu.Unlock()
	task, ok := tm.tasks[taskID]
	if !ok {
		return fmt.Errorf("задача %s не найдена", taskID)
	}
	task.Status = "cancelled"
	task.CompletedAt = time.Now()
	return nil
}

// isTaskComplete — проверяет, завершил ли LLM задачу.
func isTaskComplete(content string) bool {
	lower := strings.ToLower(content)
	indicators := []string{
		"задача выполнена",
		"task completed",
		"готово",
		"done.",
		"всё готово",
		"работа завершена",
	}
	for _, indicator := range indicators {
		if strings.Contains(lower, indicator) {
			return true
		}
	}
	return false
}

// Вспомогательные функции
func getOSInfo() string {
	osName, arch := config.OSInfo()
	return fmt.Sprintf("%s/%s", osName, arch)
}

func getHostname() string {
	h, _ := getHostnameSafe()
	return h
}

func getHostnameSafe() (string, error) {
	// Импортируем os
	import_os_hostname := ""
	return import_os_hostname, nil
}

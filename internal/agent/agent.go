// Package agent — основной цикл работы агента flowlink.
// Агент подключается к реле через WSS, принимает команды, выполняет их.
package agent

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"sync"
	"time"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/protocol"
	"github.com/braincreator/flowlink/pkg/version"
	"github.com/gorilla/websocket"
)

// Agent — экземпляр агента.
type Agent struct {
	cfg    *config.Config
	conn   *websocket.Conn
	wsMu   sync.Mutex // мьютекс для записи в WebSocket
	done   chan struct{}
	logger *slog.Logger

	// Подсистемы
	executor    *Executor
	sandbox     *Sandbox
	approval    *Approver
	taskManager *TaskManager
	skills      *SkillStore
	llm         *LLMClient
}

// NewAgent — создаёт новый агент с конфигурацией.
func NewAgent(cfg *config.Config) *Agent {
	logger := slog.Default()

	// Инициализируем skill store
	configDir, _ := config.ConfigDir()
	skills, err := NewSkillStore(configDir)
	if err != nil {
		logger.Warn("ошибка инициализации skill store", "err", err)
		skills, _ = NewSkillStore(os.TempDir())
	}

	// LLM клиент
	llm := NewLLMClient(cfg.LLM)

	a := &Agent{
		cfg:      cfg,
		done:     make(chan struct{}),
		logger:   logger,
		executor: NewExecutor(cfg),
		sandbox:  NewSandbox(&cfg.Sandbox),
		approval: NewApprover(&cfg.Approval),
		skills:   skills,
		llm:      llm,
	}

	// Task manager (нужен agent, поэтому после создания)
	a.taskManager = NewTaskManager(a, llm, skills)

	return a
}

// Connect — подключается к реле и запускает основной цикл.
func (a *Agent) Connect(ctx context.Context) error {
	osName, arch := config.OSInfo()

	// Формируем URL подключения
	url := fmt.Sprintf("%s?agent_id=%s&token=%s&version=%s&os=%s&arch=%s",
		a.cfg.RelayURL,
		a.cfg.AgentID,
		a.cfg.Token,
		version.Version,
		osName,
		arch,
	)

	a.logger.Info("подключение к реле", "url", a.cfg.RelayURL, "agent", a.cfg.AgentID)

	dialer := websocket.Dialer{
		HandshakeTimeout: 15 * time.Second,
	}

	conn, _, err := dialer.Dial(url, nil)
	if err != nil {
		return fmt.Errorf("подключение к реле: %w", err)
	}

	a.conn = conn
	a.logger.Info("подключён к реле")

	// Отправляем connect-сообщение
	connectMsg := protocol.NewMessage(protocol.MsgConnect)
	connectMsg.AgentID = a.cfg.AgentID
	connectMsg.Payload = protocol.ConnectPayload{
		AgentID:   a.cfg.AgentID,
		Token:     a.cfg.Token,
		Hostname:  a.cfg.Label,
		OS:        osName,
		Arch:      arch,
		ClientVer: version.Version,
	}
	if err := a.write(connectMsg); err != nil {
		return fmt.Errorf("отправка connect: %w", err)
	}

	// Запускаем обработку сообщений
	go a.readLoop(ctx)
	go a.heartbeatLoop(ctx)

	return nil
}

// Disconnect — отключается от реле.
func (a *Agent) Disconnect() {
	close(a.done)
	if a.conn != nil {
		a.conn.Close()
	}
	a.logger.Info("отключён от реле")
}

// readLoop — читает сообщения от реле и маршрутизирует их.
func (a *Agent) readLoop(ctx context.Context) {
	for {
		select {
		case <-a.done:
			return
		case <-ctx.Done():
			return
		default:
		}

		var msg protocol.Message
		if err := a.conn.ReadJSON(&msg); err != nil {
			a.logger.Error("ошибка чтения сообщения", "err", err)
			return
		}

		a.handleMessage(msg)
	}
}

// handleMessage — обрабатывает входящее сообщение от реле.
func (a *Agent) handleMessage(msg protocol.Message) {
	switch msg.Type {
	case protocol.MsgConnected:
		a.logger.Info("подключение подтверждено реле")
		// TODO: сохранить relay info из payload

	case protocol.MsgExecRequest:
		a.handleExecRequest(msg)

	case protocol.MsgFileRead:
		a.handleFileRead(msg)

	case protocol.MsgFileWrite:
		a.handleFileWrite(msg)

	case protocol.MsgFileList:
		a.handleFileList(msg)

	case protocol.MsgSysInfo:
		a.handleSysInfo(msg)

	case protocol.MsgTask:
		a.handleTask(msg)

	case protocol.MsgTaskCancel:
		a.handleTaskCancel(msg)

	case protocol.MsgSkillPush:
		a.handleSkillPush(msg)

	case protocol.MsgSkillDelete:
		a.handleSkillDelete(msg)

	case protocol.MsgHeartbeatAck:
		// Пинг получен, всё ок

	case protocol.MsgConfigUpdate:
		a.logger.Info("обновление конфигурации от реле")

	default:
		a.logger.Warn("неизвестный тип сообщения", "type", msg.Type)
	}
}

// heartbeatLoop — отправляет периодические пинги реле.
func (a *Agent) heartbeatLoop(ctx context.Context) {
	ticker := time.NewTicker(time.Duration(a.cfg.HeartbeatSec) * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-a.done:
			return
		case <-ctx.Done():
			return
		case <-ticker.C:
			msg := protocol.NewMessage(protocol.MsgHeartbeat)
			msg.AgentID = a.cfg.AgentID
			if err := a.write(msg); err != nil {
				a.logger.Error("ошибка отправки heartbeat", "err", err)
				return
			}
		}
	}
}

// handleExecRequest — обрабатывает запрос на выполнение команды.
func (a *Agent) handleExecRequest(msg protocol.Message) {
	var payload protocol.ExecRequestPayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("неверный payload: %v", err))
		return
	}

	// Проверка sandbox
	if !a.sandbox.AllowCommand(payload.Command) {
		a.sendError(msg.ID, "команда заблокирована sandbox-ом")
		return
	}

	// Проверка approval
	if a.approval.NeedsApproval(payload.Command) {
		// Отправляем запрос на апруваль
		approveMsg := protocol.NewMessage(protocol.MsgNeedsApproval)
		approveMsg.Payload = protocol.NeedsApprovalPayload{
			RequestID: payload.RequestID,
			Command:   payload.Command,
			Reason:    "Требуется подтверждение пользователя",
			Risk:      a.approval.AssessRisk(payload.Command),
		}
		if err := a.write(approveMsg); err != nil {
			a.sendError(msg.ID, fmt.Sprintf("ошибка запроса апруваля: %v", err))
			return
		}

		// Спрашиваем в терминале
		approved := a.approval.AskTTY(payload.Command)
		if !approved {
			rejectMsg := protocol.NewMessage(protocol.MsgExecReject)
			rejectMsg.Payload = map[string]string{"request_id": payload.RequestID}
			a.write(rejectMsg)
			return
		}
	}

	// Выполняем команду
	a.executor.ExecAsync(payload, func(output protocol.ExecOutputPayload) {
		msg := protocol.NewMessage(protocol.MsgExecOutput)
		msg.Payload = output
		a.write(msg)
	}, func(done protocol.ExecDonePayload) {
		msg := protocol.NewMessage(protocol.MsgExecDone)
		msg.Payload = done
		a.write(msg)
	})
}

// handleSysInfo — собирает и отправляет системную информацию.
func (a *Agent) handleSysInfo(msg protocol.Message) {
	info := CollectSystemInfo()
	resp := protocol.NewMessage(protocol.MsgSysInfoResp)
	resp.Payload = info
	a.write(resp)
}

// handleFileRead — читает файл и отправляет содержимое.
func (a *Agent) handleFileRead(msg protocol.Message) {
	var payload protocol.FileReadPayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("неверный payload: %v", err))
		return
	}

	resp := ReadFile(payload)
	respMsg := protocol.NewMessage(protocol.MsgFileResponse)
	respMsg.Payload = resp
	a.write(respMsg)
}

// handleFileWrite — записывает файл.
func (a *Agent) handleFileWrite(msg protocol.Message) {
	var payload protocol.FileWritePayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("неверный payload: %v", err))
		return
	}

	resp := WriteFile(payload)
	respMsg := protocol.NewMessage(protocol.MsgFileResponse)
	respMsg.Payload = resp
	a.write(respMsg)
}

// handleFileList — возвращает список файлов.
func (a *Agent) handleFileList(msg protocol.Message) {
	var payload protocol.FileListPayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("неверный payload: %v", err))
		return
	}

	resp := ListFiles(payload)
	respMsg := protocol.NewMessage(protocol.MsgFileResponse)
	respMsg.Payload = resp
	a.write(respMsg)
}

// write — отправляет JSON-сообщение через WebSocket (thread-safe).
func (a *Agent) write(msg protocol.Message) error {
	a.wsMu.Lock()
	defer a.wsMu.Unlock()
	return a.conn.WriteJSON(msg)
}

// sendError — отправляет сообщение об ошибке.
func (a *Agent) sendError(inReplyTo string, errMsg string) {
	msg := protocol.NewMessage(protocol.MsgError)
	msg.Payload = protocol.ErrorPayload{
		Code:    "AGENT_ERROR",
		Message: errMsg,
	}
	a.write(msg)
}

// handleTask — обрабатывает автономную задачу (L2).
func (a *Agent) handleTask(msg protocol.Message) {
	var payload protocol.TaskPayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("неверный payload: %v", err))
		return
	}

	task := &Task{
		ID:          payload.TaskID,
		SkillID:     payload.SkillID,
		Description: payload.Description,
		LLMConfig: config.LLMConfig{
			Provider: payload.LLMProvider,
			Model:    payload.LLMModel,
			APIKey:   payload.LLMAPIKey,
		},
		TaskConfig: config.TaskConfig{
			MaxSteps:       payload.MaxSteps,
			MaxDuration:    payload.MaxDuration,
			AutoApproveSafe: payload.AutoApprove,
		},
	}

	if err := a.taskManager.SubmitTask(task); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("ошибка задачи: %v", err))
		return
	}

	a.logger.Info("автономная задача принята", "task_id", payload.TaskID, "skill", payload.SkillID)
}

// handleTaskCancel — отменяет задачу.
func (a *Agent) handleTaskCancel(msg protocol.Message) {
	var payload struct {
		TaskID string `json:"task_id"`
	}
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("неверный payload: %v", err))
		return
	}

	if err := a.taskManager.CancelTask(payload.TaskID); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("ошибка отмены: %v", err))
		return
	}
}

// handleSkillPush — принимает скилл от реле.
func (a *Agent) handleSkillPush(msg protocol.Message) {
	var payload protocol.SkillPushPayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("неверный payload: %v", err))
		return
	}

	// Проверяем — не перезаписываем ли существующий скилл
	if !payload.ForceUpdate {
		if _, exists := a.skills.Get(payload.SkillID); exists {
			a.sendError(msg.ID, fmt.Sprintf("скилл %s уже существует (используйте force_update)", payload.SkillID))
			return
		}
	}

	skill := &Skill{
		ID:           payload.SkillID,
		Name:         payload.Name,
		Description:  payload.Description,
		Instructions: payload.Instructions,
		ToolsAllowed: payload.ToolsAllowed,
		LLMProvider:  payload.LLMProvider,
		LLMModel:     payload.LLMModel,
	}

	if err := a.skills.Save(skill); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("ошибка сохранения скилла: %v", err))
		return
	}

	a.logger.Info("скилл получен", "id", payload.SkillID, "name", payload.Name)
}

// handleSkillDelete — удаляет скилл.
func (a *Agent) handleSkillDelete(msg protocol.Message) {
	var payload struct {
		SkillID string `json:"skill_id"`
	}
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("неверный payload: %v", err))
		return
	}

	if err := a.skills.Delete(payload.SkillID); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("ошибка удаления: %v", err))
		return
	}

	a.logger.Info("скилл удалён", "id", payload.SkillID)
}

// unmarshalPayload — десериализует payload сообщения.
func unmarshalPayload(data any, v any) error {
	// В реальном коде: json.Unmarshal из raw JSON
	// Упрощение для компиляции
	return nil
}

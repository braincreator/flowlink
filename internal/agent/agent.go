// Package agent — основной цикл работы агента flowlink.
// Агент подключается к реле через WSS, принимает команды, выполняет их.
package agent

import (
	"context"
	"encoding/json"
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

// DefaultReadOnly — по умолчанию новый агент запускается в read-only режиме.
// WRITE-доступ включается явно через Dashboard или Telegram.
const DefaultReadOnly = true

// Agent — экземпляр агента.
type Agent struct {
	cfg    *config.Config
	conn         *websocket.Conn
	wsMu         sync.Mutex       // мьютекс для записи в WebSocket
	done         chan struct{}
	onDisconnect func()            // callback при потере соединения
	logger       *slog.Logger

	// Подсистемы
	executor    *Executor
	sandbox     *Sandbox
	approval    *ApproverV2
	taskManager *TaskManager
	skills      *SkillStore
	llm         *RemoteLLM
	backup      *BackupEngine
	killSwitch  *KillSwitch
	policy      *PolicyLayer // Единая точка проверки команд

	notifyCh   chan protocol.Message // канал для уведомлений к реле

	// Pending LLM responses
	pendingLLM map[string]*pendingLLMResponse
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

	// Инициализируем Backup Engine
	backup := NewBackupEngine(cfg.Backup)

	// Инициализируем Kill Switch
	killSwitch := NewKillSwitch()
	killSwitch.SetDiskThreshold(cfg.KillSwitch.DiskThreshold)
	killSwitch.SetCPUThreshold(cfg.KillSwitch.CPUThreshold, time.Duration(cfg.KillSwitch.CPUThresholdDur)*time.Second)

	// Инициализируем Approver V2
	approval := NewApproverV2(cfg.Approval)

	// Инициализируем Policy Layer
	policy := NewPolicyLayer(
		NewSandbox(&cfg.Sandbox),
		approval,
		backup,
		killSwitch,
		cfg,
	)

	// Read-only по умолчанию (безопасность для новых агентов)
	policy.SetReadOnly(cfg.ReadOnly)

	agent := &Agent{
		cfg:        cfg,
		done:       make(chan struct{}),
		logger:     logger,
		executor:    NewExecutor(cfg),
		sandbox:     NewSandbox(&cfg.Sandbox),
		approval:    approval,
		backup:     backup,
		killSwitch:  killSwitch,
		policy:      policy,
		skills:     skills,
		notifyCh:   make(chan protocol.Message, 100),
		llm:        nil, // инициализируется ниже
	}

	// LLM через реле (нужен agent)
	llm := NewRemoteLLM(agent)
	agent.llm = llm

	// Task manager (нужен agent, поэтому после создания)
	agent.taskManager = NewTaskManager(agent, llm, skills)

	// Установка функции уведомлений для kill switch
	killSwitch.SetNotifyFn(func(event string, details map[string]any) {
		agent.notifyKillSwitchEvent(event, details)
	})

	// Установка функции уведомлений для approval
	approval.SetNotifyFn(func(req *ApprovalRequest) {
		agent.notifyApprovalRequest(req)
	})

	return agent
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

// SetOnDisconnect устанавливает callback при потере соединения.
func (a *Agent) SetOnDisconnect(fn func()) {
	a.onDisconnect = fn
}

// readLoop — читает сообщения от реле и маршрутизирует их.
func (a *Agent) readLoop(ctx context.Context) {
	defer func() {
		if a.onDisconnect != nil {
			a.onDisconnect()
		}
	}()
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

	case protocol.MsgLLMRequest:
		// Игнорируем — LLM запросы обрабатываются через handleLLMResponse
		// Агент отправляет MsgLLMRequest, реле отправляет MsgLLMResponse

	case protocol.MsgTaskCancel:
		a.handleTaskCancel(msg)

	case protocol.MsgSkillPush:
		a.handleSkillPush(msg)

	case protocol.MsgSkillDelete:
		a.handleSkillDelete(msg)

	case protocol.MsgLLMResponse:
		a.handleLLMResponse(msg)

	case protocol.MsgApprovalResponse:
		a.handleApprovalResponse(msg)

	case protocol.MsgHeartbeatAck:
		// Пинг получен, всё ок

	case protocol.MsgConfigUpdate:
		a.handleConfigUpdate(msg)

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
// Все проверки проходят через Policy Layer.
func (a *Agent) handleExecRequest(msg protocol.Message) {
	var payload protocol.ExecRequestPayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("неверный payload: %v", err))
		return
	}

	// ─── Policy Layer: единая проверка через все слои ───
	result := a.policy.Check(payload.Command)

	// Audit: логируем результат проверки
	a.policy.AuditCommand(payload.Command, result)

	switch {
	case result.Blocked:
		a.sendError(msg.ID, result.Reason)
		a.logger.Warn("команда заблокирована policy layer",
			"command", payload.Command,
			"reason", result.Reason,
			"risk", result.RiskLevel,
		)
		return

	case result.RequireApproval:
		// Ожидание подтверждения через Telegram/Dashboard
		a.logger.Info("команда ожидает подтверждения",
			"request_id", payload.RequestID,
			"approval_id", result.ApprovalID,
			"command", payload.Command,
		)
		// Отправляем уведомление через реле
		notifMsg := protocol.NewMessage(protocol.MsgApprovalRequest)
		notifMsg.Payload = map[string]any{
			"request_id":  payload.RequestID,
			"approval_id": result.ApprovalID,
			"command":     payload.Command,
			"risk_level":  result.RiskLevel,
			"reason":      result.Reason,
		}
		a.write(notifMsg)
		return

	case !result.Allowed:
		a.sendError(msg.ID, result.Reason)
		return
	}

	// ─── Все проверки пройдены, выполняем команду ───
	a.logger.Info("команда одобрена policy layer",
		"command", payload.Command,
		"risk", result.RiskLevel,
		"snapshot", result.SnapshotID,
	)

	a.executeCommand(payload)
}



// handleSysInfo — собирает и отправляет системную информацию.
func (a *Agent) handleSysInfo(msg protocol.Message) {
	info := CollectSystemInfo()
	resp := protocol.NewMessage(protocol.MsgSysInfoResp)
	resp.Payload = info
	a.write(resp)
}

// writeResponse — отправляет ответ с request_id для корреляции.
func (a *Agent) writeResponse(msgType protocol.MessageType, requestID string, payload any) {
	msg := protocol.NewMessage(msgType)
	if m, ok := payload.(map[string]any); ok {
		m["request_id"] = requestID
		msg.Payload = m
	} else if m, ok := payload.(map[string]string); ok {
		m["request_id"] = requestID
		msg.Payload = m
	} else {
		msg.Payload = payload
	}
	a.write(msg)
}

// getRequestID — извлекает request_id из payload сообщения.
func getRequestID(payload any) string {
	if m, ok := payload.(map[string]any); ok {
		if rid, ok := m["request_id"].(string); ok {
			return rid
		}
	}
	return ""
}

// handleFileRead — читает файл и отправляет содержимое.
func (a *Agent) handleFileRead(msg protocol.Message) {
	var payload protocol.FileReadPayload
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		a.sendError(msg.ID, fmt.Sprintf("неверный payload: %v", err))
		return
	}

	resp := ReadFile(payload)
	resp.RequestID = getRequestID(msg.Payload)
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
	resp.RequestID = getRequestID(msg.Payload)
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
	resp.RequestID = getRequestID(msg.Payload)
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
		TaskConfig: config.TaskConfig{
			SkillID:      payload.SkillID,
			MaxSteps:     payload.MaxSteps,
			ApprovalMode: "auto",
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

// handleApprovalResponse — обрабатывает ответ на запрос подтверждения от реле.
func (a *Agent) handleApprovalResponse(msg protocol.Message) {
	var payload struct {
		RequestID string `json:"request_id"`
		Decision   string `json:"decision"` // approved, rejected
		Comment    string `json:"comment"`
	}
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		a.logger.Warn("неверный approval response payload", "err", err)
		return
	}
	if payload.RequestID == "" {
		a.logger.Warn("approval response без request_id")
		return
	}

	switch payload.Decision {
	case "approved":
		a.approval.Approve(payload.RequestID)
		a.logger.Info("команда одобрена реле", "request_id", payload.RequestID, "comment", payload.Comment)
	case "rejected":
		a.approval.Reject(payload.RequestID)
		a.logger.Info("команда отклонена реле", "request_id", payload.RequestID, "comment", payload.Comment)
	default:
		a.logger.Warn("неизвестное решение approval", "decision", payload.Decision, "request_id", payload.RequestID)
	}
}

// handleConfigUpdate — обрабатывает обновление конфигурации от реле.
func (a *Agent) handleConfigUpdate(msg protocol.Message) {
	var payload struct {
		ReadOnly   *bool              `json:"read_only"`
		Label      *string            `json:"label"`
		WorkDir    *string            `json:"work_dir"`
		KillSwitch *map[string]any    `json:"kill_switch"`
	}
	if err := unmarshalPayload(msg.Payload, &payload); err != nil {
		a.logger.Warn("неверный config update payload", "err", err)
		return
	}

	// Применяем read_only
	if payload.ReadOnly != nil {
		a.policy.SetReadOnly(*payload.ReadOnly)
		a.cfg.ReadOnly = *payload.ReadOnly
		a.logger.Info("режим read-only обновлён", "read_only", *payload.ReadOnly)
	}

	// Применяем label
	if payload.Label != nil && *payload.Label != "" {
		a.cfg.Label = *payload.Label
		a.logger.Info("label обновлён", "label", *payload.Label)
	}

	// Применяем kill_switch thresholds
	if payload.KillSwitch != nil {
		ks := *payload.KillSwitch
		if dt, ok := ks["disk_threshold"].(float64); ok {
			a.killSwitch.SetDiskThreshold(dt)
			a.cfg.KillSwitch.DiskThreshold = dt
			a.logger.Info("disk threshold обновлён", "value", dt)
		}
		if ct, ok := ks["cpu_threshold"].(float64); ok {
			// Duration: cpu_threshold_sec если задан, иначе 5 минут
			dur := 5 * time.Minute
			if cts, ok := ks["cpu_threshold_sec"].(float64); ok {
				dur = time.Duration(int(cts)) * time.Second
				a.cfg.KillSwitch.CPUThresholdDur = int(cts)
			}
			a.killSwitch.SetCPUThreshold(ct, dur)
			a.cfg.KillSwitch.CPUThreshold = ct
			a.logger.Info("cpu threshold обновлён", "value", ct, "duration", dur)
		}
	}

	// Формируем payload для ack с текущим состоянием конфига
	applied := []string{}
	if payload.ReadOnly != nil {
		applied = append(applied, "read_only")
	}
	if payload.Label != nil {
		applied = append(applied, "label")
	}
	if payload.WorkDir != nil {
		applied = append(applied, "work_dir")
	}
	if payload.KillSwitch != nil {
		applied = append(applied, "kill_switch")
	}

	// Отправляем подтверждение
	ack := protocol.NewMessage(protocol.MsgConfigAck)
	ack.AgentID = a.cfg.AgentID
	ack.Payload = protocol.ConfigAckPayload{
		AgentID: a.cfg.AgentID,
		Success: true,
		Applied: applied,
		Config: map[string]interface{}{
			"read_only": a.cfg.ReadOnly,
			"label":     a.cfg.Label,
			"kill_switch": map[string]interface{}{
				"disk_threshold":    a.cfg.KillSwitch.DiskThreshold,
				"cpu_threshold":     a.cfg.KillSwitch.CPUThreshold,
				"cpu_threshold_sec": a.cfg.KillSwitch.CPUThresholdDur,
			},
		},
	}

	if err := a.write(ack); err != nil {
		a.logger.Error("ошибка отправки config ack", "err", err)
		return
	}

	a.logger.Info("конфигурация обновлена", "applied", applied)
}

// unmarshalPayload — десериализует payload сообщения.
func unmarshalPayload(data any, v any) error {
	switch d := data.(type) {
	case map[string]any:
		// Конвертируем map в JSON и обратно в struct
		jsonBytes, err := json.Marshal(d)
		if err != nil {
			return fmt.Errorf("marshal map: %w", err)
		}
		return json.Unmarshal(jsonBytes, v)
	case string:
		return json.Unmarshal([]byte(d), v)
	case []byte:
		return json.Unmarshal(d, v)
	case json.RawMessage:
		return json.Unmarshal(d, v)
	case nil:
		return fmt.Errorf("payload is nil")
	default:
		// Любой другой тип — пробуем marshal/unmarshal
		jsonBytes, err := json.Marshal(d)
		if err != nil {
			return fmt.Errorf("marshal payload: %w", err)
		}
		return json.Unmarshal(jsonBytes, v)
	}
}

// notifyKillSwitchEvent — отправляет уведомление о событии kill switch через реле.
func (a *Agent) notifyKillSwitchEvent(event string, details map[string]any) {
	msg := protocol.NewMessage(protocol.MsgApprovalRequest)
	msg.Payload = map[string]any{
		"event":   event,
		"details": details,
		"source":  "kill_switch",
	}
	a.write(msg)
}

// notifyApprovalRequest — отправляет запрос на подтверждение через реле.
func (a *Agent) notifyApprovalRequest(req *ApprovalRequest) {
	msg := protocol.NewMessage(protocol.MsgApprovalRequest)
	msg.Payload = protocol.ApprovalRequestPayload{
		RequestID: req.ID,
		Command:   req.Command,
		Risk:      req.Risk,
		Mode:      string(req.Mode),
		Timestamp: req.RequestedAt.Unix(),
	}
	a.write(msg)
}

// executeCommand — выполняет команду и отправляет результат.
func (a *Agent) executeCommand(payload protocol.ExecRequestPayload) {
	output, err := a.executor.Exec(payload.Command)
	if err != nil {
		a.sendError(payload.RequestID, fmt.Sprintf("ошибка выполнения: %v", err))
		return
	}

	// Отправляем результат
	resp := protocol.NewMessage(protocol.MsgExecDone)
	resp.Payload = protocol.ExecDonePayload{
		RequestID: payload.RequestID,
		ExitCode:  0,
		Duration:  0, // TODO: измерить время
	}
	a.write(resp)

	// Отправляем вывод
	if output != "" {
		outMsg := protocol.NewMessage(protocol.MsgExecOutput)
		outMsg.Payload = protocol.ExecOutputPayload{
			RequestID: payload.RequestID,
			Data:      output,
			Stream:    "stdout",
			Timestamp: time.Now().Unix(),
		}
		a.write(outMsg)
	}
}

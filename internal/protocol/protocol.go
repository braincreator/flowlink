// Package protocol определяет форматы сообщений между агентом и реле.
// Протокол: JSON-сообщения через WSS (outbound от агента).
package protocol

import (
	"fmt"
	"time"
)

// MessageType — тип сообщения в протоколе.
type MessageType string

const (
	// === Подключение ===
	MsgConnect       MessageType = "connect"        // Агент → Реле: регистрация
	MsgConnected     MessageType = "connected"       // Реле → Агент: подтверждение
	MsgDisconnect    MessageType = "disconnect"      // Любой → Любой: отключение
	MsgHeartbeat     MessageType = "heartbeat"       // Агент → Реле: пинг
	MsgHeartbeatAck  MessageType = "heartbeat_ack"   // Реле → Агент: понг

	// === Выполнение команд ===
	MsgExecRequest     MessageType = "exec_request"     // Реле → Агент: выполнить команду
	MsgExecOutput      MessageType = "exec_output"      // Агент → Реле: stdout/stderr chunk
	MsgExecDone        MessageType = "exec_done"        // Агент → Реле: команда завершена
	MsgExecApprove     MessageType = "exec_approve"     // Агент → Реле: клиент разрешил
	MsgExecReject      MessageType = "exec_reject"      // Агент → Реле: клиент отклонил
	MsgNeedsApproval   MessageType = "needs_approval"   // Агент → Реле: нужна апруваль
	MsgApprovalRequest MessageType = "approval_request" // Агент → Реле: запрос подтверждения (v2)
	MsgApprovalResponse MessageType = "approval_response" // Реле → Агент: ответ на подтверждение (v2)

	// === Файловые операции ===
	MsgFileRead      MessageType = "file_read"       // Реле → Агент: прочитать файл
	MsgFileWrite     MessageType = "file_write"      // Реле → Агент: записать файл
	MsgFileList      MessageType = "file_list"       // Реле → Агент: список файлов
	MsgFileResponse  MessageType = "file_response"   // Агент → Реле: результат

	// === Системная информация ===
	MsgSysInfo       MessageType = "sys_info"        // Реле → Агент: запрос инфо
	MsgSysInfoResp   MessageType = "sys_info_resp"   // Агент → Реле: ответ с инфо

	// === Конфигурация ===
	MsgConfigUpdate  MessageType = "config_update"   // Реле → Агент: обновить конфиг
	MsgConfigAck     MessageType = "config_ack"      // Агент → Реле: конфиг обновлён

	// === Автономные задачи (L2) ===
	MsgTask          MessageType = "task"            // Реле → Агент: автономная задача
	MsgTaskProgress  MessageType = "task_progress"   // Агент → Реле: прогресс задачи
	MsgTaskDone      MessageType = "task_done"       // Агент → Реле: задача завершена
	MsgTaskCancel    MessageType = "task_cancel"     // Реле → Агент: отменить задачу

	// === Скиллы ===
	MsgSkillPush     MessageType = "skill_push"      // Реле → Агент: отправить скилл
	MsgSkillList     MessageType = "skill_list"      // Агент → Реле: список скиллов
	MsgSkillDelete   MessageType = "skill_delete"    // Реле → Агент: удалить скилл

	// === LLM через реле ===
	MsgLLMRequest    MessageType = "llm_request"      // Агент → Реле: запрос к LLM
	MsgLLMResponse   MessageType = "llm_response"     // Реле → Агент: ответ от LLM

	// === Резервное копирование ===
	MsgBackupRequest    MessageType = "backup_request"     // Relay → Agent: trigger backup
	MsgBackupResponse   MessageType = "backup_response"    // Agent → Relay: backup result
	MsgBackupList       MessageType = "backup_list"        // Relay → Agent: list snapshots
	MsgBackupListResp   MessageType = "backup_list_resp"   // Agent → Relay: snapshot list
	MsgBackupRestore    MessageType = "backup_restore"     // Relay → Agent: restore snapshot
	MsgBackupDelete     MessageType = "backup_delete"      // Relay → Agent: delete snapshot
	MsgBackupProgress   MessageType = "backup_progress"    // Agent → Relay: progress %

	// === Ошибка ===
	MsgError         MessageType = "error"           // Любой → Любой: ошибка
)

// Message — базовое сообщение протокола.
// Все сообщения сериализуются в JSON.
type Message struct {
	ID        string          `json:"id"`
	Type      MessageType     `json:"type"`
	AgentID   string          `json:"agent_id,omitempty"`
	SessionID string          `json:"session_id,omitempty"`
	Payload   jsonPayload     `json:"payload,omitempty"`
	Timestamp int64           `json:"timestamp"`
	Error     string          `json:"error,omitempty"`
}

// jsonPayload — aliased для кастомной маршалинга (no-op, просто any → JSON)
type jsonPayload = any // упрощение: payload сериализуется как есть

// ConnectPayload — данные при подключении агента.
type ConnectPayload struct {
	AgentID    string `json:"agent_id"`
	Token      string `json:"token"`
	Hostname   string `json:"hostname"`
	OS         string `json:"os"`
	Arch       string `json:"arch"`
	GoVersion  string `json:"go_version,omitempty"`
	ClientVer  string `json:"client_version"`
	PublicKey  string `json:"public_key,omitempty"`
}

// ConnectedPayload — ответ реле на подключение.
type ConnectedPayload struct {
	AgentID    string `json:"agent_id"`
	RelayID    string `json:"relay_id"`
	Interval   int    `json:"heartbeat_interval_sec"` // сколько секунд между пингами
	ServerTime int64  `json:"server_time"`
}

// ExecRequestPayload — запрос на выполнение команды.
type ExecRequestPayload struct {
	Command   string            `json:"command"`            // shell-команда
	Shell     string            `json:"shell,omitempty"`    // "/bin/sh" по умолчанию
	Env       map[string]string `json:"env,omitempty"`      // дополнительные env vars
	Dir       string            `json:"dir,omitempty"`      // рабочая директория
	Timeout   int               `json:"timeout_sec"`        // таймаут (0 = default 60)
	RequestID string            `json:"request_id"`         // ID для трекинга
}

// ExecOutputPayload — chunk вывода команды.
type ExecOutputPayload struct {
	RequestID string `json:"request_id"`
	Data      string `json:"data"`      // base64-encoded если binary
	Stream    string `json:"stream"`    // "stdout" | "stderr"
	Timestamp int64  `json:"timestamp"`
}

// ExecDonePayload — результат выполнения команды.
type ExecDonePayload struct {
	RequestID string `json:"request_id"`
	ExitCode  int    `json:"exit_code"`
	Duration  int64  `json:"duration_ms"`
	Error     string `json:"error,omitempty"`
}

// NeedsApprovalPayload — запрос на подтверждение от клиента.
type NeedsApprovalPayload struct {
	RequestID string `json:"request_id"`
	Command   string `json:"command"`
	Reason    string `json:"reason"`     // почему нужна апруваль
	Risk      string `json:"risk"`       // "low" | "medium" | "high"
}

// ApprovalRequestPayload — запрос на подтверждение (v2 с 3 режимами).
type ApprovalRequestPayload struct {
	RequestID string `json:"request_id"`
	Command   string `json:"command"`
	Risk      string `json:"risk"`       // "low" | "medium" | "high"
	Mode      string `json:"mode"`       // "auto" | "soft_ask" | "hard_ask"
	Timestamp int64  `json:"timestamp"`
}

// ApprovalResponsePayload — ответ на запрос подтверждения.
type ApprovalResponsePayload struct {
	RequestID string `json:"request_id"`
	Approved  bool   `json:"approved"`   // true = approved, false = rejected
	Reason    string `json:"reason,omitempty"`
	From      string `json:"from,omitempty"` // кто подтвердил (OpenClaw/user)
}

// FileReadPayload — запрос на чтение файла.
type FileReadPayload struct {
	Path     string `json:"path"`
	Offset   int64  `json:"offset,omitempty"`
	Length   int64  `json:"length,omitempty"`
	Encoding string `json:"encoding,omitempty"` // "utf8" | "base64"
}

// FileWritePayload — запрос на запись файла.
type FileWritePayload struct {
	Path     string `json:"path"`
	Content  string `json:"content"`
	Encoding string `json:"encoding"`    // "utf8" | "base64"
	Mode     int    `json:"mode,omitempty"`
}

// FileListPayload — запрос на список файлов.
type FileListPayload struct {
	Path  string `json:"path"`
	Depth int    `json:"depth,omitempty"` // глубина рекурсии (0 = только директория)
}

// FileResponsePayload — ответ на файловую операцию.
type FileResponsePayload struct {
	RequestID string      `json:"request_id,omitempty"`
	Path      string      `json:"path,omitempty"`
	Content   string      `json:"content,omitempty"`
	Encoding  string      `json:"encoding,omitempty"`
	Mode      int         `json:"mode,omitempty"`
	Size      int64       `json:"size,omitempty"`
	IsDir     bool        `json:"is_dir,omitempty"`
	Entries   []FileEntry `json:"entries,omitempty"`
	Error     string      `json:"error,omitempty"`
}

// FileEntry — элемент списка файлов.
type FileEntry struct {
	Name  string `json:"name"`
	Size  int64  `json:"size"`
	IsDir bool   `json:"is_dir"`
	Mode  int    `json:"mode"`
}

// SystemInfoPayload — системная информация.
type SystemInfoPayload struct {
	Hostname  string  `json:"hostname"`
	OS        string  `json:"os"`
	Arch      string  `json:"arch"`
	CPUCount  int     `json:"cpu_count"`
	CPUModel  string  `json:"cpu_model,omitempty"`
	MemTotal  uint64  `json:"mem_total_bytes"`
	MemUsed   uint64  `json:"mem_used_bytes"`
	DiskTotal uint64  `json:"disk_total_bytes"`
	DiskUsed  uint64  `json:"disk_used_bytes"`
	Uptime    uint64  `json:"uptime_seconds"`
	LoadAvg   []float64 `json:"load_avg,omitempty"`
}

// ErrorPayload — ошибка.
type ErrorPayload struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

// TaskPayload — автономная задача от реле к агенту.
type TaskPayload struct {
	TaskID        string          `json:"task_id"`
	SkillID       string          `json:"skill_id,omitempty"`
	Description   string          `json:"description"`
	LLMProvider   string          `json:"llm_provider,omitempty"`
	LLMModel      string          `json:"llm_model,omitempty"`
	LLMAPIKey     string          `json:"llm_api_key,omitempty"` // шифруется
	MaxSteps      int             `json:"max_steps,omitempty"`
	MaxDuration   int             `json:"max_duration_sec,omitempty"`
	AutoApprove   bool            `json:"auto_approve_safe,omitempty"`
}

// TaskProgressPayload — прогресс задачи от агента к реле.
type TaskProgressPayload struct {
	TaskID     string `json:"task_id"`
	StepNum    int    `json:"step_num"`
	TotalSteps int    `json:"total_steps,omitempty"`
	Tool       string `json:"tool,omitempty"`
	Status     string `json:"status"` // "step_start", "step_done", "task_done", "task_error"
	Output     string `json:"output,omitempty"`
	Error      string `json:"error,omitempty"`
}

// SkillPushPayload — отправка скилла от реле к агенту.
type SkillPushPayload struct {
	SkillID       string `json:"skill_id"`
	Name          string `json:"name"`
	Description   string `json:"description"`
	Instructions  string `json:"instructions"`
	ToolsAllowed  []string `json:"tools_allowed"`
	LLMProvider   string `json:"llm_provider,omitempty"`
	LLMModel      string `json:"llm_model,omitempty"`
	ForceUpdate   bool   `json:"force_update,omitempty"`
}

// SkillListPayload — список скиллов на агенте.
type SkillListPayload struct {
	Skills []SkillInfo `json:"skills"`
}

// SkillInfo — краткая информация о скилле.
type SkillInfo struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Description string `json:"description"`
	UpdatedAt   string `json:"updated_at"`
}

// === Backup Payloads ===

// BackupRequestPayload — запрос на создание бэкапа.
type BackupRequestPayload struct {
	RequestID   string   `json:"request_id"`
	Description string   `json:"description,omitempty"`
	Paths       []string `json:"paths,omitempty"`
}

// BackupResponsePayload — результат создания бэкапа.
type BackupResponsePayload struct {
	RequestID  string `json:"request_id"`
	SnapshotID string `json:"snapshot_id,omitempty"`
	Size       int64  `json:"size,omitempty"`
	Timestamp  int64  `json:"timestamp,omitempty"`
	Success    bool   `json:"success"`
	Error      string `json:"error,omitempty"`
}

// BackupListPayload — запрос списка снапшотов.
type BackupListPayload struct {
	RequestID string `json:"request_id"`
}

// BackupListResponsePayload — список снапшотов.
type BackupListResponsePayload struct {
	RequestID string     `json:"request_id"`
	Snapshots []Snapshot `json:"snapshots"`
	Count     int        `json:"count"`
}

// Snapshot — метаданные снапшота.
type Snapshot struct {
	ID          string   `json:"id"`
	Description string   `json:"description"`
	Timestamp   int64    `json:"timestamp"`
	Size        int64    `json:"size"`
	Paths       []string `json:"paths"`
	Filename    string   `json:"filename"`
}

// BackupRestorePayload — запрос на восстановление из снапшота.
type BackupRestorePayload struct {
	RequestID  string `json:"request_id"`
	SnapshotID string `json:"snapshot_id"`
}

// BackupRestoreResponsePayload — результат восстановления.
type BackupRestoreResponsePayload struct {
	RequestID  string `json:"request_id"`
	SnapshotID string `json:"snapshot_id"`
	Success    bool   `json:"success"`
	Error      string `json:"error,omitempty"`
}

// BackupDeletePayload — запрос на удаление снапшота.
type BackupDeletePayload struct {
	RequestID  string `json:"request_id"`
	SnapshotID string `json:"snapshot_id"`
}

// BackupDeleteResponsePayload — результат удаления снапшота.
type BackupDeleteResponsePayload struct {
	RequestID  string `json:"request_id"`
	SnapshotID string `json:"snapshot_id"`
	Success    bool   `json:"success"`
	Error      string `json:"error,omitempty"`
}

// BackupProgressPayload — прогресс создания бэкапа.
type BackupProgressPayload struct {
	RequestID string `json:"request_id"`
	Progress  int    `json:"progress"`  // 0-100
	Message   string `json:"message"`
}

// ConfigUpdatePayload — обновление конфигурации агента.
// Все поля optional — только переданные обновляются.
type ConfigUpdatePayload struct {
	AgentID    string                `json:"agent_id,omitempty"`    // ID агента
	ReadOnly   *bool                 `json:"read_only,omitempty"`   // true = read-only режим
	Label      *string               `json:"label,omitempty"`       // человекочитаемое имя
	WorkDir    *string               `json:"work_dir,omitempty"`    // рабочая директория
	KillSwitch *KillSwitchUpdateData `json:"kill_switch,omitempty"` // настройки kill switch
}

// KillSwitchUpdateData — данные для обновления kill switch.
type KillSwitchUpdateData struct {
	DiskThreshold   *float64 `json:"disk_threshold,omitempty"`   // порог диска для readonly
	CPUThreshold    *float64 `json:"cpu_threshold,omitempty"`    // порог CPU для паузы
	CPUThresholdDur *int     `json:"cpu_threshold_sec,omitempty"` // длительность превышения CPU
}

// ConfigAckPayload — подтверждение обновления конфигурации.
type ConfigAckPayload struct {
	AgentID  string                 `json:"agent_id"`            // ID агента
	Success  bool                   `json:"success"`             // true если успешно
	Config   map[string]interface{} `json:"config,omitempty"`    // текущий конфиг (для отладки)
	Error    string                 `json:"error,omitempty"`     // ошибка если не success
	Applied  []string               `json:"applied,omitempty"`   // список применённых полей
}

// NewMessage — создаёт новое сообщение с UUID и timestamp.
func NewMessage(msgType MessageType) Message {
	return Message{
		ID:        uuid(),
		Type:      msgType,
		Timestamp: time.Now().Unix(),
	}
}

func uuid() string {
	// Простой UUID v4 без внешней зависимости
	b := make([]byte, 16)
	// В реальном коде используем crypto/rand
	for i := range b {
		b[i] = byte(time.Now().UnixNano() >> (i * 4))
	}
	b[6] = (b[6] & 0x0f) | 0x40 // version 4
	b[8] = (b[8] & 0x3f) | 0x80 // variant 10
	return fmt.Sprintf("%08x-%04x-4%03x-%04x-%012x",
		b[0:4], b[4:6], b[6:8], b[8:10], b[10:16])
}

// Helper для генерации request ID
func RequestID() string {
	return uuid()
}

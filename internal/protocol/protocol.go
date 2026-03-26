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
	MsgExecRequest   MessageType = "exec_request"    // Реле → Агент: выполнить команду
	MsgExecOutput    MessageType = "exec_output"     // Агент → Реле: stdout/stderr chunk
	MsgExecDone      MessageType = "exec_done"       // Агент → Реле: команда завершена
	MsgExecApprove   MessageType = "exec_approve"    // Агент → Реле: клиент разрешил
	MsgExecReject    MessageType = "exec_reject"     // Агент → Реле: клиент отклонил
	MsgNeedsApproval MessageType = "needs_approval"  // Агент → Реле: нужна апруваль

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

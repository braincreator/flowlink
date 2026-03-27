package protocol

import (
	"encoding/json"
	"strings"
	"testing"
	"time"
)

func TestNewMessage(t *testing.T) {
	tests := []struct {
		name    string
		msgType MessageType
	}{
		{"connect", MsgConnect},
		{"exec_request", MsgExecRequest},
		{"file_read", MsgFileRead},
		{"heartbeat", MsgHeartbeat},
		{"error", MsgError},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			msg := NewMessage(tt.msgType)

			if msg.ID == "" {
				t.Error("ID должен быть не пустым")
			}

			if msg.Type != tt.msgType {
				t.Errorf("ожидался тип %s, получен %s", tt.msgType, msg.Type)
			}

			if msg.Timestamp == 0 {
				t.Error("Timestamp должен быть не нулевым")
			}

			// Проверяем что timestamp близок к текущему времени
			now := time.Now().Unix()
			if msg.Timestamp > now+5 || msg.Timestamp < now-5 {
				t.Errorf("Timestamp должен быть близок к текущему времени: got %d, expected ~%d", msg.Timestamp, now)
			}
		})
	}
}

func TestMessageTypeValues(t *testing.T) {
	tests := []struct {
		name     string
		msgType  MessageType
		expected string
	}{
		{"MsgConnect", MsgConnect, "connect"},
		{"MsgConnected", MsgConnected, "connected"},
		{"MsgDisconnect", MsgDisconnect, "disconnect"},
		{"MsgHeartbeat", MsgHeartbeat, "heartbeat"},
		{"MsgHeartbeatAck", MsgHeartbeatAck, "heartbeat_ack"},
		{"MsgExecRequest", MsgExecRequest, "exec_request"},
		{"MsgExecOutput", MsgExecOutput, "exec_output"},
		{"MsgExecDone", MsgExecDone, "exec_done"},
		{"MsgFileRead", MsgFileRead, "file_read"},
		{"MsgFileWrite", MsgFileWrite, "file_write"},
		{"MsgFileList", MsgFileList, "file_list"},
		{"MsgSysInfo", MsgSysInfo, "sys_info"},
		{"MsgError", MsgError, "error"},
		{"MsgLLMRequest", MsgLLMRequest, "llm_request"},
		{"MsgLLMResponse", MsgLLMResponse, "llm_response"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if string(tt.msgType) != tt.expected {
				t.Errorf("MessageType %s = %s, expected %s", tt.name, tt.msgType, tt.expected)
			}
		})
	}
}

func TestPayloadSerialization(t *testing.T) {
	t.Run("ConnectPayload", func(t *testing.T) {
		payload := ConnectPayload{
			AgentID:   "agent-123",
			Token:     "secret-token",
			Hostname:  "test-host",
			OS:        "linux",
			Arch:      "amd64",
			GoVersion: "go1.22",
			ClientVer: "1.0.0",
			PublicKey: "ssh-rsa AAAA...",
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации: %v", err)
		}

		var decoded ConnectPayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if decoded.AgentID != payload.AgentID {
			t.Errorf("AgentID: got %s, want %s", decoded.AgentID, payload.AgentID)
		}
		if decoded.Token != payload.Token {
			t.Errorf("Token: got %s, want %s", decoded.Token, payload.Token)
		}
		if decoded.Hostname != payload.Hostname {
			t.Errorf("Hostname: got %s, want %s", decoded.Hostname, payload.Hostname)
		}
	})

	t.Run("ExecRequestPayload", func(t *testing.T) {
		payload := ExecRequestPayload{
			Command:   "ls -la",
			Shell:     "/bin/bash",
			Env:       map[string]string{"FOO": "bar", "BAZ": "qux"},
			Dir:       "/home/user",
			Timeout:   60,
			RequestID: "req-123",
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации: %v", err)
		}

		var decoded ExecRequestPayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if decoded.Command != payload.Command {
			t.Errorf("Command: got %s, want %s", decoded.Command, payload.Command)
		}
		if decoded.Env["FOO"] != "bar" {
			t.Errorf("Env[FOO]: got %s, want bar", decoded.Env["FOO"])
		}
	})

	t.Run("ExecOutputPayload", func(t *testing.T) {
		payload := ExecOutputPayload{
			RequestID: "req-123",
			Data:      "output text\nline 2",
			Stream:    "stdout",
			Timestamp: time.Now().Unix(),
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации: %v", err)
		}

		var decoded ExecOutputPayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if decoded.Data != payload.Data {
			t.Errorf("Data: got %s, want %s", decoded.Data, payload.Data)
		}
		if decoded.Stream != "stdout" {
			t.Errorf("Stream: got %s, want stdout", decoded.Stream)
		}
	})

	t.Run("FileReadPayload", func(t *testing.T) {
		payload := FileReadPayload{
			Path:     "/tmp/test.txt",
			Offset:   100,
			Length:   500,
			Encoding: "utf8",
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации: %v", err)
		}

		var decoded FileReadPayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if decoded.Path != payload.Path {
			t.Errorf("Path: got %s, want %s", decoded.Path, payload.Path)
		}
	})

	t.Run("SystemInfoPayload", func(t *testing.T) {
		payload := SystemInfoPayload{
			Hostname:  "test-host",
			OS:        "linux",
			Arch:      "amd64",
			CPUCount:  8,
			CPUModel:  "Intel i7",
			MemTotal:  16 * 1024 * 1024 * 1024,
			MemUsed:   8 * 1024 * 1024 * 1024,
			DiskTotal: 512 * 1024 * 1024 * 1024,
			DiskUsed:  256 * 1024 * 1024 * 1024,
			Uptime:    86400,
			LoadAvg:   []float64{1.5, 1.2, 1.0},
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации: %v", err)
		}

		var decoded SystemInfoPayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if decoded.Hostname != payload.Hostname {
			t.Errorf("Hostname: got %s, want %s", decoded.Hostname, payload.Hostname)
		}
		if decoded.CPUCount != payload.CPUCount {
			t.Errorf("CPUCount: got %d, want %d", decoded.CPUCount, payload.CPUCount)
		}
	})

	t.Run("SkillPushPayload", func(t *testing.T) {
		payload := SkillPushPayload{
			SkillID:       "skill-123",
			Name:          "Test Skill",
			Description:   "Test description",
			Instructions:  "Do something",
			ToolsAllowed:  []string{"exec", "file_read"},
			LLMProvider:   "openai",
			LLMModel:      "gpt-4",
			ForceUpdate:   true,
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации: %v", err)
		}

		var decoded SkillPushPayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if decoded.SkillID != payload.SkillID {
			t.Errorf("SkillID: got %s, want %s", decoded.SkillID, payload.SkillID)
		}
		if len(decoded.ToolsAllowed) != 2 {
			t.Errorf("ToolsAllowed length: got %d, want 2", len(decoded.ToolsAllowed))
		}
	})
}

func TestMessageSerialization(t *testing.T) {
	msg := NewMessage(MsgExecRequest)
	msg.AgentID = "agent-123"
	msg.SessionID = "session-456"
	msg.Payload = map[string]string{
		"command": "ls -la",
	}

	data, err := json.Marshal(msg)
	if err != nil {
		t.Fatalf("ошибка сериализации сообщения: %v", err)
	}

	var decoded Message
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("ошибка десериализации сообщения: %v", err)
	}

	if decoded.Type != MsgExecRequest {
		t.Errorf("Type: got %s, want %s", decoded.Type, MsgExecRequest)
	}
	if decoded.AgentID != "agent-123" {
		t.Errorf("AgentID: got %s, want agent-123", decoded.AgentID)
	}
}

func TestRequestID(t *testing.T) {
	id1 := RequestID()
	id2 := RequestID()

	if id1 == "" {
		t.Error("RequestID должен быть не пустым")
	}

	if id1 == id2 {
		t.Error("RequestID должен генерировать уникальные ID")
	}

	// Проверяем формат UUID (упрощённо)
	if len(id1) < 30 {
		t.Errorf("RequestID слишком короткий: %s", id1)
	}

	// Проверяем что содержит дефисы
	if !strings.Contains(id1, "-") {
		t.Errorf("RequestID должен содержать дефисы: %s", id1)
	}
}

func TestUUID(t *testing.T) {
	uuid1 := uuid()
	uuid2 := uuid()

	if uuid1 == "" {
		t.Error("UUID должен быть не пустым")
	}

	if uuid1 == uuid2 {
		t.Error("UUID должен генерировать уникальные значения")
	}

	// Проверяем формат
	if !strings.Contains(uuid1, "-") {
		t.Errorf("UUID должен содержать дефисы: %s", uuid1)
	}
}

func TestMessageWithError(t *testing.T) {
	msg := NewMessage(MsgError)
	msg.Error = "something went wrong"

	data, err := json.Marshal(msg)
	if err != nil {
		t.Fatalf("ошибка сериализации: %v", err)
	}

	var decoded Message
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatalf("ошибка десериализации: %v", err)
	}

	if decoded.Error != "something went wrong" {
		t.Errorf("Error: got %s, want 'something went wrong'", decoded.Error)
	}
}

func TestApprovalPayloads(t *testing.T) {
	t.Run("ApprovalRequestPayload", func(t *testing.T) {
		payload := ApprovalRequestPayload{
			RequestID: "req-123",
			Command:   "rm -rf /tmp/test",
			Risk:      "high",
			Mode:      "hard_ask",
			Timestamp: time.Now().Unix(),
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации: %v", err)
		}

		var decoded ApprovalRequestPayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if decoded.Risk != "high" {
			t.Errorf("Risk: got %s, want high", decoded.Risk)
		}
	})

	t.Run("ApprovalResponsePayload", func(t *testing.T) {
		payload := ApprovalResponsePayload{
			RequestID: "req-123",
			Approved:  true,
			Reason:    "User confirmed",
			From:      "user",
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации: %v", err)
		}

		var decoded ApprovalResponsePayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if !decoded.Approved {
			t.Error("Approved должен быть true")
		}
	})
}

func TestTaskPayloads(t *testing.T) {
	t.Run("TaskPayload", func(t *testing.T) {
		payload := TaskPayload{
			TaskID:      "task-123",
			SkillID:     "skill-456",
			Description: "Test task",
			LLMProvider: "openai",
			LLMModel:    "gpt-4",
			MaxSteps:    10,
			AutoApprove: true,
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации: %v", err)
		}

		var decoded TaskPayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if decoded.TaskID != "task-123" {
			t.Errorf("TaskID: got %s, want task-123", decoded.TaskID)
		}
	})

	t.Run("TaskProgressPayload", func(t *testing.T) {
		payload := TaskProgressPayload{
			TaskID:     "task-123",
			StepNum:    5,
			TotalSteps: 10,
			Tool:       "exec",
			Status:     "step_done",
			Output:     "completed",
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации: %v", err)
		}

		var decoded TaskProgressPayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if decoded.Status != "step_done" {
			t.Errorf("Status: got %s, want step_done", decoded.Status)
		}
	})
}

func TestFilePayloads(t *testing.T) {
	t.Run("FileWritePayload", func(t *testing.T) {
		payload := FileWritePayload{
			Path:     "/tmp/test.txt",
			Content:  "Hello, World!",
			Encoding: "utf8",
			Mode:     0644,
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации: %v", err)
		}

		var decoded FileWritePayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if decoded.Content != "Hello, World!" {
			t.Errorf("Content: got %s, want 'Hello, World!'", decoded.Content)
		}
	})

	t.Run("FileResponsePayload", func(t *testing.T) {
		payload := FileResponsePayload{
			Path:     "/tmp/test.txt",
			Content:  "file content",
			Encoding: "utf8",
			Size:     12,
			IsDir:    false,
			Mode:     0644,
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации: %v", err)
		}

		var decoded FileResponsePayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if decoded.Size != 12 {
			t.Errorf("Size: got %d, want 12", decoded.Size)
		}
	})

	t.Run("FileListPayload", func(t *testing.T) {
		payload := FileListPayload{
			Path:  "/home/user",
			Depth: 2,
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации: %v", err)
		}

		var decoded FileListPayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if decoded.Depth != 2 {
			t.Errorf("Depth: got %d, want 2", decoded.Depth)
		}
	})
}

func TestEdgeCases(t *testing.T) {
	t.Run("EmptyPayload", func(t *testing.T) {
		msg := NewMessage(MsgHeartbeat)
		msg.Payload = nil

		data, err := json.Marshal(msg)
		if err != nil {
			t.Fatalf("ошибка сериализации с nil payload: %v", err)
		}

		var decoded Message
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}
	})

	t.Run("VeryLongStrings", func(t *testing.T) {
		longStr := strings.Repeat("a", 10000)
		payload := ExecRequestPayload{
			Command:   longStr,
			RequestID: "req-123",
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации длинной строки: %v", err)
		}

		var decoded ExecRequestPayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if len(decoded.Command) != 10000 {
			t.Errorf("длина Command: got %d, want 10000", len(decoded.Command))
		}
	})

	t.Run("UnicodeInPayload", func(t *testing.T) {
		payload := FileWritePayload{
			Path:     "/tmp/тест.txt",
			Content:  "Привет, мир! 🌍",
			Encoding: "utf8",
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации unicode: %v", err)
		}

		var decoded FileWritePayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if !strings.Contains(decoded.Content, "🌍") {
			t.Error("Unicode эмодзи должен сохраниться")
		}
	})

	t.Run("EmptyStrings", func(t *testing.T) {
		payload := ExecRequestPayload{
			Command:   "",
			RequestID: "",
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации пустых строк: %v", err)
		}

		var decoded ExecRequestPayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if decoded.Command != "" {
			t.Errorf("Command: got %s, want empty", decoded.Command)
		}
	})

	t.Run("SpecialCharacters", func(t *testing.T) {
		payload := ExecRequestPayload{
			Command:   "echo \"test\" && cat /tmp/file | grep 'pattern'",
			RequestID: "req-123",
		}

		data, err := json.Marshal(payload)
		if err != nil {
			t.Fatalf("ошибка сериализации спецсимволов: %v", err)
		}

		var decoded ExecRequestPayload
		if err := json.Unmarshal(data, &decoded); err != nil {
			t.Fatalf("ошибка десериализации: %v", err)
		}

		if !strings.Contains(decoded.Command, "&&") {
			t.Error("Специальные символы должны сохраниться")
		}
	})
}

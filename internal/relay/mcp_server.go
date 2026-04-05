package relay

import (
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"strings"
	"time"

	"github.com/braincreator/flowlink/internal/protocol"
	"github.com/google/uuid"
)

// MCP Server — flowlink relay как MCP сервер для OpenClaw.
// Transport: Streamable HTTP (POST /mcp + GET /mcp для SSE).
// OpenClaw подключает через mcporter и видит flowlink агентов как инструменты.

// === MCP Protocol Types ===

type mcpRequest struct {
	JSONRPC string `json:"jsonrpc"`
	ID      any    `json:"id,omitempty"`
	Method  string `json:"method"`
	Params  any    `json:"params,omitempty"`
}

type mcpResponse struct {
	JSONRPC string `json:"jsonrpc"`
	ID      any    `json:"id"`
	Result  any    `json:"result,omitempty"`
	Error   *mcpError `json:"error,omitempty"`
}

type mcpError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

type mcpTool struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	InputSchema any    `json:"inputSchema"`
}

// === Tool Definitions ===

func mcpTools() []mcpTool {
	return []mcpTool{
		{
			Name:        "flowlink_agents",
			Description: "Список всех подключённых flowlink-агентов. Возвращает ID, hostname, OS, arch, статус подключения. Используй для выбора агента перед другими командами.",
			InputSchema: map[string]any{
				"type": "object",
				"properties": map[string]any{
					"status": map[string]any{
						"type":        "string",
						"enum":        []string{"all", "online"},
						"default":     "online",
						"description": "Фильтр по статусу",
					},
				},
			},
		},
		{
			Name:        "flowlink_exec",
			Description: "Выполнить shell-команду на удалённой машине через flowlink-агента. Возвращает stdout, stderr, exit code.",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent", "command"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента (из flowlink_agents) или label (hostname)",
					},
					"command": map[string]any{
						"type":        "string",
						"description": "Shell-команда для выполнения",
					},
					"timeout": map[string]any{
						"type":        "integer",
						"default":     120,
						"description": "Таймаут в секундах",
					},
					"workdir": map[string]any{
						"type":        "string",
						"description": "Рабочая директория (опционально)",
					},
				},
			},
		},
		{
			Name:        "flowlink_read",
			Description: "Прочитать файл с удалённой машины через flowlink-агент.",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent", "path"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента или label",
					},
					"path": map[string]any{
						"type":        "string",
						"description": "Путь к файлу",
					},
				},
			},
		},
		{
			Name:        "flowlink_write",
			Description: "Записать файл на удалённую машину через flowlink-агент.",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent", "path", "content"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента или label",
					},
					"path": map[string]any{
						"type":        "string",
						"description": "Путь к файлу",
					},
					"content": map[string]any{
						"type":        "string",
						"description": "Содержимое файла",
					},
				},
			},
		},
		{
			Name:        "flowlink_list",
			Description: "Получить список файлов/директорий на удалённой машине.",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent", "path"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента или label",
					},
					"path": map[string]any{
						"type":        "string",
						"description": "Путь к директории",
					},
				},
			},
		},
		{
			Name:        "flowlink_sysinfo",
			Description: "Получить системную информацию об удалённой машине (CPU, RAM, OS, disk, network).",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента или label",
					},
				},
			},
		},
		{
			Name:        "flowlink_task",
			Description: "Запустить автономную задачу на удалённой машине (L2). Агент выполняет задачу пошагово, используя LLM через реле. Возвращает task_id для отслеживания прогресса.",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent", "description"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента или label",
					},
					"description": map[string]any{
						"type":        "string",
						"description": "Описание задачи для выполнения",
					},
					"skill_id": map[string]any{
						"type":        "string",
						"description": "ID скилла (если есть специфичный для задачи)",
					},
					"max_steps": map[string]any{
						"type":        "integer",
						"default":     20,
						"description": "Максимальное количество шагов",
					},
				},
			},
		},
		{
			Name:        "flowlink_task_status",
			Description: "Получить статус автономной задачи на удалённой машине.",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent", "task_id"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента или label",
					},
					"task_id": map[string]any{
						"type":        "string",
						"description": "ID задачи (из flowlink_task)",
					},
				},
			},
		},
		{
			Name:        "flowlink_backup",
			Description: "Создать бэкап на удалённой машине. Проверяет лимиты бэкапов и хранилища перед созданием.",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента или label",
					},
					"description": map[string]any{
						"type":        "string",
						"description": "Описание бэкапа (опционально)",
					},
					"paths": map[string]any{
						"type":        "array",
						"items":       map[string]string{"type": "string"},
						"description": "Пути для бэкапа (опционально)",
					},
				},
			},
		},
		{
			Name:        "flowlink_restore",
			Description: "Восстановить из бэкапа на удалённой машине.",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent", "snapshot_id"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента или label",
					},
					"snapshot_id": map[string]any{
						"type":        "string",
						"description": "ID снапшота для восстановления",
					},
				},
			},
		},
		{
			Name:        "flowlink_backup_list",
			Description: "List all relay-side backups. Returns JSON array of {id, description, timestamp, size, paths}.",
			InputSchema: map[string]any{
				"type":       "object",
				"properties": map[string]any{},
			},
		},
		{
			Name:        "flowlink_backup_delete",
			Description: "Delete a specific relay-side backup by snapshot ID.",
			InputSchema: map[string]any{
				"type": "object",
				"required": []string{"snapshot_id"},
				"properties": map[string]any{
					"snapshot_id": map[string]any{
						"type":        "string",
						"description": "Snapshot ID to delete",
					},
				},
			},
		},
		{
			Name:        "flowlink_kill",
			Description: "Kill switch для процессов на удалённой машине (stop/pause/resume).",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent", "action"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента или label",
					},
					"action": map[string]any{
						"type":        "string",
						"enum":        []string{"stop", "pause", "resume"},
						"description": "Действие: stop, pause, resume",
					},
					"pid": map[string]any{
						"type":        "integer",
						"description": "PID процесса (опционально, для stop)",
					},
				},
			},
		},
		{
			Name:        "flowlink_approve",
			Description: "Одобрить pending approval request на удалённой машине.",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent", "request_id"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента или label",
					},
					"request_id": map[string]any{
						"type":        "string",
						"description": "ID запроса на одобрение",
					},
					"approved": map[string]any{
						"type":        "boolean",
						"default":     true,
						"description": "Одобрить (true) или отклонить (false)",
					},
				},
			},
		},
		{
			Name:        "flowlink_logs",
			Description: "Получить логи с удалённой машины (tail).",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента или label",
					},
					"lines": map[string]any{
						"type":        "integer",
						"default":     100,
						"description": "Количество строк (default: 100)",
					},
					"service": map[string]any{
						"type":        "string",
						"description": "Имя сервиса для логов (опционально)",
					},
				},
			},
		},
		{
			Name:        "flowlink_deploy",
			Description: "Перезапустить сервис на удалённой машине.",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent", "service"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента или label",
					},
					"service": map[string]any{
						"type":        "string",
						"description": "Имя сервиса для перезапуска",
					},
					"action": map[string]any{
						"type":        "string",
						"enum":        []string{"restart", "start", "stop"},
						"default":     "restart",
						"description": "Действие: restart, start, stop (default: restart)",
					},
				},
			},
		},
		{
			Name:        "flowlink_top",
			Description: "Получить top процессов на удалённой машине (CPU, memory usage).",
			InputSchema: map[string]any{
				"type": "object",
				"required":             []string{"agent"},
				"properties": map[string]any{
					"agent": map[string]any{
						"type":        "string",
						"description": "ID агента или label",
					},
					"sort_by": map[string]any{
						"type":        "string",
						"enum":        []string{"cpu", "mem"},
						"default":     "cpu",
						"description": "Сортировать по: cpu, mem (default: cpu)",
					},
					"limit": map[string]any{
						"type":        "integer",
						"default":     20,
						"description": "Количество процессов (default: 20)",
					},
				},
			},
		},
	}
}

// === MCP HTTP Handler ===

// HandleMCPForTest — экспортированная версия handleMCP для тестов.
func (r *Relay) HandleMCPForTest(w http.ResponseWriter, req *http.Request) {
	r.handleMCP(w, req)
}

// handleMCP — основной MCP endpoint (Streamable HTTP transport).
// POST: JSON-RPC request → JSON-RPC response
// GET: SSE stream (для notifications)
func (r *Relay) handleMCP(w http.ResponseWriter, req *http.Request) {
	// Auth check
	if r.cfg.APIToken != "" {
		token := req.Header.Get("Authorization")
		if token == "" {
			token = req.URL.Query().Get("token")
		}
		if token != "Bearer "+r.cfg.APIToken && token != r.cfg.APIToken {
			http.Error(w, `{"error":"unauthorized"}`, http.StatusUnauthorized)
			return
		}
	}

	switch req.Method {
	case http.MethodGet:
		// SSE endpoint — для streamable HTTP
		w.Header().Set("Content-Type", "text/event-stream")
		w.Header().Set("Cache-Control", "no-cache")
		w.Header().Set("Connection", "keep-alive")
		flusher, ok := w.(http.Flusher)
		if !ok {
			http.Error(w, "streaming not supported", http.StatusInternalServerError)
			return
		}
		// Ping каждые 30с
		ticker := time.NewTicker(30 * time.Second)
		defer ticker.Stop()
		for {
			select {
			case <-req.Context().Done():
				return
			case <-ticker.C:
				fmt.Fprintf(w, ": ping\n\n")
				flusher.Flush()
			}
		}

	case http.MethodPost:
		r.handleMCPRequest(w, req)

	case http.MethodOptions:
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "POST, GET, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")
		w.WriteHeader(http.StatusOK)

	default:
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
	}
}

// handleMCPRequest — обрабатывает один JSON-RPC запрос.
func (r *Relay) handleMCPRequest(w http.ResponseWriter, req *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	w.Header().Set("Access-Control-Allow-Origin", "*")

	var rpcReq mcpRequest
	if err := json.NewDecoder(req.Body).Decode(&rpcReq); err != nil {
		writeMCPError(w, nil, -32700, "parse error: "+err.Error())
		return
	}

	slog.Debug("MCP запрос", "method", rpcReq.Method, "id", rpcReq.ID)

	switch rpcReq.Method {
	case "initialize":
		writeMCPResult(w, rpcReq.ID, map[string]any{
			"protocolVersion": "2024-11-05",
			"capabilities": map[string]any{
				"tools": map[string]any{},
			},
			"serverInfo": map[string]any{
				"name":    "flowlink-relay",
				"version": "0.1.0",
			},
		})

	case "notifications/initialized":
		// Ack
		w.WriteHeader(http.StatusNoContent)

	case "tools/list":
		tools := mcpTools()
		writeMCPResult(w, rpcReq.ID, map[string]any{"tools": tools})

	case "tools/call":
		r.handleMCPCall(w, rpcReq)

	default:
		writeMCPError(w, rpcReq.ID, -32601, "method not found: "+rpcReq.Method)
	}
}

// handleMCPCall — обрабатывает вызов MCP инструмента.
func (r *Relay) handleMCPCall(w http.ResponseWriter, rpcReq mcpRequest) {
	// Извлекаем параметры
	params, ok := rpcReq.Params.(map[string]any)
	if !ok {
		writeMCPError(w, rpcReq.ID, -32602, "invalid params")
		return
	}

	name, _ := params["name"].(string)
	args, _ := params["arguments"].(map[string]any)

	switch name {
	case "flowlink_agents":
		r.mcpAgents(w, rpcReq.ID, args)
	case "flowlink_exec":
		r.mcpExec(w, rpcReq.ID, args)
	case "flowlink_read":
		r.mcpRead(w, rpcReq.ID, args)
	case "flowlink_write":
		r.mcpWrite(w, rpcReq.ID, args)
	case "flowlink_list":
		r.mcpList(w, rpcReq.ID, args)
	case "flowlink_sysinfo":
		r.mcpSysinfo(w, rpcReq.ID, args)
	case "flowlink_task":
		r.mcpTask(w, rpcReq.ID, args)
	case "flowlink_task_status":
		r.mcpTaskStatus(w, rpcReq.ID, args)
	case "flowlink_backup":
		r.mcpBackup(w, rpcReq.ID, args)
	case "flowlink_restore":
		r.mcpRestore(w, rpcReq.ID, args)
	case "flowlink_backup_list":
		r.mcpBackupList(w, rpcReq.ID, args)
	case "flowlink_backup_delete":
		r.mcpBackupDelete(w, rpcReq.ID, args)
	case "flowlink_kill":
		r.mcpKill(w, rpcReq.ID, args)
	case "flowlink_approve":
		r.mcpApprove(w, rpcReq.ID, args)
	case "flowlink_logs":
		r.mcpLogs(w, rpcReq.ID, args)
	case "flowlink_deploy":
		r.mcpDeploy(w, rpcReq.ID, args)
	case "flowlink_top":
		r.mcpTop(w, rpcReq.ID, args)
	default:
		writeMCPError(w, rpcReq.ID, -32602, "unknown tool: "+name)
	}
}

// === MCP Tool Implementations ===

// mcpAgents — список агентов.
func (r *Relay) mcpAgents(w http.ResponseWriter, id any, args map[string]any) {
	agents := r.pool.List()
	result := make([]map[string]any, 0, len(agents))
	for _, ac := range agents {
		ago := time.Since(ac.LastSeen).Round(time.Second).String()
		result = append(result, map[string]any{
			"id":        ac.ID,
			"hostname":  ac.Hostname,
			"os":        ac.OS,
			"arch":      ac.Arch,
			"version":   ac.Version,
			"connected": ac.Connected.Format(time.RFC3339),
			"last_seen": ago + " ago",
			"online":    time.Since(ac.LastSeen) < 2*time.Minute,
		})
	}

	if len(result) == 0 {
		writeMCPResult(w, id, map[string]any{
			"content": []map[string]any{{
				"type": "text",
				"text": "Нет подключённых flowlink-агентов.",
			}},
		})
		return
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("Подключённых агентов: %d\n\n", len(result)))
	for _, a := range result {
		status := "🟢"
		if a["online"] == false {
			status = "🔴"
		}
		sb.WriteString(fmt.Sprintf("%s %s (%s, %s/%s) — last seen: %s\n",
			status, a["hostname"], a["id"], a["os"], a["arch"], a["last_seen"]))
	}

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": sb.String(),
		}},
	})
}

// mcpExec — выполнить команду на агенте.
func (r *Relay) mcpExec(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}
	command, ok := args["command"].(string)
	if !ok || command == "" {
		writeMCPError(w, id, -32602, "command: required")
		return
	}
	timeout := 120
	if t, ok := args["timeout"].(float64); ok {
		timeout = int(t)
	}
	workdir, _ := args["workdir"].(string)

	// Резолвим агента (по ID или hostname)
	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	// Отправляем команду через WSS
	resp, err := r.sendAndWait(ac, "exec", map[string]any{
		"command": command,
		"timeout": timeout,
		"workdir": workdir,
	}, 30*time.Second)
	if err != nil {
		writeMCPError(w, id, -32603, protocol.CodeMCPAgentError+": "+err.Error())
		return
	}

	resultText := formatExecResult(resp)
	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": resultText,
		}},
	})
}

// mcpRead — прочитать файл.
func (r *Relay) mcpRead(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}
	path, ok := args["path"].(string)
	if !ok || path == "" {
		writeMCPError(w, id, -32602, "path: required")
		return
	}

	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	resp, err := r.sendAndWait(ac, "read_file", map[string]any{
		"path": path,
	}, 15*time.Second)
	if err != nil {
		writeMCPError(w, id, -32603, err.Error())
		return
	}

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("Файл: %s\n```\n%s\n```", path, resp),
		}},
	})
}

// mcpWrite — записать файл.
func (r *Relay) mcpWrite(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}
	path, ok := args["path"].(string)
	if !ok || path == "" {
		writeMCPError(w, id, -32602, "path: required")
		return
	}
	content, _ := args["content"].(string)

	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	resp, err := r.sendAndWait(ac, "write_file", map[string]any{
		"path":     path,
		"content":  content,
		"encoding": "utf8",
	}, 15*time.Second)
	if err != nil {
		writeMCPError(w, id, -32603, err.Error())
		return
	}

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("✅ Файл записан: %s\n%s", path, resp),
		}},
	})
}

// mcpList — список файлов.
func (r *Relay) mcpList(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}
	path, ok := args["path"].(string)
	if !ok || path == "" {
		writeMCPError(w, id, -32602, "path: required")
		return
	}

	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	resp, err := r.sendAndWait(ac, "list_files", map[string]any{
		"path": path,
	}, 15*time.Second)
	if err != nil {
		writeMCPError(w, id, -32603, err.Error())
		return
	}

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("📁 %s\n%s", path, resp),
		}},
	})
}

// mcpSysinfo — системная информация.
func (r *Relay) mcpSysinfo(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}

	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	resp, err := r.sendAndWait(ac, "sysinfo", nil, 10*time.Second)
	if err != nil {
		writeMCPError(w, id, -32603, err.Error())
		return
	}

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("Системная информация %s:\n%s", agentID, resp),
		}},
	})
}

// mcpTask — запустить автономную задачу.
func (r *Relay) mcpTask(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}
	description, ok := args["description"].(string)
	if !ok || description == "" {
		writeMCPError(w, id, -32602, "description: required")
		return
	}
	skillID, _ := args["skill_id"].(string)
	maxSteps := 20
	if ms, ok := args["max_steps"].(float64); ok {
		maxSteps = int(ms)
	}

	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	taskID := uuid.New().String()[:8]

	// Отправляем задачу агенту
	msg := newMessage("task")
	msg.Payload = map[string]any{
		"task_id":     taskID,
		"description": description,
		"skill_id":    skillID,
		"max_steps":   maxSteps,
	}
	ac.SendMessage(msg)

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("✅ Задача запущена: %s\nАгент: %s\nОписание: %s\nМакс шагов: %d\n\nОтслеживай через flowlink_task_status(agent=%s, task_id=%s)",
				taskID, agentID, description, maxSteps, agentID, taskID),
		}},
	})
}

// mcpTaskStatus — статус задачи.
func (r *Relay) mcpTaskStatus(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}
	taskID, ok := args["task_id"].(string)
	if !ok || taskID == "" {
		writeMCPError(w, id, -32602, "task_id: required")
		return
	}

	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	resp, err := r.sendAndWait(ac, "task_status", map[string]any{
		"task_id": taskID,
	}, 10*time.Second)
	if err != nil {
		writeMCPError(w, id, -32603, err.Error())
		return
	}

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("Статус задачи %s: %s", taskID, resp),
		}},
	})
}

// === Helpers ===

// resolveAgent — находит агента по ID или hostname/label.
func (r *Relay) resolveAgent(selector string) (*AgentConn, error) {
	// По ID
	if ac, ok := r.pool.Get(selector); ok {
		return ac, nil
	}

	// По hostname
	agents := r.pool.List()
	for _, ac := range agents {
		if ac.Hostname == selector {
			return ac, nil
		}
	}

	return nil, protocol.Err(protocol.CodeMCPAgentNotFound, selector, len(agents))
}

// sendAndWait — отправляет команду агенту через WSS и ждёт ответ.
func (r *Relay) sendAndWait(ac *AgentConn, action string, payload any, timeout time.Duration) (string, error) {
	requestID := uuid.New().String()[:8]

	msg := newMessage(action)
	msg.Payload = payload
	// Добавляем request_id для корреляции
	if m, ok := msg.Payload.(map[string]any); ok {
		m["request_id"] = requestID
	}

	// Регистрируем ожидание
	respCh := make(chan any, 1)
	ac.RegisterCallback(requestID, func(data any) {
		respCh <- data
	})
	defer ac.RemoveCallback(requestID)

	// Отправляем
	ac.SendMessage(msg)

	// Ждём ответ
	select {
	case resp := <-respCh:
		if m, ok := resp.(map[string]any); ok {
			if errMsg, ok := m["error"].(string); ok && errMsg != "" {
				return "", fmt.Errorf("%s", errMsg)
			}
			// Сериализуем результат
			b, _ := json.MarshalIndent(m, "", "  ")
			return string(b), nil
		}
		if s, ok := resp.(string); ok {
			return s, nil
		}
		b, _ := json.MarshalIndent(resp, "", "  ")
		return string(b), nil

	case <-time.After(timeout):
		return "", protocol.Err(protocol.CodeMCPTimeout, timeout)
	}
}

// mcpBackup — создать бэкап.
func (r *Relay) mcpBackup(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}

	description, _ := args["description"].(string)
	var paths []string
	if p, ok := args["paths"].([]any); ok {
		for _, path := range p {
			if s, ok := path.(string); ok {
				paths = append(paths, s)
			}
		}
	}

	// Резолвим агента
	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	// TODO: Billing check — нужен clientID из контекста
	// Пока пропускаем проверку лимитов для MCP

	// Отправляем запрос на бэкап
	requestID := uuid.New().String()
	msg := protocol.NewMessage(protocol.MsgBackupRequest)
	msg.Payload = protocol.BackupRequestPayload{
		RequestID:   requestID,
		Description: description,
		Paths:       paths,
	}

	if err := ac.SendMessage(msg); err != nil {
		writeMCPError(w, id, -32603, "failed to send backup request: "+err.Error())
		return
	}

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("✅ Backup request sent\nRequest ID: %s\nAgent: %s", requestID, ac.ID),
		}},
	})
}

// mcpRestore — восстановить из бэкапа.
func (r *Relay) mcpRestore(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}
	snapshotID, ok := args["snapshot_id"].(string)
	if !ok || snapshotID == "" {
		writeMCPError(w, id, -32602, "snapshot_id: required")
		return
	}

	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	requestID := uuid.New().String()
	msg := protocol.NewMessage(protocol.MsgBackupRestore)
	msg.Payload = protocol.BackupRestorePayload{
		RequestID:  requestID,
		SnapshotID: snapshotID,
	}

	if err := ac.SendMessage(msg); err != nil {
		writeMCPError(w, id, -32603, "failed to send restore request: "+err.Error())
		return
	}

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("✅ Restore request sent\nRequest ID: %s\nSnapshot: %s\nAgent: %s", requestID, snapshotID, ac.ID),
		}},
	})
}

// mcpBackupList — list all relay-side backups.
func (r *Relay) mcpBackupList(w http.ResponseWriter, id any, args map[string]any) {
	if r.backupEngine == nil {
		writeMCPError(w, id, -32603, "backup engine not initialized")
		return
	}

	snapshots := r.backupEngine.List()
	result := make([]map[string]any, 0, len(snapshots))
	for _, s := range snapshots {
		result = append(result, map[string]any{
			"id":          s.ID,
			"description": s.Description,
			"timestamp":   s.Timestamp,
			"size":        s.Size,
			"paths":       s.Paths,
		})
	}

	data, err := json.MarshalIndent(result, "", "  ")
	if err != nil {
		writeMCPError(w, id, -32603, "failed to marshal backups: "+err.Error())
		return
	}

	if len(result) == 0 {
		writeMCPResult(w, id, map[string]any{
			"content": []map[string]any{{
				"type": "text",
				"text": "No relay-side backups found.",
			}},
		})
		return
	}

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("Relay-side backups (%d):\n%s", len(result), string(data)),
		}},
	})
}

// mcpBackupDelete — delete a relay-side backup by snapshot ID.
func (r *Relay) mcpBackupDelete(w http.ResponseWriter, id any, args map[string]any) {
	if r.backupEngine == nil {
		writeMCPError(w, id, -32603, "backup engine not initialized")
		return
	}

	snapshotID, ok := args["snapshot_id"].(string)
	if !ok || snapshotID == "" {
		writeMCPError(w, id, -32602, "snapshot_id: required")
		return
	}

	if err := r.backupEngine.Delete(snapshotID); err != nil {
		writeMCPError(w, id, -32603, "failed to delete backup: "+err.Error())
		return
	}

	data, _ := json.Marshal(map[string]string{"status": "deleted", "snapshot_id": snapshotID})
	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("✅ Backup deleted\n%s", string(data)),
		}},
	})
}

// mcpKill — kill switch (stop/pause/resume).
func (r *Relay) mcpKill(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}
	action, ok := args["action"].(string)
	if !ok || action == "" {
		writeMCPError(w, id, -32602, "action: required")
		return
	}

	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	var pid int
	if p, ok := args["pid"].(float64); ok {
		pid = int(p)
	}

	resp, err := r.sendAndWait(ac, "kill", map[string]any{
		"action": action,
		"pid":    pid,
	}, 10*time.Second)
	if err != nil {
		writeMCPError(w, id, -32603, err.Error())
		return
	}

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("✅ Kill action '%s' executed\nAgent: %s\nResponse: %v", action, ac.ID, resp),
		}},
	})
}

// mcpApprove — одобрить pending request.
func (r *Relay) mcpApprove(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}
	requestID, ok := args["request_id"].(string)
	if !ok || requestID == "" {
		writeMCPError(w, id, -32602, "request_id: required")
		return
	}
	approved := true
	if a, ok := args["approved"].(bool); ok {
		approved = a
	}

	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	_, err = r.sendAndWait(ac, "approve", map[string]any{
		"request_id": requestID,
		"approved":   approved,
	}, 10*time.Second)
	if err != nil {
		writeMCPError(w, id, -32603, err.Error())
		return
	}

	status := "approved"
	if !approved {
		status = "rejected"
	}
	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("✅ Request %s %s\nAgent: %s", requestID, status, ac.ID),
		}},
	})
}

// mcpLogs — tail логов.
func (r *Relay) mcpLogs(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}

	lines := 100
	if l, ok := args["lines"].(float64); ok {
		lines = int(l)
	}
	service, _ := args["service"].(string)

	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	resp, err := r.sendAndWait(ac, "logs", map[string]any{
		"lines":   lines,
		"service": service,
	}, 15*time.Second)
	if err != nil {
		writeMCPError(w, id, -32603, err.Error())
		return
	}

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("📋 Logs from %s (last %d lines):\n\n%v", ac.ID, lines, resp),
		}},
	})
}

// mcpDeploy — перезапустить сервис.
func (r *Relay) mcpDeploy(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}
	service, ok := args["service"].(string)
	if !ok || service == "" {
		writeMCPError(w, id, -32602, "service: required")
		return
	}

	action := "restart"
	if a, ok := args["action"].(string); ok {
		action = a
	}

	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	resp, err := r.sendAndWait(ac, "deploy", map[string]any{
		"service": service,
		"action":  action,
	}, 30*time.Second)
	if err != nil {
		writeMCPError(w, id, -32603, err.Error())
		return
	}

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("✅ Service '%s' %s\nAgent: %s\nResponse: %v", service, action, ac.ID, resp),
		}},
	})
}

// mcpTop — top процессов.
func (r *Relay) mcpTop(w http.ResponseWriter, id any, args map[string]any) {
	agentID, ok := args["agent"].(string)
	if !ok || agentID == "" {
		writeMCPError(w, id, -32602, "agent: required")
		return
	}

	sortBy := "cpu"
	if s, ok := args["sort_by"].(string); ok {
		sortBy = s
	}
	limit := 20
	if l, ok := args["limit"].(float64); ok {
		limit = int(l)
	}

	ac, err := r.resolveAgent(agentID)
	if err != nil {
		writeMCPError(w, id, -32602, err.Error())
		return
	}

	resp, err := r.sendAndWait(ac, "top", map[string]any{
		"sort_by": sortBy,
		"limit":   limit,
	}, 10*time.Second)
	if err != nil {
		writeMCPError(w, id, -32603, err.Error())
		return
	}

	writeMCPResult(w, id, map[string]any{
		"content": []map[string]any{{
			"type": "text",
			"text": fmt.Sprintf("📊 Top processes (sorted by %s, top %d):\n\n%v", sortBy, limit, resp),
		}},
	})
}

// formatExecResult — форматирует результат exec для MCP.
func formatExecResult(resp any) string {
	if m, ok := resp.(map[string]any); ok {
		var sb strings.Builder
		if stdout, ok := m["stdout"].(string); ok && stdout != "" {
			sb.WriteString(stdout)
		}
		if stderr, ok := m["stderr"].(string); ok && stderr != "" {
			if sb.Len() > 0 {
				sb.WriteString("\n")
			}
			sb.WriteString("[stderr] ")
			sb.WriteString(stderr)
		}
		if code, ok := m["exit_code"].(float64); ok && code != 0 {
			sb.WriteString(fmt.Sprintf("\n❌ Exit code: %d", int(code)))
		} else {
			sb.WriteString("\n✅ Exit code: 0")
		}
		if dur, ok := m["duration_ms"].(float64); ok {
			sb.WriteString(fmt.Sprintf(" (%.0fms)", dur))
		}
		return sb.String()
	}
	b, _ := json.MarshalIndent(resp, "", "  ")
	return string(b)
}

// writeMCPResult — отправляет успешный MCP ответ.
func writeMCPResult(w http.ResponseWriter, id any, result any) {
	json.NewEncoder(w).Encode(mcpResponse{
		JSONRPC: "2.0",
		ID:      id,
		Result:  result,
	})
}

// writeMCPError — отправляет MCP ошибку.
func writeMCPError(w http.ResponseWriter, id any, code int, message string) {
	json.NewEncoder(w).Encode(mcpResponse{
		JSONRPC: "2.0",
		ID:      id,
		Error:   &mcpError{Code: code, Message: message},
	})
}

// === AgentConn Callback Support ===

// RegisterCallback — регистрирует callback для request ID.
func (ac *AgentConn) RegisterCallback(requestID string, fn func(any)) {
	ac.mu.Lock()
	defer ac.mu.Unlock()
	if ac.callbacks == nil {
		ac.callbacks = make(map[string]func(any))
	}
	ac.callbacks[requestID] = fn
}

// RemoveCallback — удаляет callback.
func (ac *AgentConn) RemoveCallback(requestID string) {
	ac.mu.Lock()
	defer ac.mu.Unlock()
	delete(ac.callbacks, requestID)
}

// TriggerCallback — вызывает callback по request ID.
func (ac *AgentConn) TriggerCallback(requestID string, data any) bool {
	ac.mu.Lock()
	fn, ok := ac.callbacks[requestID]
	if ok {
		delete(ac.callbacks, requestID)
		ac.mu.Unlock()
		fn(data)
		return true
	}
	ac.mu.Unlock()
	return false
}

// newMessage — создаёт protocol message.
func newMessage(msgType string) protocol.Message {
	return protocol.Message{
		ID:        uuid.New().String()[:8],
		Type:      protocol.MessageType(msgType),
		Timestamp: time.Now().Unix(),
	}
}

package tgbot

import (
	"encoding/json"
	"fmt"
	"strings"
)

// handleCommand — маршрутизирует команды.
func (b *Bot) handleCommand(msg *tgMessage) {
	text := strings.TrimSpace(msg.Text)
	if !strings.HasPrefix(text, "/") {
		return
	}

	// Парсим команду: /cmd@botname args → cmd, args
	parts := strings.Fields(text)
	cmd := strings.TrimLeft(parts[0], "/")
	if idx := strings.Index(cmd, "@"); idx >= 0 {
		cmd = cmd[:idx]
	}
	args := strings.Join(parts[1:], " ")

	chatID := msg.Chat.ID

	b.logger.Info("команда", "cmd", cmd, "user", msg.From.Username, "chat", chatID)

	switch cmd {
	case "start":
		b.handleStart(chatID, msg.From)
	case "help":
		b.handleHelp(chatID)
	case "status":
		b.handleStatus(chatID)
	case "servers":
		b.handleServers(chatID)
	case "exec":
		b.handleExec(chatID, args)
	case "logs":
		b.handleLogs(chatID)
	case "backups":
		b.handleBackups(chatID)
	case "restore":
		b.handleRestore(chatID, args)
	case "emergency":
		b.handleEmergency(chatID)
	case "pause":
		b.handlePause(chatID)
	case "resume":
		b.handleResume(chatID)
	case "approve":
		b.handleApprove(chatID, args)
	case "reject":
		b.handleReject(chatID, args)
	case "settings":
		b.handleSettings(chatID)
	default:
		b.sendMessage(chatID, fmt.Sprintf("❓ Неизвестная команда: /%s\n\nИспользуйте /help для списка команд.", cmd))
	}
}

// handleCallback — обрабатывает нажатия inline-кнопок.
func (b *Bot) handleCallback(cb *tgCallback) {
	if !b.isAllowed(cb.From.ID) {
		b.answerCallback(cb.ID, "⛔ Доступ запрещён.")
		return
	}

	chatID := cb.Message.Chat.ID
	data := cb.Data

	switch {
	case strings.HasPrefix(data, "exec_confirm:"):
		b.confirmed[chatID] = true
		b.answerCallback(cb.ID, "✅ Подтверждено. Отправьте команду снова.")
	case strings.HasPrefix(data, "exec_cancel:"):
		b.confirmed[chatID] = false
		b.answerCallback(cb.ID, "❌ Отменено.")
	case strings.HasPrefix(data, "approve:"):
		requestID := strings.TrimPrefix(data, "approve:")
		b.handleApprove(chatID, requestID)
		b.answerCallback(cb.ID, "✅ Команда подтверждена")
	case strings.HasPrefix(data, "reject:"):
		requestID := strings.TrimPrefix(data, "reject:")
		b.handleReject(chatID, requestID)
		b.answerCallback(cb.ID, "❌ Команда отклонена")
	default:
		b.answerCallback(cb.ID, "Неизвестное действие")
	}
}

// === Обработчики команд ===

// handleStart — приветствие и привязка клиента.
func (b *Bot) handleStart(chatID int64, user *tgUser) {
	name := user.FirstName
	if user.LastName != "" {
		name += " " + user.LastName
	}

	text := fmt.Sprintf(
		"👋 Привет, *%s*!\n\n"+
			"Я — бот управления **flowlink**. Через меня можно:\n"+
			"• Просматривать статус серверов\n"+
			"• Выполнять команды на агентах\n"+
			"• Управлять бэкапами\n"+
			"• Следить за логами\n"+
			"• Экстренно останавливать всё\n\n"+
			"Используйте /help для списка команд.",
		name,
	)
	b.sendMessage(chatID, text)
}

// handleHelp — список всех команд.
func (b *Bot) handleHelp(chatID int64) {
	text := `*📖 Список команд flowlink*

*Серверы:*
/status — статус всех серверов (CPU, RAM, disk)
/servers — список подключённых агентов

*Команды:*
/exec \<сервер\> \<команда\> — выполнить команду
/logs — последние 10 действий

*Бэкапы:*
/backups — список бэкапов
/restore \<snapshot\_id\> — восстановить

*Управление:*
/emergency — 🔴 экстренная остановка всех серверов
/pause — пауза \(read-only\)
/resume — продолжить работу

*Подтверждения:*
/approve \<request\_id\> — подтвердить опасную команду
/reject \<request\_id\> — отклонить
/settings — настройки`

	b.sendMessage(chatID, text)
}

// handleStatus — статус всех серверов.
func (b *Bot) handleStatus(chatID int64) {
	data, err := b.relayGet("/api/v1/agents")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка получения статуса: %v", err))
		return
	}

	// Список агентов
	type agentInfo struct {
		ID        string `json:"id"`
		Hostname  string `json:"hostname"`
		OS        string `json:"os"`
		Arch      string `json:"arch"`
		Version   string `json:"version"`
		Connected string `json:"connected_at"`
		LastSeen  string `json:"last_seen"`
	}

	var agents []agentInfo
	if err := json.Unmarshal(data, &agents); err != nil {
		// Возможно ответ в другом формате (wrapped)
		var wrapped struct {
			Agents []agentInfo `json:"agents"`
		}
		if err2 := json.Unmarshal(data, &wrapped); err2 != nil {
			b.sendMessage(chatID, fmt.Sprintf("❌ Не удалось разобрать ответ реле: %v", err))
			return
		}
		agents = wrapped.Agents
	}

	if len(agents) == 0 {
		b.sendMessage(chatID, "📭 Нет подключённых серверов.")
		return
	}

	// Запрашиваем sysinfo для каждого
	var sb strings.Builder
	sb.WriteString("*📊 Статус серверов*\n\n")

	for _, a := range agents {
		sb.WriteString(fmt.Sprintf("*%s* (`%s`)\n", a.Hostname, a.ID[:8]))
		sb.WriteString(fmt.Sprintf("  OS: %s/%s | v%s\n", a.OS, a.Arch, a.Version))
		sb.WriteString(fmt.Sprintf("  Последний контакт: %s\n\n", a.LastSeen))

		// Пытаемся получить sysinfo
		sysData, sysErr := b.relayGet(fmt.Sprintf("/api/v1/agents/sysinfo?agent_id=%s", a.ID))
		if sysErr == nil {
			var sysInfo struct {
				CPUUsage    float64 `json:"cpu_usage"`
				MemoryUsed  uint64  `json:"memory_used"`
				MemoryTotal uint64  `json:"memory_total"`
				DiskUsed    uint64  `json:"disk_used"`
				DiskTotal   uint64  `json:"disk_total"`
			}
			if json.Unmarshal(sysData, &sysInfo) == nil {
				sb.WriteString(fmt.Sprintf("  🖥 CPU: %.1f%%\n", sysInfo.CPUUsage))
				if sysInfo.MemoryTotal > 0 {
					memPct := float64(sysInfo.MemoryUsed) / float64(sysInfo.MemoryTotal) * 100
					sb.WriteString(fmt.Sprintf("  💾 RAM: %s / %s (%.1f%%)\n",
						formatBytes(sysInfo.MemoryUsed), formatBytes(sysInfo.MemoryTotal), memPct))
				}
				if sysInfo.DiskTotal > 0 {
					diskPct := float64(sysInfo.DiskUsed) / float64(sysInfo.DiskTotal) * 100
					sb.WriteString(fmt.Sprintf("  💿 Disk: %s / %s (%.1f%%)\n",
						formatBytes(sysInfo.DiskUsed), formatBytes(sysInfo.DiskTotal), diskPct))
				}
			}
		}
		sb.WriteString("\n")
	}

	b.sendMessage(chatID, sb.String())
}

// handleServers — список подключённых агентов.
func (b *Bot) handleServers(chatID int64) {
	data, err := b.relayGet("/api/v1/agents")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	type agentInfo struct {
		ID        string `json:"id"`
		Hostname  string `json:"hostname"`
		OS        string `json:"os"`
		Arch      string `json:"arch"`
		Version   string `json:"version"`
		Connected string `json:"connected_at"`
		LastSeen  string `json:"last_seen"`
	}

	var agents []agentInfo
	if err := json.Unmarshal(data, &agents); err != nil {
		var wrapped struct {
			Agents []agentInfo `json:"agents"`
		}
		if json.Unmarshal(data, &wrapped) == nil {
			agents = wrapped.Agents
		}
	}

	if len(agents) == 0 {
		b.sendMessage(chatID, "📭 Нет подключённых агентов.")
		return
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("*🖥 Серверы (%d)*\n\n", len(agents)))

	for i, a := range agents {
		sb.WriteString(fmt.Sprintf("%d. *%s*\n", i+1, a.Hostname))
		sb.WriteString(fmt.Sprintf("   ID: `%s`\n", a.ID))
		sb.WriteString(fmt.Sprintf("   %s/%s | v%s\n", a.OS, a.Arch, a.Version))
		sb.WriteString(fmt.Sprintf("   Подключён: %s\n\n", a.Connected))
	}

	b.sendMessage(chatID, sb.String())
}

// handleExec — выполнение команды на сервере (с подтверждением).
func (b *Bot) handleExec(chatID int64, args string) {
	parts := strings.SplitN(args, " ", 2)
	if len(parts) < 2 {
		b.sendMessage(chatID, "⚠ Использование: /exec `<сервер>` `<команда>`\n\nПример:\n/exec server-1 `ls -la`")
		return
	}

	serverName := strings.Trim(parts[0], "` ")
	command := strings.TrimSpace(parts[1])

	// Проверяем подтверждение
	if !b.confirmed[chatID] {
		kb := &tgInlineKeyboard{
			InlineKeyboard: [][]tgButton{
				{
					{Text: "✅ Выполнить", CallbackData: "exec_confirm:"},
					{Text: "❌ Отмена", CallbackData: "exec_cancel:"},
				},
			},
		}
		b.sendMessageWithKeyboard(chatID,
			fmt.Sprintf("⚠ *Подтвердите выполнение*\n\nСервер: `%s`\nКоманда: `%s`\n\nНажмите ✅ для подтверждения.", serverName, command),
			kb,
		)
		return
	}

	// Сбрасываем подтверждение
	b.confirmed[chatID] = false

	// Отправляем команду на реле
	b.sendMessage(chatID, fmt.Sprintf("⏳ Выполняю на `%s`: `%s`...", serverName, command))

	payload := map[string]any{
		"agent_id": serverName,
		"command":  command,
	}

	output, err := b.relayStreamPost("/api/v1/agents/exec", payload, 4096)
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка выполнения: %v", err))
		return
	}

	// Обрезаем длинный вывод
	if len(output) > 3000 {
		output = output[:3000] + "\n... (обрезано)"
	}

	b.sendMessage(chatID, fmt.Sprintf("✅ *Результат*\n```\n%s\n```", output))
}

// handleLogs — последние 10 записей из audit log.
func (b *Bot) handleLogs(chatID int64) {
	payload := map[string]any{
		"limit": 10,
	}

	data, err := b.relayPost("/api/v1/audit", payload)
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка получения логов: %v", err))
		return
	}

	type auditEntry struct {
		ID        string `json:"id"`
		Timestamp string `json:"timestamp"`
		AgentID   string `json:"agent_id"`
		Action    string `json:"action"`
		Command   string `json:"command,omitempty"`
		RiskLevel string `json:"risk_level"`
		Result    string `json:"result"`
		DurationMs int64  `json:"duration_ms"`
	}

	var entries []auditEntry
	if err := json.Unmarshal(data, &entries); err != nil {
		var wrapped struct {
			Entries []auditEntry `json:"entries"`
		}
		if json.Unmarshal(data, &wrapped) == nil {
			entries = wrapped.Entries
		}
	}

	if len(entries) == 0 {
		b.sendMessage(chatID, "📭 Audit log пуст.")
		return
	}

	var sb strings.Builder
	sb.WriteString("*📋 Последние действия*\n\n")

	for _, e := range entries {
		riskEmoji := "🟢"
		switch e.RiskLevel {
		case "medium":
			riskEmoji = "🟡"
		case "high":
			riskEmoji = "🔴"
		}

		resultEmoji := "✅"
		if e.Result != "success" {
			resultEmoji = "❌"
		}

		shortID := e.ID
		if len(shortID) > 8 {
			shortID = shortID[:8]
		}

		sb.WriteString(fmt.Sprintf("%s %s %s `%s` — %s", riskEmoji, resultEmoji, e.Action, shortID, e.AgentID))
		if e.Command != "" {
			cmd := e.Command
			if len(cmd) > 50 {
				cmd = cmd[:50] + "..."
			}
			sb.WriteString(fmt.Sprintf(" `%s`", cmd))
		}
		sb.WriteString(fmt.Sprintf(" (%dms)\n", e.DurationMs))
	}

	b.sendMessage(chatID, sb.String())
}

// handleBackups — список бэкапов.
func (b *Bot) handleBackups(chatID int64) {
	// Проверяем есть ли endpoint для бэкапов в реле
	data, err := b.relayGet("/api/v1/backups")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("ℹ Бэкапы недоступны через API реле. Используйте CLI агента.\n\nОшибка: %v", err))
		return
	}

	b.sendMessage(chatID, fmt.Sprintf("📦 *Бэкапы*\n\n```json\n%s\n```", string(data)))
}

// handleRestore — восстановление бэкапа.
func (b *Bot) handleRestore(chatID int64, snapshotID string) {
	snapshotID = strings.TrimSpace(snapshotID)
	if snapshotID == "" {
		b.sendMessage(chatID, "⚠ Использование: /restore `<snapshot_id>`")
		return
	}

	kb := &tgInlineKeyboard{
		InlineKeyboard: [][]tgButton{
			{
				{Text: "✅ Подтвердить восстановление", CallbackData: "approve:" + snapshotID},
				{Text: "❌ Отмена", CallbackData: "reject:" + snapshotID},
			},
		},
	}

	b.sendMessageWithKeyboard(chatID,
		fmt.Sprintf("⚠ *Подтвердите восстановление*\n\nSnapshot: `%s`\n\nВнимание: это перезапишет текущее состояние!", snapshotID),
		kb,
	)
}

// handleEmergency — экстренная остановка всех серверов.
func (b *Bot) handleEmergency(chatID int64) {
	kb := &tgInlineKeyboard{
		InlineKeyboard: [][]tgButton{
			{
				{Text: "🔴 STOP ALL", CallbackData: "approve:emergency-stop"},
				{Text: "❌ Отмена", CallbackData: "reject:emergency-stop"},
			},
		},
	}

	b.sendMessageWithKeyboard(chatID,
		"🚨 *ЭКСТРЕННАЯ ОСТАНОВКА*\n\nВсе серверы будут немедленно остановлены.\nЭто действие необратимо!\n\nНажмите 🔴 для подтверждения.",
		kb,
	)
}

// handlePause — пауза (read-only режим).
func (b *Bot) handlePause(chatID int64) {
	data, err := b.relayPost("/api/v1/agents/pause", nil)
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}
	b.sendMessage(chatID, fmt.Sprintf("⏸ *Пауза включена*\nВсе агенты перешли в read-only режим.\n\n%s", string(data)))
}

// handleResume — продолжить работу.
func (b *Bot) handleResume(chatID int64) {
	_, err := b.relayPost("/api/v1/agents/resume", nil)
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}
	b.sendMessage(chatID, "▶ *Работа возобновлена*\nАгенты снова активны.")
}

// handleApprove — подтверждение опасной команды.
func (b *Bot) handleApprove(chatID int64, requestID string) {
	requestID = strings.TrimSpace(requestID)
	if requestID == "" {
		b.sendMessage(chatID, "⚠ Использование: /approve `<request_id>`")
		return
	}

	payload := map[string]any{
		"request_id": requestID,
		"decision":   "approve",
	}

	data, err := b.relayPost("/api/v1/approval/decide", payload)
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка подтверждения: %v", err))
		return
	}

	b.sendMessage(chatID, fmt.Sprintf("✅ Запрос `%s` подтверждён.\n\n%s", requestID[:8], string(data)))
}

// handleReject — отклонение опасной команды.
func (b *Bot) handleReject(chatID int64, requestID string) {
	requestID = strings.TrimSpace(requestID)
	if requestID == "" {
		b.sendMessage(chatID, "⚠ Использование: /reject `<request_id>`")
		return
	}

	payload := map[string]any{
		"request_id": requestID,
		"decision":   "reject",
	}

	_, err := b.relayPost("/api/v1/approval/decide", payload)
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка отклонения: %v", err))
		return
	}

	b.sendMessage(chatID, fmt.Sprintf("❌ Запрос `%s` отклонён.", requestID[:8]))
}

// handleSettings — отображение настроек.
func (b *Bot) handleSettings(chatID int64) {
	var sb strings.Builder
	sb.WriteString("*⚙ Настройки*\n\n")
	sb.WriteString(fmt.Sprintf("*Approval Mode:* не настроен через API\n"))
	sb.WriteString(fmt.Sprintf("*Уведомления:* %v\n", b.cfg.NotifyOn))
	sb.WriteString(fmt.Sprintf("*Доступ:* %d пользователей\n", len(b.cfg.AllowedIDs)))

	b.sendMessage(chatID, sb.String())
}

// === Утилиты ===

// formatBytes — форматирует байты в человекочитаемый вид.
func formatBytes(b uint64) string {
	const (
		KB = 1024
		MB = KB * 1024
		GB = MB * 1024
	)
	switch {
	case b >= GB:
		return fmt.Sprintf("%.1f GB", float64(b)/float64(GB))
	case b >= MB:
		return fmt.Sprintf("%.1f MB", float64(b)/float64(MB))
	case b >= KB:
		return fmt.Sprintf("%.1f KB", float64(b)/float64(KB))
	default:
		return fmt.Sprintf("%d B", b)
	}
}

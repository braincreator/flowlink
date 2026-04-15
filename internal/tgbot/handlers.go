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

	b.logger.Info("command received", "cmd", cmd, "user", msg.From.Username, "chat", chatID)

	switch cmd {
	// === Управление серверами ===
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

	// === Бэкапы ===
	case "backups":
		b.handleBackups(chatID)
	case "restore":
		b.handleRestore(chatID, args)

	// === Управление агентами ===
	case "emergency":
		b.handleEmergency(chatID)
	case "pause":
		b.handlePause(chatID)
	case "resume":
		b.handleResume(chatID)
	case "readonly":
		b.handleReadonly(chatID, args)
	case "policy":
		b.handlePolicy(chatID)

	// === Подтверждения ===
	case "approve":
		b.handleApprove(chatID, args)
	case "reject":
		b.handleReject(chatID, args)
	case "settings":
		b.handleSettings(chatID)

	// === Устройства ===
	case "devices":
		b.handleDevices(chatID)
	case "approve_device":
		b.handleApproveDevice(chatID, args)
	case "reject_device":
		b.handleRejectDevice(chatID, args)
	case "revoke":
		b.handleRevoke(chatID, args)
	case "device_info":
		b.handleDeviceInfo(chatID, args)

	// === E2EE ===
	case "keys":
		b.handleKeys(chatID)
	case "rotate":
		b.handleRotateKeys(chatID)

	// === Биллинг ===
	case "plans":
		b.handlePlans(chatID)
	case "billing":
		b.handleBilling(chatID)
	case "subscribe":
		b.handleSubscribe(chatID, args)
	case "myplan":
		b.handleMyPlan(chatID)
	case "invoices":
		b.handleInvoices(chatID)
	case "usage":
		b.handleUsage(chatID)
	case "payments":
		b.handlePaymentMethods(chatID)

	// === Безопасность ===
	case "shield":
		b.handleShieldAlerts(chatID)

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
	case strings.HasPrefix(data, "pairing_approve:"):
		code := strings.TrimPrefix(data, "pairing_approve:")
		b.handleApproveDevice(chatID, code)
		b.answerCallback(cb.ID, "✅ Устройство одобрено")
	case strings.HasPrefix(data, "pairing_reject:"):
		code := strings.TrimPrefix(data, "pairing_reject:")
		b.handleRejectDevice(chatID, code)
		b.answerCallback(cb.ID, "❌ Устройство отклонено")
	case strings.HasPrefix(data, "subscribe:"):
		planID := strings.TrimPrefix(data, "subscribe:")
		b.doSubscribe(chatID, planID)
	case strings.HasPrefix(data, "confirm_cancel_sub:"):
		subID := strings.TrimPrefix(data, "confirm_cancel_sub:")
		b.doCancelSubscription(chatID, subID)
	default:
		b.answerCallback(cb.ID, "Неизвестное действие")
	}
}

// ═══════════════════════════════════════════════
// Основные команды
// ═══════════════════════════════════════════════

// handleStart — приветствие.
func (b *Bot) handleStart(chatID int64, user *tgUser) {
	name := user.FirstName
	if user.LastName != "" {
		name += " " + user.LastName
	}

	text := fmt.Sprintf(
		"👋 Привет, *%s*!\n\n"+
			"Я — бот управления **flowlink**. Через меня можно:\n"+
			"• 📊 Просматривать статус серверов\n"+
			"• ⌨️ Выполнять команды на агентах\n"+
			"• 💾 Управлять бэкапами\n"+
			"• 🛡 Следить за безопасностью\n"+
			"• 💳 Управлять подпиской и биллингом\n\n"+
			"Используйте /help для списка команд.",
		name,
	)
	b.sendMessage(chatID, text)
}

// handleHelp — список всех команд.
func (b *Bot) handleHelp(chatID int64) {
	text := `*📖 Справка flowlink*

*🖥 Серверы:*
/status — статус всех серверов
/servers — список подключённых агентов
/exec \<сервер\> \<команда\> — выполнить команду

*📋 Аудит и логи:*
/logs — последние действия (audit)
/approvals — ожидающие подтверждения
/shield — оповещения безопасности

*💾 Бэкапы:*
/backups — список бэкапов
/restore \<snapshot\_id\> — восстановить

*⚙️ Управление:*
/emergency — 🔴 экстренная остановка
/pause — пауза (read-only)
/resume — продолжить работу
/readonly on|off — переключить режим
/policy — статус Policy Layer

*📱 Устройства:*
/devices — список устройств
/approve\_device \<код\> — одобрить
/revoke \<id\> — отозвать доступ

*🔑 E2EE:*
/keys — ключи шифрования
/rotate — ротация ключей

*💳 Биллинг:*
/plans — доступные тарифы
/billing — статус подписки
/myplan — текущий план
/subscribe \<plan\_id\> — подписаться
/invoices — история платежей
/usage — статистика использования
/payments — способы оплаты`

	b.sendMessage(chatID, text)
}

// ═══════════════════════════════════════════════
// Серверы и агенты
// ═══════════════════════════════════════════════

// handleStatus — статус всех серверов (GET /api/agents).
func (b *Bot) handleStatus(chatID int64) {
	data, err := b.relayGet("/api/agents")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка получения статуса: %v", err))
		return
	}

	var agents []struct {
		ID        string `json:"id"`
		Hostname  string `json:"hostname"`
		OS        string `json:"os"`
		Arch      string `json:"arch"`
		Version   string `json:"version"`
		Connected string `json:"connected_at"`
		LastSeen  string `json:"last_seen"`
	}

	if err := json.Unmarshal(data, &agents); err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Не удалось разобрать ответ: %v\n\n%s", err, truncate(string(data), 500)))
		return
	}

	if len(agents) == 0 {
		b.sendMessage(chatID, "📭 Нет подключённых серверов.")
		return
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("*📊 Статус серверов (%d)*\n\n", len(agents)))

	for _, a := range agents {
		shortID := a.ID
		if len(shortID) > 12 {
			shortID = shortID[:12]
		}
		sb.WriteString(fmt.Sprintf("*%s* (`%s`)\n", a.Hostname, shortID))
		sb.WriteString(fmt.Sprintf("  OS: %s/%s | v%s\n", a.OS, a.Arch, a.Version))
		sb.WriteString(fmt.Sprintf("  Последний контакт: %s\n\n", a.LastSeen))
	}

	b.sendMessage(chatID, sb.String())
}

// handleServers — список подключённых агентов (GET /api/agents).
func (b *Bot) handleServers(chatID int64) {
	data, err := b.relayGet("/api/agents")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	var agents []struct {
		ID        string `json:"id"`
		Hostname  string `json:"hostname"`
		OS        string `json:"os"`
		Arch      string `json:"arch"`
		Version   string `json:"version"`
		Connected string `json:"connected_at"`
		LastSeen  string `json:"last_seen"`
	}

	if err := json.Unmarshal(data, &agents); err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка парсинга: %v", err))
		return
	}

	if len(agents) == 0 {
		b.sendMessage(chatID, "📭 Нет подключённых агентов.")
		return
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("*🖥 Серверы (%d)*\n\n", len(agents)))

	for i, a := range agents {
		shortID := a.ID
		if len(shortID) > 12 {
			shortID = shortID[:12]
		}
		sb.WriteString(fmt.Sprintf("%d. *%s*\n", i+1, a.Hostname))
		sb.WriteString(fmt.Sprintf("   ID: `%s`\n", shortID))
		sb.WriteString(fmt.Sprintf("   %s/%s | v%s\n", a.OS, a.Arch, a.Version))
		sb.WriteString(fmt.Sprintf("   Подключён: %s\n\n", a.Connected))
	}

	b.sendMessage(chatID, sb.String())
}

// handleExec — выполнение команды на сервере (POST /api/exec/{agent_id}).
func (b *Bot) handleExec(chatID int64, args string) {
	parts := strings.SplitN(args, " ", 2)
	if len(parts) < 2 {
		b.sendMessage(chatID, "⚠ Использование: /exec `<сервер>` `<команда>`\n\nПример:\n/exec server-1 `ls -la`")
		return
	}

	agentID := strings.Trim(parts[0], "` ")
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
			fmt.Sprintf("⚠ *Подтвердите выполнение*\n\nСервер: `%s`\nКоманда: `%s`\n\nНажмите ✅ для подтверждения.", agentID, command),
			kb,
		)
		return
	}

	// Сбрасываем подтверждение
	b.confirmed[chatID] = false

	b.sendMessage(chatID, fmt.Sprintf("⏳ Выполняю на `%s`: `%s`...", agentID, command))

	payload := map[string]any{
		"command": command,
	}

	output, err := b.relayStreamPost("/api/exec/"+agentID, payload, 4096)
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка выполнения: %v", err))
		return
	}

	if len(output) > 3000 {
		output = output[:3000] + "\n... (обрезано)"
	}

	b.sendMessage(chatID, fmt.Sprintf("✅ *Результат*\n```\n%s\n```", output))
}

// handleLogs — последние записи из audit log (GET /api/audit).
func (b *Bot) handleLogs(chatID int64) {
	data, err := b.relayGet("/api/audit?limit=10")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка получения логов: %v", err))
		return
	}

	// Пытаемся распарсить как массив
	var entries []json.RawMessage
	if err := json.Unmarshal(data, &entries); err != nil {
		// Возможно wrapped в объект
		var wrapped struct {
			Entries []json.RawMessage `json:"entries"`
			Events  []json.RawMessage `json:"events"`
		}
		if json.Unmarshal(data, &wrapped) == nil {
			entries = wrapped.Entries
			if len(entries) == 0 {
				entries = wrapped.Events
			}
		}
	}

	if len(entries) == 0 {
		b.sendMessage(chatID, "📭 Audit log пуст.")
		return
	}

	var sb strings.Builder
	sb.WriteString("*📋 Последние действия*\n\n")

	for _, raw := range entries {
		var e struct {
			ID        string `json:"id"`
			Timestamp string `json:"timestamp"`
			CreatedAt string `json:"created_at"`
			AgentID   string `json:"agent_id"`
			Action    string `json:"action"`
			Command   string `json:"command"`
			RiskLevel string `json:"risk_level"`
			Result    string `json:"result"`
			Status    string `json:"status"`
			DurationMs int64  `json:"duration_ms"`
		}
		if err := json.Unmarshal(raw, &e); err != nil {
			continue
		}

		resultEmoji := "✅"
		if e.Result == "failed" || e.Status == "error" {
			resultEmoji = "❌"
		}

		shortID := e.ID
		if len(shortID) > 8 {
			shortID = shortID[:8]
		}

		ts := e.Timestamp
		if ts == "" {
			ts = e.CreatedAt
		}

		sb.WriteString(fmt.Sprintf("%s %s `%s`", resultEmoji, e.Action, shortID))
		if e.Command != "" {
			cmd := e.Command
			if len(cmd) > 40 {
				cmd = cmd[:40] + "..."
			}
			sb.WriteString(fmt.Sprintf(" `%s`", cmd))
		}
		if ts != "" {
			sb.WriteString(fmt.Sprintf(" — %s", ts[:19]))
		}
		sb.WriteString("\n")
	}

	b.sendMessage(chatID, sb.String())
}

// ═══════════════════════════════════════════════
// Бэкапы
// ═══════════════════════════════════════════════

// handleBackups — список бэкапов.
func (b *Bot) handleBackups(chatID int64) {
	data, err := b.relayGet("/api/audit?limit=20&action=backup")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("ℹ Бэкапы: %v", err))
		return
	}

	b.sendMessage(chatID, fmt.Sprintf("📦 *Бэкапы*\n\n```\n%s\n```", truncate(string(data), 3000)))
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

// ═══════════════════════════════════════════════
// Управление агентами
// ═══════════════════════════════════════════════

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
	data, err := b.relayPost("/api/config/reload", map[string]any{"read_only": true})
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}
	b.sendMessage(chatID, fmt.Sprintf("⏸ *Пауза включена*\nВсе агенты перешли в read-only режим.\n\n%s", string(data)))
}

// handleResume — продолжить работу.
func (b *Bot) handleResume(chatID int64) {
	_, err := b.relayPost("/api/config/reload", map[string]any{"read_only": false})
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}
	b.sendMessage(chatID, "▶ *Работа возобновлена*\nАгенты снова активны.")
}

// handleReadonly — переключение read-only режима.
func (b *Bot) handleReadonly(chatID int64, mode string) {
	mode = strings.TrimSpace(strings.ToLower(mode))

	var enable bool
	switch mode {
	case "on", "1", "true":
		enable = true
	case "off", "0", "false":
		enable = false
	default:
		b.sendMessage(chatID, "⚠ Использование: /readonly `on` или /readonly `off`")
		return
	}

	data, err := b.relayPost("/api/config/reload", map[string]any{"read_only": enable})
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	status := "🔒 Read-only"
	if !enable {
		status = "🔓 Read-write"
	}
	b.sendMessage(chatID, fmt.Sprintf("%s режим %s\n\n%s", status, mode, string(data)))
}

// handlePolicy — отображение статуса Policy Layer.
func (b *Bot) handlePolicy(chatID int64) {
	data, err := b.relayGet("/api/shield/stats")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	var sb strings.Builder
	sb.WriteString("🛡 *Policy Layer Status*\n\n")

	var status map[string]any
	if err := json.Unmarshal(data, &status); err == nil {
		for k, v := range status {
			sb.WriteString(fmt.Sprintf("  *%s:* %v\n", k, v))
		}
	} else {
		sb.WriteString(string(data))
	}

	b.sendMessage(chatID, sb.String())
}

// ═══════════════════════════════════════════════
// Подтверждения (GET /api/approvals)
// ═══════════════════════════════════════════════

// handleApprove — подтверждение опасной команды (POST /api/approvals/{id}/approve).
func (b *Bot) handleApprove(chatID int64, requestID string) {
	requestID = strings.TrimSpace(requestID)
	if requestID == "" {
		b.handleApprovalsList(chatID)
		return
	}

	data, err := b.relayPost("/api/approvals/"+requestID+"/approve", nil)
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка подтверждения: %v", err))
		return
	}

	b.sendMessage(chatID, fmt.Sprintf("✅ Запрос `%s` подтверждён.\n\n%s", requestID, string(data)))
}

// handleReject — отклонение опасной команды (POST /api/approvals/{id}/reject).
func (b *Bot) handleReject(chatID int64, requestID string) {
	requestID = strings.TrimSpace(requestID)
	if requestID == "" {
		b.handleApprovalsList(chatID)
		return
	}

	_, err := b.relayPost("/api/approvals/"+requestID+"/reject", nil)
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка отклонения: %v", err))
		return
	}

	b.sendMessage(chatID, fmt.Sprintf("❌ Запрос `%s` отклонён.", requestID))
}

// handleApprovalsList — список ожидающих подтверждений (GET /api/approvals).
func (b *Bot) handleApprovalsList(chatID int64) {
	data, err := b.relayGet("/api/approvals")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	var approvals []struct {
		ID        string `json:"id"`
		AgentID   string `json:"agent_id"`
		Command   string `json:"command"`
		RiskLevel string `json:"risk_level"`
		CreatedAt string `json:"created_at"`
	}

	if err := json.Unmarshal(data, &approvals); err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка парсинга: %v", err))
		return
	}

	if len(approvals) == 0 {
		b.sendMessage(chatID, "✅ Нет ожидающих подтверждений.")
		return
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("*⏳ Ожидают подтверждения (%d)*\n\n", len(approvals)))

	for _, a := range approvals {
		shortID := a.ID
		if len(shortID) > 8 {
			shortID = shortID[:8]
		}
		cmd := a.Command
		if len(cmd) > 40 {
			cmd = cmd[:40] + "..."
		}

		sb.WriteString(fmt.Sprintf("  `%s` — %s\n", shortID, cmd))
		sb.WriteString(fmt.Sprintf("  Агент: `%s` | Риск: %s\n\n", a.AgentID, a.RiskLevel))
	}

	sb.WriteString("💡 Используйте: /approve `<id>` или /reject `<id>`")
	b.sendMessage(chatID, sb.String())
}

// handleSettings — отображение настроек.
func (b *Bot) handleSettings(chatID int64) {
	var sb strings.Builder
	sb.WriteString("*⚙ Настройки*\n\n")
	sb.WriteString(fmt.Sprintf("*Реле:* %s\n", b.relayURL))
	sb.WriteString(fmt.Sprintf("*Уведомления:* %v\n", b.cfg.NotifyOn))
	sb.WriteString(fmt.Sprintf("*Доступ:* %d пользователей\n", len(b.cfg.AllowedIDs)))

	b.sendMessage(chatID, sb.String())
}

// ═══════════════════════════════════════════════
// Устройства (GET /api/devices)
// ═══════════════════════════════════════════════

// handleDevices — список устройств.
func (b *Bot) handleDevices(chatID int64) {
	data, err := b.relayGet("/api/devices")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка получения устройств: %v", err))
		return
	}

	var devices []struct {
		ID       string `json:"id"`
		Name     string `json:"name"`
		Status   string `json:"status"`
		LastSeen string `json:"last_seen_at"`
		OS       string `json:"os"`
	}

	if err := json.Unmarshal(data, &devices); err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка парсинга: %v", err))
		return
	}

	if len(devices) == 0 {
		b.sendMessage(chatID, "📱 *Устройства*\n\n_Нет зарегистрированных устройств._")
		return
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("📱 *Устройства (%d)*\n\n", len(devices)))
	for _, d := range devices {
		status := "⏳"
		switch d.Status {
		case "approved", "active":
			status = "✅"
		case "revoked":
			status = "🔒"
		case "pending":
			status = "🟡"
		}
		shortID := d.ID
		if len(shortID) > 12 {
			shortID = shortID[:12]
		}
		sb.WriteString(fmt.Sprintf("%s *%s* (`%s`)\n", status, d.Name, shortID))
	}

	b.sendMessage(chatID, sb.String())
}

// handleApproveDevice — одобрение устройства.
func (b *Bot) handleApproveDevice(chatID int64, code string) {
	code = strings.TrimSpace(code)
	if code == "" {
		b.sendMessage(chatID, "⚠ Использование: /approve_device `<код>`")
		return
	}
	data, err := b.relayPost("/api/devices/confirm", map[string]any{"code": code})
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}
	b.sendMessage(chatID, fmt.Sprintf("✅ Устройство одобрено.\n\n%s", string(data)))
}

// handleRejectDevice — отклонение устройства.
func (b *Bot) handleRejectDevice(chatID int64, code string) {
	code = strings.TrimSpace(code)
	if code == "" {
		b.sendMessage(chatID, "⚠ Использование: /reject_device `<код>`")
		return
	}
	_, err := b.relayPost("/api/devices/confirm", map[string]any{"code": code, "reject": true})
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}
	b.sendMessage(chatID, fmt.Sprintf("❌ Устройство с кодом `%s` отклонено.", code))
}

// handleRevoke — отзыв доступа устройства (DELETE /api/devices/{id}).
func (b *Bot) handleRevoke(chatID int64, deviceID string) {
	deviceID = strings.TrimSpace(deviceID)
	if deviceID == "" {
		b.sendMessage(chatID, "⚠ Использование: /revoke `<id_устройства>`")
		return
	}
	_, err := b.relayDelete("/api/devices/" + deviceID)
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}
	b.sendMessage(chatID, fmt.Sprintf("🔒 Доступ устройства `%s` отозван.", deviceID))
}

// handleDeviceInfo — подробная информация об устройстве.
func (b *Bot) handleDeviceInfo(chatID int64, deviceID string) {
	deviceID = strings.TrimSpace(deviceID)
	if deviceID == "" {
		b.sendMessage(chatID, "⚠ Использование: /device_info `<id_устройства>`")
		return
	}
	data, err := b.relayGet("/api/devices/" + deviceID)
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	var d struct {
		ID        string `json:"id"`
		Name      string `json:"name"`
		Status    string `json:"status"`
		LastSeen  string `json:"last_seen_at"`
		OS        string `json:"os"`
		CreatedAt string `json:"created_at"`
	}
	if err := json.Unmarshal(data, &d); err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка парсинга: %v", err))
		return
	}

	shortID := d.ID
	if len(shortID) > 12 {
		shortID = shortID[:12]
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("📱 *Устройство* `%s`\n\n", d.Name))
	sb.WriteString(fmt.Sprintf("🆔 ID: `%s`\n", shortID))
	sb.WriteString(fmt.Sprintf("📊 Статус: %s\n", d.Status))
	if d.OS != "" {
		sb.WriteString(fmt.Sprintf("💻 ОС: %s\n", d.OS))
	}
	if d.LastSeen != "" {
		sb.WriteString(fmt.Sprintf("👀 Последняя активность: %s\n", d.LastSeen))
	}
	if d.CreatedAt != "" {
		sb.WriteString(fmt.Sprintf("📅 Создано: %s", d.CreatedAt))
	}
	b.sendMessage(chatID, sb.String())
}

// ═══════════════════════════════════════════════
// E2EE ключи
// ═══════════════════════════════════════════════

// handleKeys — показывает информацию о ключах E2EE.
func (b *Bot) handleKeys(chatID int64) {
	data, err := b.relayGet("/api/config")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	var config map[string]any
	if err := json.Unmarshal(data, &config); err == nil {
		if keys, ok := config["keys"]; ok {
			b.sendMessage(chatID, fmt.Sprintf("🔑 *Ключи E2EE*\n\n```json\n%s\n```", truncate(string(mustMarshal(keys)), 2000)))
			return
		}
	}

	b.sendMessage(chatID, "🔑 *Ключи E2EE*\n\n_Информация недоступна через текущий API._")
}

// handleRotateKeys — ротация ключей E2EE.
func (b *Bot) handleRotateKeys(chatID int64) {
	data, err := b.relayPost("/api/config/reload", nil)
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}
	b.sendMessage(chatID, fmt.Sprintf("🔄 *Конфигурация перезагружена*\n\n%s", string(data)))
}

// ═══════════════════════════════════════════════
// Биллинг (через relay API)
// ═══════════════════════════════════════════════

// handlePlans — показывает доступные тарифы (GET /api/plans).
func (b *Bot) handlePlans(chatID int64) {
	data, err := b.relayGet("/api/plans")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	var plans []struct {
		ID          string `json:"id"`
		Name        string `json:"name"`
		Description string `json:"description"`
		Tier        int    `json:"tier"`
		PriceKopecks uint64 `json:"price_kopecks"`
		TrialDays   *uint16 `json:"trial_days"`
		Features    []string `json:"features"`
		Limits      json.RawMessage `json:"limits"`
	}

	if err := json.Unmarshal(data, &plans); err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка парсинга: %v", err))
		return
	}

	var sb strings.Builder
	sb.WriteString("📋 *Тарифные планы FlowLink*\n\n")

	for _, p := range plans {
		sb.WriteString(fmt.Sprintf("📦 *%s*\n", p.Name))
		if p.PriceKopecks == 0 {
			sb.WriteString("   💰 Бесплатно")
			if p.TrialDays != nil {
				sb.WriteString(fmt.Sprintf(" (%d дней)", *p.TrialDays))
			}
			sb.WriteString("\n")
		} else {
			sb.WriteString(fmt.Sprintf("   💰 *%s ₽*/мес\n", formatKopecks(p.PriceKopecks)))
		}

		if p.Description != "" {
			sb.WriteString(fmt.Sprintf("   📝 %s\n", p.Description))
		}

		if len(p.Features) > 0 {
			maxF := 4
			if len(p.Features) < maxF {
				maxF = len(p.Features)
			}
			for _, f := range p.Features[:maxF] {
				sb.WriteString(fmt.Sprintf("   ✅ %s\n", f))
			}
			if len(p.Features) > maxF {
				sb.WriteString(fmt.Sprintf("   ...и ещё %d\n", len(p.Features)-maxF))
			}
		}

		sb.WriteString("\n")
	}

	sb.WriteString("💡 *Подписаться:* /subscribe `<plan_id>`")
	b.sendMessage(chatID, sb.String())
}

// handleBilling — статус подписки (GET /api/billing).
func (b *Bot) handleBilling(chatID int64) {
	data, err := b.relayGet("/api/billing")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	var info struct {
		PlanID          string          `json:"plan_id"`
		PlanName        string          `json:"plan_name"`
		Active          bool            `json:"active"`
		BalanceRub      string          `json:"balance_rub"`
		ExpiresAt       *string         `json:"expires_at"`
		AvailablePlans  []any           `json:"available_plans"`
	}

	if err := json.Unmarshal(data, &info); err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка парсинга: %v", err))
		return
	}

	statusEmoji := "✅"
	if !info.Active {
		statusEmoji = "❌"
	}

	msg := fmt.Sprintf(
		"💳 *Биллинг*\n\n"+
			"%s Статус: *%s*\n"+
			"📦 Текущий план: *%s*\n"+
			"💰 Баланс: *%s*\n",
		statusEmoji,
		boolStr(info.Active, "Активна", "Неактивна"),
		info.PlanName,
		info.BalanceRub,
	)

	if info.ExpiresAt != nil && *info.ExpiresAt != "" {
		msg += fmt.Sprintf("📅 Истекает: *%s*\n", *info.ExpiresAt)
	}

	msg += fmt.Sprintf("\n📋 Доступные планы: %d\n", len(info.AvailablePlans))
	msg += "\n💡 /plans — все тарифы | /subscribe — подписаться"
	b.sendMessage(chatID, msg)
}

// handleMyPlan — подробности текущего плана (GET /api/billing).
func (b *Bot) handleMyPlan(chatID int64) {
	data, err := b.relayGet("/api/billing")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	var info struct {
		PlanID     string          `json:"plan_id"`
		PlanName   string          `json:"plan_name"`
		Active     bool            `json:"active"`
		BalanceRub string          `json:"balance_rub"`
		Limits     json.RawMessage `json:"limits"`
		Usage      json.RawMessage `json:"usage"`
	}

	if err := json.Unmarshal(data, &info); err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка парсинга: %v", err))
		return
	}

	msg := fmt.Sprintf("📦 *Текущий план: %s*\n\n", info.PlanName)

	// Лимиты
	if len(info.Limits) > 0 && string(info.Limits) != "null" {
		var limits map[string]any
		if json.Unmarshal(info.Limits, &limits) == nil {
			msg += "📏 *Лимиты:*\n"
			for k, v := range limits {
				label := formatLimitLabel(k)
				val := formatLimitValue(k, v)
				msg += fmt.Sprintf("  %s: %s\n", label, val)
			}
			msg += "\n"
		}
	}

	// Использование
	if len(info.Usage) > 0 && string(info.Usage) != "null" {
		var usage map[string]any
		if json.Unmarshal(info.Usage, &usage) == nil {
			msg += "📊 *Использование:*\n"
			for k, v := range usage {
				label := formatLimitLabel(k)
				msg += fmt.Sprintf("  %s: %v\n", label, v)
			}
		}
	}

	msg += "\n💡 /usage — детальная статистика | /plans — сменить план"
	b.sendMessage(chatID, msg)
}

// handleSubscribe — подписка на план (POST /api/billing/change-plan).
func (b *Bot) handleSubscribe(chatID int64, planID string) {
	planID = strings.TrimSpace(planID)
	if planID == "" {
		b.handlePlans(chatID)
		return
	}

	// Подтверждение
	msg := fmt.Sprintf("💳 *Подписка на FlowLink*\n\n📦 План: *%s*\n\nПодтвердите смену тарифа.", planID)

	kb := &tgInlineKeyboard{
		InlineKeyboard: [][]tgButton{
			{
				{Text: "✅ Подписаться", CallbackData: "subscribe:" + planID},
				{Text: "❌ Отмена", CallbackData: "dismiss"},
			},
		},
	}
	b.sendMessageWithKeyboard(chatID, msg, kb)
}

// doSubscribe — выполняет подписку через relay API.
func (b *Bot) doSubscribe(chatID int64, planID string) {
	b.sendMessage(chatID, fmt.Sprintf("⏳ Подписываю на план *%s*...", planID))

	data, err := b.relayPost("/api/billing/change-plan", map[string]any{
		"plan_id": planID,
	})
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка подписки: %v", err))
		return
	}

	// Проверяем успешность
	var resp struct {
		PlanID  string `json:"plan_id"`
		Message string `json:"message"`
		Error   string `json:"error"`
		Invoice any    `json:"invoice"`
	}
	if err := json.Unmarshal(data, &resp); err != nil {
		b.sendMessage(chatID, fmt.Sprintf("✅ Результат:\n```%s```", truncate(string(data), 1000)))
		return
	}

	if resp.Error != "" {
		b.sendMessage(chatID, fmt.Sprintf("❌ %s", resp.Error))
		return
	}

	msg := fmt.Sprintf("✅ *План изменён!*\n\n📦 Новый план: *%s*\n", resp.PlanID)

	if resp.Message != "" {
		msg += fmt.Sprintf("📋 %s\n", resp.Message)
	}

	b.sendMessage(chatID, msg)
}

// handleInvoices — история платежей (GET /api/billing/invoices).
func (b *Bot) handleInvoices(chatID int64) {
	data, err := b.relayGet("/api/billing/invoices")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	var invoices []struct {
		ID                string  `json:"id"`
		Number            string  `json:"number"`
		Status            string  `json:"status"`
		TotalKopecks      int64   `json:"total_kopecks"`
		Currency          string  `json:"currency"`
		CreatedAt         string  `json:"created_at"`
		PaidAt            *string `json:"paid_at"`
		PaymentMethod     *string `json:"payment_method"`
	}

	if err := json.Unmarshal(data, &invoices); err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка парсинга: %v", err))
		return
	}

	if len(invoices) == 0 {
		b.sendMessage(chatID, "📭 Нет счетов.")
		return
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("🧾 *Счета (%d)*\n\n", len(invoices)))

	for _, inv := range invoices {
		statusEmoji := "⏳"
		switch inv.Status {
		case "paid":
			statusEmoji = "✅"
		case "overdue":
			statusEmoji = "🔴"
		case "cancelled":
			statusEmoji = "❌"
		}

		shortID := inv.ID
		if len(shortID) > 8 {
			shortID = shortID[:8]
		}

		sb.WriteString(fmt.Sprintf("%s `%s` — *%s ₽* (%s)\n",
			statusEmoji,
			shortID,
			formatKopecksInt(inv.TotalKopecks),
			inv.Status,
		))

		if inv.PaidAt != nil && *inv.PaidAt != "" {
			sb.WriteString(fmt.Sprintf("   Оплачен: %s\n", (*inv.PaidAt)[:19]))
		} else {
			sb.WriteString(fmt.Sprintf("   Создан: %s\n", inv.CreatedAt[:19]))
		}
	}

	b.sendMessage(chatID, sb.String())
}

// handleUsage — статистика использования (GET /api/billing/usage).
func (b *Bot) handleUsage(chatID int64) {
	data, err := b.relayGet("/api/billing/usage")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	b.sendMessage(chatID, fmt.Sprintf("📊 *Использование*\n\n```json\n%s\n```", truncate(string(data), 2000)))
}

// handlePaymentMethods — доступные способы оплаты (GET /api/billing/payments/methods).
func (b *Bot) handlePaymentMethods(chatID int64) {
	data, err := b.relayGet("/api/billing/payments/methods")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	var methods []struct {
		ID   string `json:"id"`
		Name string `json:"name"`
	}
	if err := json.Unmarshal(data, &methods); err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка парсинга: %v", err))
		return
	}

	var sb strings.Builder
	sb.WriteString("💳 *Способы оплаты*\n\n")

	if len(methods) == 0 {
		sb.WriteString("_Нет настроенных способов оплаты._")
	} else {
		for _, m := range methods {
			sb.WriteString(fmt.Sprintf("  ✅ %s (`%s`)\n", m.Name, m.ID))
		}
	}

	b.sendMessage(chatID, sb.String())
}

// doCancelSubscription — отмена подписки (POST /api/billing/subscriptions/{id}/cancel).
func (b *Bot) doCancelSubscription(chatID int64, subID string) {
	data, err := b.relayPost("/api/billing/subscriptions/"+subID+"/cancel", nil)
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка отмены: %v", err))
		return
	}
	b.sendMessage(chatID, fmt.Sprintf("✅ Подписка `%s` отменена.\n\n%s", subID, string(data)))
}

// ═══════════════════════════════════════════════
// Безопасность (Shield)
// ═══════════════════════════════════════════════

// handleShieldAlerts — оповещения безопасности (GET /api/shield/alerts).
func (b *Bot) handleShieldAlerts(chatID int64) {
	data, err := b.relayGet("/api/shield/alerts")
	if err != nil {
		b.sendMessage(chatID, fmt.Sprintf("❌ Ошибка: %v", err))
		return
	}

	b.sendMessage(chatID, fmt.Sprintf("🛡 *Shield Alerts*\n\n```json\n%s\n```", truncate(string(data), 3000)))
}

// ═══════════════════════════════════════════════
// Утилиты
// ═══════════════════════════════════════════════

// formatKopecks — форматирует копейки в "1 990 ₽".
func formatKopecks(k uint64) string {
	rubles := k / 100
	if rubles == 0 && k > 0 {
		return fmt.Sprintf("%d коп.", k)
	}
	// Добавляем пробелы для тысяч
	s := fmt.Sprintf("%d", rubles)
	if len(s) > 3 {
		// Добавляем пробел каждые 3 цифры
		var result []byte
		for i := len(s) - 1; i >= 0; i-- {
			if (len(s)-i-1)%3 == 0 && len(s)-i-1 > 0 {
				result = append([]byte{' '}, result...)
			}
			result = append([]byte{s[i]}, result...)
		}
		s = string(result)
	}
	return s + " ₽"
}

// formatKopecksInt — форматирование int64 копеек.
func formatKopecksInt(k int64) string {
	if k < 0 {
		return "-" + formatKopecks(uint64(-k))
	}
	return formatKopecks(uint64(k))
}

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

// truncate — обрезает строку до maxLen.
func truncate(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}

// boolStr — возвращает a если true, b если false.
func boolStr(v bool, a, b string) string {
	if v {
		return a
	}
	return b
}

// mustMarshal — сериализует в JSON (panic on error, для internal use).
func mustMarshal(v any) []byte {
	data, _ := json.Marshal(v)
	return data
}

// formatLimitLabel — переводит ключ лимита в человекочитаемый вид.
func formatLimitLabel(key string) string {
	labels := map[string]string{
		"max_hosts":            "🖥 Серверы",
		"max_users":            "👤 Пользователи",
		"backup_storage_mb":    "💾 Хранилище бэкапов",
		"max_snapshots":        "📦 Снапшоты",
		"retention_days":       "📅 Хранение логов",
		"audit_retention_days": "📋 Аудит",
		"max_file_size_mb":     "📄 Макс. файл",
		"exec_timeout_sec":     "⏱ Таймаут команд",
		"shield_level":         "🛡 Shield",
	}
	if l, ok := labels[key]; ok {
		return l
	}
	return key
}

// formatLimitValue — форматирует значение лимита.
func formatLimitValue(key string, v any) string {
	switch val := v.(type) {
	case float64:
		if val == 0 {
			return "∞"
		}
		if strings.HasSuffix(key, "_mb") {
			return formatBytes(uint64(val) * 1024 * 1024)
		}
		if strings.HasSuffix(key, "_sec") {
			return fmt.Sprintf("%.0f сек", val)
		}
		if strings.HasSuffix(key, "_days") {
			return fmt.Sprintf("%.0f дней", val)
		}
		return fmt.Sprintf("%.0f", val)
	case string:
		return val
	default:
		return fmt.Sprintf("%v", v)
	}
}

// ensure these symbols are referenced (suppress unused warnings)
var _ = formatBytes
var _ = boolStr

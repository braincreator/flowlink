// Package integration — уведомления клиентам.
package integration

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"net/smtp"
	"net/url"
	"time"
)

// Notifier — отправляет уведомления клиентам о событиях подписки.
type Notifier struct {
	tgBotToken string // Telegram bot token for client notifications
	tgAPI      string // Telegram API URL
	smtpHost   string // SMTP server host
	smtpPort   int    // SMTP server port
	smtpUser   string // SMTP username
	smtpPass   string // SMTP password
	logger     *slog.Logger
}

// NotificationType — тип уведомления.
type NotificationType string

const (
	NotifWelcome          NotificationType = "welcome"
	NotifProvisioned      NotificationType = "provisioned"
	NotifPaymentFailed    NotificationType = "payment_failed"
	NotifSubscriptionEnd  NotificationType = "subscription_end"
	NotifPlanChanged      NotificationType = "plan_changed"
	NotifAutohealed       NotificationType = "autohealed"
)

// Notification — структура уведомления.
type Notification struct {
	Type        NotificationType
	CustomerID  string
	TelegramID  string // if known
	Email       string
	Subject     string
	Body        string // Markdown for Telegram, HTML for email
	Credentials *ConnectionCredentials // for welcome/provisioned
}

// NewNotifier — создаёт notifier.
func NewNotifier(tgBotToken, tgAPI, smtpHost string, smtpPort int, smtpUser, smtpPass string, logger *slog.Logger) *Notifier {
	if logger == nil {
		logger = slog.Default()
	}
	if tgAPI == "" {
		tgAPI = "https://api.telegram.org"
	}
	return &Notifier{
		tgBotToken: tgBotToken,
		tgAPI:      tgAPI,
		smtpHost:   smtpHost,
		smtpPort:   smtpPort,
		smtpUser:   smtpUser,
		smtpPass:   smtpPass,
		logger:     logger,
	}
}

// Send — отправляет уведомление через доступные каналы.
// Tries Telegram first (instant), falls back to email.
func (n *Notifier) Send(ctx context.Context, notif *Notification) error {
	n.logger.Info("sending notification",
		"type", notif.Type,
		"customer_id", notif.CustomerID,
		"has_telegram", notif.TelegramID != "",
		"has_email", notif.Email != "",
	)

	// Try Telegram first
	if notif.TelegramID != "" && n.tgBotToken != "" {
		if err := n.sendTelegram(ctx, notif); err != nil {
			n.logger.Error("failed to send Telegram notification", "err", err, "customer_id", notif.CustomerID)
			// Fall back to email
		} else {
			return nil // Success
		}
	}

	// Fall back to email
	if notif.Email != "" && n.smtpHost != "" {
		if err := n.sendEmail(ctx, notif); err != nil {
			n.logger.Error("failed to send email notification", "err", err, "customer_id", notif.CustomerID)
			return err
		}
		return nil
	}

	return fmt.Errorf("no delivery channel available")
}

// SendWelcome — отправляет welcome сообщение с credentials.
func (n *Notifier) SendWelcome(ctx context.Context, customerID, telegramID, email string, creds *ConnectionCredentials) error {
	body := fmt.Sprintf("🎉 **Добро пожаловать в FlowLink!**\n\nВаш relay сервер готов к работе.\n\n**Данные для подключения:**\n- **Client ID:** `%s`\n- **API Token:** `%s`\n- **Relay URL:** `%s`\n\n**Быстрая установка:**\n```bash\n%s\n```\n\n**Документация:** https://docs.flowlink.dev\n\nЕсли возникнут вопросы — обращайтесь в поддержку.\n", creds.ClientID, creds.APIToken, creds.RelayURL, creds.SetupCommand)

	notif := &Notification{
		Type:        NotifWelcome,
		CustomerID:  customerID,
		TelegramID:  telegramID,
		Email:       email,
		Subject:     "🎉 FlowLink: Ваш сервер готов",
		Body:        body,
		Credentials: creds,
	}

	return n.Send(ctx, notif)
}

// SendPaymentReminder — отправляет напоминание об оплате.
func (n *Notifier) SendPaymentReminder(ctx context.Context, customerID, telegramID, email string, daysLeft int) error {
	body := fmt.Sprintf("⚠️ **Напоминание об оплате**\n\nОсталось **%d дней** до окончания подписки.\n\nДля продолжения работы сервиса необходимо оплатить подписку.\n\n**Ссылка на оплату:** https://flowlink.dev/billing\n\nЕсли оплата уже прошла — проигнорируйте это сообщение.\n", daysLeft)

	notif := &Notification{
		Type:       NotifPaymentFailed,
		CustomerID: customerID,
		TelegramID: telegramID,
		Email:      email,
		Subject:    fmt.Sprintf("⚠️ FlowLink: Осталось %d дней", daysLeft),
		Body:       body,
	}

	return n.Send(ctx, notif)
}

// sendTelegram отправляет сообщение в Telegram
func (n *Notifier) sendTelegram(ctx context.Context, notif *Notification) error {
	// Parse mode: Markdown
	payload := map[string]interface{}{
		"chat_id":    notif.TelegramID,
		"text":       notif.Body,
		"parse_mode": "Markdown",
	}

	body, err := json.Marshal(payload)
	if err != nil {
		return fmt.Errorf("failed to marshal Telegram payload: %w", err)
	}

	apiURL := fmt.Sprintf("%s/bot%s/sendMessage", n.tgAPI, n.tgBotToken)

	req, err := http.NewRequestWithContext(ctx, "POST", apiURL, bytes.NewReader(body))
	if err != nil {
		return fmt.Errorf("failed to create request: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")

	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return fmt.Errorf("Telegram API request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		var errResp struct {
			Description string `json:"description"`
		}
		json.NewDecoder(resp.Body).Decode(&errResp)
		return fmt.Errorf("Telegram API error: %s", errResp.Description)
	}

	n.logger.Info("Telegram notification sent", "customer_id", notif.CustomerID, "telegram_id", notif.TelegramID)

	return nil
}

// sendEmail отправляет email через SMTP
func (n *Notifier) sendEmail(ctx context.Context, notif *Notification) error {
	// Construct email message
	from := n.smtpUser
	to := notif.Email
	subject := notif.Subject

	// Convert Markdown to HTML (simple)
	htmlBody := markdownToHTML(notif.Body)

	msg := fmt.Sprintf("From: %s\r\n"+
		"To: %s\r\n"+
		"Subject: %s\r\n"+
		"MIME-version: 1.0;\r\n"+
		"Content-Type: text/html; charset=\"UTF-8\";\r\n"+
		"\r\n"+
		"%s", from, to, subject, htmlBody)

	// Send via SMTP
	auth := smtp.PlainAuth("", n.smtpUser, n.smtpPass, n.smtpHost)
	addr := fmt.Sprintf("%s:%d", n.smtpHost, n.smtpPort)

	if err := smtp.SendMail(addr, auth, from, []string{to}, []byte(msg)); err != nil {
		return fmt.Errorf("SMTP send failed: %w", err)
	}

	n.logger.Info("Email notification sent", "customer_id", notif.CustomerID, "email", notif.Email)

	return nil
}

// markdownToHTML — простая конвертация Markdown → HTML
func markdownToHTML(md string) string {
	// **bold** → <b>bold</b>
	// `code` → <code>code</code>
	// ```code``` → <pre>code</pre>

	html := md

	// Bold
	html = replaceAll(html, "**", "<b>", "</b>")

	// Code blocks (```...```)
	for {
		start := findUnescaped(html, "```")
		if start == -1 {
			break
		}
		end := findUnescaped(html[start+3:], "```")
		if end == -1 {
			break
		}
		end += start + 3

		code := html[start+3 : end]
		html = html[:start] + "<pre><code>" + escapeHTML(code) + "</code></pre>" + html[end+3:]
	}

	// Inline code (`...`)
	html = replaceAll(html, "`", "<code>", "</code>")

	// Line breaks
	html = replaceNewlines(html)

	return html
}

// replaceAll replaces pairs of delimiters with tags
func replaceAll(s, delim, openTag, closeTag string) string {
	result := ""
	parts := splitUnescaped(s, delim)
	for i, part := range parts {
		result += part
		if i < len(parts)-1 {
			if i%2 == 0 {
				result += openTag
			} else {
				result += closeTag
			}
		}
	}
	return result
}

// splitUnescaped splits by delimiter, ignoring escaped ones
func splitUnescaped(s, delim string) []string {
	var result []string
	current := ""
	i := 0
	for i < len(s) {
		if i+len(delim) <= len(s) && s[i:i+len(delim)] == delim {
			result = append(result, current)
			current = ""
			i += len(delim)
		} else {
			current += string(s[i])
			i++
		}
	}
	result = append(result, current)
	return result
}

// findUnescaped finds delimiter position, ignoring escaped
func findUnescaped(s, delim string) int {
	return findUnescapedFrom(s, delim, 0)
}

func findUnescapedFrom(s, delim string, start int) int {
	for i := start; i <= len(s)-len(delim); i++ {
		if s[i:i+len(delim)] == delim {
			// Check if escaped
			if i > 0 && s[i-1] == '\\' {
				continue
			}
			return i
		}
	}
	return -1
}

// escapeHTML escapes HTML special chars
func escapeHTML(s string) string {
	s = replaceAll(s, "&", "&amp;", "&amp;") // Special case
	s = replaceAll(s, "<", "&lt;", "&lt;")
	s = replaceAll(s, ">", "&gt;", "&gt;")
	s = replaceAll(s, "\"", "&quot;", "&quot;")
	s = replaceAll(s, "'", "&#39;", "&#39;")
	return s
}

// replaceNewlines converts \n to <br>
func replaceNewlines(s string) string {
	result := ""
	for _, c := range s {
		if c == '\n' {
			result += "<br>"
		} else {
			result += string(c)
		}
	}
	return result
}

// ValidateTelegramID validates Telegram ID format
func ValidateTelegramID(id string) error {
	if id == "" {
		return fmt.Errorf("empty telegram ID")
	}
	// Should be numeric or start with @
	if id[0] != '@' {
		// Check if numeric
		for _, c := range id {
			if c < '0' || c > '9' {
				return fmt.Errorf("invalid telegram ID format")
			}
		}
	}
	return nil
}

// ValidateEmail validates email format
func ValidateEmail(email string) error {
	if email == "" {
		return fmt.Errorf("empty email")
	}
	if !contains(email, "@") || !contains(email, ".") {
		return fmt.Errorf("invalid email format")
	}
	return nil
}

func contains(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

// URL encode helper
func urlEncode(s string) string {
	return url.QueryEscape(s)
}

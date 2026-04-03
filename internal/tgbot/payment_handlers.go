// Package tgbot — обработчики платёжных команд Telegram-бота.
package tgbot

import (
	"fmt"
	"log/slog"
	"net/http"

	"github.com/braincreator/flowlink/internal/billing"
)

// PaymentHandlers — обработчики платежей для Telegram-бота.
type PaymentHandlers struct {
	planStore    *billing.PlanStore
	invoiceStore *billing.InvoiceStore
	gateway      billing.PaymentGateway
	bot          *Bot
	logger       *slog.Logger
}

// NewPaymentHandlers — создаёт обработчики платежей.
func NewPaymentHandlers(
	planStore *billing.PlanStore,
	invoiceStore *billing.InvoiceStore,
	gateway billing.PaymentGateway,
	bot *Bot,
	logger *slog.Logger,
) *PaymentHandlers {
	if logger == nil {
		logger = slog.Default()
	}
	return &PaymentHandlers{
		planStore:    planStore,
		invoiceStore: invoiceStore,
		gateway:      gateway,
		bot:          bot,
		logger:       logger,
	}
}

// HandleSubscribe — обрабатывает /subscribe <plan_id>.
func (ph *PaymentHandlers) HandleSubscribe(chatID int64, planID string) {
	if planID == "" {
		ph.bot.sendMessage(chatID, "⚠ Использование: /subscribe `<план>`\n\nДоступные планы:\n• *Free* — бесплатно\n• *Starter* — $19/мес\n• *Pro* — $49/мес\n• *Enterprise* — по запросу")
		return
	}

	plan, ok := ph.planStore.GetPlan(planID)
	if !ok {
		ph.bot.sendMessage(chatID, fmt.Sprintf("❌ План «%s» не найден. Используйте /subscribe <free|starter|pro|enterprise>", planID))
		return
	}

	if plan.PriceMonthly == 0 {
		ph.bot.sendMessage(chatID, fmt.Sprintf("✅ План *%s* активирован (бесплатно).\n\nФункции:\n%s", plan.Name, formatFeatures(plan.Features)))
		return
	}

	// Создаём счёт
	clientID := fmt.Sprintf("tg:%d", chatID)
	inv, err := ph.invoiceStore.GenerateInvoice(clientID, planID)
	if err != nil {
		ph.logger.Error("failed to create invoice", "err", err, "client", clientID, "plan", planID)
		ph.bot.sendMessage(chatID, "❌ Ошибка создания счёта. Попробуйте позже.")
		return
	}

	// Создаём SBP платёж
	session, err := ph.gateway.CreatePayment(inv, "")
	if err != nil {
		ph.logger.Error("failed to create payment", "err", err, "invoice", inv.ID)
		ph.bot.sendMessage(chatID, "❌ Ошибка создания платежа. Попробуйте позже.")
		return
	}

	rubAmount := billing.USDtoRUB(plan.PriceMonthly)
	msg := fmt.Sprintf(
		"💳 *Оплата подписки*\n\n"+
			"📦 План: *%s*\n"+
			"💰 Сумма: *%.2f ₽* ($%.2f по курсу ЦБ РФ)\n"+
			"🧾 Счёт: `%s`\n\n"+
			"📱 Отсканируйте QR-код в приложении банка для оплаты через СБП.\n\n"+
			"⏰ Счёт действителен 7 дней.",
		plan.Name, rubAmount, plan.PriceMonthly, inv.ID,
	)

	// Отправляем сообщение
	ph.bot.sendMessage(chatID, msg)

	// Отправляем QR-код как фото
	if session.PaymentURL != "" {
		ph.bot.sendPhoto(chatID, session.PaymentURL)
	} else if session.QRPayload != "" {
		// Генерируем QR локально
		qrData, err := billing.GenerateQRCode(session.QRPayload, 300)
		if err == nil {
			ph.bot.sendPhotoBytes(chatID, qrData)
		}
	}

	ph.logger.Info("payment session created", "chat", chatID, "plan", planID, "invoice", inv.ID, "payment", session.PaymentID)
}

// HandlePaymentStatus — обрабатывает /payment_status.
func (ph *PaymentHandlers) HandlePaymentStatus(chatID int64) {
	clientID := fmt.Sprintf("tg:%d", chatID)
	invoices := ph.invoiceStore.ListInvoices(clientID)

	if len(invoices) == 0 {
		ph.bot.sendMessage(chatID, "📭 У вас нет счетов.")
		return
	}

	// Берём последний счёт
	last := invoices[len(invoices)-1]
	statusEmoji := "⏳"
	switch last.Status {
	case billing.InvoiceStatusPaid:
		statusEmoji = "✅"
	case billing.InvoiceStatusOverdue:
		statusEmoji = "⚠️"
	case billing.InvoiceStatusCancelled:
		statusEmoji = "❌"
	}

	msg := fmt.Sprintf(
		"🧾 *Последний счёт*\n\n"+
			"%s Статус: *%s*\n"+
			"💰 Сумма: *%.2f ₽*\n"+
			"📦 План: %s\n"+
			"📅 Создан: %s\n"+
			"⏰ До: %s\n",
		statusEmoji, last.Status,
		last.Amount, last.PlanID,
		last.CreatedAt.Format("02.01.2006"),
		last.DueDate.Format("02.01.2006"),
	)

	if last.PaidAt != nil {
		msg += fmt.Sprintf("✅ Оплачен: %s\n", last.PaidAt.Format("02.01.2006 15:04"))
	}

	ph.bot.sendMessage(chatID, msg)
}

// HandleWebhook — HTTP handler для вебхуков от Точки.
func (ph *PaymentHandlers) HandleWebhook(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}

	signature := r.Header.Get("X-Signature")
	body, err := readRequestBody(r)
	if err != nil {
		ph.logger.Error("webhook read body failed", "err", err)
		http.Error(w, "bad request", http.StatusBadRequest)
		return
	}

	evt, err := ph.gateway.WebhookVerify(body, signature)
	if err != nil {
		ph.logger.Error("webhook verify failed", "err", err)
		http.Error(w, "forbidden", http.StatusForbidden)
		return
	}

	ph.logger.Info("webhook received", "event", evt.Event, "invoice", evt.InvoiceID, "payment", evt.PaymentID)

	// Обрабатываем событие
	switch evt.Event {
	case "payment.paid":
		if err := ph.invoiceStore.MarkPaid(evt.InvoiceID); err != nil {
			ph.logger.Error("failed to mark invoice paid", "err", err, "invoice", evt.InvoiceID)
		} else {
			ph.notifyPaymentSuccess(evt.InvoiceID)
		}
	default:
		ph.logger.Info("unhandled webhook event", "event", evt.Event)
	}

	w.WriteHeader(http.StatusOK)
}

// notifyPaymentSuccess — отправляет уведомление об успешной оплате.
func (ph *PaymentHandlers) notifyPaymentSuccess(invoiceID string) {
	inv, ok := ph.invoiceStore.GetInvoice(invoiceID)
	if !ok {
		return
	}

	chatID := parseChatID(inv.ClientID)
	if chatID == 0 {
		return
	}

	plan, _ := ph.planStore.GetPlan(inv.PlanID)
	planName := inv.PlanID
	if plan.ID != "" {
		planName = plan.Name
	}

	msg := fmt.Sprintf(
		"✅ *Оплата получена!*\n\n"+
			"📦 План: *%s*\n"+
			"💰 Сумма: *%.2f ₽*\n"+
			"🧾 Счёт: `%s`\n\n"+
			"Спасибо за подписку! 🎉",
		planName, inv.Amount, inv.ID,
	)
	ph.bot.sendMessage(chatID, msg)
}

// parseChatID — извлекает chatID из clientID формата "tg:123456".
func parseChatID(clientID string) int64 {
	var chatID int64
	fmt.Sscanf(clientID, "tg:%d", &chatID)
	return chatID
}

// readRequestBody — читает body из HTTP request.
func readRequestBody(r *http.Request) ([]byte, error) {
	defer r.Body.Close()
	body := make([]byte, r.ContentLength)
	n, err := r.Body.Read(body)
	if err != nil && err.Error() != "EOF" {
		return nil, err
	}
	return body[:n], nil
}

// formatFeatures — форматирует список фич для отображения.
func formatFeatures(features []string) string {
	result := ""
	for _, f := range features {
		result += fmt.Sprintf("• %s\n", f)
	}
	return result
}

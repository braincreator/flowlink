// Package tgbot — обработчики платёжных команд Telegram-бота.
package tgbot

import (
	"fmt"
	"log/slog"
	"net/http"
	"strings"

	"github.com/braincreator/flowlink/internal/billing"
)

// PaymentHandlers — обработчики платежей для Telegram-бота.
type PaymentHandlers struct {
	planStore        *billing.PlanStore
	invoiceStore     *billing.InvoiceStore
	subscriptionStore *billing.SubscriptionStore
	gateway          billing.PaymentGateway
	bot              *Bot
	logger           *slog.Logger
}

// NewPaymentHandlers — создаёт обработчики платежей.
func NewPaymentHandlers(
	planStore *billing.PlanStore,
	invoiceStore *billing.InvoiceStore,
	subscriptionStore *billing.SubscriptionStore,
	gateway billing.PaymentGateway,
	bot *Bot,
	logger *slog.Logger,
) *PaymentHandlers {
	if logger == nil {
		logger = slog.Default()
	}
	return &PaymentHandlers{
		planStore:        planStore,
		invoiceStore:     invoiceStore,
		subscriptionStore: subscriptionStore,
		gateway:          gateway,
		bot:              bot,
		logger:           logger,
	}
}

// HandleSubscribe — обрабатывает /subscribe [plan_id] [period].
func (ph *PaymentHandlers) HandleSubscribe(chatID int64, args []string) {
	clientID := fmt.Sprintf("tg:%d", chatID)

	// Если аргументов нет — показать список планов
	if len(args) == 0 {
		ph.showPlans(chatID)
		return
	}

	planID := args[0]
	period := billing.PeriodMonthly
	if len(args) > 1 {
		switch args[1] {
		case "monthly", "m":
			period = billing.PeriodMonthly
		case "quarterly", "q":
			period = billing.PeriodQuarterly
		case "yearly", "y":
			period = billing.PeriodYearly
		default:
			ph.bot.sendMessage(chatID, "⚠ Неверный период. Используйте: monthly, quarterly, yearly")
			return
		}
	}

	// Проверяем план
	plan, ok := ph.planStore.GetPlan(planID)
	if !ok {
		ph.bot.sendMessage(chatID, fmt.Sprintf("❌ План «%s» не найден. Используйте /subscribe без параметров для списка планов.", planID))
		return
	}

	// Бесплатный план
	if plan.PriceMonthly == 0 {
		ph.bot.sendMessage(chatID, fmt.Sprintf("✅ План *%s* активирован (бесплатно).\n\nФункции:\n%s", plan.Name, formatFeatures(plan.Features)))
		return
	}

	// Проверяем, нет ли уже активной подписки
	subs := ph.subscriptionStore.ListSubscriptions(clientID)
	for _, sub := range subs {
		if sub.Status == billing.SubscriptionStatusActive {
			ph.bot.sendMessage(chatID, fmt.Sprintf("⚠ У вас уже есть активная подписка: *%s* (%s).\n\nИспользуйте /my_subscription для информации или /cancel для отмены.", sub.PlanID, sub.Period))
			return
		}
	}

	// Создаём счёт
	customerEmail := fmt.Sprintf("tg%d@flowlink.flow-masters.ru", chatID)
	inv, err := ph.invoiceStore.GenerateInvoice(clientID, planID)
	if err != nil {
		ph.logger.Error("failed to create invoice", "err", err, "client", clientID, "plan", planID)
		ph.bot.sendMessage(chatID, "❌ Ошибка создания счёта. Попробуйте позже.")
		return
	}

	// Создаём платёж (первый платёж с save_payment_method)
	session, err := ph.gateway.CreatePayment(inv, "")
	if err != nil {
		ph.logger.Error("failed to create payment", "err", err, "invoice", inv.ID)
		ph.bot.sendMessage(chatID, "❌ Ошибка создания платежа. Попробуйте позже.")
		return
	}

	// Создаём pending подписку
	sub, err := ph.subscriptionStore.CreateSubscription(
		clientID,
		customerEmail,
		planID,
		period,
		"", // payment_method_id придёт в webhook
		session.PaymentID,
	)
	if err != nil {
		ph.logger.Error("failed to create subscription", "err", err, "client", clientID)
		ph.bot.sendMessage(chatID, "❌ Ошибка создания подписки. Попробуйте позже.")
		return
	}

	// Рассчитываем цену
	prices := plan.GetPrices()
	var price billing.PlanPrice
	for _, p := range prices {
		if p.Period == period {
			price = p
			break
		}
	}

	rubAmount := billing.USDtoRUB(price.Total)
	msg := fmt.Sprintf(
		"💳 *Подписка на FlowLink*\n\n"+
			"📦 План: *%s*\n"+
			"📅 Период: *%s*\n"+
			"💰 Сумма: *%.2f ₽* ($%.2f)\n",
		plan.Name, formatPeriod(period), rubAmount, price.Total,
	)

	if price.Savings > 0 {
		msg += fmt.Sprintf("🎉 Экономия: *%.2f ₽* (%s)\n", billing.USDtoRUB(price.Savings), price.SavingsPct)
	}

	msg += fmt.Sprintf(
		"🧾 Счёт: `%s`\n\n"+
			"💳 Нажмите кнопку ниже для оплаты картой.\n"+
			"🔄 Карта будет сохранена для автоматического продления.",
		inv.ID,
	)

	// Отправляем сообщение с inline кнопкой
	keyboard := &tgInlineKeyboard{
		InlineKeyboard: [][]tgButton{
			{{Text: "💳 Оплатить картой", URL: session.PaymentURL}},
			{{Text: "❌ Отмена", CallbackData: "cancel_sub:" + sub.ID}},
		},
	}
	ph.bot.sendMessageWithKeyboard(chatID, msg, keyboard)

	ph.logger.Info("subscription payment created", "chat", chatID, "plan", planID, "period", period, "invoice", inv.ID, "payment", session.PaymentID)
}

// showPlans — показывает список планов с ценами.
func (ph *PaymentHandlers) showPlans(chatID int64) {
	plans := ph.planStore.ListPlans()
	msg := "📋 *Тарифные планы FlowLink*\n\n"

	for _, plan := range plans {
		if plan.ID == "enterprise" {
			continue // Enterprise по запросу
		}

		msg += fmt.Sprintf("📦 *%s*\n", plan.Name)
		if plan.PriceMonthly == 0 {
			msg += "   💰 Бесплатно\n"
		} else {
			prices := plan.GetPrices()
			for _, p := range prices {
				rubTotal := billing.USDtoRUB(p.Total)
				periodName := formatPeriod(p.Period)
				savingsInfo := ""
				if p.SavingsPct != "" {
					savingsInfo = fmt.Sprintf(" (экономия %s)", p.SavingsPct)
				}
				msg += fmt.Sprintf("   • %s: *%.0f ₽*%s\n", periodName, rubTotal, savingsInfo)
			}
		}
		msg += fmt.Sprintf("   %s\n\n", strings.Join(plan.Features[:min(3, len(plan.Features))], ", "))
	}

	msg += "💡 *Как подписаться:*\n"
	msg += "`/subscribe starter monthly` — Starter на месяц\n"
	msg += "`/subscribe pro yearly` — Pro на год (экономия 30%%)\n"
	msg += "`/subscribe free` — Free навсегда"

	ph.bot.sendMessage(chatID, msg)
}

// HandleMySubscription — обрабатывает /my_subscription.
func (ph *PaymentHandlers) HandleMySubscription(chatID int64) {
	clientID := fmt.Sprintf("tg:%d", chatID)
	subs := ph.subscriptionStore.ListSubscriptions(clientID)

	if len(subs) == 0 {
		ph.bot.sendMessage(chatID, "📭 У вас нет подписок.\n\nИспользуйте /subscribe для выбора плана.")
		return
	}

	// Показываем все подписки
	for _, sub := range subs {
		plan, _ := ph.planStore.GetPlan(sub.PlanID)
		planName := sub.PlanID
		if plan.ID != "" {
			planName = plan.Name
		}

		statusEmoji := "⏳"
		switch sub.Status {
		case billing.SubscriptionStatusActive:
			statusEmoji = "✅"
		case billing.SubscriptionStatusCancelled:
			statusEmoji = "❌"
		case billing.SubscriptionStatusExpired:
			statusEmoji = "⚠️"
		case billing.SubscriptionStatusPending:
			statusEmoji = "💳"
		}

		msg := fmt.Sprintf(
			"%s *Подписка*\n\n"+
				"📦 План: *%s*\n"+
				"📅 Период: *%s*\n"+
				"📊 Статус: *%s*\n",
			statusEmoji, planName, formatPeriod(sub.Period), sub.Status,
		)

		if sub.Status == billing.SubscriptionStatusActive {
			msg += fmt.Sprintf(
				"📅 Следующее списание: *%s*\n"+
					"💰 Сумма: *%.0f ₽*\n",
				sub.NextBillingDate.Format("02.01.2006"),
				billing.USDtoRUB(getSubscriptionAmount(plan, sub.Period)),
			)
		}

		msg += fmt.Sprintf(
			"🕐 Начало: *%s*\n",
			sub.StartedAt.Format("02.01.2006"),
		)

		if sub.CancelledAt != nil {
			msg += fmt.Sprintf("❌ Отменена: *%s*\n", sub.CancelledAt.Format("02.01.2006"))
		}

		// Добавляем кнопки
		if sub.Status == billing.SubscriptionStatusActive {
			keyboard := &tgInlineKeyboard{
				InlineKeyboard: [][]tgButton{
					{{Text: "❌ Отменить подписку", CallbackData: "cancel_sub:" + sub.ID}},
				},
			}
			ph.bot.sendMessageWithKeyboard(chatID, msg, keyboard)
		} else {
			ph.bot.sendMessage(chatID, msg)
		}
	}
}

// HandleCancel — обрабатывает /cancel [subscription_id].
func (ph *PaymentHandlers) HandleCancel(chatID int64, args []string) {
	clientID := fmt.Sprintf("tg:%d", chatID)
	subs := ph.subscriptionStore.ListSubscriptions(clientID)

	if len(subs) == 0 {
		ph.bot.sendMessage(chatID, "📭 У вас нет подписок для отмены.")
		return
	}

	// Если не указан ID — отменяем первую активную
	var targetSub *billing.Subscription
	if len(args) > 0 {
		subID := args[0]
		var ok bool
		targetSub, ok = ph.subscriptionStore.GetSubscription(subID)
		if !ok {
			ph.bot.sendMessage(chatID, fmt.Sprintf("❌ Подписка «%s» не найдена.", subID))
			return
		}
	} else {
		for _, sub := range subs {
			if sub.Status == billing.SubscriptionStatusActive {
				targetSub = sub
				break
			}
		}
	}

	if targetSub == nil {
		ph.bot.sendMessage(chatID, "⚠ Нет активных подписок для отмены.")
		return
	}

	if targetSub.Status != billing.SubscriptionStatusActive {
		ph.bot.sendMessage(chatID, fmt.Sprintf("⚠ Подписка уже %s.", targetSub.Status))
		return
	}

	// Подтверждение отмены
	msg := fmt.Sprintf(
		"⚠ *Подтверждение отмены*\n\n"+
			"📦 План: *%s*\n"+
			"📅 Период: *%s*\n"+
			"💰 Вы потеряете доступ: *%s*\n\n"+
			"Вы уверены?",
		targetSub.PlanID, formatPeriod(targetSub.Period),
		targetSub.NextBillingDate.Format("02.01.2006"),
	)

	keyboard := &tgInlineKeyboard{
		InlineKeyboard: [][]tgButton{
			{
				{Text: "✅ Да, отменить", CallbackData: "confirm_cancel:" + targetSub.ID},
				{Text: "❌ Нет, оставить", CallbackData: "dismiss"},
			},
		},
	}
	ph.bot.sendMessageWithKeyboard(chatID, msg, keyboard)
}

// HandleCallback — обрабатывает callback от inline кнопок.
func (ph *PaymentHandlers) HandleCallback(chatID int64, callbackData string) {
	parts := strings.SplitN(callbackData, ":", 2)
	action := parts[0]

	switch action {
	case "cancel_sub":
		if len(parts) < 2 {
			return
		}
		subID := parts[1]
		ph.HandleCancel(chatID, []string{subID})

	case "confirm_cancel":
		if len(parts) < 2 {
			return
		}
		subID := parts[1]
		ph.cancelSubscription(chatID, subID, false)

	case "dismiss":
		// Ничего не делаем, кнопка просто закрывается
	}
}

// cancelSubscription — отменяет подписку.
func (ph *PaymentHandlers) cancelSubscription(chatID int64, subID string, refund bool) {
	err := ph.subscriptionStore.CancelSubscription(subID, refund)
	if err != nil {
		ph.logger.Error("failed to cancel subscription", "err", err, "sub", subID)
		ph.bot.sendMessage(chatID, "❌ Ошибка отмены подписки. Попробуйте позже.")
		return
	}

	ph.bot.sendMessage(chatID, "✅ Подписка отменена.\n\nВы можете продолжать пользоваться сервисом до конца оплаченного периода.")
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
		// Отмечаем счёт оплаченным
		if err := ph.invoiceStore.MarkPaid(evt.InvoiceID); err != nil {
			ph.logger.Error("failed to mark invoice paid", "err", err, "invoice", evt.InvoiceID)
		}

		// Активируем подписку (если есть payment_method_id)
		ph.activateSubscription(evt.InvoiceID, evt.PaymentMethodID)

		// Уведомляем клиента
		ph.notifyPaymentSuccess(evt.InvoiceID)

	case "payment.rejected":
		ph.logger.Warn("payment rejected", "invoice", evt.InvoiceID, "payment", evt.PaymentID)
		// Можно уведомить клиента

	case "payment.refunded":
		ph.logger.Info("payment refunded", "invoice", evt.InvoiceID, "payment", evt.PaymentID)
	}

	w.WriteHeader(http.StatusOK)
}

// activateSubscription — активирует подписку после успешной оплаты.
func (ph *PaymentHandlers) activateSubscription(invoiceID, paymentMethodID string) {
	inv, ok := ph.invoiceStore.GetInvoice(invoiceID)
	if !ok {
		return
	}

	// Ищем pending подписку для этого клиента
	subs := ph.subscriptionStore.ListSubscriptions(inv.ClientID)
	for _, sub := range subs {
		if sub.Status == billing.SubscriptionStatusPending {
			// Обновляем подписку с payment_method_id и активируем
			sub.PaymentMethodID = paymentMethodID
			sub.Status = billing.SubscriptionStatusActive
			sub.LastPaymentID = inv.ID
			// Сохраняем (нужен метод UpdateSubscription в store)
			ph.logger.Info("subscription activated", "sub", sub.ID, "customer", inv.ClientID, "method", paymentMethodID)
			break
		}
	}
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
			"🎉 Подписка активирована!\n"+
			"🔄 Карта сохранена для автоматического продления.",
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

// formatPeriod — форматирует период для отображения.
func formatPeriod(period billing.BillingPeriod) string {
	switch period {
	case billing.PeriodMonthly:
		return "Месяц"
	case billing.PeriodQuarterly:
		return "Квартал (3 мес)"
	case billing.PeriodYearly:
		return "Год"
	default:
		return string(period)
	}
}

// getSubscriptionAmount — возвращает сумму подписки для периода.
func getSubscriptionAmount(plan billing.Plan, period billing.BillingPeriod) float64 {
	prices := plan.GetPrices()
	for _, p := range prices {
		if p.Period == period {
			return p.Total
		}
	}
	return plan.PriceMonthly
}

// min — возвращает минимум из двух чисел.
func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

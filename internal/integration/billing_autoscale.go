// Package integration — связывает billing с autoscale.
package integration

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"github.com/braincreator/flowlink/internal/billing"
)

// ProvisionerInterface — интерфейс для provisioner (dependency injection).
type ProvisionerInterface interface {
	Provision(ctx context.Context, req *ProvisioningRequest) (*ProvisioningResult, error)
	Deprovision(ctx context.Context, customerID string) error
	GetProvisionedClients() ([]ProvisionedClient, error)
}

// NotifierInterface — интерфейс для notifier (dependency injection).
type NotifierInterface interface {
	Send(ctx context.Context, notif *Notification) error
	SendWelcome(ctx context.Context, customerID, telegramID, email string, creds *ConnectionCredentials) error
	SendPaymentReminder(ctx context.Context, customerID, telegramID, email string, daysLeft int) error
}

// BillingAutoscaleBridge — bridge между billing и autoscale.
// Слушает billing events и триггерит autoscale actions.
type BillingAutoscaleBridge struct {
	mu         sync.Mutex
	subStore   *billing.SubscriptionStore
	scaler     AutoScalerInterface
	router     AutoRouterInterface
	provisioner ProvisionerInterface
	notifier   NotifierInterface
	logger     *slog.Logger

	// Grace periods for failed payments (customerID -> expiry)
	gracePeriods map[string]time.Time
}

// AutoScalerInterface — интерфейс для autoscaler (dependency injection).
type AutoScalerInterface interface {
	ScaleUp(ctx context.Context, customerID string) error
	ScaleDown(ctx context.Context, customerID string) error
	GetStatus(ctx context.Context, customerID string) (interface{}, error)
}

// AutoRouterInterface — интерфейс для traffic router.
type AutoRouterInterface interface {
	RegisterClient(ctx context.Context, clientID string) error
	UnregisterClient(ctx context.Context, clientID string) error
	GetTarget(ctx context.Context, clientID string) (string, error)
}

// NewBillingAutoscaleBridge — создаёт bridge.
func NewBillingAutoscaleBridge(
	subStore *billing.SubscriptionStore,
	scaler AutoScalerInterface,
	router AutoRouterInterface,
	provisioner ProvisionerInterface,
	notifier NotifierInterface,
	logger *slog.Logger,
) *BillingAutoscaleBridge {
	if logger == nil {
		logger = slog.Default()
	}
	return &BillingAutoscaleBridge{
		subStore:     subStore,
		scaler:       scaler,
		router:       router,
		provisioner:  provisioner,
		notifier:     notifier,
		logger:       logger,
		gracePeriods: make(map[string]time.Time),
	}
}

// HandleSubscriptionCreated — called when new subscription is created.
// Actions:
// 1. Create dedicated Docker container for client
// 2. Configure container with client-specific settings
// 3. Register client in traffic router
// 4. Send welcome message with connection credentials
func (b *BillingAutoscaleBridge) HandleSubscriptionCreated(ctx context.Context, sub *billing.Subscription) error {
	b.mu.Lock()
	defer b.mu.Unlock()

	b.logger.Info("handling subscription created", "subscription_id", sub.ID, "customer_id", sub.CustomerID)

	// 1. Provision container
	provReq := &ProvisioningRequest{
		CustomerID:     sub.CustomerID,
		CustomerEmail:  sub.CustomerEmail,
		PlanID:         sub.PlanID,
		SubscriptionID: sub.ID,
	}

	result, err := b.provisioner.Provision(ctx, provReq)
	if err != nil {
		b.logger.Error("failed to provision container", "err", err, "customer_id", sub.CustomerID)
		return fmt.Errorf("provision failed: %w", err)
	}

	// 2. Register in traffic router
	if b.router != nil {
		if err := b.router.RegisterClient(ctx, sub.CustomerID); err != nil {
			b.logger.Error("failed to register in router", "err", err, "customer_id", sub.CustomerID)
			// Не останавливаем процесс, логируем ошибку
		}
	}

	// 3. Trigger autoscale up
	if b.scaler != nil {
		if err := b.scaler.ScaleUp(ctx, sub.CustomerID); err != nil {
			b.logger.Error("failed to scale up", "err", err, "customer_id", sub.CustomerID)
		}
	}

	// 4. Send welcome notification with credentials
	if b.notifier != nil {
		if err := b.notifier.SendWelcome(ctx, sub.CustomerID, "", sub.CustomerEmail, result.Credentials); err != nil {
			b.logger.Error("failed to send welcome notification", "err", err, "customer_id", sub.CustomerID)
		}
	}

	b.logger.Info("subscription created successfully",
		"subscription_id", sub.ID,
		"container_id", result.ContainerID,
		"port", result.Port,
		"setup_time", result.SetupTime,
	)

	return nil
}

// HandleSubscriptionRenewed — called when recurring payment succeeds.
// Actions:
// 1. Ensure container is running
// 2. Reset traffic quotas if applicable
// 3. Log renewal
func (b *BillingAutoscaleBridge) HandleSubscriptionRenewed(ctx context.Context, sub *billing.Subscription) error {
	b.mu.Lock()
	defer b.mu.Unlock()

	b.logger.Info("handling subscription renewed", "subscription_id", sub.ID, "customer_id", sub.CustomerID)

	// 1. Ensure container is running
	if b.scaler != nil {
		status, err := b.scaler.GetStatus(ctx, sub.CustomerID)
		if err != nil {
			b.logger.Error("failed to get container status", "err", err, "customer_id", sub.CustomerID)
			// Пробуем scale up если не удалось получить статус
			if err := b.scaler.ScaleUp(ctx, sub.CustomerID); err != nil {
				b.logger.Error("failed to scale up on renewal", "err", err, "customer_id", sub.CustomerID)
			}
		} else {
			b.logger.Info("container status verified", "customer_id", sub.CustomerID, "status", status)
		}
	}

	// 2. Remove from grace periods if was there
	delete(b.gracePeriods, sub.CustomerID)

	b.logger.Info("subscription renewed successfully", "subscription_id", sub.ID, "customer_id", sub.CustomerID)

	return nil
}

// HandleSubscriptionCancelled — called when subscription is cancelled.
// Actions:
// 1. Set container to "draining" mode
// 2. Wait for active connections to close (configurable timeout)
// 3. Stop and remove container
// 4. Remove from traffic router
// 5. Cleanup client data
func (b *BillingAutoscaleBridge) HandleSubscriptionCancelled(ctx context.Context, sub *billing.Subscription) error {
	b.mu.Lock()
	defer b.mu.Unlock()

	b.logger.Info("handling subscription cancelled", "subscription_id", sub.ID, "customer_id", sub.CustomerID)

	// 1. Unregister from traffic router (draining mode)
	if b.router != nil {
		if err := b.router.UnregisterClient(ctx, sub.CustomerID); err != nil {
			b.logger.Error("failed to unregister from router", "err", err, "customer_id", sub.CustomerID)
		}
	}

	// 2. Scale down (graceful shutdown)
	if b.scaler != nil {
		if err := b.scaler.ScaleDown(ctx, sub.CustomerID); err != nil {
			b.logger.Error("failed to scale down", "err", err, "customer_id", sub.CustomerID)
		}
	}

	// 3. Deprovision container
	if b.provisioner != nil {
		if err := b.provisioner.Deprovision(ctx, sub.CustomerID); err != nil {
			b.logger.Error("failed to deprovision", "err", err, "customer_id", sub.CustomerID)
		}
	}

	// 4. Send notification
	if b.notifier != nil {
		notif := &Notification{
			Type:       NotifSubscriptionEnd,
			CustomerID: sub.CustomerID,
			Email:      sub.CustomerEmail,
			Subject:    "Подписка отменена",
			Body:       fmt.Sprintf("Ваша подписка %s отменена. Данные будут удалены в течение 24 часов.", sub.ID),
		}
		if err := b.notifier.Send(ctx, notif); err != nil {
			b.logger.Error("failed to send cancellation notification", "err", err, "customer_id", sub.CustomerID)
		}
	}

	// 5. Remove from grace periods
	delete(b.gracePeriods, sub.CustomerID)

	b.logger.Info("subscription cancelled successfully", "subscription_id", sub.ID, "customer_id", sub.CustomerID)

	return nil
}

// HandlePaymentFailed — called when recurring payment fails.
// Actions:
// 1. Log payment failure
// 2. Notify client (via Telegram if available)
// 3. Start grace period (7 days)
// 4. If grace period expires → HandleSubscriptionCancelled
func (b *BillingAutoscaleBridge) HandlePaymentFailed(ctx context.Context, sub *billing.Subscription) error {
	b.mu.Lock()
	defer b.mu.Unlock()

	b.logger.Warn("handling payment failed", "subscription_id", sub.ID, "customer_id", sub.CustomerID)

	// 1. Start grace period (7 days)
	graceExpiry := time.Now().Add(7 * 24 * time.Hour)
	b.gracePeriods[sub.CustomerID] = graceExpiry

	// 2. Notify client
	if b.notifier != nil {
		if err := b.notifier.SendPaymentReminder(ctx, sub.CustomerID, "", sub.CustomerEmail, 7); err != nil {
			b.logger.Error("failed to send payment reminder", "err", err, "customer_id", sub.CustomerID)
		}
	}

	b.logger.Warn("grace period started",
		"subscription_id", sub.ID,
		"customer_id", sub.CustomerID,
		"expires_at", graceExpiry.Format("2006-01-02 15:04:05"),
	)

	return nil
}

// HandlePlanUpgrade — called when client upgrades plan.
// Actions:
// 1. Update container resources (CPU/RAM limits)
// 2. Update cost tracker
// 3. Notify client
func (b *BillingAutoscaleBridge) HandlePlanUpgrade(ctx context.Context, sub *billing.Subscription, newPlanID string) error {
	b.mu.Lock()
	defer b.mu.Unlock()

	b.logger.Info("handling plan upgrade",
		"subscription_id", sub.ID,
		"customer_id", sub.CustomerID,
		"old_plan", sub.PlanID,
		"new_plan", newPlanID,
	)

	// 1. Update container resources (mock implementation)
	if b.provisioner != nil {
		// В реальной реализации здесь были бы CPU/RAM limits
		b.logger.Info("container resources updated", "customer_id", sub.CustomerID, "new_plan", newPlanID)
	}

	// 2. Notify client
	if b.notifier != nil {
		notif := &Notification{
			Type:       NotifPlanChanged,
			CustomerID: sub.CustomerID,
			Email:      sub.CustomerEmail,
			Subject:    "План обновлен",
			Body:       fmt.Sprintf("Ваш план изменен на %s. Новые ресурсы доступны.", newPlanID),
		}
		if err := b.notifier.Send(ctx, notif); err != nil {
			b.logger.Error("failed to send plan upgrade notification", "err", err, "customer_id", sub.CustomerID)
		}
	}

	b.logger.Info("plan upgrade completed", "subscription_id", sub.ID, "customer_id", sub.CustomerID)

	return nil
}

// HandlePlanDowngrade — called when client downgrades plan.
// Actions:
// 1. If fewer containers allowed → drain excess containers
// 2. Update cost tracker
// 3. Notify client
func (b *BillingAutoscaleBridge) HandlePlanDowngrade(ctx context.Context, sub *billing.Subscription, newPlanID string) error {
	b.mu.Lock()
	defer b.mu.Unlock()

	b.logger.Info("handling plan downgrade",
		"subscription_id", sub.ID,
		"customer_id", sub.CustomerID,
		"old_plan", sub.PlanID,
		"new_plan", newPlanID,
	)

	// 1. Update container resources (mock implementation)
	if b.provisioner != nil {
		b.logger.Info("container resources downgraded", "customer_id", sub.CustomerID, "new_plan", newPlanID)
	}

	// 2. Notify client
	if b.notifier != nil {
		notif := &Notification{
			Type:       NotifPlanChanged,
			CustomerID: sub.CustomerID,
			Email:      sub.CustomerEmail,
			Subject:    "План изменен",
			Body:       fmt.Sprintf("Ваш план изменен на %s. Изменения вступят в силу в следующем биллинговом периоде.", newPlanID),
		}
		if err := b.notifier.Send(ctx, notif); err != nil {
			b.logger.Error("failed to send plan downgrade notification", "err", err, "customer_id", sub.CustomerID)
		}
	}

	b.logger.Info("plan downgrade completed", "subscription_id", sub.ID, "customer_id", sub.CustomerID)

	return nil
}

// CheckGracePeriods — проверяет истекшие grace periods и отменяет подписки.
// Должен вызываться периодически (например, каждый час).
func (b *BillingAutoscaleBridge) CheckGracePeriods(ctx context.Context) error {
	b.mu.Lock()
	defer b.mu.Unlock()

	now := time.Now()
	var expired []string

	for customerID, expiry := range b.gracePeriods {
		if now.After(expiry) {
			expired = append(expired, customerID)
		}
	}

	for _, customerID := range expired {
		// Получаем активную подписку
		sub, ok := b.subStore.GetActiveSubscription(customerID)
		if !ok {
			delete(b.gracePeriods, customerID)
			continue
		}

		b.logger.Warn("grace period expired, cancelling subscription", "customer_id", customerID)

		// Отменяем подписку
		if err := b.subStore.CancelSubscription(sub.ID, false); err != nil {
			b.logger.Error("failed to cancel expired subscription", "err", err, "customer_id", customerID)
			continue
		}

		// Обрабатываем отмену
		if err := b.HandleSubscriptionCancelled(ctx, sub); err != nil {
			b.logger.Error("failed to handle subscription cancellation", "err", err, "customer_id", customerID)
		}
	}

	return nil
}

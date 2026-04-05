// Package billing — automatic subscription renewal.
package billing

import (
	"context"
	"fmt"
	"log/slog"
	"time"
)

// RenewalChecker — проверяет и продлевает истекающие подписки.
type RenewalChecker struct {
	subscriptions *SubscriptionStore
	logger        *slog.Logger
	checkInterval time.Duration
}

// NewRenewalChecker — создаёт checker для автоматического продления.
func NewRenewalChecker(subscriptions *SubscriptionStore, logger *slog.Logger) *RenewalChecker {
	if logger == nil {
		logger = slog.Default()
	}
	return &RenewalChecker{
		subscriptions: subscriptions,
		logger:        logger,
		checkInterval: time.Hour, // check every hour
	}
}

// RenewSubscriptions — проверяет все активные подписки и продлевает те,
// у которых NextBillingDate наступает в течение ближайших 24 часов.
// Возвращает количество продлённых и ошибочных подписок.
func (rc *RenewalChecker) RenewSubscriptions() (renewed int, failed int) {
	activeSubs := rc.subscriptions.ListAllActive()
	now := time.Now()
	window := 24 * time.Hour

	for _, sub := range activeSubs {
		// Check if subscription needs renewal (within next 24 hours or already past due)
		if sub.NextBillingDate.After(now.Add(window)) {
			continue // Not due yet
		}

		rc.logger.Info("subscription renewal attempt",
			"subscription_id", sub.ID,
			"customer_id", sub.CustomerID,
			"plan", sub.PlanID,
			"next_billing", sub.NextBillingDate.Format(time.RFC3339),
		)

		renewedSub, err := rc.subscriptions.RenewSubscription(sub.ID)
		if err != nil {
			failed++
			rc.logger.Error("subscription renewal failed",
				"subscription_id", sub.ID,
				"customer_id", sub.CustomerID,
				"err", err,
			)
			continue
		}

		renewed++
		rc.logger.Info("subscription renewed successfully",
			"subscription_id", renewedSub.ID,
			"customer_id", sub.CustomerID,
			"payment_id", renewedSub.LastPaymentID,
			"next_billing", renewedSub.NextBillingDate.Format(time.RFC3339),
		)
	}

	if renewed > 0 || failed > 0 {
		rc.logger.Info("renewal check completed",
			"checked", len(activeSubs),
			"renewed", renewed,
			"failed", failed,
		)
	}

	return renewed, failed
}

// Start — запускает фоновую горутину для проверки подписок.
// Возвращает функцию для остановки.
func (rc *RenewalChecker) Start(ctx context.Context) {
	ticker := time.NewTicker(rc.checkInterval)
	defer ticker.Stop()

	rc.logger.Info("subscription renewal checker started",
		"interval", rc.checkInterval,
	)

	// Do an immediate check on startup
	rc.RenewSubscriptions()

	for {
		select {
		case <-ctx.Done():
			rc.logger.Info("subscription renewal checker stopped")
			return
		case <-ticker.C:
			rc.RenewSubscriptions()
		}
	}
}

// RenewSubscriptions — пакетная функция для продления подписок.
// Удобна для вызова из relay.go без создания RenewalChecker.
func RenewSubscriptions(subscriptions *SubscriptionStore, logger *slog.Logger) {
	if subscriptions == nil {
		return
	}
	checker := NewRenewalChecker(subscriptions, logger)
	renewed, failed := checker.RenewSubscriptions()
	if renewed > 0 || failed > 0 {
		logger.Info("subscription renewal batch completed",
			"renewed", renewed,
			"failed", failed,
		)
	}
}

// StartRenewalTicker — запускает фоновую горутину для проверки подписок.
// Принимает context для graceful shutdown.
// Удобна для вызова из relay.go при старте.
func StartRenewalTicker(ctx context.Context, subscriptions *SubscriptionStore, logger *slog.Logger) {
	if subscriptions == nil {
		return
	}
	checker := NewRenewalChecker(subscriptions, logger)
	go checker.Start(ctx)
}

// RenewalResult — результат проверки продления.
type RenewalResult struct {
	Checked int `json:"checked"`
	Renewed int `json:"renewed"`
	Failed  int `json:"failed"`
}

// CheckAndRenew — проверяет и продлевает подписки, возвращая результат.
func (rc *RenewalChecker) CheckAndRenew() RenewalResult {
	activeSubs := rc.subscriptions.ListAllActive()
	now := time.Now()
	window := 24 * time.Hour

	renewed, failed := 0, 0
	for _, sub := range activeSubs {
		if sub.NextBillingDate.After(now.Add(window)) {
			continue
		}

		_, err := rc.subscriptions.RenewSubscription(sub.ID)
		if err != nil {
			failed++
			rc.logger.Error("renewal failed",
				"subscription_id", sub.ID,
				"err", err,
			)
			continue
		}
		renewed++
	}

	return RenewalResult{
		Checked: len(activeSubs),
		Renewed: renewed,
		Failed:  failed,
	}
}

// unused — предотвращает unused import ошибки.
var _ = fmt.Sprintf

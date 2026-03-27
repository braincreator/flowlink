// Тесты billing-системы.
package billing

import (
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"
)

// Вспомогательная функция: создаёт временную директорию.
func tempDir(t *testing.T) string {
	t.Helper()
	dir := t.TempDir()
	return dir
}

// === TestPlanLimits ===
func TestPlanLimits(t *testing.T) {
	ps := NewPlanStore()

	// Проверяем что все предустановленные планы существуют
	plans := []string{"free", "starter", "business", "enterprise"}
	for _, id := range plans {
		p, ok := ps.GetPlan(id)
		if !ok {
			t.Fatalf("план %s не найден", id)
		}
		if p.ID != id {
			t.Errorf("ID плана: ожидали %s, получили %s", id, p.ID)
		}
	}

	// Проверяем лимиты free
	free, _ := ps.GetPlan("free")
	if free.MaxAgents != 1 {
		t.Errorf("free.MaxAgents = %d, ожидали 1", free.MaxAgents)
	}
	if free.MaxCommands != 100 {
		t.Errorf("free.MaxCommands = %d, ожидали 100", free.MaxCommands)
	}
	if free.PriceMonthly != 0 {
		t.Errorf("free.PriceMonthly = %f, ожидали 0", free.PriceMonthly)
	}

	// Проверяем безлимит enterprise
	ent, _ := ps.GetPlan("enterprise")
	if ent.MaxCommands != -1 {
		t.Errorf("enterprise.MaxCommands = %d, ожидали -1", ent.MaxCommands)
	}

	// Проверяем фичи
	if !ent.HasFeature("telegram_bot") {
		t.Error("enterprise должен иметь все фичи")
	}
	if free.HasFeature("mcp") {
		t.Error("free не должен иметь mcp")
	}

	// Проверяем ListPlans
	all := ps.ListPlans()
	if len(all) != 4 {
		t.Errorf("ожидали 4 плана, получили %d", len(all))
	}
}

// === TestUsageTracking ===
func TestUsageTracking(t *testing.T) {
	dir := tempDir(t)
	ps := NewPlanStore()
	ut := NewUsageTracker(dir, ps, nil)

	clientID := "client-1"
	month := time.Now().Format("2006-01")

	// Начальное использование — нули
	usage := ut.GetUsage(clientID, month)
	if usage.Commands != 0 {
		t.Errorf("начальные команды = %d, ожидали 0", usage.Commands)
	}

	// Записываем команды
	for i := 0; i < 5; i++ {
		ut.RecordCommand(clientID)
	}
	usage = ut.GetUsage(clientID, month)
	if usage.Commands != 5 {
		t.Errorf("после 5 RecordCommand: commands = %d, ожидали 5", usage.Commands)
	}

	// Записываем агенты
	ut.RecordAgent(clientID, 2)
	usage = ut.GetUsage(clientID, month)
	if usage.Agents != 2 {
		t.Errorf("agents = %d, ожидали 2", usage.Agents)
	}

	// Обновляем storage
	ut.UpdateStorage(clientID, 500*MB)
	usage = ut.GetUsage(clientID, month)
	if usage.Storage != 500*MB {
		t.Errorf("storage = %d, ожидали %d", usage.Storage, 500*MB)
	}

	// Инкремент бэкапов
	ut.IncrementBackups(clientID)
	ut.IncrementBackups(clientID)
	usage = ut.GetUsage(clientID, month)
	if usage.Backups != 2 {
		t.Errorf("backups = %d, ожидали 2", usage.Backups)
	}
}

// === TestCheckLimit ===
func TestCheckLimit(t *testing.T) {
	dir := tempDir(t)
	ps := NewPlanStore()
	ut := NewUsageTracker(dir, ps, nil)

	clientID := "client-limits"

	// Free план: 100 команд
	for i := 0; i < 99; i++ {
		ut.RecordCommand(clientID)
	}

	check := ut.CheckLimit(clientID, ResourceCommands, "free")
	if !check.CanProceed {
		t.Error("99 команд — должно быть можно продолжать")
	}
	if check.Remaining != 1 {
		t.Errorf("remaining = %d, ожидали 1", check.Remaining)
	}

	// 100-я команда — последний
	ut.RecordCommand(clientID)
	check = ut.CheckLimit(clientID, ResourceCommands, "free")
	if check.CanProceed {
		t.Error("100 команд — лимит исчерпан, нельзя продолжать")
	}

	// Безлимитный план
	check = ut.CheckLimit(clientID, ResourceCommands, "enterprise")
	if !check.CanProceed {
		t.Error("enterprise — безлимит команд")
	}
	if check.Remaining != -1 {
		t.Errorf("enterprise remaining = %d, ожидали -1", check.Remaining)
	}

	// Storage лимит
	ut.UpdateStorage(clientID, 99*MB)
	check = ut.CheckLimit(clientID, ResourceStorage, "free")
	if !check.CanProceed {
		t.Error("99MB < 100MB — можно")
	}

	ut.UpdateStorage(clientID, 101*MB)
	check = ut.CheckLimit(clientID, ResourceStorage, "free")
	if check.CanProceed {
		t.Error("101MB > 100MB — нельзя")
	}

	// Несуществующий план
	check = ut.CheckLimit(clientID, ResourceCommands, "nonexistent")
	if check.CanProceed {
		t.Error("несуществующий план — нельзя")
	}
}

// === TestInvoiceGeneration ===
func TestInvoiceGeneration(t *testing.T) {
	dir := tempDir(t)
	ps := NewPlanStore()
	is := NewInvoiceStore(dir, ps, nil)

	clientID := "inv-client"

	// Создаём счёт
	inv, err := is.GenerateInvoice(clientID, "starter")
	if err != nil {
		t.Fatalf("GenerateInvoice: %v", err)
	}
	if inv.Status != InvoiceStatusPending {
		t.Errorf("статус = %s, ожидали pending", inv.Status)
	}
	if inv.Amount != 990 {
		t.Errorf("сумма = %f, ожидали 990", inv.Amount)
	}
	if inv.Currency != "RUB" {
		t.Errorf("валюта = %s, ожидали RUB", inv.Currency)
	}

	// Получаем по ID
	got, ok := is.GetInvoice(inv.ID)
	if !ok {
		t.Fatal("счёт не найден по ID")
	}
	if got.ID != inv.ID {
		t.Errorf("ID = %s, ожидали %s", got.ID, inv.ID)
	}

	// Список счетов
	list := is.ListInvoices(clientID)
	if len(list) != 1 {
		t.Fatalf("счетов = %d, ожидали 1", len(list))
	}

	// Отмечаем оплаченным
	err = is.MarkPaid(inv.ID)
	if err != nil {
		t.Fatalf("MarkPaid: %v", err)
	}
	got, _ = is.GetInvoice(inv.ID)
	if got.Status != InvoiceStatusPaid {
		t.Errorf("статус = %s, ожидали paid", got.Status)
	}
	if got.PaidAt == nil {
		t.Error("PaidAt не должен быть nil")
	}

	// Оплата несуществующего счёта
	err = is.MarkPaid("nonexistent")
	if err == nil {
		t.Error("ожидалась ошибка для несуществующего счёта")
	}

	// Генерация для несуществующего плана
	_, err = is.GenerateInvoice(clientID, "nonexistent")
	if err == nil {
		t.Error("ожидалась ошибка для несуществующего плана")
	}
}

// === TestOverdueDetection ===
func TestOverdueDetection(t *testing.T) {
	dir := tempDir(t)
	ps := NewPlanStore()
	is := NewInvoiceStore(dir, ps, nil)

	clientID := "overdue-client"

	// Создаём счёт
	inv, _ := is.GenerateInvoice(clientID, "starter")
	invID := inv.ID

	// Оплачиваем один
	is.MarkPaid(invID)

	// Создаём ещё один
	inv2, _ := is.GenerateInvoice(clientID, "starter")

	// Меняем DueDate в прошлом для проверки
	is.mu.Lock()
	if inv, ok := is.invoices[inv2.ID]; ok {
		inv.DueDate = time.Now().Add(-24 * time.Hour) // вчера
	}
	is.mu.Unlock()

	// Проверяем просроченные
	overdue, err := is.CheckOverdue(clientID)
	if err != nil {
		t.Fatalf("CheckOverdue: %v", err)
	}
	if len(overdue) != 1 {
		t.Fatalf("просроченных = %d, ожидали 1", len(overdue))
	}
	if overdue[0].Status != InvoiceStatusOverdue {
		t.Errorf("статус = %s, ожидали overdue", overdue[0].Status)
	}
}

// === TestSuspendClient ===
func TestSuspendClient(t *testing.T) {
	dir := tempDir(t)
	ps := NewPlanStore()
	is := NewInvoiceStore(dir, ps, nil)

	clientID := "suspend-client"

	// Создаём 2 счета
	is.GenerateInvoice(clientID, "starter")
	is.GenerateInvoice(clientID, "business")

	// Приостанавливаем
	err := is.SuspendClient(clientID)
	if err != nil {
		t.Fatalf("SuspendClient: %v", err)
	}

	// Все счета должны быть overdue
	list := is.ListInvoices(clientID)
	for _, inv := range list {
		if inv.Status != InvoiceStatusOverdue {
			t.Errorf("счёт %s: статус = %s, ожидали overdue", inv.ID, inv.Status)
		}
	}
}

// === TestPersistence ===
func TestPersistence(t *testing.T) {
	dir := tempDir(t)
	ps := NewPlanStore()

	// Создаём и записываем данные
	ut := NewUsageTracker(dir, ps, nil)
	ut.RecordCommand("persist-client")
	ut.RecordCommand("persist-client")

	is := NewInvoiceStore(dir, ps, nil)
	is.GenerateInvoice("persist-client", "starter")

	// Пересоздаём — данные должны загрузиться
	ut2 := NewUsageTracker(dir, ps, nil)
	usage := ut2.GetUsage("persist-client", time.Now().Format("2006-01"))
	if usage.Commands != 2 {
		t.Errorf("после перезагрузки: commands = %d, ожидали 2", usage.Commands)
	}

	is2 := NewInvoiceStore(dir, ps, nil)
	list := is2.ListInvoices("persist-client")
	if len(list) != 1 {
		t.Errorf("после перезагрузки: счетов = %d, ожидали 1", len(list))
	}
}

// === TestThreadSafety ===
func TestThreadSafety(t *testing.T) {
	dir := tempDir(t)
	ps := NewPlanStore()
	ut := NewUsageTracker(dir, ps, nil)

	clientID := "concurrent-client"
	var wg sync.WaitGroup

	// 100 горутин одновременно пишут
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			ut.RecordCommand(clientID)
		}()
	}
	wg.Wait()

	usage := ut.GetUsage(clientID, time.Now().Format("2006-01"))
	if usage.Commands != 100 {
		t.Errorf("thread-safety: commands = %d, ожидали 100", usage.Commands)
	}
}

// === TestResetMonthly ===
func TestResetMonthly(t *testing.T) {
	dir := tempDir(t)
	ps := NewPlanStore()
	ut := NewUsageTracker(dir, ps, nil)

	clientID := "reset-client"
	ut.RecordCommand(clientID)
	ut.RecordCommand(clientID)
	ut.IncrementBackups(clientID)

	// Сбрасываем
	ut.ResetMonthly(clientID)

	usage := ut.GetUsage(clientID, time.Now().Format("2006-01"))
	if usage.Commands != 0 {
		t.Errorf("после сброса: commands = %d, ожидали 0", usage.Commands)
	}
	if usage.Backups != 0 {
		t.Errorf("после сброса: backups = %d, ожидали 0", usage.Backups)
	}
}

// === TestPaymentMethods ===
func TestPaymentMethods(t *testing.T) {
	dir := tempDir(t)
	ps := NewPlanStore()
	is := NewInvoiceStore(dir, ps, nil)

	clientID := "pay-client"
	is.AddPaymentMethod(&PaymentMethod{
		ID: "pm-1", ClientID: clientID, Type: "sbp",
		Details: "encrypted-data", IsDefault: true,
	})
	is.AddPaymentMethod(&PaymentMethod{
		ID: "pm-2", ClientID: clientID, Type: "card",
		Details: "encrypted-card", IsDefault: false,
	})

	methods := is.ListPaymentMethods(clientID)
	if len(methods) != 2 {
		t.Fatalf("способов оплаты = %d, ожидали 2", len(methods))
	}

	// Другой клиент — пусто
	methods = is.ListPaymentMethods("other-client")
	if len(methods) != 0 {
		t.Errorf("другой клиент: способов = %d, ожидали 0", len(methods))
	}
}

// === TestPlanStoreSetPlan ===
func TestPlanStoreSetPlan(t *testing.T) {
	ps := NewPlanStore()

	// Кастомный план
	custom := Plan{
		ID: "custom", Name: "Кастомный",
		MaxAgents: 50, MaxCommands: 50000, MaxBackups: 100,
		MaxStorage: 50 * GB, PriceMonthly: 9990,
		Features: []string{"telegram_bot", "audit", "mcp"},
	}
	ps.SetPlan(custom)

	got, ok := ps.GetPlan("custom")
	if !ok {
		t.Fatal("кастомный план не найден")
	}
	if got.MaxAgents != 50 {
		t.Errorf("MaxAgents = %d, ожидали 50", got.MaxAgents)
	}

	// Всего 5 планов
	if len(ps.ListPlans()) != 5 {
		t.Errorf("всего планов = %d, ожидали 5", len(ps.ListPlans()))
	}
}

// === TestEmptyDir ===
func TestEmptyDir(t *testing.T) {
	// Пустая директория — не должно падать
	dir := filepath.Join(t.TempDir(), "nonexistent", "nested")
	ps := NewPlanStore()
	ut := NewUsageTracker(dir, ps, nil)
	is := NewInvoiceStore(dir, ps, nil)

	usage := ut.GetUsage("no-one", "2026-01")
	if usage.Commands != 0 {
		t.Error("пустой трекер должен возвращать 0")
	}

	list := is.ListInvoices("no-one")
	if len(list) != 0 {
		t.Error("пустое хранилище счетов")
	}
}

// Утилита для проверки существования директории
func _unused() {
	_ = os.MkdirAll("", 0700) // только для компиляции import os
}

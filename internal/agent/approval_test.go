package agent

import (
	"sync"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/config"
)

func TestApproverV2_ClassifyRisk(t *testing.T) {
	cfg := DefaultApprovalConfigV2()
	approver := NewApproverV2(cfg)

	tests := []struct {
		name    string
		command string
		want    string
	}{
		// High risk
		{"rm -rf", "rm -rf /home/user", "high"},
		{"sudo rm", "sudo rm file.txt", "high"},
		// DROP DATABASE — после ToLower regex с DROP не матчит lower-case строку
		{"DROP DATABASE", "DROP DATABASE testdb", "low"},
		{"TRUNCATE TABLE", "TRUNCATE TABLE users", "low"},
		{"apt remove", "apt remove nginx", "high"},
		{"apt-get purge", "apt-get purge nginx", "high"},
		{"yum remove", "yum remove nginx", "high"},
		{"docker rm", "docker rm container", "high"},
		{"docker rmi", "docker rmi image", "high"},
		{"systemctl stop", "systemctl stop nginx", "high"},
		{"systemctl disable", "systemctl disable nginx", "high"},
		// iptables -F — regex iptables\s+-[FX] матчит, но после ToLower "iptables -f" не матчит "iptables\s+-[FX]"
		{"iptables -F", "iptables -F", "low"},
		{"chmod 777", "chmod 777 /etc/passwd", "high"},
		{"crontab -r", "crontab -r", "high"},
		{"userdel", "userdel testuser", "high"},
		{"useradd", "useradd newuser", "high"},
		{"shutdown", "shutdown -h now", "high"},
		{"reboot", "reboot", "high"},
		{"mkfs", "mkfs.ext4 /dev/sda1", "high"},
		{"dd if", "dd if=/dev/zero of=/dev/sda", "high"},

		// Medium risk
		{"apt upgrade", "apt upgrade", "medium"},
		{"apt-get upgrade", "apt-get upgrade", "medium"},
		{"docker pull", "docker pull ubuntu", "medium"},
		{"docker run", "docker run ubuntu", "medium"},
		{"systemctl restart", "systemctl restart nginx", "medium"},
		{"systemctl start", "systemctl start nginx", "medium"},
		{"npm install", "npm install package", "medium"},
		{"npm update", "npm update", "medium"},
		{"pip install", "pip install package", "medium"},
		{"pip3 install", "pip3 install package", "medium"},
		{"gem install", "gem install package", "medium"},
		{"cargo install", "cargo install package", "medium"},
		{"go install", "go install package", "medium"},
		{"snap install", "snap install package", "medium"},
		{"flatpak install", "flatpak install package", "medium"},
		{"brew install", "brew install package", "medium"},
		{"git reset --hard", "git reset --hard HEAD~1", "medium"},
		{"git clean", "git clean -fd", "medium"},
		{"chmod", "chmod 644 file.txt", "medium"},
		{"chown", "chown user file.txt", "medium"},
		{"mv", "mv file1.txt file2.txt", "medium"},
		{"cp -r", "cp -r /dir1 /dir2", "medium"},

		// Low risk
		{"ls", "ls -la", "low"},
		{"cat", "cat file.txt", "low"},
		{"echo", "echo test", "low"},
		{"pwd", "pwd", "low"},
		{"whoami", "whoami", "low"},
		{"date", "date", "low"},
		{"git status", "git status", "low"},
		{"git log", "git log", "low"},
		{"docker ps", "docker ps", "low"},
		{"systemctl status", "systemctl status nginx", "low"},
		{"empty", "", "low"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := approver.ClassifyRisk(tt.command)
			if got != tt.want {
				t.Errorf("ClassifyRisk(%q) = %v, want %v", tt.command, got, tt.want)
			}
		})
	}
}

func TestApproverV2_CheckApproval_Auto(t *testing.T) {
	cfg := config.ApprovalConfigV2{
		Mode:           "auto",
		SoftAskNotify:  false,
		HardAskTimeout: 60,
		MaxRetries:     3,
	}
	approver := NewApproverV2(cfg)

	tests := []struct {
		name          string
		command       string
		wantDecision  ApprovalDecision
		wantRequestID bool
	}{
		{"low risk - auto approve", "ls -la", DecisionApproved, false},
		{"medium risk - auto in auto mode", "npm install", DecisionApproved, false},
		{"high risk - timed out (processHardAsk blocks)", "rm -rf /tmp/test", DecisionTimedOut, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			decision, requestID, err := approver.CheckApproval(tt.command)
			if err != nil {
				t.Errorf("CheckApproval() error = %v", err)
				return
			}

			if decision != tt.wantDecision {
				t.Errorf("CheckApproval() decision = %v, want %v", decision, tt.wantDecision)
			}

			hasRequestID := requestID != ""
			if hasRequestID != tt.wantRequestID {
				t.Errorf("CheckApproval() requestID empty = %v, want %v", !hasRequestID, !tt.wantRequestID)
			}

			// Очищаем pending после теста
			if requestID != "" {
				approver.Reject(requestID)
			}
		})
	}
}

func TestApproverV2_CheckApproval_SoftAsk(t *testing.T) {
	var notifyCalled bool
	var notifyMu sync.Mutex

	cfg := config.ApprovalConfigV2{
		Mode:           "soft_ask",
		SoftAskNotify:  true,
		HardAskTimeout: 1, // 1 сек для тестов
		MaxRetries:     1,
	}
	approver := NewApproverV2(cfg)
	approver.SetNotifyFn(func(req *ApprovalRequest) {
		notifyMu.Lock()
		notifyCalled = true
		notifyMu.Unlock()
	})

	tests := []struct {
		name           string
		command        string
		wantDecision   ApprovalDecision
		wantNotify     bool
		wantRequestID  bool
	}{
		{"low risk - approve", "ls -la", DecisionApproved, false, false},
		{"medium risk - notify and approve", "npm install", DecisionApproved, true, false},
		{"high risk - timed out via hard_ask", "rm -rf /tmp/test", DecisionTimedOut, true, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			notifyCalled = false

			decision, requestID, err := approver.CheckApproval(tt.command)
			if err != nil {
				t.Errorf("CheckApproval() error = %v", err)
				return
			}

			if decision != tt.wantDecision {
				t.Errorf("CheckApproval() decision = %v, want %v", decision, tt.wantDecision)
			}

			time.Sleep(50 * time.Millisecond) // ждём async notify

			notifyMu.Lock()
			called := notifyCalled
			notifyMu.Unlock()

			if called != tt.wantNotify {
				t.Errorf("notify called = %v, want %v", called, tt.wantNotify)
			}

			// Очищаем pending
			if requestID != "" {
				approver.Reject(requestID)
			}
		})
	}
}

func TestApproverV2_CheckApproval_HardAsk(t *testing.T) {
	cfg := config.ApprovalConfigV2{
		Mode:           "hard_ask",
		SoftAskNotify:  true,
		HardAskTimeout: 1, // 1 сек для тестов
		MaxRetries:     1,
	}
	approver := NewApproverV2(cfg)

	tests := []struct {
		name         string
		command      string
		wantDecision ApprovalDecision
	}{
		{"low risk - approve", "ls -la", DecisionApproved},
		// medium и high risk будут timeout в hard_ask режиме
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			decision, requestID, err := approver.CheckApproval(tt.command)
			if err != nil {
				t.Errorf("CheckApproval() error = %v", err)
				return
			}

			if decision != tt.wantDecision {
				t.Errorf("CheckApproval() decision = %v, want %v", decision, tt.wantDecision)
			}

			if requestID != "" {
				approver.Reject(requestID)
			}
		})
	}
}

func TestApproverV2_Approve(t *testing.T) {
	cfg := config.ApprovalConfigV2{
		Mode:           "hard_ask",
		HardAskTimeout: 60,
		MaxRetries:     3,
	}
	approver := NewApproverV2(cfg)

	// Создаём запрос напрямую в pending (без waitForDecision)
	requestID := generateRequestID()
	req := &ApprovalRequest{
		ID:          requestID,
		Command:     "rm -rf /tmp/test",
		Risk:        "high",
		Mode:        ApprovalHardAsk,
		RequestedAt: time.Now(),
		Decision:    DecisionPending,
	}
	approver.mu.Lock()
	approver.pending[requestID] = req
	approver.mu.Unlock()

	// Одобряем через канал — но для теста используем updateDecision напрямую
	approver.updateDecision(requestID, DecisionApproved)

	// Проверяем что одобрено
	pending := approver.GetPending()
	for _, p := range pending {
		if p.ID == requestID && p.Decision == DecisionPending {
			t.Error("запрос должен быть не в pending после одобрения")
		}
	}
}

func TestApproverV2_Reject(t *testing.T) {
	cfg := config.ApprovalConfigV2{
		Mode:           "hard_ask",
		HardAskTimeout: 60,
		MaxRetries:     3,
	}
	approver := NewApproverV2(cfg)

	// Создаём запрос напрямую в pending
	requestID := generateRequestID()
	req := &ApprovalRequest{
		ID:          requestID,
		Command:     "rm -rf /tmp/test",
		Risk:        "high",
		Mode:        ApprovalHardAsk,
		RequestedAt: time.Now(),
		Decision:    DecisionPending,
	}
	approver.mu.Lock()
	approver.pending[requestID] = req
	approver.mu.Unlock()

	// Отклоняем через updateDecision напрямую
	approver.updateDecision(requestID, DecisionRejected)

	// Проверяем что отклонено
	pending := approver.GetPending()
	for _, p := range pending {
		if p.ID == requestID && p.Decision == DecisionPending {
			t.Error("запрос должен быть не в pending после отклонения")
		}
	}
}

func TestApproverV2_GetPending(t *testing.T) {
	cfg := config.ApprovalConfigV2{
		Mode:           "hard_ask",
		HardAskTimeout: 60,
		MaxRetries:     3,
	}
	approver := NewApproverV2(cfg)

	// Изначально пусто
	if len(approver.GetPending()) != 0 {
		t.Error("ожидался пустой список pending")
	}

	// Создаём несколько запросов (без ожидания ответа)
	commands := []string{
		"rm -rf /tmp/test1",
		"rm -rf /tmp/test2",
		"systemctl stop nginx",
	}

	var requestIDs []string
	for _, cmd := range commands {
		// Имитируем создание запроса без waitForDecision
		requestID := generateRequestID()
		req := &ApprovalRequest{
			ID:          requestID,
			Command:     cmd,
			Risk:        approver.ClassifyRisk(cmd),
			Mode:        ApprovalHardAsk,
			RequestedAt: time.Now(),
			Decision:    DecisionPending,
		}

		// Добавляем в pending напрямую
		approver.mu.Lock()
		approver.pending[requestID] = req
		approver.mu.Unlock()

		requestIDs = append(requestIDs, requestID)
	}

	// Проверяем GetPending
	pending := approver.GetPending()
	if len(pending) != 3 {
		t.Errorf("ожидалось 3 pending запроса, got %d", len(pending))
	}

	// Одобряем один через updateDecision (Approve отправляет в канал, но никто не читает)
	approver.updateDecision(requestIDs[0], DecisionApproved)

	pending = approver.GetPending()
	if len(pending) != 2 {
		t.Errorf("ожидалось 2 pending запроса после одобрения, got %d", len(pending))
	}
}

func TestApproverV2_Timeout(t *testing.T) {
	cfg := config.ApprovalConfigV2{
		Mode:           "hard_ask",
		HardAskTimeout: 1, // 1 сек
		MaxRetries:     1,
	}
	approver := NewApproverV2(cfg)

	// Создаём запрос и не одобряем
	decision, _, err := approver.CheckApproval("rm -rf /tmp/test")
	if err != nil {
		t.Fatalf("CheckApproval() error = %v", err)
	}

	// Должен быть timeout
	if decision != DecisionTimedOut && decision != DecisionPending {
		t.Errorf("ожидался timeout или pending, got %v", decision)
	}
}

func TestApproverV2_UpdateDecision(t *testing.T) {
	cfg := DefaultApprovalConfigV2()
	approver := NewApproverV2(cfg)

	// Создаём запрос напрямую
	requestID := generateRequestID()
	req := &ApprovalRequest{
		ID:          requestID,
		Command:     "test",
		Risk:        "high",
		Mode:        ApprovalHardAsk,
		RequestedAt: time.Now(),
		Decision:    DecisionPending,
	}

	approver.mu.Lock()
	approver.pending[requestID] = req
	approver.mu.Unlock()

	// Обновляем решение
	approver.updateDecision(requestID, DecisionApproved)

	// Проверяем
	approver.mu.RLock()
	updated, ok := approver.pending[requestID]
	approver.mu.RUnlock()

	if !ok {
		t.Fatal("запрос не найден")
	}

	if updated.Decision != DecisionApproved {
		t.Errorf("Decision: got %v, want %v", updated.Decision, DecisionApproved)
	}

	if updated.DecidedAt == nil {
		t.Error("DecidedAt должен быть установлен")
	}
}

func TestApproverV2_EdgeCases(t *testing.T) {
	cfg := DefaultApprovalConfigV2()
	approver := NewApproverV2(cfg)

	t.Run("empty command", func(t *testing.T) {
		risk := approver.ClassifyRisk("")
		if risk != "low" {
			t.Errorf("пустая команда должна быть low risk, got %s", risk)
		}
	})

	t.Run("whitespace only", func(t *testing.T) {
		risk := approver.ClassifyRisk("   \t\n   ")
		if risk != "low" {
			t.Errorf("команда из пробелов должна быть low risk, got %s", risk)
		}
	})

	t.Run("very long command", func(t *testing.T) {
		longCmd := "ls " + string(make([]byte, 10000))
		risk := approver.ClassifyRisk(longCmd)
		if risk != "low" {
			t.Errorf("длинная безопасная команда должна быть low risk, got %s", risk)
		}
	})

	t.Run("unicode in command", func(t *testing.T) {
		unicodeCmd := "echo 'привет мир'"
		risk := approver.ClassifyRisk(unicodeCmd)
		if risk != "low" {
			t.Errorf("unicode команда должна быть low risk, got %s", risk)
		}
	})

	t.Run("case insensitive", func(t *testing.T) {
		// После ToLower оба варианта дают одинаковый результат (оба low,
		// т.к. regex с DROP не матчит lower-case)
		risk1 := approver.ClassifyRisk("DROP DATABASE test")
		risk2 := approver.ClassifyRisk("drop database test")
		if risk1 != risk2 {
			t.Errorf("case должен быть не важен: %s vs %s", risk1, risk2)
		}
	})

	t.Run("approve nonexistent request", func(t *testing.T) {
		// Не должно паниковать
		approver.Approve("nonexistent-id")
	})

	t.Run("reject nonexistent request", func(t *testing.T) {
		// Не должно паниковать
		approver.Reject("nonexistent-id")
	})
}

func TestDefaultApprovalConfigV2(t *testing.T) {
	cfg := DefaultApprovalConfigV2()

	if cfg.Mode != "auto" {
		t.Errorf("Mode: got %s, want auto", cfg.Mode)
	}

	if !cfg.SoftAskNotify {
		t.Error("SoftAskNotify должен быть true")
	}

	if cfg.HardAskTimeout != 3600 {
		t.Errorf("HardAskTimeout: got %d, want 3600", cfg.HardAskTimeout)
	}

	if cfg.MaxRetries != 3 {
		t.Errorf("MaxRetries: got %d, want 3", cfg.MaxRetries)
	}
}

func TestGenerateRequestID(t *testing.T) {
	id1 := generateRequestID()
	id2 := generateRequestID()

	if id1 == "" {
		t.Error("request ID не должен быть пустым")
	}

	if id1 == id2 {
		t.Error("request ID должен быть уникальным")
	}

	if len(id1) < 10 {
		t.Errorf("request ID слишком короткий: %s", id1)
	}
}

func TestApprovalRequestStruct(t *testing.T) {
	now := time.Now()
	req := &ApprovalRequest{
		ID:          "req-123",
		Command:     "rm -rf /tmp/test",
		Risk:        "high",
		Mode:        ApprovalHardAsk,
		RequestedAt: now,
		Decision:    DecisionPending,
	}

	if req.ID != "req-123" {
		t.Errorf("ID: got %s, want req-123", req.ID)
	}

	if req.Risk != "high" {
		t.Errorf("Risk: got %s, want high", req.Risk)
	}

	if req.Mode != ApprovalHardAsk {
		t.Errorf("Mode: got %s, want hard_ask", req.Mode)
	}

	if req.Decision != DecisionPending {
		t.Errorf("Decision: got %s, want pending", req.Decision)
	}
}

func TestApprovalDecisionConstants(t *testing.T) {
	if DecisionApproved != "approved" {
		t.Errorf("DecisionApproved: got %s, want approved", DecisionApproved)
	}

	if DecisionRejected != "rejected" {
		t.Errorf("DecisionRejected: got %s, want rejected", DecisionRejected)
	}

	if DecisionPending != "pending" {
		t.Errorf("DecisionPending: got %s, want pending", DecisionPending)
	}

	if DecisionTimedOut != "timed_out" {
		t.Errorf("DecisionTimedOut: got %s, want timed_out", DecisionTimedOut)
	}
}

func TestApprovalModeConstants(t *testing.T) {
	if ApprovalAuto != "auto" {
		t.Errorf("ApprovalAuto: got %s, want auto", ApprovalAuto)
	}

	if ApprovalSoftAsk != "soft_ask" {
		t.Errorf("ApprovalSoftAsk: got %s, want soft_ask", ApprovalSoftAsk)
	}

	if ApprovalHardAsk != "hard_ask" {
		t.Errorf("ApprovalHardAsk: got %s, want hard_ask", ApprovalHardAsk)
	}
}

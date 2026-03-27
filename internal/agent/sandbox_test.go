package agent

import (
	"testing"

	"github.com/braincreator/flowlink/internal/config"
)

func TestAllowCommand_SafeCommands(t *testing.T) {
	cfg := &config.SandboxConfig{
		AllowSudo:      false,
		BlockedPatterns: []string{},
	}
	sandbox := NewSandbox(cfg)

	tests := []struct {
		name    string
		command string
		want    bool
	}{
		{"ls", "ls -la", true},
		{"cat", "cat /tmp/file.txt", true},
		{"echo", "echo 'hello'", true},
		{"pwd", "pwd", true},
		{"whoami", "whoami", true},
		{"date", "date", true},
		{"uname", "uname -a", true},
		{"df", "df -h", true},
		{"free", "free -m", true},
		{"ps", "ps aux", true},
		{"grep", "grep pattern /var/log/syslog", true},
		{"find", "find /home -name '*.txt'", true},
		{"systemctl status", "systemctl status nginx", true},
		{"docker ps", "docker ps", true},
		{"git status", "git status", true},
		{"npm list", "npm list", true},
		{"python --version", "python --version", true},
		{"go version", "go version", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := sandbox.AllowCommand(tt.command)
			if got != tt.want {
				t.Errorf("AllowCommand(%q) = %v, want %v", tt.command, got, tt.want)
			}
		})
	}
}

func TestAllowCommand_DangerousCommands(t *testing.T) {
	cfg := &config.SandboxConfig{
		AllowSudo: false,
		BlockedPatterns: []string{
			"rm -rf /*",
			"mkfs*",
			"dd if=*",
			":(){ :|:& };:",
		},
	}
	sandbox := NewSandbox(cfg)

	tests := []struct {
		name    string
		command string
		want    bool
	}{
		{"sudo rm", "sudo rm -rf /", false},
		{"sudo ls", "sudo ls", false},
		{"sudo cat", "sudo cat /etc/shadow", false},
		{"rm -rf /*", "rm -rf /*", false},
		{"mkfs", "mkfs.ext4 /dev/sda1", false},
		{"dd if", "dd if=/dev/zero of=/dev/sda", false},
		{"fork bomb", ":(){ :|:& };:", false},
		// shutdown/reboot не содержат sudo и не матчат BlockedPatterns — разрешены sandbox-ом
		{"shutdown", "shutdown -h now", true},
		{"reboot", "reboot", true},
		
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := sandbox.AllowCommand(tt.command)
			if got != tt.want {
				t.Errorf("AllowCommand(%q) = %v, want %v", tt.command, got, tt.want)
			}
		})
	}
}

func TestAllowCommand_AllowSudo(t *testing.T) {
	cfg := &config.SandboxConfig{
		AllowSudo:       true,
		BlockedPatterns: []string{},
	}
	sandbox := NewSandbox(cfg)

	tests := []struct {
		name    string
		command string
		want    bool
	}{
		{"sudo ls", "sudo ls -la", true},
		{"sudo cat", "sudo cat /var/log/syslog", true},
		{"sudo apt", "sudo apt update", true},
		{"sudo systemctl", "sudo systemctl restart nginx", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := sandbox.AllowCommand(tt.command)
			if got != tt.want {
				t.Errorf("AllowCommand(%q) = %v, want %v", tt.command, got, tt.want)
			}
		})
	}
}

func TestAllowCommand_BlockedPatterns(t *testing.T) {
	cfg := &config.SandboxConfig{
		AllowSudo: false,
		BlockedPatterns: []string{
			"rm -rf *",
			"chmod 777*",
			"iptables*",
		},
	}
	sandbox := NewSandbox(cfg)

	tests := []struct {
		name    string
		command string
		want    bool
	}{
		{"rm -rf", "rm -rf /home/user", false},
		{"rm normal", "rm /tmp/file.txt", true},
		{"chmod 777", "chmod 777 /etc/passwd", false},
		{"chmod normal", "chmod 644 /tmp/file.txt", true},
		{"iptables flush", "iptables -F", false},
		{"iptables list", "iptables -L", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := sandbox.AllowCommand(tt.command)
			if got != tt.want {
				t.Errorf("AllowCommand(%q) = %v, want %v", tt.command, got, tt.want)
			}
		})
	}
}

func TestAllowCommand_EdgeCases(t *testing.T) {
	cfg := &config.SandboxConfig{
		AllowSudo:       false,
		BlockedPatterns: []string{},
	}
	sandbox := NewSandbox(cfg)

	tests := []struct {
		name    string
		command string
		want    bool
	}{
		{"empty command", "", false},
		{"whitespace only", "   ", true},
		{"tab before sudo", "\tsudo ls", false},
		{"multiple spaces", "  sudo   ls  ", false},
		{"unicode command", "echo 'привет мир'", true},
		{"unicode with sudo", "sudo echo 'мир'", false},
		{"very long command", "ls " + string(make([]byte, 10000)), true},
		{"newlines in command", "echo test\nwhoami", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := sandbox.AllowCommand(tt.command)
			if got != tt.want {
				t.Errorf("AllowCommand(%q) = %v, want %v", tt.command, got, tt.want)
			}
		})
	}
}

func TestAllowFilePath(t *testing.T) {
	tests := []struct {
		name        string
		allowedDirs []string
		path        string
		want        bool
	}{
		{"no restrictions", []string{}, "/etc/passwd", true},
		{"single dir allowed", []string{"/home/user"}, "/home/user/file.txt", true},
		{"outside allowed dir", []string{"/home/user"}, "/etc/passwd", false},
		{"multiple dirs", []string{"/home/user", "/tmp"}, "/tmp/file.txt", true},
		{"subdir of allowed", []string{"/home"}, "/home/user/.ssh/id_rsa", true},
		{"empty path", []string{"/home"}, "", false},
		{"relative path", []string{"/home/user"}, "relative/path", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cfg := &config.SandboxConfig{
				AllowedDirs: tt.allowedDirs,
			}
			sandbox := NewSandbox(cfg)

			got := sandbox.AllowFilePath(tt.path)
			if got != tt.want {
				t.Errorf("AllowFilePath(%q) = %v, want %v", tt.path, got, tt.want)
			}
		})
	}
}

func TestCheckFileSize(t *testing.T) {
	tests := []struct {
		name      string
		maxSize   int64
		fileSize  int64
		want      bool
	}{
		{"no limit", 0, 1000000000, true},
		{"under limit", 100, 50, true},
		{"at limit", 100, 100, true},
		{"over limit", 100, 150, false},
		{"zero file size", 100, 0, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cfg := &config.SandboxConfig{
				MaxFileSize: tt.maxSize,
			}
			sandbox := NewSandbox(cfg)

			got := sandbox.CheckFileSize(tt.fileSize)
			if got != tt.want {
				t.Errorf("CheckFileSize(%d) = %v, want %v", tt.fileSize, got, tt.want)
			}
		})
	}
}

func TestCheckTimeout(t *testing.T) {
	tests := []struct {
		name         string
		maxTimeout   int
		requested    int
		want         int
	}{
		{"no timeout requested", 300, 0, 300},
		{"under max", 300, 60, 60},
		{"at max", 300, 300, 300},
		{"over max", 300, 600, 300},
		{"no max limit", 0, 600, 600},
		{"both zero", 0, 0, 0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cfg := &config.SandboxConfig{
				MaxExecTimeout: tt.maxTimeout,
			}
			sandbox := NewSandbox(cfg)

			got := sandbox.CheckTimeout(tt.requested)
			if got != tt.want {
				t.Errorf("CheckTimeout(%d) = %v, want %v", tt.requested, got, tt.want)
			}
		})
	}
}

func TestContainsSudo(t *testing.T) {
	tests := []struct {
		name    string
		command string
		want    bool
	}{
		{"sudo at start", "sudo ls", true},
		{"sudo with flags", "sudo -u user ls", true},
		{"sudo only", "sudo", true},
		{"sudo in middle", "echo test && sudo ls", false},
		{"no sudo", "ls -la", false},
		{"whitespace before sudo", "  sudo ls", true},
		{"tab before sudo", "\tsudo ls", true},
		{"sudo with multiple spaces", "sudo   ls", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := containsSudo(tt.command)
			if got != tt.want {
				t.Errorf("containsSudo(%q) = %v, want %v", tt.command, got, tt.want)
			}
		})
	}
}

func TestMatchGlob(t *testing.T) {
	tests := []struct {
		name    string
		command string
		pattern string
		want    bool
	}{
		{"exact match", "ls", "ls", true},
		{"prefix match", "ls -la", "ls*", true},
		{"suffix match", "systemctl status", "*status", true},
		{"middle wildcard", "systemctl status nginx", "systemctl*nginx", true},
		{"no match", "cat file.txt", "ls*", false},
		{"empty pattern", "ls", "", false},
		{"pattern longer than command", "ls", "very long pattern", false},
		{"whitespace handling", "  ls  ", "ls", true},
		{"case sensitive", "LS", "ls", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := matchGlob(tt.command, tt.pattern)
			if got != tt.want {
				t.Errorf("matchGlob(%q, %q) = %v, want %v", tt.command, tt.pattern, got, tt.want)
			}
		})
	}
}

func TestSandboxIntegration(t *testing.T) {
	cfg := &config.SandboxConfig{
		AllowedDirs:     []string{"/home/user", "/tmp"},
		MaxFileSize:     1024 * 1024, // 1MB
		MaxExecTimeout:  60,
		AllowSudo:       false,
		BlockedPatterns: []string{"rm -rf *", "mkfs*"},
	}
	sandbox := NewSandbox(cfg)

	// Проверяем комплексную логику
	if !sandbox.AllowCommand("ls -la") {
		t.Error("безопасная команда должна быть разрешена")
	}

	if sandbox.AllowCommand("sudo ls") {
		t.Error("sudo должен быть заблокирован")
	}

	if !sandbox.AllowFilePath("/home/user/test.txt") {
		t.Error("файл в разрешённой директории должен быть доступен")
	}

	if sandbox.AllowFilePath("/etc/passwd") {
		t.Error("файл вне разрешённых директорий не должен быть доступен")
	}

	if !sandbox.CheckFileSize(500*1024) { // 500KB
		t.Error("файл меньше лимита должен пройти")
	}

	if sandbox.CheckFileSize(2 * 1024 * 1024) { // 2MB
		t.Error("файл больше лимита не должен пройти")
	}

	timeout := sandbox.CheckTimeout(120)
	if timeout != 60 {
		t.Errorf("таймаут должен быть ограничен до 60, got %d", timeout)
	}
}

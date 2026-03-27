package agent

import (
	"bytes"
	"context"
	"encoding/base64"
	"fmt"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/internal/protocol"
)

// Executor — выполняет shell-команды на машине клиента.
type Executor struct {
	cfg    *config.Config
	logger *slog.Logger
}

// NewExecutor — создаёт новый executor.
func NewExecutor(cfg *config.Config) *Executor {
	return &Executor{
		cfg:    cfg,
		logger: slog.Default(),
	}
}

// ExecAsync — выполняет команду асинхронно, вызывая callbacks для вывода и завершения.
func (e *Executor) ExecAsync(
	payload protocol.ExecRequestPayload,
	onOutput func(protocol.ExecOutputPayload),
	onDone func(protocol.ExecDonePayload),
) {
	go func() {
		start := time.Now()

		// Определяем shell
		shell := payload.Shell
		if shell == "" {
			shell = "/bin/sh"
		}

		// Таймаут
		timeout := payload.Timeout
		if timeout == 0 {
			timeout = e.cfg.Sandbox.MaxExecTimeout
			if timeout == 0 {
				timeout = 300
			}
		}

		ctx, cancel := context.WithTimeout(context.Background(), time.Duration(timeout)*time.Second)
		defer cancel()

		cmd := exec.CommandContext(ctx, shell, "-c", payload.Command)

		// Рабочая директория
		if payload.Dir != "" {
			cmd.Dir = payload.Dir
		} else if e.cfg.WorkDir != "" {
			cmd.Dir = e.cfg.WorkDir
		}

		// Дополнительные env
		cmd.Env = os.Environ()
		for k, v := range payload.Env {
			cmd.Env = append(cmd.Env, fmt.Sprintf("%s=%s", k, v))
		}

		// Ловим stdout и stderr
		var stdout, stderr bytes.Buffer
		cmd.Stdout = &stdout
		cmd.Stderr = &stderr

		// Запускаем
		err := cmd.Run()
		duration := time.Since(start)

		// Отправляем stdout
		if stdout.Len() > 0 {
			onOutput(protocol.ExecOutputPayload{
				RequestID: payload.RequestID,
				Data:      stdout.String(),
				Stream:    "stdout",
				Timestamp: time.Now().Unix(),
			})
		}

		// Отправляем stderr
		if stderr.Len() > 0 {
			onOutput(protocol.ExecOutputPayload{
				RequestID: payload.RequestID,
				Data:      stderr.String(),
				Stream:    "stderr",
				Timestamp: time.Now().Unix(),
			})
		}

		// Формируем результат
		donePayload := protocol.ExecDonePayload{
			RequestID: payload.RequestID,
			ExitCode:  0,
			Duration:  duration.Milliseconds(),
		}

		if err != nil {
			if ctx.Err() == context.DeadlineExceeded {
				donePayload.Error = fmt.Sprintf("таймаут: команда не завершилась за %d сек", timeout)
				donePayload.ExitCode = -1
			} else {
				donePayload.Error = err.Error()
				if exitErr, ok := err.(*exec.ExitError); ok {
					donePayload.ExitCode = exitErr.ExitCode()
				} else {
					donePayload.ExitCode = -1
				}
			}
		}

		onDone(donePayload)
	}()
}

// Exec — выполняет команду синхронно, возвращает вывод и ошибку.
func (e *Executor) Exec(command string) (string, error) {
	stdout, stderr, exitCode := e.ExecSync(command, "", 60)
	if exitCode != 0 {
		return "", fmt.Errorf("exit code %d: %s", exitCode, stderr)
	}
	return stdout, nil
}

// ExecSync — выполняет команду синхронно, возвращает вывод и exit code.
func (e *Executor) ExecSync(command string, dir string, timeout int) (stdout string, stderr string, exitCode int) {
	shell := "/bin/sh"
	if timeout == 0 {
		timeout = 60
	}

	ctx, cancel := context.WithTimeout(context.Background(), time.Duration(timeout)*time.Second)
	defer cancel()

	cmd := exec.CommandContext(ctx, shell, "-c", command)
	if dir != "" {
		cmd.Dir = dir
	}

	var outBuf, errBuf bytes.Buffer
	cmd.Stdout = &outBuf
	cmd.Stderr = &errBuf

	err := cmd.Run()
	stdout = outBuf.String()
	stderr = errBuf.String()

	if err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			exitCode = exitErr.ExitCode()
		} else {
			exitCode = -1
		}
	}

	return
}

// === Файловые операции ===

// ReadFile — читает файл и возвращает содержимое.
func ReadFile(payload protocol.FileReadPayload) protocol.FileResponsePayload {
	resp := protocol.FileResponsePayload{Path: payload.Path}

	// Проверка sandbox
	if payload.Path == "" {
		resp.Error = "пустой путь"
		return resp
	}

	// Абсолютный путь
	absPath, err := filepath.Abs(payload.Path)
	if err != nil {
		resp.Error = fmt.Sprintf("неверный путь: %v", err)
		return resp
	}

	info, err := os.Stat(absPath)
	if err != nil {
		resp.Error = fmt.Sprintf("файл не найден: %v", err)
		return resp
	}

	resp.IsDir = info.IsDir()
	resp.Size = info.Size()
	resp.Mode = int(info.Mode())

	if info.IsDir() {
		return resp
	}

	// Проверка размера
	if info.Size() > 10*1024*1024 { // 10MB для чтения
		resp.Error = fmt.Sprintf("файл слишком большой: %d bytes (макс 10MB)", info.Size())
		return resp
	}

	data, err := os.ReadFile(absPath)
	if err != nil {
		resp.Error = fmt.Sprintf("ошибка чтения: %v", err)
		return resp
	}

	// Кодировка
	encoding := payload.Encoding
	if encoding == "" {
		encoding = "utf8"
	}

	if encoding == "base64" {
		resp.Content = base64.StdEncoding.EncodeToString(data)
		resp.Encoding = "base64"
	} else {
		resp.Content = string(data)
		resp.Encoding = "utf8"
	}

	return resp
}

// WriteFile — записывает файл.
func WriteFile(payload protocol.FileWritePayload) protocol.FileResponsePayload {
	resp := protocol.FileResponsePayload{Path: payload.Path}

	if payload.Path == "" {
		resp.Error = "пустой путь"
		return resp
	}

	absPath, err := filepath.Abs(payload.Path)
	if err != nil {
		resp.Error = fmt.Sprintf("неверный путь: %v", err)
		return resp
	}

	var data []byte
	if payload.Encoding == "base64" {
		data, err = base64.StdEncoding.DecodeString(payload.Content)
		if err != nil {
			resp.Error = fmt.Sprintf("ошибка декодирования base64: %v", err)
			return resp
		}
	} else {
		data = []byte(payload.Content)
	}

	mode := os.FileMode(0644)
	if payload.Mode != 0 {
		mode = os.FileMode(payload.Mode)
	}

	// Создаём родительские директории
	dir := filepath.Dir(absPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		resp.Error = fmt.Sprintf("ошибка создания директории: %v", err)
		return resp
	}

	if err := os.WriteFile(absPath, data, mode); err != nil {
		resp.Error = fmt.Sprintf("ошибка записи: %v", err)
		return resp
	}

	info, _ := os.Stat(absPath)
	if info != nil {
		resp.Size = info.Size()
		resp.Mode = int(info.Mode())
	}

	return resp
}

// ListFiles — возвращает список файлов в директории.
func ListFiles(payload protocol.FileListPayload) protocol.FileResponsePayload {
	resp := protocol.FileResponsePayload{Path: payload.Path}

	if payload.Path == "" {
		payload.Path = "."
	}

	absPath, err := filepath.Abs(payload.Path)
	if err != nil {
		resp.Error = fmt.Sprintf("неверный путь: %v", err)
		return resp
	}

	entries, err := os.ReadDir(absPath)
	if err != nil {
		resp.Error = fmt.Sprintf("ошибка чтения директории: %v", err)
		return resp
	}

	resp.IsDir = true
	resp.Entries = make([]protocol.FileEntry, 0, len(entries))

	for _, entry := range entries {
		info, err := entry.Info()
		if err != nil {
			continue
		}
		resp.Entries = append(resp.Entries, protocol.FileEntry{
			Name:  entry.Name(),
			Size:  info.Size(),
			IsDir: entry.IsDir(),
			Mode:  int(info.Mode()),
		})
	}

	return resp
}

// === Системная информация ===

// CollectSystemInfo — собирает системную информацию.
func CollectSystemInfo() protocol.SystemInfoPayload {
	hostname, _ := os.Hostname()
	osName, arch := config.OSInfo()

	// CPU
	cpuCount := getCPUCount()
	cpuModel := getCPUModel()

	// RAM
	memTotal, memUsed := getMemoryInfo()

	// Disk
	diskTotal, diskUsed := getDiskInfo()

	// Uptime
	uptime := getUptime()

	// Load average
	loadAvg := getLoadAvg()

	return protocol.SystemInfoPayload{
		Hostname:  hostname,
		OS:        osName,
		Arch:      arch,
		CPUCount:  cpuCount,
		CPUModel:  cpuModel,
		MemTotal:  memTotal,
		MemUsed:   memUsed,
		DiskTotal: diskTotal,
		DiskUsed:  diskUsed,
		Uptime:    uptime,
		LoadAvg:   loadAvg,
	}
}

// === Вспомогательные функции (кроссплатформенные) ===

func getCPUCount() int {
	// Попытка через /proc (Linux)
	if data, err := os.ReadFile("/proc/cpuinfo"); err == nil {
		count := strings.Count(string(data), "processor")
		if count > 0 {
			return count
		}
	}
	// Fallback
	return 1
}

func getCPUModel() string {
	if data, err := os.ReadFile("/proc/cpuinfo"); err == nil {
		for _, line := range strings.Split(string(data), "\n") {
			if strings.HasPrefix(line, "model name") {
				parts := strings.SplitN(line, ":", 2)
				if len(parts) == 2 {
					return strings.TrimSpace(parts[1])
				}
			}
		}
	}
	// macOS: sysctl
	if out, _, _ := new(Executor).ExecSync("sysctl -n machdep.cpu.brand_string", "", 5); out != "" {
		return strings.TrimSpace(out)
	}
	return "unknown"
}

func getMemoryInfo() (total, used uint64) {
	// macOS
	if out, _, code := new(Executor).ExecSync("sysctl -n hw.memsize", "", 5); code == 0 && out != "" {
		fmt.Sscanf(strings.TrimSpace(out), "%d", &total)
	}

	// Использованная память (macOS)
	if _, _, code := new(Executor).ExecSync("vm_stat | awk '/Pages active/ || /Pages wired/ || /Pages occupied/'", "", 5); code == 0 {
		// TODO: парсинг vm_stat для used memory
		_ = used
	}

	// Linux fallback
	if total == 0 {
		if data, err := os.ReadFile("/proc/meminfo"); err == nil {
			for _, line := range strings.Split(string(data), "\n") {
				if strings.HasPrefix(line, "MemTotal:") {
					fmt.Sscanf(line, "MemTotal: %d kB", &total)
					total *= 1024
				}
				if strings.HasPrefix(line, "MemAvailable:") {
					var avail uint64
					fmt.Sscanf(line, "MemAvailable: %d kB", &avail)
					used = total - avail*1024
				}
			}
		}
	}

	return
}

func getDiskInfo() (total, used uint64) {
	if out, _, code := new(Executor).ExecSync("df -k / | tail -1 | awk '{print $2,$3}'", "", 5); code == 0 {
		fmt.Sscanf(strings.TrimSpace(out), "%d %d", &total, &used)
		total *= 1024
		used *= 1024
	}
	return
}

func getUptime() uint64 {
	if out, _, code := new(Executor).ExecSync("cat /proc/uptime | awk '{print int($1)}'", "", 5); code == 0 {
		var u uint64
		fmt.Sscanf(strings.TrimSpace(out), "%d", &u)
		return u
	}
	// macOS
	if _, _, code := new(Executor).ExecSync("uptime | awk '{print $3}' | sed 's/,//'", "", 5); code == 0 {
		return 0 // TODO: parse macOS uptime
	}
	return 0
}

func getLoadAvg() []float64 {
	if out, _, code := new(Executor).ExecSync("cat /proc/loadavg | awk '{print $1,$2,$3}'", "", 5); code == 0 {
		var l1, l2, l3 float64
		fmt.Sscanf(strings.TrimSpace(out), "%f %f %f", &l1, &l2, &l3)
		return []float64{l1, l2, l3}
	}
	// macOS
	if out, _, code := new(Executor).ExecSync("sysctl -n vm.loadavg | awk '{print $2,$3,$4}'", "", 5); code == 0 {
		var l1, l2, l3 float64
		fmt.Sscanf(strings.TrimSpace(out), "{%f, %f, %f}", &l1, &l2, &l3)
		return []float64{l1, l2, l3}
	}
	return nil
}

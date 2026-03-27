package agent

import (
	"compress/gzip"
	"encoding/csv"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/google/uuid"
)

// AuditEntry — одна запись в audit log (локальный формат клиента).
type AuditEntry struct {
	ID           string    `json:"id"`
	Timestamp    time.Time `json:"timestamp"`
	AgentID      string    `json:"agent_id"`
	ClientID     string    `json:"client_id"`
	Action       string    `json:"action"`                 // exec, read_file, write_file, ...
	Command      string    `json:"command,omitempty"`
	Path         string    `json:"path,omitempty"`
	RiskLevel    string    `json:"risk_level"`             // low, medium, high
	ApprovalMode string    `json:"approval_mode"`          // auto, soft_ask, hard_ask
	BackupID     string    `json:"backup_id,omitempty"`
	Result       string    `json:"result"`                 // success, error, blocked, timeout
	ExitCode     int       `json:"exit_code,omitempty"`
	DurationMs   int64     `json:"duration_ms"`
	Error        string    `json:"error,omitempty"`
	ClientIP     string    `json:"client_ip,omitempty"`
}

// AuditQuery — фильтр для поиска по audit log.
type AuditQuery struct {
	AgentID    string     `json:"agent_id,omitempty"`
	ClientID   string     `json:"client_id,omitempty"`
	Action     string     `json:"action,omitempty"`
	RiskLevel  string     `json:"risk_level,omitempty"`
	Result     string     `json:"result,omitempty"`
	From       *time.Time `json:"from,omitempty"`
	To         *time.Time `json:"to,omitempty"`
	Limit      int        `json:"limit,omitempty"`
	Offset     int        `json:"offset,omitempty"`
}

// AuditStats — статистика audit log.
type AuditStats struct {
	TotalEntries  int            `json:"total_entries"`
	ByAction      map[string]int `json:"by_action"`
	ByRiskLevel   map[string]int `json:"by_risk_level"`
	ByResult      map[string]int `json:"by_result"`
	TodayCount    int            `json:"today_count"`
	Last24hCount  int            `json:"last_24h_count"`
	OldestEntry   *time.Time     `json:"oldest_entry,omitempty"`
	NewestEntry   *time.Time     `json:"newest_entry,omitempty"`
}

// AuditLogger — записывает действия в JSONL-файл (локальный клиентский лог).
type AuditLogger struct {
	mu          sync.Mutex
	baseDir     string
	currentFile *os.File
	currentDate string
	maxSize     int64 // макс. размер файла в байтах (100MB)
	retention   int   // дней хранения (90)
}

// NewAuditLogger — создаёт новый audit logger для агента.
func NewAuditLogger(baseDir string) (*AuditLogger, error) {
	if baseDir == "" {
		home, _ := os.UserHomeDir()
		baseDir = filepath.Join(home, ".flowlink", "audit")
	}

	if err := os.MkdirAll(baseDir, 0755); err != nil {
		return nil, fmt.Errorf("ошибка создания директории audit: %w", err)
	}

	logger := &AuditLogger{
		baseDir:   baseDir,
		maxSize:   100 * 1024 * 1024, // 100MB
		retention: 90,                // 90 дней
	}

	// Запускаем фоновую ротацию и очистку
	go logger.backgroundTasks()

	return logger, nil
}

// Log — записывает entry в JSONL-файл (append-only).
func (l *AuditLogger) Log(entry AuditEntry) error {
	l.mu.Lock()
	defer l.mu.Unlock()

	// Генерируем ID если нет
	if entry.ID == "" {
		entry.ID = uuid.New().String()
	}
	if entry.Timestamp.IsZero() {
		entry.Timestamp = time.Now()
	}

	// Проверяем нужно ли открыть новый файл (по дате)
	today := entry.Timestamp.Format("2006-01-02")
	if l.currentDate != today || l.currentFile == nil {
		if l.currentFile != nil {
			l.currentFile.Close()
		}

		filename := filepath.Join(l.baseDir, fmt.Sprintf("audit-%s.jsonl", today))
		f, err := os.OpenFile(filename, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
		if err != nil {
			return fmt.Errorf("ошибка открытия audit файла: %w", err)
		}

		l.currentFile = f
		l.currentDate = today
	}

	// Сериализуем в JSON
	data, err := json.Marshal(entry)
	if err != nil {
		return fmt.Errorf("ошибка сериализации entry: %w", err)
	}

	// Записываем строку
	if _, err := fmt.Fprintf(l.currentFile, "%s\n", data); err != nil {
		return fmt.Errorf("ошибка записи entry: %w", err)
	}

	// Flush на диск
	l.currentFile.Sync()

	return nil
}

// Query — поиск по audit log с фильтрацией.
func (l *AuditLogger) Query(query AuditQuery) ([]AuditEntry, error) {
	l.mu.Lock()
	defer l.mu.Unlock()

	var results []AuditEntry

	// Определяем диапазон дат для сканирования
	startDate := time.Now().AddDate(0, 0, -l.retention)
	if query.From != nil && query.From.After(startDate) {
		startDate = *query.From
	}

	endDate := time.Now()
	if query.To != nil && query.To.Before(endDate) {
		endDate = *query.To
	}

	// Сканируем файлы в диапазоне дат
	for d := startDate; !d.After(endDate); d = d.AddDate(0, 0, 1) {
		filename := filepath.Join(l.baseDir, fmt.Sprintf("audit-%s.jsonl", d.Format("2006-01-02")))
		
		entries, err := l.readFile(filename)
		if err != nil {
			if os.IsNotExist(err) {
				continue
			}
			return nil, err
		}

		// Фильтруем
		for _, entry := range entries {
			if !l.matchQuery(entry, query) {
				continue
			}
			results = append(results, entry)
		}
	}

	// Сортируем по времени (новые первыми)
	sort.Slice(results, func(i, j int) bool {
		return results[i].Timestamp.After(results[j].Timestamp)
	})

	// Применяем offset и limit
	if query.Offset > 0 && query.Offset < len(results) {
		results = results[query.Offset:]
	} else if query.Offset >= len(results) {
		return []AuditEntry{}, nil
	}

	if query.Limit > 0 && query.Limit < len(results) {
		results = results[:query.Limit]
	}

	return results, nil
}

// Recent — возвращает последние N записей.
func (l *AuditLogger) Recent(n int) ([]AuditEntry, error) {
	return l.Query(AuditQuery{Limit: n})
}

// Export — экспорт audit log в CSV или JSON.
func (l *AuditLogger) Export(format string, query AuditQuery) ([]byte, error) {
	entries, err := l.Query(query)
	if err != nil {
		return nil, err
	}

	switch strings.ToLower(format) {
	case "csv":
		return l.exportCSV(entries)
	case "json":
		return json.MarshalIndent(entries, "", "  ")
	default:
		return nil, fmt.Errorf("неподдерживаемый формат: %s", format)
	}
}

// Stats — возвращает статистику по audit log.
func (l *AuditLogger) Stats() (*AuditStats, error) {
	l.mu.Lock()
	defer l.mu.Unlock()

	stats := &AuditStats{
		ByAction:    make(map[string]int),
		ByRiskLevel: make(map[string]int),
		ByResult:    make(map[string]int),
	}

	now := time.Now()
	today := now.Format("2006-01-02")
	last24h := now.Add(-24 * time.Hour)

	// Сканируем все файлы за retention период
	for d := now.AddDate(0, 0, -l.retention); !d.After(now); d = d.AddDate(0, 0, 1) {
		filename := filepath.Join(l.baseDir, fmt.Sprintf("audit-%s.jsonl", d.Format("2006-01-02")))
		
		entries, err := l.readFile(filename)
		if err != nil {
			if os.IsNotExist(err) {
				continue
			}
			return nil, err
		}

		for _, entry := range entries {
			stats.TotalEntries++
			stats.ByAction[entry.Action]++
			stats.ByRiskLevel[entry.RiskLevel]++
			stats.ByResult[entry.Result]++

			// Подсчёт за сегодня
			if entry.Timestamp.Format("2006-01-02") == today {
				stats.TodayCount++
			}

			// Подсчёт за последние 24 часа
			if entry.Timestamp.After(last24h) {
				stats.Last24hCount++
			}

			// Oldest/Newest
			if stats.OldestEntry == nil || entry.Timestamp.Before(*stats.OldestEntry) {
				stats.OldestEntry = &entry.Timestamp
			}
			if stats.NewestEntry == nil || entry.Timestamp.After(*stats.NewestEntry) {
				stats.NewestEntry = &entry.Timestamp
			}
		}
	}

	return stats, nil
}

// Rotate — ротация логов (сжатие файлов > maxSize).
func (l *AuditLogger) Rotate() error {
	l.mu.Lock()
	defer l.mu.Unlock()

	// Закрываем текущий файл
	if l.currentFile != nil {
		l.currentFile.Close()
		l.currentFile = nil
		l.currentDate = ""
	}

	// Ищем файлы > maxSize
	files, err := filepath.Glob(filepath.Join(l.baseDir, "audit-*.jsonl"))
	if err != nil {
		return err
	}

	for _, file := range files {
		info, err := os.Stat(file)
		if err != nil {
			continue
		}

		if info.Size() > l.maxSize {
			// Сжимаем в .gz
			if err := l.compressFile(file); err != nil {
				continue
			}
			// Удаляем оригинал
			os.Remove(file)
		}
	}

	return nil
}

// Prune — удаление записей старше N дней.
func (l *AuditLogger) Prune(olderThanDays int) error {
	if olderThanDays <= 0 {
		olderThanDays = l.retention
	}

	l.mu.Lock()
	defer l.mu.Unlock()

	cutoff := time.Now().AddDate(0, 0, -olderThanDays)

	// Ищем все файлы (включая сжатые)
	files, err := filepath.Glob(filepath.Join(l.baseDir, "audit-*.jsonl*"))
	if err != nil {
		return err
	}

	for _, file := range files {
		// Извлекаем дату из имени файла
		base := filepath.Base(file)
		// audit-2006-01-02.jsonl или audit-2006-01-02.jsonl.gz
		dateStr := strings.TrimPrefix(base, "audit-")
		dateStr = strings.TrimSuffix(dateStr, ".jsonl.gz")
		dateStr = strings.TrimSuffix(dateStr, ".jsonl")

		fileDate, err := time.Parse("2006-01-02", dateStr)
		if err != nil {
			continue
		}

		if fileDate.Before(cutoff) {
			os.Remove(file)
		}
	}

	return nil
}

// Close — закрывает audit logger.
func (l *AuditLogger) Close() error {
	l.mu.Lock()
	defer l.mu.Unlock()

	if l.currentFile != nil {
		return l.currentFile.Close()
	}
	return nil
}

// === Вспомогательные методы ===

func (l *AuditLogger) readFile(filename string) ([]AuditEntry, error) {
	var entries []AuditEntry

	// Проверяем сжатый файл
	gzFile := filename + ".gz"
	if _, err := os.Stat(gzFile); err == nil {
		f, err := os.Open(gzFile)
		if err != nil {
			return nil, err
		}
		defer f.Close()

		gz, err := gzip.NewReader(f)
		if err != nil {
			return nil, err
		}
		defer gz.Close()

		decoder := json.NewDecoder(gz)
		for {
			var entry AuditEntry
			if err := decoder.Decode(&entry); err != nil {
				if err == io.EOF {
					break
				}
				return nil, err
			}
			entries = append(entries, entry)
		}

		return entries, nil
	}

	// Обычный файл
	f, err := os.Open(filename)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	decoder := json.NewDecoder(f)
	for {
		var entry AuditEntry
		if err := decoder.Decode(&entry); err != nil {
			if err == io.EOF {
				break
			}
			return nil, err
		}
		entries = append(entries, entry)
	}

	return entries, nil
}

func (l *AuditLogger) matchQuery(entry AuditEntry, query AuditQuery) bool {
	if query.AgentID != "" && entry.AgentID != query.AgentID {
		return false
	}
	if query.ClientID != "" && entry.ClientID != query.ClientID {
		return false
	}
	if query.Action != "" && entry.Action != query.Action {
		return false
	}
	if query.RiskLevel != "" && entry.RiskLevel != query.RiskLevel {
		return false
	}
	if query.Result != "" && entry.Result != query.Result {
		return false
	}
	if query.From != nil && entry.Timestamp.Before(*query.From) {
		return false
	}
	if query.To != nil && entry.Timestamp.After(*query.To) {
		return false
	}
	return true
}

func (l *AuditLogger) exportCSV(entries []AuditEntry) ([]byte, error) {
	var buf strings.Builder
	writer := csv.NewWriter(&buf)

	// Header
	header := []string{"id", "timestamp", "agent_id", "client_id", "action", "command", 
		"path", "risk_level", "approval_mode", "backup_id", "result", "exit_code", 
		"duration_ms", "error", "client_ip"}
	writer.Write(header)

	// Rows
	for _, e := range entries {
		row := []string{
			e.ID,
			e.Timestamp.Format(time.RFC3339),
			e.AgentID,
			e.ClientID,
			e.Action,
			e.Command,
			e.Path,
			e.RiskLevel,
			e.ApprovalMode,
			e.BackupID,
			e.Result,
			fmt.Sprintf("%d", e.ExitCode),
			fmt.Sprintf("%d", e.DurationMs),
			e.Error,
			e.ClientIP,
		}
		writer.Write(row)
	}

	writer.Flush()
	return []byte(buf.String()), writer.Error()
}

func (l *AuditLogger) compressFile(filename string) error {
	// Открываем исходный файл
	src, err := os.Open(filename)
	if err != nil {
		return err
	}
	defer src.Close()

	// Создаём сжатый файл
	dst, err := os.Create(filename + ".gz")
	if err != nil {
		return err
	}
	defer dst.Close()

	// Сжимаем
	gz := gzip.NewWriter(dst)
	defer gz.Close()

	if _, err := io.Copy(gz, src); err != nil {
		return err
	}

	return nil
}

func (l *AuditLogger) backgroundTasks() {
	ticker := time.NewTicker(1 * time.Hour)
	defer ticker.Stop()

	for range ticker.C {
		// Ротация раз в час
		l.Rotate()
		
		// Очистка старых логов раз в сутки
		l.Prune(0)
	}
}

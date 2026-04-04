package relay

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

	"github.com/braincreator/flowlink/internal/audit"
	"github.com/google/uuid"
)

// AuditEntry — одна запись в audit log.
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
	HMAC         string    `json:"hmac,omitempty"`         // HMAC-SHA256 подпись
	Tampered     bool      `json:"tampered,omitempty"`     // true если HMAC невалиден
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

// AuditLogger — записывает действия в JSONL-файл.
type AuditLogger struct {
	mu          sync.Mutex
	baseDir     string
	currentFile *os.File
	currentDate string
	maxSize     int64    // макс. размер файла в байтах (100MB)
	retention   int      // дней хранения (90)
	hmacSecret  []byte   // секрет для HMAC подписи
}

// NewAuditLogger — создаёт новый audit logger.
func NewAuditLogger(baseDir string) (*AuditLogger, error) {
	return NewAuditLoggerWithHMAC(baseDir, "")
}

// NewAuditLoggerWithHMAC — создаёт audit logger с указанным путём к HMAC ключу.
func NewAuditLoggerWithHMAC(baseDir, hmacKeyPath string) (*AuditLogger, error) {
	if baseDir == "" {
		home, _ := os.UserHomeDir()
		baseDir = filepath.Join(home, ".flowlink", "audit")
	}

	if err := os.MkdirAll(baseDir, 0755); err != nil {
		return nil, fmt.Errorf("ошибка создания директории audit: %w", err)
	}

	// Загружаем или генерируем HMAC секрет
	hmacSecret, err := audit.LoadOrGenerateHMACSecret(hmacKeyPath)
	if err != nil {
		return nil, fmt.Errorf("ошибка загрузки HMAC ключа: %w", err)
	}

	logger := &AuditLogger{
		baseDir:    baseDir,
		maxSize:    100 * 1024 * 1024, // 100MB
		retention:  90,                // 90 дней
		hmacSecret: hmacSecret,
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

	// Конвертируем в map для HMAC
	entryMap := l.entryToMap(entry)

	// Добавляем HMAC подпись
	if len(l.hmacSecret) > 0 {
		entryMap[audit.HMACField] = audit.SignEntry(entryMap, l.hmacSecret)
	}

	// Сериализуем в JSON
	data, err := json.Marshal(entryMap)
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
	// Используем readFileWithValidation для HMAC проверки
	return l.readFileWithValidation(filename)
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

// VerifyAll — проверяет целостность всех логов и возвращает результат.
func (l *AuditLogger) VerifyAll() (*AuditVerifyResult, error) {
	l.mu.Lock()
	defer l.mu.Unlock()

	result := &AuditVerifyResult{
		ByDate: make(map[string]DateVerifyResult),
	}

	// Сканируем все файлы за retention период
	now := time.Now()
	for d := now.AddDate(0, 0, -l.retention); !d.After(now); d = d.AddDate(0, 0, 1) {
		filename := filepath.Join(l.baseDir, fmt.Sprintf("audit-%s.jsonl", d.Format("2006-01-02")))
		
		dateResult := DateVerifyResult{Date: d.Format("2006-01-02")}
		
		entries, err := l.readFileWithValidation(filename)
		if err != nil {
			if os.IsNotExist(err) {
				continue
			}
				dateResult.Error = err.Error()
				result.ByDate[dateResult.Date] = dateResult
				continue
		}

		for _, entry := range entries {
			result.TotalEntries++
			dateResult.Total++
			if entry.Tampered {
				result.TamperedEntries++
				dateResult.Tampered++
				result.TamperedIDs = append(result.TamperedIDs, entry.ID)
			}
		}

		if dateResult.Total > 0 {
			result.ByDate[dateResult.Date] = dateResult
		}
	}

	result.Valid = result.TamperedEntries == 0
	return result, nil
}

// AuditVerifyResult — результат верификации всех логов.
type AuditVerifyResult struct {
	Valid           bool                        `json:"valid"`
	TotalEntries    int                         `json:"total_entries"`
	TamperedEntries int                         `json:"tampered_entries"`
	TamperedIDs     []string                    `json:"tampered_ids,omitempty"`
	ByDate          map[string]DateVerifyResult `json:"by_date,omitempty"`
}

// DateVerifyResult — результат верификации за одну дату.
type DateVerifyResult struct {
	Date     string `json:"date"`
	Total    int    `json:"total"`
	Tampered int    `json:"tampered"`
	Error    string `json:"error,omitempty"`
}

// entryToMap — конвертирует AuditEntry в map[string]interface{}.
func (l *AuditLogger) entryToMap(entry AuditEntry) map[string]interface{} {
	m := make(map[string]interface{})
	m["id"] = entry.ID
	m["timestamp"] = entry.Timestamp
	m["agent_id"] = entry.AgentID
	m["client_id"] = entry.ClientID
	m["action"] = entry.Action
	m["risk_level"] = entry.RiskLevel
	m["approval_mode"] = entry.ApprovalMode
	m["result"] = entry.Result
	m["duration_ms"] = entry.DurationMs
	
	if entry.Command != "" {
		m["command"] = entry.Command
	}
	if entry.Path != "" {
		m["path"] = entry.Path
	}
	if entry.BackupID != "" {
		m["backup_id"] = entry.BackupID
	}
	if entry.ExitCode != 0 {
		m["exit_code"] = entry.ExitCode
	}
	if entry.Error != "" {
		m["error"] = entry.Error
	}
	if entry.ClientIP != "" {
		m["client_ip"] = entry.ClientIP
	}
	
	return m
}

// readFileWithValidation — читает файл и валидирует HMAC.
func (l *AuditLogger) readFileWithValidation(filename string) ([]AuditEntry, error) {
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
			var rawEntry map[string]interface{}
			if err := decoder.Decode(&rawEntry); err != nil {
				if err == io.EOF {
					break
				}
				return nil, err
			}
			entry := l.mapToEntry(rawEntry)
			// Валидируем HMAC
			if len(l.hmacSecret) > 0 && !audit.VerifyEntry(rawEntry, l.hmacSecret) {
				entry.Tampered = true
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
		var rawEntry map[string]interface{}
		if err := decoder.Decode(&rawEntry); err != nil {
			if err == io.EOF {
				break
			}
			return nil, err
		}
		entry := l.mapToEntry(rawEntry)
		// Валидируем HMAC
		if len(l.hmacSecret) > 0 && !audit.VerifyEntry(rawEntry, l.hmacSecret) {
			entry.Tampered = true
		}
		entries = append(entries, entry)
	}

	return entries, nil
}

// mapToEntry — конвертирует map в AuditEntry.
func (l *AuditLogger) mapToEntry(m map[string]interface{}) AuditEntry {
	entry := AuditEntry{}
	
	if v, ok := m["id"].(string); ok {
		entry.ID = v
	}
	if v, ok := m["timestamp"].(string); ok {
		entry.Timestamp, _ = time.Parse(time.RFC3339, v)
	}
	if v, ok := m["agent_id"].(string); ok {
		entry.AgentID = v
	}
	if v, ok := m["client_id"].(string); ok {
		entry.ClientID = v
	}
	if v, ok := m["action"].(string); ok {
		entry.Action = v
	}
	if v, ok := m["command"].(string); ok {
		entry.Command = v
	}
	if v, ok := m["path"].(string); ok {
		entry.Path = v
	}
	if v, ok := m["risk_level"].(string); ok {
		entry.RiskLevel = v
	}
	if v, ok := m["approval_mode"].(string); ok {
		entry.ApprovalMode = v
	}
	if v, ok := m["backup_id"].(string); ok {
		entry.BackupID = v
	}
	if v, ok := m["result"].(string); ok {
		entry.Result = v
	}
	if v, ok := m["exit_code"].(float64); ok {
		entry.ExitCode = int(v)
	}
	if v, ok := m["duration_ms"].(float64); ok {
		entry.DurationMs = int64(v)
	}
	if v, ok := m["error"].(string); ok {
		entry.Error = v
	}
	if v, ok := m["client_ip"].(string); ok {
		entry.ClientIP = v
	}
	if v, ok := m["hmac"].(string); ok {
		entry.HMAC = v
	}
	
	return entry
}

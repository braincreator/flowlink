// Package agent — Backup Engine для flowlink.
// Автоматическое создание резервных копий перед деструктивными действиями.
package agent

import (
	"archive/tar"
	"compress/gzip"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	"github.com/braincreator/flowlink/internal/config"
)

// DefaultBackupConfig — конфигурация по умолчанию.
func DefaultBackupConfig() config.BackupConfig {
	home, _ := os.UserHomeDir()
	return config.BackupConfig{
		MaxSnapshots:  50,
		MaxTotalSize:  5 * 1024 * 1024 * 1024, // 5GB
		RetentionDays: 7,
		BackupDir:     filepath.Join(home, ".flowlink", "backups"),
	}
}

// Snapshot — метаданные снапшота.
type Snapshot struct {
	ID          string   `json:"id"`
	Description string   `json:"description"`
	Timestamp   int64    `json:"timestamp"`
	Size        int64    `json:"size"`
	Paths       []string `json:"paths"`
	Filename    string   `json:"filename"`
}

// FileChange — изменение файла (для diff).
type FileChange struct {
	Path    string `json:"path"`
	Change  string `json:"change"` // "added", "modified", "deleted"
	OldSize int64  `json:"old_size,omitempty"`
	NewSize int64  `json:"new_size,omitempty"`
}

// BackupEngine — движок резервного копирования.
type BackupEngine struct {
	cfg    config.BackupConfig
	logger *slog.Logger
}

// NewBackupEngine — создаёт новый backup engine.
func NewBackupEngine(cfg config.BackupConfig) *BackupEngine {
	if cfg.BackupDir == "" {
		cfg = DefaultBackupConfig()
	}

	// Создаём директорию для бэкапов
	if err := os.MkdirAll(cfg.BackupDir, 0755); err != nil {
		slog.Warn("ошибка создания директории бэкапов", "err", err)
	}

	return &BackupEngine{
		cfg:    cfg,
		logger: slog.Default(),
	}
}

// CreateBefore — создаёт снапшот перед деструктивным действием.
// Возвращает ID снапшота или ошибку.
func (b *BackupEngine) CreateBefore(paths []string, reason string) (string, error) {
	if len(paths) == 0 {
		return "", fmt.Errorf("пустой список путей для бэкапа")
	}

	// Генерируем ID снапшота
	timestamp := time.Now()
	snapshotID := fmt.Sprintf("%d_%s", timestamp.Unix(), sanitizeDescription(reason))
	filename := snapshotID + ".tar.gz"
	backupPath := filepath.Join(b.cfg.BackupDir, filename)

	b.logger.Info("создание бэкапа",
		"snapshot_id", snapshotID,
		"paths", len(paths),
		"reason", reason,
	)

	// Создаём tar.gz архив
	size, err := b.createTarGz(paths, backupPath)
	if err != nil {
		return "", fmt.Errorf("ошибка создания архива: %w", err)
	}

	// Сохраняем метаданные
	snapshot := Snapshot{
		ID:          snapshotID,
		Description: reason,
		Timestamp:   timestamp.Unix(),
		Size:        size,
		Paths:       paths,
		Filename:    filename,
	}

	if err := b.saveMetadata(snapshot); err != nil {
		b.logger.Warn("ошибка сохранения метаданных", "err", err)
	}

	// Очистка старых бэкапов
	go b.Cleanup()

	return snapshotID, nil
}

// createTarGz — создаёт tar.gz архив из списка путей.
func (b *BackupEngine) createTarGz(paths []string, outputPath string) (int64, error) {
	file, err := os.Create(outputPath)
	if err != nil {
		return 0, fmt.Errorf("создание файла архива: %w", err)
	}
	defer file.Close()

	gzWriter := gzip.NewWriter(file)
	defer gzWriter.Close()

	tarWriter := tar.NewWriter(gzWriter)
	defer tarWriter.Close()

	var totalSize int64

	for _, path := range paths {
		// Расширяем glob-паттерны
		matches, err := filepath.Glob(path)
		if err != nil {
			b.logger.Warn("ошибка glob", "path", path, "err", err)
			matches = []string{path}
		}

		for _, match := range matches {
			size, err := b.addToArchive(tarWriter, match)
			if err != nil {
				b.logger.Warn("ошибка добавления в архив", "path", match, "err", err)
				continue
			}
			totalSize += size
		}
	}

	return totalSize, nil
}

// addToArchive — добавляет файл или директорию в архив.
func (b *BackupEngine) addToArchive(tarWriter *tar.Writer, path string) (int64, error) {
	_, err := os.Stat(path)
	if err != nil {
		return 0, err
	}

	var totalSize int64

	// Рекурсивно обходим директории
	err = filepath.Walk(path, func(filePath string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}

		// Создаём заголовок
		header, err := tar.FileInfoHeader(info, "")
		if err != nil {
			return err
		}

		// Относительный путь внутри архива
		relPath, err := filepath.Rel(filepath.Dir(path), filePath)
		if err != nil {
			relPath = filePath
		}
		header.Name = relPath

		// Записываем заголовок
		if err := tarWriter.WriteHeader(header); err != nil {
			return err
		}

		// Если это обычный файл — копируем содержимое
		if !info.IsDir() {
			file, err := os.Open(filePath)
			if err != nil {
				return err
			}
			defer file.Close()

			copied, err := io.Copy(tarWriter, file)
			if err != nil {
				return err
			}
			totalSize += copied
		}

		return nil
	})

	return totalSize, err
}

// Restore — восстанавливает файлы из снапшота.
func (b *BackupEngine) Restore(snapshotID string) error {
	snapshot, err := b.getSnapshot(snapshotID)
	if err != nil {
		return fmt.Errorf("снапшот не найден: %w", err)
	}

	backupPath := filepath.Join(b.cfg.BackupDir, snapshot.Filename)

	b.logger.Info("восстановление из бэкапа",
		"snapshot_id", snapshotID,
		"file", snapshot.Filename,
	)

	// Открываем архив
	file, err := os.Open(backupPath)
	if err != nil {
		return fmt.Errorf("ошибка открытия архива: %w", err)
	}
	defer file.Close()

	gzReader, err := gzip.NewReader(file)
	if err != nil {
		return fmt.Errorf("ошибка распаковки gzip: %w", err)
	}
	defer gzReader.Close()

	tarReader := tar.NewReader(gzReader)

	// Извлекаем файлы
	for {
		header, err := tarReader.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return fmt.Errorf("ошибка чтения архива: %w", err)
		}

		// Определяем путь назначения (в текущую директорию)
		targetPath := filepath.Join(".", header.Name)

		switch header.Typeflag {
		case tar.TypeDir:
			if err := os.MkdirAll(targetPath, os.FileMode(header.Mode)); err != nil {
				return fmt.Errorf("ошибка создания директории: %w", err)
			}
		case tar.TypeReg:
			// Создаём родительские директории
			if err := os.MkdirAll(filepath.Dir(targetPath), 0755); err != nil {
				return fmt.Errorf("ошибка создания родительской директории: %w", err)
			}

			// Создаём файл
			outFile, err := os.OpenFile(targetPath, os.O_CREATE|os.O_WRONLY, os.FileMode(header.Mode))
			if err != nil {
				return fmt.Errorf("ошибка создания файла: %w", err)
			}

			if _, err := io.Copy(outFile, tarReader); err != nil {
				outFile.Close()
				return fmt.Errorf("ошибка записи файла: %w", err)
			}
			outFile.Close()
		}
	}

	b.logger.Info("восстановление завершено", "snapshot_id", snapshotID)
	return nil
}

// List — возвращает список всех снапшотов.
func (b *BackupEngine) List() []Snapshot {
	metadataPath := filepath.Join(b.cfg.BackupDir, "meta.json")

	data, err := os.ReadFile(metadataPath)
	if err != nil {
		return []Snapshot{}
	}

	var snapshots []Snapshot
	if err := json.Unmarshal(data, &snapshots); err != nil {
		return []Snapshot{}
	}

	return snapshots
}

// Diff — сравнивает текущее состояние со снапшотом (опционально).
func (b *BackupEngine) Diff(snapshotID string) ([]FileChange, error) {
	snapshot, err := b.getSnapshot(snapshotID)
	if err != nil {
		return nil, fmt.Errorf("снапшот не найден: %w", err)
	}

	var changes []FileChange

	// Простая проверка: сравниваем существование и размер файлов
	for _, path := range snapshot.Paths {
		matches, _ := filepath.Glob(path)
		if len(matches) == 0 {
			matches = []string{path}
		}

		for _, match := range matches {
			info, err := os.Stat(match)
			if os.IsNotExist(err) {
				changes = append(changes, FileChange{
					Path:   match,
					Change: "deleted",
				})
				continue
			}
			if err != nil {
				continue
			}

			// Простая эвристика: если файл изменился после снапшота
			if info.ModTime().Unix() > snapshot.Timestamp {
				changes = append(changes, FileChange{
					Path:    match,
					Change:  "modified",
					NewSize: info.Size(),
				})
			}
		}
	}

	return changes, nil
}

// Cleanup — удаляет старые бэкапы согласно политике.
func (b *BackupEngine) Cleanup() error {
	snapshots := b.List()
	if len(snapshots) == 0 {
		return nil
	}

	var (
		keepSnapshots  []Snapshot
		deletedCount   int
		deletedSize    int64
		totalSize      int64
		now            = time.Now()
		retentionCutoff = now.AddDate(0, 0, -b.cfg.RetentionDays)
	)

	// Сортируем по времени (новые первые)
	for i := 0; i < len(snapshots); i++ {
		for j := i + 1; j < len(snapshots); j++ {
			if snapshots[i].Timestamp < snapshots[j].Timestamp {
				snapshots[i], snapshots[j] = snapshots[j], snapshots[i]
			}
		}
	}

	// Удаляем по правилам
	for i, snapshot := range snapshots {
		snapshotTime := time.Unix(snapshot.Timestamp, 0)

		// Правило 1: старше retention периода
		if snapshotTime.Before(retentionCutoff) {
			b.deleteSnapshot(snapshot)
			deletedCount++
			deletedSize += snapshot.Size
			continue
		}

		// Правило 2: превышен лимит количества
		if i >= b.cfg.MaxSnapshots {
			b.deleteSnapshot(snapshot)
			deletedCount++
			deletedSize += snapshot.Size
			continue
		}

		keepSnapshots = append(keepSnapshots, snapshot)
		totalSize += snapshot.Size
	}

	// Правило 3: превышен лимит размера (удаляем самые старые)
	for len(keepSnapshots) > 0 && totalSize > b.cfg.MaxTotalSize {
		oldest := keepSnapshots[len(keepSnapshots)-1]
		b.deleteSnapshot(oldest)
		deletedCount++
		deletedSize += oldest.Size
		totalSize -= oldest.Size
		keepSnapshots = keepSnapshots[:len(keepSnapshots)-1]
	}

	if deletedCount > 0 {
		b.logger.Info("очистка бэкапов",
			"deleted", deletedCount,
			"freed_bytes", deletedSize,
			"kept", len(keepSnapshots),
		)
	}

	// Обновляем метаданные
	return b.saveAllMetadata(keepSnapshots)
}

// getSnapshot — находит снапшот по ID.
func (b *BackupEngine) getSnapshot(snapshotID string) (*Snapshot, error) {
	snapshots := b.List()
	for _, snapshot := range snapshots {
		if snapshot.ID == snapshotID {
			return &snapshot, nil
		}
	}
	return nil, fmt.Errorf("снапшот не найден")
}

// deleteSnapshot — удаляет файл снапшота.
func (b *BackupEngine) deleteSnapshot(snapshot Snapshot) {
	backupPath := filepath.Join(b.cfg.BackupDir, snapshot.Filename)
	if err := os.Remove(backupPath); err != nil {
		b.logger.Warn("ошибка удаления бэкапа", "file", snapshot.Filename, "err", err)
	}
}

// saveMetadata — добавляет метаданные снапшота.
func (b *BackupEngine) saveMetadata(snapshot Snapshot) error {
	snapshots := b.List()
	snapshots = append(snapshots, snapshot)
	return b.saveAllMetadata(snapshots)
}

// saveAllMetadata — сохраняет все метаданные.
func (b *BackupEngine) saveAllMetadata(snapshots []Snapshot) error {
	metadataPath := filepath.Join(b.cfg.BackupDir, "meta.json")

	data, err := json.MarshalIndent(snapshots, "", "  ")
	if err != nil {
		return fmt.Errorf("сериализация метаданных: %w", err)
	}

	return os.WriteFile(metadataPath, data, 0644)
}

// sanitizeDescription — очищает описание для использования в имени файла.
func sanitizeDescription(s string) string {
	// Удаляем небезопасные символы
	reg := regexp.MustCompile(`[^a-zA-Z0-9_-]`)
	safe := reg.ReplaceAllString(s, "_")
	if len(safe) > 30 {
		safe = safe[:30]
	}
	return safe
}

// DetectAffectedPaths — определяет какие пути затрагивает команда.
func DetectAffectedPaths(command string) []string {
	var paths []string

	// Простая эвристика для популярных команд
	// rm путь
	if strings.HasPrefix(command, "rm ") {
		args := strings.TrimPrefix(command, "rm ")
		args = strings.TrimPrefix(args, "-rf ")
		args = strings.TrimPrefix(args, "-r ")
		args = strings.TrimPrefix(args, "-f ")
		if args != "" && args != "/*" {
			paths = append(paths, strings.Fields(args)[0])
		}
	}

	// systemctl stop/restart service
	if strings.Contains(command, "systemctl ") {
		// systemctl останавливает сервисы, не файлы
		// Можно бэкапить конфиги: /etc/systemd/system/*.service
		return []string{"/etc/systemd/system"}
	}

	// docker rm container
	if strings.Contains(command, "docker rm") {
		// Docker контейнеры — бэкап не применим
		return []string{}
	}

	// apt remove package
	if strings.Contains(command, "apt remove") || strings.Contains(command, "apt-get remove") {
		// Пакеты — можно бэкапить конфиги в /etc
		return []string{"/etc"}
	}

	// DROP DATABASE/TABLE (SQL)
	if strings.Contains(strings.ToUpper(command), "DROP ") {
		// SQL команды — бэкап через mysqldump/pg_dump (не реализовано)
		return []string{}
	}

	return paths
}

// IsDestructive — определяет, является ли команда деструктивной.
func IsDestructive(command string) bool {
	destructivePatterns := []string{
		"rm ", "rmdir", "unlink",
		"DROP ", "TRUNCATE ", "DELETE ",
		"apt remove", "apt-get remove", "apt purge", "apt-get purge",
		"yum remove", "dnf remove",
		"docker rm", "docker rmi", "docker system prune",
		"systemctl stop", "systemctl disable", "systemctl mask",
		"iptables -F", "iptables -X",
		"chmod 777",
		"crontab -r",
		"userdel", "useradd",
	}

	cmd := strings.ToLower(command)
	for _, pattern := range destructivePatterns {
		if strings.Contains(cmd, strings.ToLower(pattern)) {
			return true
		}
	}

	return false
}

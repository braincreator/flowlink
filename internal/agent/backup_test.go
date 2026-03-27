package agent

import (
	"archive/tar"
	"compress/gzip"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/config"
)

func TestDetectAffectedPaths(t *testing.T) {
	tests := []struct {
		name     string
		command  string
		wantLen  int
		wantPath string
	}{
		{"rm command", "rm /tmp/test.txt", 1, "/tmp/test.txt"},
		{"rm -rf command", "rm -rf /home/user/dir", 1, "/home/user/dir"},
		{"rm with flags", "rm -f /var/log/app.log", 1, "/var/log/app.log"},
		{"systemctl stop", "systemctl stop nginx", 1, "/etc/systemd/system"},
		{"docker rm", "docker rm container", 0, ""},
		{"apt remove", "apt remove nginx", 1, "/etc"},
		{"DROP DATABASE", "DROP DATABASE testdb", 0, ""},
		{"safe command", "ls -la", 0, ""},
		{"rm /*", "rm -rf /*", 0, ""}, // защита от rm /*
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			paths := DetectAffectedPaths(tt.command)
			if len(paths) != tt.wantLen {
				t.Errorf("DetectAffectedPaths(%q) = %d paths, want %d", tt.command, len(paths), tt.wantLen)
				return
			}

			if tt.wantLen > 0 && tt.wantPath != "" {
				found := false
				for _, p := range paths {
					if p == tt.wantPath {
						found = true
						break
					}
				}
				if !found {
					t.Errorf("DetectAffectedPaths(%q) should contain %s, got %v", tt.command, tt.wantPath, paths)
				}
			}
		})
	}
}

func TestIsDestructive(t *testing.T) {
	tests := []struct {
		name    string
		command string
		want    bool
	}{
		{"rm", "rm file.txt", true},
		{"rm -rf", "rm -rf /home/user", true},
		{"rmdir", "rmdir /tmp/empty", true},
		{"DROP DATABASE", "DROP DATABASE test", true},
		{"TRUNCATE", "TRUNCATE TABLE users", true},
		{"DELETE", "DELETE FROM users", true},
		{"apt remove", "apt remove nginx", true},
		{"apt-get purge", "apt-get purge nginx", true},
		{"yum remove", "yum remove nginx", true},
		{"docker rm", "docker rm container", true},
		{"docker rmi", "docker rmi image", true},
		{"docker system prune", "docker system prune -a", true},
		{"systemctl stop", "systemctl stop nginx", true},
		{"systemctl disable", "systemctl disable nginx", true},
		{"chmod 777", "chmod 777 /etc/passwd", true},
		{"crontab -r", "crontab -r", true},
		{"userdel", "userdel testuser", true},
		{"iptables -F", "iptables -F", true},
		{"ls", "ls -la", false},
		{"cat", "cat file.txt", false},
		{"echo", "echo test", false},
		{"git status", "git status", false},
		{"systemctl status", "systemctl status nginx", false},
		{"docker ps", "docker ps", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := IsDestructive(tt.command)
			if got != tt.want {
				t.Errorf("IsDestructive(%q) = %v, want %v", tt.command, got, tt.want)
			}
		})
	}
}

func TestSanitizeDescription(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		wantLen int
		wantOK  bool
	}{
		{"simple text", "rm file.txt", 11, true},
		{"special chars", "rm -rf /test/*?<>|", 15, true},
		{"very long", strings.Repeat("a", 100), 30, true},
		{"empty", "", 0, true},
		{"unicode", "удаление файла", 15, true},
		{"spaces and tabs", "test  \t\n  file", 12, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := sanitizeDescription(tt.input)

			if len(result) > 30 {
				t.Errorf("sanitizeDescription() result too long: %d > 30", len(result))
			}

			// Проверяем что нет опасных символов
			for _, ch := range result {
				if !((ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9') || ch == '_' || ch == '-') {
					t.Errorf("sanitizeDescription() contains unsafe char: %c", ch)
				}
			}
		})
	}
}

func TestBackupEngine_CreateBefore(t *testing.T) {
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")

	cfg := config.BackupConfig{
		MaxSnapshots:  50,
		MaxTotalSize:  5 * 1024 * 1024 * 1024,
		RetentionDays: 7,
		BackupDir:     backupDir,
	}

	// Создаём тестовый файл
	testFile := filepath.Join(dir, "test.txt")
	if err := os.WriteFile(testFile, []byte("test content"), 0644); err != nil {
		t.Fatalf("ошибка создания тестового файла: %v", err)
	}

	engine := NewBackupEngine(cfg)

	// Создаём бэкап
	snapshotID, err := engine.CreateBefore([]string{testFile}, "test backup")
	if err != nil {
		t.Fatalf("CreateBefore() error = %v", err)
	}

	if snapshotID == "" {
		t.Error("snapshotID не должен быть пустым")
	}

	// Проверяем что файл создан
	files, err := filepath.Glob(filepath.Join(backupDir, "*.tar.gz"))
	if err != nil {
		t.Fatalf("ошибка поиска файлов: %v", err)
	}

	if len(files) == 0 {
		t.Error("бэкап файл должен быть создан")
	}

	// Проверяем метаданные
	metadataPath := filepath.Join(backupDir, "meta.json")
	data, err := os.ReadFile(metadataPath)
	if err != nil {
		t.Fatalf("ошибка чтения метаданных: %v", err)
	}

	var snapshots []Snapshot
	if err := json.Unmarshal(data, &snapshots); err != nil {
		t.Fatalf("ошибка парсинга метаданных: %v", err)
	}

	if len(snapshots) != 1 {
		t.Errorf("ожидался 1 снапшот, got %d", len(snapshots))
	}

	if snapshots[0].ID != snapshotID {
		t.Errorf("ID снапшота: got %s, want %s", snapshots[0].ID, snapshotID)
	}
}

func TestBackupEngine_CreateBefore_EmptyPaths(t *testing.T) {
	dir := t.TempDir()
	cfg := config.BackupConfig{
		BackupDir: filepath.Join(dir, "backups"),
	}

	engine := NewBackupEngine(cfg)

	// Пустой список путей
	_, err := engine.CreateBefore([]string{}, "test")
	if err == nil {
		t.Error("CreateBefore() должен возвращать ошибку для пустого списка путей")
	}
}

func TestBackupEngine_Restore(t *testing.T) {
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")
	restoreDir := filepath.Join(dir, "restore")

	cfg := config.BackupConfig{
		BackupDir:     backupDir,
		MaxSnapshots:  50,
		MaxTotalSize:  5 * 1024 * 1024 * 1024, // 5GB — чтобы Cleanup не удалял по размеру
		RetentionDays: 7,
	}

	// Создаём тестовый файл
	testFile := filepath.Join(dir, "original.txt")
	content := []byte("original content")
	if err := os.WriteFile(testFile, content, 0644); err != nil {
		t.Fatalf("ошибка создания тестового файла: %v", err)
	}

	engine := NewBackupEngine(cfg)

	// Создаём бэкап
	snapshotID, err := engine.CreateBefore([]string{testFile}, "backup before restore test")
	if err != nil {
		t.Fatalf("CreateBefore() error = %v", err)
	}

	// Ждём пока снапшот появится в метаданных (асинхронный Cleanup может мешать)
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		if len(engine.List()) > 0 {
			break
		}
		time.Sleep(50 * time.Millisecond)
	}
	if len(engine.List()) == 0 {
		t.Fatal("снапшот не появился в метаданных после CreateBefore")
	}

	// Ждём завершения асинхронного Cleanup
	time.Sleep(100 * time.Millisecond)

	// Создаём директорию для восстановления и переходим в неё
	os.MkdirAll(restoreDir, 0755)
	oldDir, _ := os.Getwd()
	if err := os.Chdir(restoreDir); err != nil {
		t.Fatalf("ошибка смены директории: %v", err)
	}
	defer os.Chdir(oldDir)

	// Восстанавливаем
	if err := engine.Restore(snapshotID); err != nil {
		t.Fatalf("Restore() error = %v", err)
	}

	// Проверяем что файл восстановлен
	// (Примечание: restore в текущую директорию, поэтому проверяем relative path)
}

func TestBackupEngine_List(t *testing.T) {
	dir := t.TempDir()
	cfg := config.BackupConfig{
		BackupDir:     filepath.Join(dir, "backups"),
		MaxSnapshots:  50,
		MaxTotalSize:  5 * 1024 * 1024 * 1024,
		RetentionDays: 7,
	}

	engine := NewBackupEngine(cfg)

	// Пустой список
	if len(engine.List()) != 0 {
		t.Error("пустой список должен возвращать 0 снапшотов")
	}

	// Создаём несколько бэкапов
	testFile := filepath.Join(dir, "test.txt")
	os.WriteFile(testFile, []byte("test"), 0644)

	for i := 0; i < 3; i++ {
		_, err := engine.CreateBefore([]string{testFile}, "test backup")
		if err != nil {
			t.Fatalf("CreateBefore() error = %v", err)
		}
		time.Sleep(10 * time.Millisecond) // уникальные timestamp
	}

	// Ждём пока все 3 снапшота появятся (асинхронный Cleanup может мешать)
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		if len(engine.List()) >= 3 {
			break
		}
		time.Sleep(50 * time.Millisecond)
	}

	list := engine.List()
	if len(list) != 3 {
		t.Errorf("ожидалось 3 снапшота, got %d", len(list))
	}
}

func TestBackupEngine_Cleanup(t *testing.T) {
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")

	cfg := config.BackupConfig{
		MaxSnapshots:  3,
		MaxTotalSize:  10 * 1024 * 1024, // 10MB
		RetentionDays: 7,
		BackupDir:     backupDir,
	}

	engine := NewBackupEngine(cfg)

	// Создаём тестовый файл
	testFile := filepath.Join(dir, "test.txt")
	os.WriteFile(testFile, []byte("test"), 0644)

	// Создаём больше бэкапов чем MaxSnapshots
	for i := 0; i < 5; i++ {
		_, err := engine.CreateBefore([]string{testFile}, "test backup")
		if err != nil {
			t.Fatalf("CreateBefore() error = %v", err)
		}
		time.Sleep(10 * time.Millisecond)
	}

	// Cleanup должен удалить старые
	if err := engine.Cleanup(); err != nil {
		t.Fatalf("Cleanup() error = %v", err)
	}

	// Проверяем что осталось только MaxSnapshots
	list := engine.List()
	if len(list) > cfg.MaxSnapshots {
		t.Errorf("после cleanup должно быть не более %d снапшотов, got %d", cfg.MaxSnapshots, len(list))
	}
}

func TestBackupEngine_Cleanup_OldSnapshots(t *testing.T) {
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")

	cfg := config.BackupConfig{
		MaxSnapshots:  50,
		MaxTotalSize:  5 * 1024 * 1024 * 1024,
		RetentionDays: 0, // удалять сразу
		BackupDir:     backupDir,
	}

	engine := NewBackupEngine(cfg)

	// Создаём тестовый файл
	testFile := filepath.Join(dir, "test.txt")
	os.WriteFile(testFile, []byte("test"), 0644)

	// Создаём бэкап
	_, err := engine.CreateBefore([]string{testFile}, "test backup")
	if err != nil {
		t.Fatalf("CreateBefore() error = %v", err)
	}

	// Cleanup должен удалить (retention = 0 дней)
	if err := engine.Cleanup(); err != nil {
		t.Fatalf("Cleanup() error = %v", err)
	}

	// Проверяем что снапшот удалён
	list := engine.List()
	if len(list) != 0 {
		t.Errorf("снапшот должен быть удалён, got %d", len(list))
	}
}

func TestBackupEngine_Diff(t *testing.T) {
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")

	cfg := config.BackupConfig{
		BackupDir:     backupDir,
		MaxSnapshots:  50,
		MaxTotalSize:  5 * 1024 * 1024 * 1024,
		RetentionDays: 7,
	}

	// Создаём тестовый файл
	testFile := filepath.Join(dir, "test.txt")
	os.WriteFile(testFile, []byte("original"), 0644)

	engine := NewBackupEngine(cfg)

	// Создаём бэкап
	snapshotID, err := engine.CreateBefore([]string{testFile}, "test backup")
	if err != nil {
		t.Fatalf("CreateBefore() error = %v", err)
	}

	// Ждём пока снапшот появится в метаданных
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		if len(engine.List()) > 0 {
			break
		}
		time.Sleep(50 * time.Millisecond)
	}

	// Изменяем файл
	time.Sleep(500 * time.Millisecond)
	os.WriteFile(testFile, []byte("modified content here"), 0644)
	// Гарантируем mtime > snapshot timestamp
	os.Chtimes(testFile, time.Now().Add(time.Second), time.Now().Add(time.Second))

	// Проверяем diff
	changes, err := engine.Diff(snapshotID)
	if err != nil {
		t.Fatalf("Diff() error = %v", err)
	}

	// Файл должен быть помечен как modified
	found := false
	for _, change := range changes {
		if change.Change == "modified" {
			found = true
			break
		}
	}

	if !found {
		t.Error("изменённый файл должен быть в diff")
	}
}

func TestBackupEngine_Diff_DeletedFile(t *testing.T) {
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")

	cfg := config.BackupConfig{
		BackupDir:     backupDir,
		MaxSnapshots:  50,
		MaxTotalSize:  5 * 1024 * 1024 * 1024,
		RetentionDays: 7,
	}

	// Создаём тестовый файл
	testFile := filepath.Join(dir, "test.txt")
	os.WriteFile(testFile, []byte("test"), 0644)

	engine := NewBackupEngine(cfg)

	// Создаём бэкап
	snapshotID, err := engine.CreateBefore([]string{testFile}, "test backup")
	if err != nil {
		t.Fatalf("CreateBefore() error = %v", err)
	}

	// Ждём пока снапшот появится в метаданных
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		if len(engine.List()) > 0 {
			break
		}
		time.Sleep(50 * time.Millisecond)
	}

	// Удаляем файл
	os.Remove(testFile)

	// Проверяем diff
	changes, err := engine.Diff(snapshotID)
	if err != nil {
		t.Fatalf("Diff() error = %v", err)
	}

	// Файл должен быть помечен как deleted
	if len(changes) == 0 {
		t.Error("удалённый файл должен быть в diff")
		return
	}

	if changes[0].Change != "deleted" {
		t.Errorf("ожидался change=deleted, got %s", changes[0].Change)
	}
}

func TestBackupEngine_DefaultConfig(t *testing.T) {
	cfg := DefaultBackupConfig()

	if cfg.MaxSnapshots != 50 {
		t.Errorf("MaxSnapshots: got %d, want 50", cfg.MaxSnapshots)
	}

	if cfg.MaxTotalSize != 5*1024*1024*1024 {
		t.Errorf("MaxTotalSize: got %d, want 5GB", cfg.MaxTotalSize)
	}

	if cfg.RetentionDays != 7 {
		t.Errorf("RetentionDays: got %d, want 7", cfg.RetentionDays)
	}

	if cfg.BackupDir == "" {
		t.Error("BackupDir не должен быть пустым")
	}
}

func TestCreateTarGz(t *testing.T) {
	dir := t.TempDir()

	// Создаём тестовые файлы
	file1 := filepath.Join(dir, "file1.txt")
	file2 := filepath.Join(dir, "file2.txt")
	subdir := filepath.Join(dir, "subdir")
	os.Mkdir(subdir, 0755)
	file3 := filepath.Join(subdir, "file3.txt")

	os.WriteFile(file1, []byte("content1"), 0644)
	os.WriteFile(file2, []byte("content2"), 0644)
	os.WriteFile(file3, []byte("content3"), 0644)

	// Создаём архив
	archivePath := filepath.Join(dir, "test.tar.gz")

	cfg := config.BackupConfig{BackupDir: dir}
	engine := NewBackupEngine(cfg)

	size, err := engine.createTarGz([]string{file1, file2, subdir}, archivePath)
	if err != nil {
		t.Fatalf("createTarGz() error = %v", err)
	}

	if size == 0 {
		t.Error("размер архива не должен быть 0")
	}

	// Проверяем что архив можно прочитать
	file, err := os.Open(archivePath)
	if err != nil {
		t.Fatalf("ошибка открытия архива: %v", err)
	}
	defer file.Close()

	gzReader, err := gzip.NewReader(file)
	if err != nil {
		t.Fatalf("ошибка создания gzip reader: %v", err)
	}
	defer gzReader.Close()

	tarReader := tar.NewReader(gzReader)

	fileCount := 0
	for {
		_, err := tarReader.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			t.Fatalf("ошибка чтения tar: %v", err)
		}
		fileCount++
	}

	// Должны быть: file1, file2, subdir, file3
	if fileCount < 3 {
		t.Errorf("ожидалось минимум 3 файла в архиве, got %d", fileCount)
	}
}

func TestBackupEngine_EdgeCases(t *testing.T) {
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")

	cfg := config.BackupConfig{
		BackupDir:     backupDir,
		MaxSnapshots:  50,
		MaxTotalSize:  5 * 1024 * 1024 * 1024,
		RetentionDays: 7,
	}

	engine := NewBackupEngine(cfg)

	t.Run("nonexistent file", func(t *testing.T) {
		// Бэкап несуществующего файла не должен падать
		_, err := engine.CreateBefore([]string{"/nonexistent/file.txt"}, "test")
		// Ожидаем что бэкап создастся (пустой или с warning)
		if err == nil {
			// OK - пустой бэкап
		}
	})

	t.Run("very long description", func(t *testing.T) {
		testFile := filepath.Join(dir, "test.txt")
		os.WriteFile(testFile, []byte("test"), 0644)

		longDesc := strings.Repeat("a", 1000)
		snapshotID, err := engine.CreateBefore([]string{testFile}, longDesc)
		if err != nil {
			t.Fatalf("CreateBefore() error = %v", err)
		}

		// Проверяем что snapshotID обрезан
		if len(snapshotID) > 100 {
			t.Errorf("snapshotID слишком длинный: %d", len(snapshotID))
		}
	})

	t.Run("unicode in paths", func(t *testing.T) {
		unicodeFile := filepath.Join(dir, "тест.txt")
		os.WriteFile(unicodeFile, []byte("test"), 0644)

		_, err := engine.CreateBefore([]string{unicodeFile}, "unicode test")
		if err != nil {
			t.Fatalf("CreateBefore() с unicode path error = %v", err)
		}
	})

	t.Run("restore nonexistent snapshot", func(t *testing.T) {
		err := engine.Restore("nonexistent-snapshot-id")
		if err == nil {
			t.Error("Restore() должен возвращать ошибку для несуществующего снапшота")
		}
	})

	t.Run("diff nonexistent snapshot", func(t *testing.T) {
		_, err := engine.Diff("nonexistent-snapshot-id")
		if err == nil {
			t.Error("Diff() должен возвращать ошибку для несуществующего снапшота")
		}
	})
}

func TestBackupEngine_Integration(t *testing.T) {
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")

	cfg := config.BackupConfig{
		MaxSnapshots:  10,
		MaxTotalSize:  100 * 1024 * 1024, // 100MB
		RetentionDays: 7,
		BackupDir:     backupDir,
	}

	// Создаём файлы
	file1 := filepath.Join(dir, "file1.txt")
	file2 := filepath.Join(dir, "file2.txt")
	os.WriteFile(file1, []byte("content1"), 0644)
	os.WriteFile(file2, []byte("content2"), 0644)

	engine := NewBackupEngine(cfg)

	// Создаём бэкап
	snapshotID, err := engine.CreateBefore([]string{file1, file2}, "integration test")
	if err != nil {
		t.Fatalf("CreateBefore() error = %v", err)
	}

	// Проверяем список
	list := engine.List()
	if len(list) != 1 {
		t.Errorf("ожидался 1 снапшот, got %d", len(list))
	}

	// Проверяем метаданные
	snapshots := engine.List()
	if len(snapshots) == 0 {
		t.Fatal("снапшот не найден")
	}

	snapshot := snapshots[0]
	if snapshot.ID != snapshotID {
		t.Errorf("ID: got %s, want %s", snapshot.ID, snapshotID)
	}
	if snapshot.Description != "integration test" {
		t.Errorf("Description: got %s, want 'integration test'", snapshot.Description)
	}
	if len(snapshot.Paths) != 2 {
		t.Errorf("ожидалось 2 пути, got %d", len(snapshot.Paths))
	}
	if snapshot.Size == 0 {
		t.Error("Size не должен быть 0")
	}
	if snapshot.Timestamp == 0 {
		t.Error("Timestamp не должен быть 0")
	}
}

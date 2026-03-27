package agent

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/config"
)

func TestBackupDebug3(t *testing.T) {
	dir := t.TempDir()
	backupDir := filepath.Join(dir, "backups")

	cfg := config.BackupConfig{
		BackupDir:     backupDir,
		MaxSnapshots:  50,
		RetentionDays: 7,
	}

	testFile := filepath.Join(dir, "test.txt")
	os.WriteFile(testFile, []byte("test"), 0644)

	engine := NewBackupEngine(cfg)

	// Вызываем CreateBefore без go Cleanup — вручную проверяем
	snapshotID := "123_test"
	filename := snapshotID + ".tar.gz"
	backupPath := filepath.Join(backupDir, filename)
	
	size, err := engine.createTarGz([]string{testFile}, backupPath)
	t.Logf("createTarGz: size=%d err=%v", size, err)

	snapshot := struct {
		ID          string   `json:"id"`
		Description string   `json:"description"`
		Timestamp   int64    `json:"timestamp"`
		Size        int64    `json:"size"`
		Paths       []string `json:"paths"`
		Filename    string   `json:"filename"`
	}{
		ID:          snapshotID,
		Description: "test",
		Timestamp:   time.Now().Unix(),
		Size:        size,
		Paths:       []string{testFile},
		Filename:    filename,
	}

	err = engine.saveMetadata(snapshot)
	t.Logf("saveMetadata: err=%v", err)

	time.Sleep(200 * time.Millisecond)

	data, _ := os.ReadFile(filepath.Join(backupDir, "meta.json"))
	t.Logf("meta.json: %s", string(data))

	// Теперь вызываем Cleanup
	err = engine.Cleanup()
	t.Logf("Cleanup: err=%v", err)

	data2, _ := os.ReadFile(filepath.Join(backupDir, "meta.json"))
	t.Logf("meta.json after cleanup: %s", string(data2))
}

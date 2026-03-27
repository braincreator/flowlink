package agent

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/config"
)

func TestBackupDebug(t *testing.T) {
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

	snapshotID, err := engine.CreateBefore([]string{testFile}, "test backup")
	t.Logf("CreateBefore: id=%s err=%v", snapshotID, err)

	time.Sleep(500 * time.Millisecond)

	// Проверяем meta.json напрямую
	data, _ := os.ReadFile(filepath.Join(backupDir, "meta.json"))
	t.Logf("meta.json content: %s", string(data))

	t.Logf("List(): %d", len(engine.List()))
	for _, s := range engine.List() {
		t.Logf("  snapshot: id=%s ts=%d", s.ID, s.Timestamp)
	}
}

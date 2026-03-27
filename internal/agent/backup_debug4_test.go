package agent

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/braincreator/flowlink/internal/config"
)

func TestBackupDebug4(t *testing.T) {
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

	// НЕ ждём — сразу читаем
	data, _ := os.ReadFile(filepath.Join(backupDir, "meta.json"))
	t.Logf("meta.json immediately: %s", string(data))

	time.Sleep(100 * time.Millisecond)
	data2, _ := os.ReadFile(filepath.Join(backupDir, "meta.json"))
	t.Logf("meta.json after 100ms: %s", string(data2))

	// Проверяем List напрямую
	list := engine.List()
	t.Logf("List count: %d", len(list))
	for _, s := range list {
		now := time.Now()
		cutoff := now.AddDate(0, 0, -7)
		st := time.Unix(s.Timestamp, 0)
		t.Logf("  id=%s ts=%d before_cutoff=%v", s.ID, s.Timestamp, st.Before(cutoff))
	}

	// Ручной Cleanup
	t.Log("Calling Cleanup manually...")
	engine.Cleanup()
	data3, _ := os.ReadFile(filepath.Join(backupDir, "meta.json"))
	t.Logf("meta.json after manual cleanup: %s", string(data3))
}

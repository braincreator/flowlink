package relay

import (
	"os"
	"testing"
)

func TestMain(m *testing.M) {
	// Signal constructors to skip background goroutines in test mode
	os.Setenv("FLOWLINK_TEST_MODE", "1")
	code := m.Run()
	os.Exit(code)
}

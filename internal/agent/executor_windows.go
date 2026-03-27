//go:build windows

package agent

// getShellCommand — возвращает shell для Windows.
func getShellCommand() string {
	return "cmd.exe"
}

// getShellArgs — аргументы для Windows shell.
func getShellArgs(command string) []string {
	return []string{"/C", command}
}

//go:build !windows

package agent

// getShellCommand — возвращает shell для Unix-систем.
func getShellCommand() string {
	return "/bin/sh"
}

// getShellArgs — аргументы для Unix-систем shell.
func getShellArgs(command string) []string {
	return []string{"-c", command}
}

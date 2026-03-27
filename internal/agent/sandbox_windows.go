//go:build windows

package agent

// windowsBlockedPatterns — Windows-specific опасные команды, блокируемые sandbox.
var windowsBlockedPatterns = []string{
	"format ",
	"del /f /s /q C:\\",
	"rd /s /q C:\\",
	"reg delete",
	"taskkill /f",
	"net user",
	"net localgroup administrators",
	"powershell -command Remove-Item",
	"cmd /c format",
	"bcdedit",
	"diskpart",
}

package main

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"runtime"
	"syscall"
	"time"

	"github.com/braincreator/flowlink/internal/agent"
	"github.com/braincreator/flowlink/internal/config"
	"github.com/braincreator/flowlink/pkg/version"
)

func main() {
	if len(os.Args) < 2 {
		printUsage()
		os.Exit(1)
	}

	// Parse global flags
	verbose := false
	for i := 1; i < len(os.Args); i++ {
		if os.Args[i] == "-v" || os.Args[i] == "--verbose" {
			verbose = true
		}
	}

	// Setup logging
	logLevel := slog.LevelInfo
	if verbose {
		logLevel = slog.LevelDebug
	}
	logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: logLevel}))
	slog.SetDefault(logger)

	// Route to subcommands
	switch os.Args[1] {
	case "version", "--version", "-v":
		cmdVersion()

	case "agent":
		cmdAgent(os.Args[2:])

	case "init", "--init":
		cmdInit(os.Args[2:])

	case "emergency":
		cmdEmergency(os.Args[2:])

	case "help", "--help", "-h":
		printUsage()

	default:
		fmt.Fprintf(os.Stderr, "Unknown command: %s\n\n", os.Args[1])
		printUsage()
		os.Exit(1)
	}
}

func printUsage() {
	fmt.Printf("flowlink %s - Remote Execution Agent\n\n", version.Version)
	fmt.Println("Usage:")
	fmt.Println("  flowlink init                    Interactive initialization")
	fmt.Println("  flowlink agent start             Start agent (foreground)")
	fmt.Println("  flowlink agent stop              Stop running agent")
	fmt.Println("  flowlink agent status            Show connection status")
	fmt.Println("  flowlink version                 Show version info")
	fmt.Println("  flowlink emergency               Emergency stop all operations")
	fmt.Println()
	fmt.Println("Flags:")
	fmt.Println("  -v, --verbose    Enable debug logging")
	fmt.Println("  -h, --help       Show this help")
	fmt.Println()
	fmt.Println("Examples:")
	fmt.Println("  flowlink init --relay wss://relay.example.com/ws --label server1")
	fmt.Println("  flowlink agent start")
	fmt.Println("  flowlink agent status")
}

// ====================
// Version Command
// ====================

func cmdVersion() {
	fmt.Printf("flowlink %s (%s/%s)\n", version.Version, runtime.GOOS, runtime.GOARCH)
	fmt.Printf("  commit: %s\n", version.GitCommit)
	fmt.Printf("  built:  %s\n", version.BuildDate)
}

// ====================
// Init Command
// ====================

func cmdInit(args []string) {
	var (
		relayURL string
		label    string
		token    string
		approval string
	)

	// Parse flags
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--relay", "-r":
			if i+1 < len(args) {
				relayURL = args[i+1]
				i++
			}
		case "--label", "-l":
			if i+1 < len(args) {
				label = args[i+1]
				i++
			}
		case "--token", "-t":
			if i+1 < len(args) {
				token = args[i+1]
				i++
			}
		case "--approval", "-a":
			if i+1 < len(args) {
				approval = args[i+1]
				i++
			}
		}
	}

	// Interactive prompts if not provided
	if relayURL == "" {
		fmt.Print("Relay URL [wss://relay.flowmasters.ru/ws]: ")
		fmt.Scanln(&relayURL)
		if relayURL == "" {
			relayURL = "wss://relay.flowmasters.ru/ws"
		}
	}

	if label == "" {
		hostname, _ := os.Hostname()
		fmt.Printf("Agent label [%s]: ", hostname)
		fmt.Scanln(&label)
		if label == "" {
			label = hostname
		}
	}

	// Generate config
	cfg := config.DefaultConfig()

	// Generate agent_id
	idBytes := make([]byte, 16)
	if _, err := rand.Read(idBytes); err != nil {
		slog.Error("failed to generate agent_id", "err", err)
		os.Exit(1)
	}
	cfg.AgentID = hex.EncodeToString(idBytes)

	// Generate or use provided token
	if token != "" {
		cfg.Token = token
	} else {
		tokenBytes := make([]byte, 32)
		if _, err := rand.Read(tokenBytes); err != nil {
			slog.Error("failed to generate token", "err", err)
			os.Exit(1)
		}
		cfg.Token = hex.EncodeToString(tokenBytes)
	}

	// Set user-provided values
	cfg.RelayURL = relayURL
	cfg.Label = label
	if approval != "" {
		cfg.Approval.Mode = approval
	}

	// Save config
	if err := config.SaveConfig(&cfg); err != nil {
		slog.Error("failed to save config", "err", err)
		os.Exit(1)
	}

	// Print credentials
	fmt.Println()
	fmt.Println("✅ FlowLink initialized successfully!")
	fmt.Println()
	fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
	fmt.Println("  Agent Credentials")
	fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
	fmt.Printf("  Agent ID:  %s\n", cfg.AgentID)
	fmt.Printf("  Token:     %s\n", cfg.Token)
	fmt.Printf("  Relay:     %s\n", cfg.RelayURL)
	fmt.Printf("  Label:     %s\n", cfg.Label)
	fmt.Printf("  Approval:  %s\n", cfg.Approval.Mode)
	fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
	fmt.Println()
	fmt.Println("⚠️  IMPORTANT: Save these credentials!")
	fmt.Println("   Send Agent ID and Token to the relay operator.")
	fmt.Println()
	fmt.Println("Config saved to: ~/.flowlink/config.json")
	fmt.Println()
	fmt.Println("Next steps:")
	fmt.Println("  1. Send credentials to relay operator")
	fmt.Println("  2. Start agent: flowlink agent start")
	fmt.Println()
}

// ====================
// Agent Command
// ====================

func cmdAgent(args []string) {
	if len(args) < 1 {
		fmt.Fprintln(os.Stderr, "Usage: flowlink agent {start|stop|status}")
		os.Exit(1)
	}

	switch args[0] {
	case "start":
		agentStart()
	case "stop":
		agentStop()
	case "status":
		agentStatus()
	default:
		fmt.Fprintf(os.Stderr, "Unknown agent command: %s\n", args[0])
		os.Exit(1)
	}
}

func agentStart() {
	cfg, err := config.LoadConfig()
	if err != nil {
		slog.Error("failed to load config", "err", err)
		fmt.Fprintln(os.Stderr, "Run 'flowlink init' first")
		os.Exit(1)
	}

	if cfg.AgentID == "" || cfg.Token == "" {
		fmt.Fprintln(os.Stderr, "Config not initialized. Run: flowlink init")
		os.Exit(1)
	}

	slog.Info("starting flowlink agent",
		"version", version.Version,
		"agent", cfg.AgentID,
		"label", cfg.Label,
		"relay", cfg.RelayURL,
	)

	a := agent.NewAgent(cfg)

	// Graceful shutdown
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		sig := <-sigCh
		slog.Info("received signal, shutting down...", "signal", sig)
		cancel()
		a.Disconnect()
		os.Exit(0)
	}()

	// Connect to relay with auto-reconnect
	maxRetries := 0 // 0 = infinite
	for attempt := 0; maxRetries == 0 || attempt < maxRetries; attempt++ {
		if attempt > 0 {
			backoff := time.Duration(min(30, 2<<uint(min(attempt, 4)))) * time.Second
			slog.Info("reconnecting...", "attempt", attempt+1, "backoff", backoff)
			select {
			case <-ctx.Done():
				return
			case <-time.After(backoff):
			}
		}
		if err := a.Connect(ctx); err != nil {
			slog.Error("connection failed", "err", err)
			if attempt >= 10 {
				slog.Error("too many retries, waiting 60s before continuing")
				select {
				case <-ctx.Done():
					return
				case <-time.After(60 * time.Second):
				}
			}
			continue
		}
		slog.Info("connected to relay")
		// Connection lost — reconnect
		slog.Warn("connection lost, will reconnect...")
	}

	// Block
	<-ctx.Done()
}

func agentStop() {
	// Find and kill running flowlink process
	pid, err := findFlowlinkPID()
	if err != nil || pid == 0 {
		fmt.Println("FlowLink agent is not running")
		return
	}

	// Send SIGTERM
	process, err := os.FindProcess(pid)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to find process: %v\n", err)
		os.Exit(1)
	}

	if err := process.Signal(syscall.SIGTERM); err != nil {
		fmt.Fprintf(os.Stderr, "Failed to stop agent: %v\n", err)
		os.Exit(1)
	}

	fmt.Println("✅ FlowLink agent stopped")

	// Wait for process to exit
	for i := 0; i < 10; i++ {
		time.Sleep(500 * time.Millisecond)
		if !isProcessRunning(pid) {
			return
		}
	}

	// Force kill if still running
	fmt.Println("Agent didn't stop gracefully, force killing...")
	process.Signal(syscall.SIGKILL)
}

func agentStatus() {
	cfg, err := config.LoadConfig()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to load config: %v\n", err)
		os.Exit(1)
	}

	fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
	fmt.Println("  FlowLink Agent Status")
	fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
	fmt.Println()

	// Check if running
	pid, err := findFlowlinkPID()
	if err != nil || pid == 0 {
		fmt.Println("  Status:     ❌ Not running")
	} else {
		fmt.Printf("  Status:     ✅ Running (PID: %d)\n", pid)
	}

	fmt.Printf("  Agent ID:   %s\n", cfg.AgentID)
	fmt.Printf("  Label:      %s\n", cfg.Label)
	fmt.Printf("  Relay:      %s\n", cfg.RelayURL)
	fmt.Printf("  Approval:   %s\n", cfg.Approval.Mode)
	fmt.Printf("  Version:    %s\n", version.Version)
	fmt.Println()
	fmt.Println("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
}

// ====================
// Emergency Command
// ====================

func cmdEmergency(args []string) {
	fmt.Println("🚨 EMERGENCY STOP")
	fmt.Println()
	fmt.Println("Stopping all FlowLink operations...")

	// Kill all flowlink processes
	pid, err := findFlowlinkPID()
	if err != nil || pid == 0 {
		fmt.Println("No running FlowLink agent found")
		return
	}

	process, _ := os.FindProcess(pid)
	if process != nil {
		process.Signal(syscall.SIGKILL)
		fmt.Printf("✅ Killed process %d\n", pid)
	}

	// Create emergency lock file
	lockFile := os.ExpandEnv("$HOME/.flowlink/.emergency_lock")
	if err := os.WriteFile(lockFile, []byte(time.Now().Format(time.RFC3339)), 0600); err != nil {
		slog.Warn("failed to create emergency lock", "err", err)
	}

	fmt.Println()
	fmt.Println("Emergency lock created. Agent will not restart automatically.")
	fmt.Println("To resume: rm ~/.flowlink/.emergency_lock && flowlink agent start")
}

// ====================
// Helpers
// ====================

func findFlowlinkPID() (int, error) {
	// Read PID file if exists
	pidFile := os.ExpandEnv("$HOME/.flowlink/flowlink.pid")
	if data, err := os.ReadFile(pidFile); err == nil {
		var pid int
		if _, err := fmt.Sscanf(string(data), "%d", &pid); err == nil {
			if isProcessRunning(pid) {
				return pid, nil
			}
		}
	}

	// Fallback: check if current process is running agent
	// In production, this would use pgrep or similar
	return 0, fmt.Errorf("not found")
}

func isProcessRunning(pid int) bool {
	process, err := os.FindProcess(pid)
	if err != nil {
		return false
	}
	err = process.Signal(syscall.Signal(0))
	return err == nil
}

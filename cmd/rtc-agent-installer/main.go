package main

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/remote-terminal-cloud/agent/internal/agent"
)

func main() {
	if err := run(os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, err.Error())
		os.Exit(1)
	}
}

func run(args []string) error {
	if len(args) == 0 {
		printHelp()
		return nil
	}

	command := strings.ToLower(strings.TrimSpace(args[0]))
	switch command {
	case "help", "-h", "--help":
		printHelp()
		return nil
	case "windows":
		return runWindowsCommand(args[1:])
	default:
		return fmt.Errorf("unknown installer command: %s", args[0])
	}
}

func runWindowsCommand(args []string) error {
	if runtime.GOOS != "windows" {
		return fmt.Errorf("windows installer commands can only run on Windows")
	}
	if len(args) == 0 {
		printWindowsHelp()
		return nil
	}

	action := strings.ToLower(strings.TrimSpace(args[0]))
	switch action {
	case "init-config":
		return agent.EnsureManagedConfigFile()
	case "save-token":
		if len(args) < 2 {
			return fmt.Errorf("windows save-token requires a token value")
		}
		return agent.SaveManagedRegistrationToken(args[1])
	case "stop-service":
		return agent.StopWindowsService(resolveInstallRootArg(args[1:]))
	case "install-service":
		installRoot, token := parseInstallArgs(args[1:])
		return agent.InstallWindowsService(installRoot, token)
	case "uninstall-service":
		return agent.UninstallWindowsService(resolveInstallRootArg(args[1:]))
	case "open-config-dir":
		return agent.OpenPathInExplorer(agent.GetManagedConfigDir())
	case "open-logs":
		return agent.OpenPathInExplorer(agent.GetManagedLogsDir())
	default:
		return fmt.Errorf("unknown windows installer action: %s", args[0])
	}
}

func parseInstallArgs(args []string) (string, string) {
	installRoot := ""
	token := ""
	if len(args) > 0 {
		installRoot = args[0]
	}
	if len(args) > 1 {
		token = args[1]
	}
	return installRoot, token
}

func resolveInstallRootArg(args []string) string {
	if len(args) == 0 {
		return ""
	}
	return strings.TrimSpace(args[0])
}

func printHelp() {
	fmt.Println("Usage:")
	fmt.Println("  rtc-agent-installer windows <command>")
	fmt.Println("")
	fmt.Println("Commands:")
	fmt.Println("  windows init-config")
	fmt.Println("  windows save-token <token>")
	fmt.Println("  windows stop-service [install-root]")
	fmt.Println("  windows install-service [install-root] [token]")
	fmt.Println("  windows uninstall-service [install-root]")
	fmt.Println("  windows open-config-dir")
	fmt.Println("  windows open-logs")
}

func printWindowsHelp() {
	printHelp()
}

func executableDir() string {
	exe, err := os.Executable()
	if err != nil {
		return "."
	}
	return filepath.Dir(exe)
}

package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/remote-terminal-cloud/agent/internal/agent"
	"github.com/remote-terminal-cloud/agent/internal/buildinfo"
	"github.com/remote-terminal-cloud/agent/internal/protocol"
)

const (
	missingConfigRetry   = 30 * time.Second
	runtimeRetryInterval = 10 * time.Second
)

func main() {
	agentVersion := buildinfo.LoadVersionFromFile()

	if handled, err := handleCommand(os.Args[1:]); handled {
		if err != nil {
			fmt.Printf("[remote-terminal-cloud-agent] command error: %v\n", err)
			os.Exit(1)
		}
		return
	}

	for {
		if err := runAgentOnce(context.Background(), agentVersion); err != nil {
			fmt.Printf("[remote-terminal-cloud-agent] runtime error; retrying %v\n", err)
			time.Sleep(runtimeRetryInterval)
		}
	}
}

func handleCommand(args []string) (bool, error) {
	if len(args) == 0 {
		return false, nil
	}

	command := strings.ToLower(strings.TrimSpace(args[0]))
	rest := args[1:]

	switch command {
	case "configure", "conf":
		return true, runConfigureCommand()
	case "version", "ver":
		return true, runVersionCommand()
	case "paths", "path":
		return true, runPathsCommand()
	case "config":
		return true, runConfigCommand()
	case "status":
		return true, runStatusCommand()
	case "doctor", "diag", "diagnose":
		return true, runDoctorCommand()
	case "shells", "shell":
		return true, runShellsCommand()
	case "help", "-h", "--help":
		printHelp(rest)
		return true, nil
	default:
		printHelp(nil)
		return true, fmt.Errorf("unknown command: %s", args[0])
	}
}

func runConfigureCommand() error {
	config := agent.GetRuntimeConfig()
	fmt.Printf("[remote-terminal-cloud-agent] config file: %s\n", config.ConfigFilePath)

	if !agent.IsInteractiveInputAvailable() {
		return fmt.Errorf("configure requires an interactive terminal")
	}

	currentTokenSource := "config file"
	if agent.HasRegistrationTokenEnvOverride() {
		currentTokenSource = "environment variable RTC_REGISTRATION_TOKEN"
	}

	if config.RegistrationToken != nil {
		fmt.Printf("[remote-terminal-cloud-agent] a registration token is already available from %s.\n", currentTokenSource)
	}

	token, err := agent.PromptAndPersistRegistrationToken(config.ConfigFilePath)
	if err != nil {
		return err
	}
	if token == nil {
		fmt.Println("[remote-terminal-cloud-agent] no token saved.")
		return nil
	}

	fmt.Println("[remote-terminal-cloud-agent] configuration updated successfully.")
	return nil
}

func runVersionCommand() error {
	version := buildinfo.LoadVersionFromFile()
	fmt.Printf("rtc-agent version %s\n", version)
	fmt.Printf("server base URL: %s\n", buildinfo.ServerBaseURL)
	return nil
}

func runPathsCommand() error {
	config := agent.GetRuntimeConfig()
	fmt.Printf("config file: %s\n", config.ConfigFilePath)
	fmt.Printf("preferences file: %s\n", config.PreferencesFilePath)
	fmt.Printf("working directory: %s\n", mustGetwd())
	fmt.Printf("config dir: %s\n", filepath.Dir(config.ConfigFilePath))
	return nil
}

func runConfigCommand() error {
	config := agent.GetRuntimeConfig()

	tokenStatus := "missing"
	tokenSource := "none"
	if config.RegistrationToken != nil {
		tokenStatus = "configured"
		tokenSource = "config file"
		if agent.HasRegistrationTokenEnvOverride() {
			tokenSource = "environment variable RTC_REGISTRATION_TOKEN"
		}
	}

	fmt.Printf("server base URL: %s\n", config.ServerBaseURL)
	fmt.Printf("registration token: %s\n", tokenStatus)
	fmt.Printf("registration token source: %s\n", tokenSource)
	fmt.Printf("config file: %s\n", config.ConfigFilePath)
	fmt.Printf("preferences file: %s\n", config.PreferencesFilePath)
	fmt.Printf("run heartbeat: %t\n", config.RunHeartbeat)
	fmt.Printf("run tunnel: %t\n", config.RunTunnel)
	fmt.Printf("default shell: %s\n", config.DefaultShellType)
	fmt.Printf("enabled shells: %s\n", joinShells(config.EnabledShellTypes))
	return nil
}

func runStatusCommand() error {
	version := buildinfo.LoadVersionFromFile()
	config := agent.GetRuntimeConfig()
	snapshot, err := agent.CollectHostSnapshot(version, config.EnabledShellTypes)
	if err != nil {
		return err
	}

	effectiveDefaultShell := resolveDefaultShell(config.DefaultShellType, snapshot.Diagnostics.AvailableShells)

	fmt.Printf("agent version: %s\n", version)
	fmt.Printf("hostname: %s\n", snapshot.Hostname)
	fmt.Printf("platform: %s/%s\n", snapshot.Platform, snapshot.Arch)
	fmt.Printf("server base URL: %s\n", config.ServerBaseURL)
	fmt.Printf("config file: %s\n", config.ConfigFilePath)
	fmt.Printf("registration token: %s\n", boolLabel(config.RegistrationToken != nil, "configured", "missing"))
	fmt.Printf("heartbeat enabled: %t\n", config.RunHeartbeat)
	fmt.Printf("tunnel enabled: %t\n", config.RunTunnel)
	fmt.Printf("configured default shell: %s\n", config.DefaultShellType)
	fmt.Printf("effective default shell: %s\n", effectiveDefaultShell)
	fmt.Printf("available shells: %s\n", joinShells(snapshot.Diagnostics.AvailableShells))
	fmt.Printf("ssh available: %t\n", snapshot.Diagnostics.SSHCheck.Available)
	fmt.Printf("ssh detail: %s\n", snapshot.Diagnostics.SSHCheck.Detail)
	return nil
}

func runDoctorCommand() error {
	version := buildinfo.LoadVersionFromFile()
	config := agent.GetRuntimeConfig()
	snapshot, err := agent.CollectHostSnapshot(version, config.EnabledShellTypes)
	if err != nil {
		return err
	}

	effectiveDefaultShell := resolveDefaultShell(config.DefaultShellType, snapshot.Diagnostics.AvailableShells)

	fmt.Println("Doctor summary")
	fmt.Printf("- Agent version: %s\n", version)
	fmt.Printf("- Server base URL: %s\n", config.ServerBaseURL)
	fmt.Printf("- Config file: %s\n", config.ConfigFilePath)
	fmt.Printf("- Preferences file: %s\n", config.PreferencesFilePath)
	fmt.Printf("- Registration token: %s\n", boolLabel(config.RegistrationToken != nil, "configured", "missing"))
	fmt.Printf("- Available shells: %s\n", joinShells(snapshot.Diagnostics.AvailableShells))
	fmt.Printf("- Effective default shell: %s\n", effectiveDefaultShell)
	fmt.Printf("- SSH check: %s\n", snapshot.Diagnostics.SSHCheck.Detail)
	fmt.Printf("- Service manager: %s\n", snapshot.Diagnostics.ServiceManager)
	fmt.Printf("- Install formats: %s\n", strings.Join(snapshot.Diagnostics.InstallFormats, ", "))
	fmt.Printf("- Default log path: %s\n", snapshot.Diagnostics.DefaultLogPath)

	if len(snapshot.Diagnostics.Notes) > 0 {
		fmt.Println("Notes:")
		for _, note := range snapshot.Diagnostics.Notes {
			fmt.Printf("- %s\n", note)
		}
	}

	if config.RegistrationToken == nil {
		fmt.Println("Suggestion: run `rtc-agent configure` to save a registration token.")
	}
	if len(snapshot.Diagnostics.AvailableShells) == 0 {
		fmt.Println("Suggestion: install at least one supported shell or adjust RTC_ENABLED_SHELLS.")
	}
	if !snapshot.Diagnostics.SSHCheck.Available {
		fmt.Println("Suggestion: enable or install the local SSH service if your deployment depends on SSH.")
	}

	return nil
}

func runShellsCommand() error {
	version := buildinfo.LoadVersionFromFile()
	config := agent.GetRuntimeConfig()
	snapshot, err := agent.CollectHostSnapshot(version, config.EnabledShellTypes)
	if err != nil {
		return err
	}

	effectiveDefaultShell := resolveDefaultShell(config.DefaultShellType, snapshot.Diagnostics.AvailableShells)

	fmt.Printf("configured default shell: %s\n", config.DefaultShellType)
	fmt.Printf("effective default shell: %s\n", effectiveDefaultShell)
	fmt.Printf("enabled shells: %s\n", joinShells(config.EnabledShellTypes))
	fmt.Printf("detected available shells: %s\n", joinShells(snapshot.Diagnostics.AvailableShells))
	return nil
}

func printHelp(args []string) {
	if len(args) > 0 {
		printCommandHelp(strings.ToLower(strings.TrimSpace(args[0])))
		return
	}

	fmt.Println("Usage:")
	fmt.Println("  rtc-agent                    Start the agent")
	fmt.Println("  rtc-agent help [command]     Show help for all commands or one command")
	fmt.Println("")
	fmt.Println("Commands:")
	fmt.Println("  configure, conf             Prompt for and save the registration token")
	fmt.Println("  version, ver                Show agent version and server base URL")
	fmt.Println("  paths, path                 Show config, preferences, and working paths")
	fmt.Println("  config                      Show effective runtime configuration")
	fmt.Println("  status                      Show current runtime status summary")
	fmt.Println("  doctor, diag, diagnose      Run a local diagnostics summary")
	fmt.Println("  shells, shell               Show shell configuration and detection")
	fmt.Println("  help                        Show CLI help")
	fmt.Println("")
	fmt.Println("Examples:")
	fmt.Println("  rtc-agent configure")
	fmt.Println("  rtc-agent status")
	fmt.Println("  rtc-agent help doctor")
}

func printCommandHelp(command string) {
	switch command {
	case "configure", "conf":
		fmt.Println("rtc-agent configure")
		fmt.Println("  Open an interactive prompt to save the registration token into config.json.")
	case "version", "ver":
		fmt.Println("rtc-agent version")
		fmt.Println("  Show the agent version and the built-in server base URL.")
	case "paths", "path":
		fmt.Println("rtc-agent paths")
		fmt.Println("  Show important local file paths used by the agent.")
	case "config":
		fmt.Println("rtc-agent config")
		fmt.Println("  Show the effective runtime configuration without printing the token value.")
	case "status":
		fmt.Println("rtc-agent status")
		fmt.Println("  Show the current host, shell, token, SSH, and feature status.")
	case "doctor", "diag", "diagnose":
		fmt.Println("rtc-agent doctor")
		fmt.Println("  Run local diagnostics and print actionable suggestions.")
	case "shells", "shell":
		fmt.Println("rtc-agent shells")
		fmt.Println("  Show configured and detected shell availability.")
	case "help", "-h", "--help":
		fmt.Println("rtc-agent help [command]")
		fmt.Println("  Show the top-level help or command-specific help.")
	default:
		fmt.Printf("No detailed help for command: %s\n", command)
		fmt.Println("Run `rtc-agent help` to see available commands.")
	}
}

func runAgentOnce(ctx context.Context, agentVersion string) error {
	config := agent.GetRuntimeConfig()

	snapshot, err := agent.CollectHostSnapshot(agentVersion, config.EnabledShellTypes)
	if err != nil {
		return err
	}

	effectiveDefaultShell := resolveDefaultShell(config.DefaultShellType, snapshot.Diagnostics.AvailableShells)

	fmt.Printf("[remote-terminal-cloud-agent] config file: %s\n", config.ConfigFilePath)
	fmt.Println("[remote-terminal-cloud-agent] host snapshot")
	if err := writeJSON(os.Stdout, snapshot); err != nil {
		return err
	}
	fmt.Printf("[remote-terminal-cloud-agent] shell capabilities: %s\n", joinShells(snapshot.Diagnostics.AvailableShells))

	if effectiveDefaultShell != config.DefaultShellType {
		fmt.Printf("[remote-terminal-cloud-agent] RTC_DEFAULT_SHELL=%s is unavailable; fallback to %s.\n", config.DefaultShellType, effectiveDefaultShell)
	}
	if len(snapshot.Diagnostics.AvailableShells) == 0 {
		fmt.Println("[remote-terminal-cloud-agent] no shells available after detection/config filtering.")
	}
	if !snapshot.Diagnostics.SSHCheck.Available {
		fmt.Println("[remote-terminal-cloud-agent] SSH precheck failed.")
	}

	if config.RegistrationToken == nil {
		token, err := agent.PromptAndPersistRegistrationToken(config.ConfigFilePath)
		if err != nil {
			return err
		}
		if token != nil {
			config.RegistrationToken = token
		}
	}

	if config.RegistrationToken == nil {
		if agent.IsInteractiveInputAvailable() {
			fmt.Printf("[remote-terminal-cloud-agent] registration token is still empty. Update %s or set RTC_REGISTRATION_TOKEN, then the agent will retry automatically.\n", config.ConfigFilePath)
		} else {
			fmt.Printf("[remote-terminal-cloud-agent] waiting for configuration: set RTC_REGISTRATION_TOKEN in %s or environment, then the service will retry automatically.\n", config.ConfigFilePath)
		}
		time.Sleep(missingConfigRetry)
		return nil
	}

	apiClient := agent.NewAPIClient()
	session, err := apiClient.RegisterAgent(ctx, config.ServerBaseURL, *config.RegistrationToken, snapshot)
	if err != nil {
		return err
	}
	fmt.Printf("[remote-terminal-cloud-agent] registered device %s\n", session.DeviceID)

	if !config.RunHeartbeat && !config.RunTunnel {
		fmt.Println("[remote-terminal-cloud-agent] heartbeat and tunnel are both disabled; retrying later.")
		time.Sleep(missingConfigRetry)
		return nil
	}

	runnerCtx, cancel := context.WithCancel(ctx)
	defer cancel()

	errCh := make(chan error, 2)

	if config.RunHeartbeat {
		go func(initialSession agent.RegisteredAgentSession) {
			currentSession := initialSession
			for {
				select {
				case <-runnerCtx.Done():
					return
				case <-time.After(time.Duration(currentSession.HeartbeatIntervalSeconds) * time.Second):
				}

				heartbeatSnapshot, err := agent.CollectHostSnapshot(agentVersion, config.EnabledShellTypes)
				if err != nil {
					errCh <- err
					return
				}

				currentSession, err = apiClient.SendHeartbeat(runnerCtx, config.ServerBaseURL, currentSession, heartbeatSnapshot)
				if err != nil {
					errCh <- err
					return
				}

				fmt.Printf("[remote-terminal-cloud-agent] heartbeat ok for %s; next interval %ds\n", currentSession.DeviceID, currentSession.HeartbeatIntervalSeconds)
			}
		}(session)
	} else {
		fmt.Println("[remote-terminal-cloud-agent] heartbeat disabled by RTC_DISABLE_HEARTBEAT=1")
	}

	if config.RunTunnel {
		go func(currentSession agent.RegisteredAgentSession) {
			errCh <- agent.RunAgentTunnel(runnerCtx, config.ServerBaseURL, currentSession, effectiveDefaultShell, config.PreferencesFilePath)
		}(session)
	} else {
		fmt.Println("[remote-terminal-cloud-agent] tunnel disabled by RTC_DISABLE_TUNNEL=1")
	}

	err = <-errCh
	cancel()
	return err
}

func resolveDefaultShell(configured protocol.ShellType, available []protocol.ShellType) protocol.ShellType {
	for _, shell := range available {
		if shell == configured {
			return configured
		}
	}
	for _, shell := range available {
		if shell == protocol.ShellSystemDefault {
			return protocol.ShellSystemDefault
		}
	}
	if len(available) > 0 {
		return available[0]
	}
	return configured
}

func joinShells(items []protocol.ShellType) string {
	if len(items) == 0 {
		return "none"
	}
	result := ""
	for index, item := range items {
		if index > 0 {
			result += ", "
		}
		result += string(item)
	}
	return result
}

func writeJSON(file *os.File, value any) error {
	encoder := json.NewEncoder(file)
	encoder.SetIndent("", "  ")
	return encoder.Encode(value)
}

func boolLabel(value bool, whenTrue string, whenFalse string) string {
	if value {
		return whenTrue
	}
	return whenFalse
}

func mustGetwd() string {
	wd, err := os.Getwd()
	if err != nil {
		return "."
	}
	return wd
}

package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
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
	for {
		if err := runAgentOnce(context.Background(), agentVersion); err != nil {
			fmt.Printf("[remote-terminal-cloud-agent] runtime error; retrying %v\n", err)
			time.Sleep(runtimeRetryInterval)
		}
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
		fmt.Printf("[remote-terminal-cloud-agent] waiting for configuration: set RTC_REGISTRATION_TOKEN in %s or environment, then the service will retry automatically.\n", config.ConfigFilePath)
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

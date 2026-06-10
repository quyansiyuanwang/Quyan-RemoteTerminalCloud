package agent

import (
	"fmt"
	"os"
	"runtime"
	"strings"
	"time"

	"github.com/remote-terminal-cloud/agent/internal/protocol"
)

type platformProfile struct {
	platform       protocol.PlatformID
	installFormats []string
	serviceManager string
	defaultLogPath string
	capabilities   protocol.AgentCapabilities
	notes          []string
	probeSSH       func() protocol.SSHCheck
}

func baseCapabilities() protocol.AgentCapabilities {
	return protocol.AgentCapabilities{
		SSHForward:       true,
		NativePTY:        true,
		SelfUpdate:       true,
		ProxyAware:       true,
		ServiceManaged:   true,
		SessionRecording: false,
	}
}

func windowsProfile() platformProfile {
	return platformProfile{
		platform:       protocol.PlatformWindows,
		installFormats: []string{"msi", "exe"},
		serviceManager: "Windows Service",
		defaultLogPath: `%ProgramData%/remote-terminal-cloud-agent/logs`,
		capabilities:   baseCapabilities(),
		notes: []string{
			"MVP expects local OpenSSH Server.",
			"Validate UAC, Defender and localized paths.",
		},
		probeSSH: func() protocol.SSHCheck {
			result := runCommand("powershell", []string{
				"-NoProfile",
				"-Command",
				"try { (Get-Service sshd -ErrorAction Stop).Status } catch { 'missing' }",
			}, 3*time.Second)
			detail := result.Stdout
			if detail == "" {
				detail = "missing"
			}
			return protocol.SSHCheck{
				Available: detail != "missing",
				Detail:    "sshd service status: " + detail,
			}
		},
	}
}

func linuxProfile() platformProfile {
	return platformProfile{
		platform:       protocol.PlatformLinux,
		installFormats: []string{"deb", "rpm", "binary"},
		serviceManager: "systemd",
		defaultLogPath: "/var/log/remote-terminal-cloud-agent/",
		capabilities:   baseCapabilities(),
		notes: []string{
			"MVP expects local sshd.",
			"Validate glibc, SELinux and filesystem constraints.",
		},
		probeSSH: func() protocol.SSHCheck {
			result := runCommand("sh", []string{"-lc", "command -v sshd >/dev/null && echo present || echo missing"}, 3*time.Second)
			if result.Stdout == "present" {
				return protocol.SSHCheck{Available: true, Detail: "sshd binary detected."}
			}
			return protocol.SSHCheck{Available: false, Detail: "sshd binary missing."}
		},
	}
}

func macProfile() platformProfile {
	return platformProfile{
		platform:       protocol.PlatformMacOS,
		installFormats: []string{"pkg", "signed-helper"},
		serviceManager: "launchd",
		defaultLogPath: "/Library/Logs/remote-terminal-cloud-agent/",
		capabilities:   baseCapabilities(),
		notes: []string{
			"MVP expects Remote Login/OpenSSH.",
			"Validate notarization and Full Disk Access.",
		},
		probeSSH: func() protocol.SSHCheck {
			result := runCommand("sh", []string{
				"-lc",
				"systemsetup -getremotelogin 2>/dev/null | tr -d '\\r' || echo unavailable",
			}, 3*time.Second)
			detail := result.Stdout
			if detail == "" {
				detail = "unavailable"
			}
			return protocol.SSHCheck{
				Available: containsInsensitive(detail, "on"),
				Detail:    "Remote Login status: " + detail,
			}
		},
	}
}

func containsInsensitive(value string, target string) bool {
	return strings.Contains(strings.ToLower(value), strings.ToLower(target))
}

func currentPlatformProfile() (platformProfile, error) {
	switch runtime.GOOS {
	case "windows":
		return windowsProfile(), nil
	case "linux":
		return linuxProfile(), nil
	case "darwin":
		return macProfile(), nil
	default:
		return platformProfile{}, fmt.Errorf("unsupported platform: %s", runtime.GOOS)
	}
}

func CollectHostSnapshot(agentVersion string, enabledShells []protocol.ShellType) (protocol.HostSnapshot, error) {
	profile, err := currentPlatformProfile()
	if err != nil {
		return protocol.HostSnapshot{}, err
	}

	hostname, _ := os.Hostname()
	availableShells := detectConfiguredAvailableShells(enabledShells)

	return protocol.HostSnapshot{
		Hostname:     hostname,
		Platform:     profile.platform,
		Arch:         runtime.GOARCH,
		AgentVersion: agentVersion,
		Capabilities: profile.capabilities,
		Diagnostics: protocol.HostDiagnostics{
			InstallFormats:  profile.installFormats,
			ServiceManager:  profile.serviceManager,
			DefaultLogPath:  profile.defaultLogPath,
			AvailableShells: availableShells,
			SSHCheck:        profile.probeSSH(),
			Notes:           profile.notes,
		},
	}, nil
}

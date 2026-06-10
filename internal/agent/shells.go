package agent

import (
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"time"

	"github.com/remote-terminal-cloud/agent/internal/protocol"
)

var supportedShellTypes = map[protocol.ShellType]struct{}{
	protocol.ShellSystemDefault: {},
	protocol.ShellCmd:           {},
	protocol.ShellPowerShell:    {},
	protocol.ShellPwsh:          {},
	protocol.ShellBash:          {},
	protocol.ShellZsh:           {},
	protocol.ShellSh:            {},
}

type shellLaunch struct {
	executable string
	args       []string
	shellType  protocol.ShellType
}

func isSupportedShellType(value string) bool {
	_, ok := supportedShellTypes[protocol.ShellType(value)]
	return ok
}

func normalizeShellList(values []string) []protocol.ShellType {
	if len(values) == 0 {
		return nil
	}

	seen := make(map[protocol.ShellType]struct{}, len(values))
	result := make([]protocol.ShellType, 0, len(values))
	for _, value := range values {
		trimmed := strings.TrimSpace(value)
		if !isSupportedShellType(trimmed) {
			continue
		}

		shellType := protocol.ShellType(trimmed)
		if _, exists := seen[shellType]; exists {
			continue
		}

		seen[shellType] = struct{}{}
		result = append(result, shellType)
	}

	return result
}

func commandExists(command string) bool {
	if runtime.GOOS == "windows" {
		return runCommand("where.exe", []string{command}, 3*time.Second).OK
	}

	return runCommand("sh", []string{"-lc", "command -v " + command + " >/dev/null 2>&1"}, 3*time.Second).OK
}

func resolveExecutablePath(command string) string {
	trimmed := strings.TrimSpace(command)
	if trimmed == "" {
		return ""
	}

	if filepath.IsAbs(trimmed) {
		return trimmed
	}

	resolved, err := exec.LookPath(trimmed)
	if err == nil && strings.TrimSpace(resolved) != "" {
		return resolved
	}

	if runtime.GOOS == "windows" {
		result := runCommand("where.exe", []string{trimmed}, 3*time.Second)
		if result.OK {
			for _, line := range strings.Split(result.Stdout, "\n") {
				candidate := strings.TrimSpace(line)
				if candidate != "" {
					return candidate
				}
			}
		}
	}

	return trimmed
}

func detectAvailableShells() []protocol.ShellType {
	shells := []protocol.ShellType{protocol.ShellSystemDefault}

	if runtime.GOOS == "windows" {
		shells = append(shells, protocol.ShellCmd)
		if commandExists("powershell.exe") {
			shells = append(shells, protocol.ShellPowerShell)
		}
		if commandExists("pwsh.exe") {
			shells = append(shells, protocol.ShellPwsh)
		}
		return shells
	}

	candidates := []protocol.ShellType{
		protocol.ShellBash,
		protocol.ShellZsh,
		protocol.ShellSh,
		protocol.ShellPwsh,
	}
	for _, candidate := range candidates {
		if commandExists(string(candidate)) {
			shells = append(shells, candidate)
		}
	}

	return shells
}

func detectConfiguredAvailableShells(enabled []protocol.ShellType) []protocol.ShellType {
	detected := detectAvailableShells()
	if len(enabled) == 0 {
		return detected
	}

	enabledSet := make(map[protocol.ShellType]struct{}, len(enabled))
	for _, item := range enabled {
		enabledSet[item] = struct{}{}
	}

	filtered := make([]protocol.ShellType, 0, len(detected))
	for _, item := range detected {
		if _, ok := enabledSet[item]; ok {
			filtered = append(filtered, item)
		}
	}

	return filtered
}

func resolveEffectiveDefaultShell(configured protocol.ShellType, available []protocol.ShellType) protocol.ShellType {
	for _, item := range available {
		if item == configured {
			return configured
		}
	}
	for _, item := range available {
		if item == protocol.ShellSystemDefault {
			return protocol.ShellSystemDefault
		}
	}
	if len(available) > 0 {
		return available[0]
	}
	return configured
}

func resolveShellLaunch(requested protocol.ShellType, defaultShell protocol.ShellType) (shellLaunch, error) {
	normalized := requested
	if normalized == protocol.ShellSystemDefault {
		normalized = defaultShell
	}

	if runtime.GOOS == "windows" {
		switch normalized {
		case protocol.ShellSystemDefault, protocol.ShellCmd:
			comspec := os.Getenv("ComSpec")
			if strings.TrimSpace(comspec) == "" {
				comspec = "cmd.exe"
			}
			comspec = resolveExecutablePath(comspec)
			return shellLaunch{
				executable: comspec,
				args:       []string{"/d", "/k", "chcp 65001>nul"},
				shellType:  protocol.ShellCmd,
			}, nil
		case protocol.ShellPowerShell:
			return shellLaunch{
				executable: resolveExecutablePath("powershell.exe"),
				args:       []string{"-NoLogo", "-NoExit", "-Command", "[Console]::InputEncoding=[System.Text.UTF8Encoding]::new($false); [Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); $OutputEncoding=[Console]::OutputEncoding; chcp 65001 > $null"},
				shellType:  protocol.ShellPowerShell,
			}, nil
		case protocol.ShellPwsh:
			return shellLaunch{
				executable: resolveExecutablePath("pwsh.exe"),
				args:       []string{"-NoLogo", "-NoExit", "-Command", "[Console]::InputEncoding=[System.Text.UTF8Encoding]::new($false); [Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); $OutputEncoding=[Console]::OutputEncoding; chcp 65001 > $null"},
				shellType:  protocol.ShellPwsh,
			}, nil
		default:
			return shellLaunch{}, errors.New("shell is not supported on Windows")
		}
	}

	switch normalized {
	case protocol.ShellSystemDefault:
		systemShell := strings.TrimSpace(os.Getenv("SHELL"))
		if systemShell == "" {
			systemShell = "/bin/bash"
		}
		systemShell = resolveExecutablePath(systemShell)
		return shellLaunch{
			executable: systemShell,
			args:       []string{"-i"},
			shellType:  protocol.ShellSystemDefault,
		}, nil
	case protocol.ShellBash, protocol.ShellZsh, protocol.ShellSh:
		return shellLaunch{
			executable: resolveExecutablePath(string(normalized)),
			args:       []string{"-i"},
			shellType:  normalized,
		}, nil
	case protocol.ShellPwsh:
		return shellLaunch{
			executable: resolveExecutablePath("pwsh"),
			args:       []string{"-NoLogo"},
			shellType:  protocol.ShellPwsh,
		}, nil
	default:
		return shellLaunch{}, errors.New("shell is not supported on this platform")
	}
}

func cleanPath(input string) string {
	if strings.TrimSpace(input) == "" {
		return ""
	}
	return filepath.Clean(input)
}

package agent

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/remote-terminal-cloud/agent/internal/buildinfo"
	"github.com/remote-terminal-cloud/agent/internal/protocol"
)

type ManagerPaths struct {
	InstallRoot       string
	AgentBinaryPath   string
	ConfigDir         string
	ConfigFilePath    string
	PreferencesPath   string
	LogsDir           string
	ManagerBinaryPath string
}

type ManagerStatus struct {
	Version               string
	ServerBaseURL         string
	TokenConfigured       bool
	TokenSource           string
	ConfigFilePath        string
	PreferencesPath       string
	LogsDir               string
	ServiceState          string
	Platform              string
	Arch                  string
	ConfiguredDefaultShell string
	AvailableShells       []protocol.ShellType
	SSHAvailable          bool
	SSHDetail             string
	RunHeartbeat          bool
	RunTunnel             bool
}

func GetManagerPaths() (ManagerPaths, error) {
	installRoot, err := resolveInstallRoot()
	if err != nil {
		return ManagerPaths{}, err
	}

	configDir := filepath.Dir(getDefaultConfigFilePath())
	preferencesPath := getDefaultPreferencesFilePath()
	logsDir := filepath.Join(os.Getenv("ProgramData"), "RemoteTerminalCloudAgent", "logs")
	if strings.TrimSpace(os.Getenv("ProgramData")) == "" {
		logsDir = filepath.Join(os.Getenv("ALLUSERSPROFILE"), "RemoteTerminalCloudAgent", "logs")
	}
	if strings.TrimSpace(logsDir) == "" {
		logsDir = filepath.Join(installRoot, "logs")
	}

	managerBinaryName := "rtc-agent-manager"
	agentBinaryName := "rtc-agent"
	if runtime.GOOS == "windows" {
		managerBinaryName += ".exe"
		agentBinaryName += ".exe"
	}

	agentBinaryPath := resolveInstalledBinaryPath(installRoot, agentBinaryName)
	managerBinaryPath := resolveInstalledBinaryPath(installRoot, managerBinaryName)

	return ManagerPaths{
		InstallRoot:       installRoot,
		AgentBinaryPath:   agentBinaryPath,
		ConfigDir:         configDir,
		ConfigFilePath:    getDefaultConfigFilePath(),
		PreferencesPath:   preferencesPath,
		LogsDir:           logsDir,
		ManagerBinaryPath: managerBinaryPath,
	}, nil
}

func resolveInstallRoot() (string, error) {
	exePath, err := os.Executable()
	if err == nil && strings.TrimSpace(exePath) != "" {
		exeDir := filepath.Dir(exePath)
		if strings.EqualFold(filepath.Base(exeDir), "bin") {
			return filepath.Dir(exeDir), nil
		}
		if fileExists(filepath.Join(exeDir, "rtc-agent.exe")) || fileExists(filepath.Join(exeDir, "rtc-agent")) {
			return exeDir, nil
		}
		if fileExists(filepath.Join(exeDir, "bin", "rtc-agent.exe")) || fileExists(filepath.Join(exeDir, "bin", "rtc-agent")) {
			return exeDir, nil
		}
	}

	wd, err := os.Getwd()
	if err != nil {
		return "", err
	}
	if fileExists(filepath.Join(wd, "rtc-agent.exe")) || fileExists(filepath.Join(wd, "rtc-agent")) {
		return wd, nil
	}
	if fileExists(filepath.Join(wd, "bin", "rtc-agent.exe")) || fileExists(filepath.Join(wd, "bin", "rtc-agent")) {
		return wd, nil
	}

	return "", fmt.Errorf("could not resolve install root")
}

func resolveInstalledBinaryPath(installRoot string, binaryName string) string {
	binPath := filepath.Join(installRoot, "bin", binaryName)
	if fileExists(binPath) {
		return binPath
	}
	return filepath.Join(installRoot, binaryName)
}

func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func RunAgentCommand(args ...string) (string, error) {
	paths, err := GetManagerPaths()
	if err != nil {
		return "", err
	}

	cmd := exec.Command(paths.AgentBinaryPath, args...)
	cmd.Env = os.Environ()
	output, err := cmd.CombinedOutput()
	return strings.TrimSpace(string(output)), err
}

func GetManagerStatus() (ManagerStatus, error) {
	buildinfo.LoadVersionFromFile()
	config := GetRuntimeConfig()
	snapshot, err := CollectHostSnapshot(buildinfo.Version, config.EnabledShellTypes)
	if err != nil {
		return ManagerStatus{}, err
	}

	tokenSource := "none"
	if config.RegistrationToken != nil {
		tokenSource = "config file"
		if HasRegistrationTokenEnvOverride() {
			tokenSource = "environment variable RTC_REGISTRATION_TOKEN"
		}
	}

	paths, err := GetManagerPaths()
	if err != nil {
		return ManagerStatus{}, err
	}

	return ManagerStatus{
		Version:                buildinfo.Version,
		ServerBaseURL:          config.ServerBaseURL,
		TokenConfigured:        config.RegistrationToken != nil,
		TokenSource:            tokenSource,
		ConfigFilePath:         paths.ConfigFilePath,
		PreferencesPath:        paths.PreferencesPath,
		LogsDir:                paths.LogsDir,
		ServiceState:           detectServiceState(),
		Platform:               string(snapshot.Platform),
		Arch:                   snapshot.Arch,
		ConfiguredDefaultShell: string(config.DefaultShellType),
		AvailableShells:        snapshot.Diagnostics.AvailableShells,
		SSHAvailable:           snapshot.Diagnostics.SSHCheck.Available,
		SSHDetail:              snapshot.Diagnostics.SSHCheck.Detail,
		RunHeartbeat:           config.RunHeartbeat,
		RunTunnel:              config.RunTunnel,
	}, nil
}

func SaveRegistrationToken(token string) error {
	paths, err := GetManagerPaths()
	if err != nil {
		return err
	}
	normalized := normalizeTemplateString(stringPtr(token))
	if normalized == nil {
		return fmt.Errorf("token cannot be empty")
	}
	return persistRegistrationToken(paths.ConfigFilePath, *normalized)
}

func EnsureConfigFile() error {
	paths, err := GetManagerPaths()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(paths.ConfigDir, 0o755); err != nil {
		return err
	}
	if fileExists(paths.ConfigFilePath) {
		return nil
	}

	payload := fileConfig{}
	content, err := json.MarshalIndent(payload, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(paths.ConfigFilePath, content, 0o644)
}

func detectServiceState() string {
	if runtime.GOOS != "windows" {
		return "unsupported"
	}
	return DetectWindowsServiceState()
}

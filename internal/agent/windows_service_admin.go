//go:build windows

package agent

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
	"unsafe"

	"golang.org/x/sys/windows"
	"golang.org/x/sys/windows/svc"
	"golang.org/x/sys/windows/svc/mgr"

	"github.com/remote-terminal-cloud/agent/internal/protocol"
)

const (
	WindowsServiceName        = "RemoteTerminalCloudAgent"
	windowsStopTimeout        = 30 * time.Second
	windowsWaitPollInterval   = 500 * time.Millisecond
	windowsConfigTemplateName = "agent.config.json"
)

func GetManagedConfigDir() string {
	programData := strings.TrimSpace(os.Getenv("ProgramData"))
	if programData == "" {
		programData = strings.TrimSpace(os.Getenv("ALLUSERSPROFILE"))
	}
	if programData == "" {
		programData = filepath.Join(os.Getenv("SystemDrive")+"\\", "ProgramData")
	}
	return filepath.Join(programData, WindowsServiceName)
}

func GetManagedConfigFilePath() string {
	return filepath.Join(GetManagedConfigDir(), "config.json")
}

func GetManagedLogsDir() string {
	return filepath.Join(GetManagedConfigDir(), "logs")
}

func GetManagedPreferencesFilePath() string {
	return filepath.Join(GetManagedConfigDir(), "preferences.json")
}

func EnsureManagedConfigFile() error {
	return ensureManagedConfigFileWithInstallRoot("")
}

func SaveManagedRegistrationToken(token string) error {
	normalized := normalizeTemplateString(stringPtr(token))
	if normalized == nil {
		return fmt.Errorf("token cannot be empty")
	}
	if err := EnsureManagedConfigFile(); err != nil {
		return err
	}
	return persistRegistrationToken(GetManagedConfigFilePath(), *normalized)
}

func InstallWindowsService(installRoot string, registrationToken string) error {
	resolvedRoot, err := resolveWindowsInstallRoot(installRoot)
	if err != nil {
		return err
	}

	if err := ensureManagedConfigFileWithInstallRoot(resolvedRoot); err != nil {
		return err
	}
	if strings.TrimSpace(registrationToken) != "" {
		if err := persistRegistrationToken(GetManagedConfigFilePath(), strings.TrimSpace(registrationToken)); err != nil {
			return err
		}
	}

	paths, err := getWindowsServiceBinaryPaths(resolvedRoot)
	if err != nil {
		return err
	}

	_ = StopWindowsService(resolvedRoot)
	_ = runWinSW(paths.winSWExe, "uninstall")
	if err := runWinSW(paths.winSWExe, "install"); err != nil {
		return err
	}
	if err := runWinSW(paths.winSWExe, "start"); err != nil {
		return err
	}
	return nil
}

func UninstallWindowsService(installRoot string) error {
	resolvedRoot, err := resolveWindowsInstallRoot(installRoot)
	if err != nil {
		return err
	}
	paths, err := getWindowsServiceBinaryPaths(resolvedRoot)
	if err != nil {
		return err
	}
	_ = StopWindowsService(resolvedRoot)
	return runWinSW(paths.winSWExe, "uninstall")
}

func StopWindowsService(installRoot string) error {
	resolvedRoot, err := resolveWindowsInstallRoot(installRoot)
	if err != nil {
		return err
	}

	paths, pathErr := getWindowsServiceBinaryPaths(resolvedRoot)
	if pathErr == nil {
		_ = runWinSW(paths.winSWExe, "stop")
	}

	_ = stopWindowsServiceViaSCM()
	_ = waitForWindowsServiceState(svc.Stopped, windowsStopTimeout)
	_ = terminateManagedProcesses(resolvedRoot)
	return waitForWindowsServiceState(svc.Stopped, windowsStopTimeout)
}

func StartWindowsService() error {
	manager, err := mgr.Connect()
	if err != nil {
		return err
	}
	defer manager.Disconnect()

	service, err := manager.OpenService(WindowsServiceName)
	if err != nil {
		return err
	}
	defer service.Close()

	if err := service.Start(); err != nil {
		return err
	}
	return nil
}

func RestartWindowsService(installRoot string) error {
	if err := StopWindowsService(installRoot); err != nil {
		return err
	}
	return StartWindowsService()
}

func DetectWindowsServiceState() string {
	manager, err := mgr.Connect()
	if err != nil {
		return "Unknown"
	}
	defer manager.Disconnect()

	service, err := manager.OpenService(WindowsServiceName)
	if err != nil {
		return "Not installed"
	}
	defer service.Close()

	status, err := service.Query()
	if err != nil {
		return "Unknown"
	}
	return formatWindowsServiceState(status.State)
}

func OpenPathInExplorer(path string) error {
	if err := os.MkdirAll(path, 0o755); err != nil {
		return err
	}
	return windows.ShellExecute(0, windows.StringToUTF16Ptr("open"), windows.StringToUTF16Ptr("explorer.exe"), windows.StringToUTF16Ptr(path), nil, windows.SW_SHOWNORMAL)
}

type windowsServiceBinaryPaths struct {
	agentExe string
	winSWExe string
	winSWXML string
}

func getWindowsServiceBinaryPaths(installRoot string) (windowsServiceBinaryPaths, error) {
	agentExe := filepath.Join(installRoot, "bin", "rtc-agent.exe")
	winSWExe := filepath.Join(installRoot, "service", "RemoteTerminalCloudAgentService.exe")
	winSWXML := filepath.Join(installRoot, "service", "RemoteTerminalCloudAgentService.xml")

	for _, required := range []string{agentExe, winSWExe, winSWXML} {
		if !fileExists(required) {
			return windowsServiceBinaryPaths{}, fmt.Errorf("required file missing: %s", required)
		}
	}

	return windowsServiceBinaryPaths{
		agentExe: agentExe,
		winSWExe: winSWExe,
		winSWXML: winSWXML,
	}, nil
}

func resolveWindowsInstallRoot(explicitRoot string) (string, error) {
	if strings.TrimSpace(explicitRoot) != "" {
		return filepath.Abs(strings.TrimSpace(explicitRoot))
	}
	return resolveInstallRoot()
}

func ensureManagedConfigFileWithInstallRoot(installRoot string) error {
	configDir := GetManagedConfigDir()
	if err := os.MkdirAll(configDir, 0o755); err != nil {
		return err
	}
	if err := os.MkdirAll(GetManagedLogsDir(), 0o755); err != nil {
		return err
	}
	if fileExists(GetManagedConfigFilePath()) {
		return nil
	}

	if templatePath := findWindowsConfigTemplate(installRoot); templatePath != "" {
		content, err := os.ReadFile(templatePath)
		if err == nil {
			return os.WriteFile(GetManagedConfigFilePath(), content, 0o644)
		}
	}

	payload := fileConfig{
		RunHeartbeat:        boolPtr(true),
		RunTunnel:           boolPtr(true),
		DefaultShellType:    string(protocol.ShellSystemDefault),
		EnabledShellTypes:   []string{string(protocol.ShellSystemDefault), string(protocol.ShellCmd), string(protocol.ShellPowerShell), string(protocol.ShellPwsh)},
		PreferencesFilePath: GetManagedPreferencesFilePath(),
	}
	content, err := json.MarshalIndent(payload, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(GetManagedConfigFilePath(), content, 0o644)
}

func findWindowsConfigTemplate(installRoot string) string {
	candidates := []string{}
	if strings.TrimSpace(installRoot) != "" {
		candidates = append(candidates,
			filepath.Join(installRoot, windowsConfigTemplateName),
			filepath.Join(installRoot, "packaging", "windows", windowsConfigTemplateName),
		)
	}
	for _, candidate := range candidates {
		if fileExists(candidate) {
			return candidate
		}
	}
	return ""
}

func runWinSW(winSWExe string, action string) error {
	cmd := exec.Command(winSWExe, action)
	cmd.Dir = filepath.Dir(winSWExe)
	output, err := cmd.CombinedOutput()
	if err != nil {
		message := strings.TrimSpace(string(output))
		if message == "" {
			return err
		}
		return fmt.Errorf("%s: %s", action, message)
	}
	return nil
}

func stopWindowsServiceViaSCM() error {
	manager, err := mgr.Connect()
	if err != nil {
		return err
	}
	defer manager.Disconnect()

	service, err := manager.OpenService(WindowsServiceName)
	if err != nil {
		return nil
	}
	defer service.Close()

	status, err := service.Control(svc.Stop)
	if err != nil {
		if errors.Is(err, windows.ERROR_SERVICE_NOT_ACTIVE) {
			return nil
		}
		return err
	}
	if status.State == svc.Stopped {
		return nil
	}
	return nil
}

func waitForWindowsServiceState(target svc.State, timeout time.Duration) error {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		manager, err := mgr.Connect()
		if err != nil {
			time.Sleep(windowsWaitPollInterval)
			continue
		}

		service, err := manager.OpenService(WindowsServiceName)
		if err != nil {
			manager.Disconnect()
			if target == svc.Stopped {
				return nil
			}
			time.Sleep(windowsWaitPollInterval)
			continue
		}

		status, queryErr := service.Query()
		service.Close()
		manager.Disconnect()
		if queryErr == nil && status.State == target {
			return nil
		}
		time.Sleep(windowsWaitPollInterval)
	}

	if target == svc.Stopped {
		return nil
	}
	return fmt.Errorf("service %s did not reach expected state", WindowsServiceName)
}

func terminateManagedProcesses(installRoot string) error {
	snapshot, err := windows.CreateToolhelp32Snapshot(windows.TH32CS_SNAPPROCESS, 0)
	if err != nil {
		return err
	}
	defer windows.CloseHandle(snapshot)

	rootPrefix, err := filepath.Abs(installRoot)
	if err != nil {
		return err
	}
	rootPrefix = strings.TrimRight(rootPrefix, "\\/") + string(os.PathSeparator)

	entry := windows.ProcessEntry32{
		Size: uint32(unsafe.Sizeof(windows.ProcessEntry32{})),
	}
	if err := windows.Process32First(snapshot, &entry); err != nil {
		return err
	}

	for {
		if shouldTerminateManagedProcess(entry.ProcessID, rootPrefix) {
			handle, openErr := windows.OpenProcess(windows.PROCESS_TERMINATE, false, entry.ProcessID)
			if openErr == nil {
				_ = windows.TerminateProcess(handle, 1)
				_ = windows.CloseHandle(handle)
			}
		}

		if err := windows.Process32Next(snapshot, &entry); err != nil {
			if errors.Is(err, windows.ERROR_NO_MORE_FILES) {
				break
			}
			return err
		}
	}

	return nil
}

func shouldTerminateManagedProcess(processID uint32, rootPrefix string) bool {
	processHandle, err := windows.OpenProcess(windows.PROCESS_QUERY_LIMITED_INFORMATION, false, processID)
	if err != nil {
		return false
	}
	defer windows.CloseHandle(processHandle)

	buffer := make([]uint16, windows.MAX_PATH)
	size := uint32(len(buffer))
	if err := windows.QueryFullProcessImageName(processHandle, 0, &buffer[0], &size); err != nil {
		return false
	}

	fullPath := windows.UTF16ToString(buffer[:size])
	if fullPath == "" {
		return false
	}

	normalizedPath := strings.TrimSpace(fullPath)
	if !strings.HasPrefix(strings.ToLower(normalizedPath), strings.ToLower(rootPrefix)) {
		return false
	}

	baseName := strings.ToLower(filepath.Base(normalizedPath))
	return baseName == "rtc-agent.exe" || baseName == "remoteterminalcloudagentservice.exe"
}

func formatWindowsServiceState(state svc.State) string {
	switch state {
	case svc.Stopped:
		return "Stopped"
	case svc.StartPending:
		return "Start pending"
	case svc.StopPending:
		return "Stop pending"
	case svc.Running:
		return "Running"
	case svc.ContinuePending:
		return "Continue pending"
	case svc.PausePending:
		return "Pause pending"
	case svc.Paused:
		return "Paused"
	default:
		return "Unknown"
	}
}

func boolPtr(value bool) *bool {
	return &value
}

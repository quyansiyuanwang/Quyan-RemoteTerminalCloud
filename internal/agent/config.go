package agent

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/joho/godotenv"
	"golang.org/x/term"

	"github.com/remote-terminal-cloud/agent/internal/buildinfo"
	"github.com/remote-terminal-cloud/agent/internal/protocol"
)

type fileConfig struct {
	RegistrationToken   *string  `json:"registrationToken"`
	RunHeartbeat        *bool    `json:"runHeartbeat"`
	RunTunnel           *bool    `json:"runTunnel"`
	DefaultShellType    string   `json:"defaultShellType"`
	EnabledShellTypes   []string `json:"enabledShellTypes"`
	PreferencesFilePath string   `json:"preferencesFilePath"`
}

type RuntimeConfig struct {
	ServerBaseURL       string
	RegistrationToken   *string
	RunHeartbeat        bool
	RunTunnel           bool
	DefaultShellType    protocol.ShellType
	EnabledShellTypes   []protocol.ShellType
	PreferencesFilePath string
	ConfigFilePath      string
}

var dotenvLoaded bool

func ensureDotenvLoaded() {
	if dotenvLoaded {
		return
	}

	candidates := []string{
		filepath.Join(mustGetwd(), ".env"),
		filepath.Join(filepath.Dir(mustGetwd()), ".env"),
	}

	for _, candidate := range candidates {
		if _, err := os.Stat(candidate); err == nil {
			_ = godotenv.Overload(candidate)
			break
		}
	}

	dotenvLoaded = true
}

func mustGetwd() string {
	wd, err := os.Getwd()
	if err != nil {
		return "."
	}
	return wd
}

func readStringEnv(name string) *string {
	value := strings.TrimSpace(os.Getenv(name))
	if value == "" {
		return nil
	}
	return &value
}

func normalizeTemplateString(value *string) *string {
	if value == nil {
		return nil
	}
	switch strings.TrimSpace(*value) {
	case "", "replace-with-real-token":
		return nil
	default:
		trimmed := strings.TrimSpace(*value)
		return &trimmed
	}
}

func readBooleanEnv(name string) *bool {
	value := readStringEnv(name)
	if value == nil {
		return nil
	}

	switch strings.ToLower(*value) {
	case "1", "true", "yes", "on":
		result := true
		return &result
	case "0", "false", "no", "off":
		result := false
		return &result
	default:
		return nil
	}
}

func getDefaultPreferencesFilePath() string {
	home, _ := os.UserHomeDir()
	switch runtime.GOOS {
	case "windows":
		appData := strings.TrimSpace(os.Getenv("APPDATA"))
		if appData == "" {
			appData = filepath.Join(home, "AppData", "Roaming")
		}
		return filepath.Join(appData, "remote-terminal-cloud-agent", "preferences.json")
	case "darwin":
		return filepath.Join(home, "Library", "Application Support", "remote-terminal-cloud-agent", "preferences.json")
	default:
		stateHome := strings.TrimSpace(os.Getenv("XDG_STATE_HOME"))
		if stateHome == "" {
			stateHome = filepath.Join(home, ".local", "state")
		}
		return filepath.Join(stateHome, "remote-terminal-cloud-agent", "preferences.json")
	}
}

func getDefaultConfigFilePath() string {
	home, _ := os.UserHomeDir()
	switch runtime.GOOS {
	case "windows":
		appData := strings.TrimSpace(os.Getenv("APPDATA"))
		if appData == "" {
			appData = filepath.Join(home, "AppData", "Roaming")
		}
		return filepath.Join(appData, "remote-terminal-cloud-agent", "config.json")
	case "darwin":
		return filepath.Join(home, "Library", "Application Support", "remote-terminal-cloud-agent", "config.json")
	default:
		configHome := strings.TrimSpace(os.Getenv("XDG_CONFIG_HOME"))
		if configHome == "" {
			configHome = filepath.Join(home, ".config")
		}
		return filepath.Join(configHome, "remote-terminal-cloud-agent", "config.json")
	}
}

func readConfigFile(path string) fileConfig {
	content, err := os.ReadFile(path)
	if err != nil {
		return fileConfig{}
	}

	var cfg fileConfig
	if err := json.Unmarshal(content, &cfg); err != nil {
		return fileConfig{}
	}

	cfg.RegistrationToken = normalizeTemplateString(cfg.RegistrationToken)
	cfg.PreferencesFilePath = strings.TrimSpace(cfg.PreferencesFilePath)
	return cfg
}

func IsInteractiveInputAvailable() bool {
	stat, err := os.Stdin.Stat()
	if err != nil {
		return false
	}
	return (stat.Mode() & os.ModeCharDevice) != 0
}

func PromptAndPersistRegistrationToken(configFilePath string) (*string, error) {
	if !IsInteractiveInputAvailable() {
		return nil, nil
	}

	fmt.Printf("[remote-terminal-cloud-agent] registration token is not configured. Enter token to save into %s\n", configFilePath)
	fmt.Print("[remote-terminal-cloud-agent] token (press Enter to skip): ")

	var tokenText string
	if term.IsTerminal(int(os.Stdin.Fd())) {
		rawToken, err := term.ReadPassword(int(os.Stdin.Fd()))
		fmt.Println()
		if err != nil {
			return nil, err
		}
		tokenText = string(rawToken)
	} else {
		reader := bufio.NewReader(os.Stdin)
		line, err := reader.ReadString('\n')
		if err != nil {
			return nil, err
		}
		tokenText = line
	}

	token := normalizeTemplateString(stringPtr(tokenText))
	if token == nil {
		return nil, nil
	}

	if err := persistRegistrationToken(configFilePath, *token); err != nil {
		return nil, err
	}

	fmt.Printf("[remote-terminal-cloud-agent] token saved to %s\n", configFilePath)
	return token, nil
}

func HasRegistrationTokenEnvOverride() bool {
	return normalizeTemplateString(readStringEnv("RTC_REGISTRATION_TOKEN")) != nil
}

func persistRegistrationToken(configFilePath string, token string) error {
	cfg := readConfigFile(configFilePath)
	cfg.RegistrationToken = stringPtr(token)

	content, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return err
	}

	if err := os.MkdirAll(filepath.Dir(configFilePath), 0o755); err != nil {
		return err
	}

	return os.WriteFile(configFilePath, content, 0o644)
}

func stringPtr(value string) *string {
	return &value
}

func GetRuntimeConfig() RuntimeConfig {
	ensureDotenvLoaded()

	configFilePath := getDefaultConfigFilePath()
	if envPath := readStringEnv("RTC_CONFIG_FILE"); envPath != nil {
		configFilePath = *envPath
	}

	fileCfg := readConfigFile(configFilePath)
	configuredDefaultShell := readStringEnv("RTC_DEFAULT_SHELL")
	enabledShellEnv := readStringEnv("RTC_ENABLED_SHELLS")
	heartbeatDisabled := readBooleanEnv("RTC_DISABLE_HEARTBEAT")
	tunnelDisabled := readBooleanEnv("RTC_DISABLE_TUNNEL")

	defaultShell := protocol.ShellSystemDefault
	if configuredDefaultShell != nil && isSupportedShellType(*configuredDefaultShell) {
		defaultShell = protocol.ShellType(*configuredDefaultShell)
	} else if isSupportedShellType(fileCfg.DefaultShellType) {
		defaultShell = protocol.ShellType(fileCfg.DefaultShellType)
	}

	var enabledShells []protocol.ShellType
	if enabledShellEnv != nil {
		enabledShells = normalizeShellList(strings.Split(*enabledShellEnv, ","))
	} else {
		enabledShells = normalizeShellList(fileCfg.EnabledShellTypes)
	}

	serverBaseURL := buildinfo.ServerBaseURL

	registrationToken := normalizeTemplateString(readStringEnv("RTC_REGISTRATION_TOKEN"))
	if registrationToken == nil {
		registrationToken = fileCfg.RegistrationToken
	}

	runHeartbeat := true
	if heartbeatDisabled != nil {
		runHeartbeat = !*heartbeatDisabled
	} else if fileCfg.RunHeartbeat != nil {
		runHeartbeat = *fileCfg.RunHeartbeat
	}

	runTunnel := true
	if tunnelDisabled != nil {
		runTunnel = !*tunnelDisabled
	} else if fileCfg.RunTunnel != nil {
		runTunnel = *fileCfg.RunTunnel
	}

	preferencesFilePath := getDefaultPreferencesFilePath()
	if envValue := readStringEnv("RTC_PREFERENCES_FILE"); envValue != nil {
		preferencesFilePath = *envValue
	} else if strings.TrimSpace(fileCfg.PreferencesFilePath) != "" {
		preferencesFilePath = fileCfg.PreferencesFilePath
	}

	return RuntimeConfig{
		ServerBaseURL:       serverBaseURL,
		RegistrationToken:   registrationToken,
		RunHeartbeat:        runHeartbeat,
		RunTunnel:           runTunnel,
		DefaultShellType:    defaultShell,
		EnabledShellTypes:   enabledShells,
		PreferencesFilePath: preferencesFilePath,
		ConfigFilePath:      configFilePath,
	}
}

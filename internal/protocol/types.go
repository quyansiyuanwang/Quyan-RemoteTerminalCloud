package protocol

type PlatformID string

const (
	PlatformWindows PlatformID = "windows"
	PlatformLinux   PlatformID = "linux"
	PlatformMacOS   PlatformID = "macos"
)

type ShellType string

const (
	ShellSystemDefault ShellType = "system-default"
	ShellCmd           ShellType = "cmd"
	ShellPowerShell    ShellType = "powershell"
	ShellPwsh          ShellType = "pwsh"
	ShellBash          ShellType = "bash"
	ShellZsh           ShellType = "zsh"
	ShellSh            ShellType = "sh"
)

type AgentCapabilities struct {
	SSHForward       bool `json:"sshForward"`
	NativePTY        bool `json:"nativePty"`
	SelfUpdate       bool `json:"selfUpdate"`
	ProxyAware       bool `json:"proxyAware"`
	ServiceManaged   bool `json:"serviceManaged"`
	SessionRecording bool `json:"sessionRecording"`
}

type SSHCheck struct {
	Available bool   `json:"available"`
	Detail    string `json:"detail"`
}

type HostDiagnostics struct {
	InstallFormats  []string    `json:"installFormats"`
	ServiceManager  string      `json:"serviceManager"`
	DefaultLogPath  string      `json:"defaultLogPath"`
	AvailableShells []ShellType `json:"availableShells"`
	SSHCheck        SSHCheck    `json:"sshCheck"`
	Notes           []string    `json:"notes"`
}

type HostSnapshot struct {
	Hostname     string            `json:"hostname"`
	Platform     PlatformID        `json:"platform"`
	Arch         string            `json:"arch"`
	AgentVersion string            `json:"agentVersion"`
	Capabilities AgentCapabilities `json:"capabilities"`
	Diagnostics  HostDiagnostics   `json:"diagnostics"`
}

type AgentRegistrationRequest struct {
	RegistrationToken string       `json:"registrationToken"`
	Snapshot          HostSnapshot `json:"snapshot"`
}

type AgentRegistrationResponse struct {
	DeviceID                 string `json:"deviceId"`
	HeartbeatIntervalSeconds int    `json:"heartbeatIntervalSeconds"`
	HeartbeatToken           string `json:"heartbeatToken"`
	WebSocketURL             string `json:"websocketUrl,omitempty"`
	AcceptedAt               string `json:"acceptedAt"`
}

type AgentHeartbeatRequest struct {
	DeviceID       string       `json:"deviceId"`
	HeartbeatToken string       `json:"heartbeatToken"`
	Snapshot       HostSnapshot `json:"snapshot"`
}

type AgentHeartbeatResponse struct {
	OK                           bool   `json:"ok"`
	NextHeartbeatIntervalSeconds int    `json:"nextHeartbeatIntervalSeconds"`
	WebSocketURL                 string `json:"websocketUrl,omitempty"`
	ServerTime                   string `json:"serverTime"`
}

type DirectoryEntry struct {
	Name string `json:"name"`
	Path string `json:"path"`
}

type RemoteTerminalShortcutModifier string

const (
	ShortcutModifierCtrl  RemoteTerminalShortcutModifier = "ctrl"
	ShortcutModifierAlt   RemoteTerminalShortcutModifier = "alt"
	ShortcutModifierShift RemoteTerminalShortcutModifier = "shift"
	ShortcutModifierMeta  RemoteTerminalShortcutModifier = "meta"
)

type RemoteTerminalShortcutKind string

const (
	ShortcutKindSequence RemoteTerminalShortcutKind = "sequence"
	ShortcutKindKey      RemoteTerminalShortcutKind = "key"
)

type RemoteTerminalShortcutData struct {
	ID        string                           `json:"id"`
	Label     string                           `json:"label"`
	Kind      RemoteTerminalShortcutKind       `json:"kind"`
	Sequence  []string                         `json:"sequence"`
	Key       string                           `json:"key,omitempty"`
	Modifiers []RemoteTerminalShortcutModifier `json:"modifiers,omitempty"`
	Preset    bool                             `json:"preset,omitempty"`
}

type RemoteTerminalQuickCommandData struct {
	ID      string `json:"id"`
	Label   string `json:"label"`
	Command string `json:"command"`
}

type RemoteTerminalAgentPreferencesData struct {
	DefaultWorkingDirectory string                           `json:"defaultWorkingDirectory,omitempty"`
	Shortcuts               []RemoteTerminalShortcutData     `json:"shortcuts"`
	QuickCommands           []RemoteTerminalQuickCommandData `json:"quickCommands"`
}

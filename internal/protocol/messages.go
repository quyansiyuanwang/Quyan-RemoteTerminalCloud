package protocol

type SessionStartMessage struct {
	Type             string    `json:"type"`
	SessionID        string    `json:"sessionId"`
	Mode             string    `json:"mode"`
	ShellType        ShellType `json:"shellType"`
	WorkingDirectory string    `json:"workingDirectory,omitempty"`
}

type SessionInputMessage struct {
	Type      string `json:"type"`
	SessionID string `json:"sessionId"`
	Data      string `json:"data"`
}

type SessionResizeMessage struct {
	Type      string `json:"type"`
	SessionID string `json:"sessionId"`
	Cols      int    `json:"cols"`
	Rows      int    `json:"rows"`
}

type SessionStopMessage struct {
	Type      string `json:"type"`
	SessionID string `json:"sessionId"`
}

type DirectoryBrowseRequestMessage struct {
	Type      string `json:"type"`
	RequestID string `json:"requestId"`
	Path      string `json:"path,omitempty"`
}

type DirectoryBrowseResultMessage struct {
	Type        string           `json:"type"`
	RequestID   string           `json:"requestId"`
	OK          bool             `json:"ok"`
	Message     string           `json:"message,omitempty"`
	CurrentPath string           `json:"currentPath"`
	ParentPath  string           `json:"parentPath,omitempty"`
	Items       []DirectoryEntry `json:"items"`
}

type PreferencesGetMessage struct {
	Type      string `json:"type"`
	RequestID string `json:"requestId"`
}

type PreferencesSetMessage struct {
	Type        string                             `json:"type"`
	RequestID   string                             `json:"requestId"`
	Preferences RemoteTerminalAgentPreferencesData `json:"preferences"`
}

type PreferencesResultMessage struct {
	Type        string                             `json:"type"`
	RequestID   string                             `json:"requestId"`
	OK          bool                               `json:"ok"`
	Message     string                             `json:"message,omitempty"`
	Preferences RemoteTerminalAgentPreferencesData `json:"preferences"`
}

type SessionReadyMessage struct {
	Type      string `json:"type"`
	SessionID string `json:"sessionId"`
}

type SessionOutputMessage struct {
	Type      string `json:"type"`
	SessionID string `json:"sessionId"`
	Stream    string `json:"stream"`
	Data      string `json:"data"`
}

type SessionExitMessage struct {
	Type      string `json:"type"`
	SessionID string `json:"sessionId"`
	ExitCode  *int   `json:"exitCode"`
}

type SessionErrorMessage struct {
	Type      string `json:"type"`
	SessionID string `json:"sessionId"`
	Message   string `json:"message"`
}

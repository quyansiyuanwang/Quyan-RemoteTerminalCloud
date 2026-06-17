use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PlatformId {
    Windows,
    Linux,
    Macos,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ShellType {
    SystemDefault,
    Cmd,
    Powershell,
    Pwsh,
    Bash,
    Zsh,
    Sh,
}

impl ShellType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SystemDefault => "system-default",
            Self::Cmd => "cmd",
            Self::Powershell => "powershell",
            Self::Pwsh => "pwsh",
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Sh => "sh",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    pub ssh_forward: bool,
    pub native_pty: bool,
    pub self_update: bool,
    pub proxy_aware: bool,
    pub service_managed: bool,
    pub session_recording: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SshCheck {
    pub available: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HostDiagnostics {
    pub install_formats: Vec<String>,
    pub service_manager: String,
    pub default_log_path: String,
    pub available_shells: Vec<ShellType>,
    pub ssh_check: SshCheck,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HostSnapshot {
    pub hostname: String,
    pub platform: Option<PlatformId>,
    pub arch: String,
    pub agent_version: String,
    pub capabilities: AgentCapabilities,
    pub diagnostics: HostDiagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRegistrationRequest {
    pub registration_token: String,
    pub device_fingerprint: String,
    pub fingerprint_version: String,
    pub snapshot: HostSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentRegistrationResponse {
    pub device_id: String,
    pub heartbeat_interval_seconds: i32,
    pub heartbeat_token: String,
    #[serde(default)]
    pub websocket_url: String,
    pub accepted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHeartbeatRequest {
    pub device_id: String,
    pub heartbeat_token: String,
    pub snapshot: HostSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentHeartbeatResponse {
    pub ok: bool,
    pub next_heartbeat_interval_seconds: i32,
    #[serde(default)]
    pub websocket_url: String,
    pub server_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RemoteTerminalShortcutModifier {
    Ctrl,
    Alt,
    Shift,
    Meta,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RemoteTerminalShortcutKind {
    Sequence,
    Key,
}

impl Default for RemoteTerminalShortcutKind {
    fn default() -> Self {
        Self::Sequence
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTerminalShortcutData {
    pub id: String,
    pub label: String,
    pub kind: RemoteTerminalShortcutKind,
    #[serde(default)]
    pub sequence: Vec<String>,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub modifiers: Vec<RemoteTerminalShortcutModifier>,
    #[serde(default)]
    pub preset: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteTerminalQuickCommandData {
    pub id: String,
    pub label: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTerminalAgentPreferencesData {
    #[serde(default)]
    pub default_working_directory: String,
    #[serde(default)]
    pub shortcuts: Vec<RemoteTerminalShortcutData>,
    #[serde(default)]
    pub quick_commands: Vec<RemoteTerminalQuickCommandData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartMessage {
    pub r#type: String,
    pub session_id: String,
    pub mode: String,
    pub shell_type: ShellType,
    #[serde(default)]
    pub working_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInputMessage {
    pub r#type: String,
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResizeMessage {
    pub r#type: String,
    pub session_id: String,
    pub cols: i32,
    pub rows: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStopMessage {
    pub r#type: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryBrowseRequestMessage {
    pub r#type: String,
    pub request_id: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryBrowseResultMessage {
    pub r#type: String,
    pub request_id: String,
    pub ok: bool,
    #[serde(default)]
    pub message: String,
    pub current_path: String,
    #[serde(default)]
    pub parent_path: String,
    #[serde(default)]
    pub items: Vec<DirectoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesGetMessage {
    pub r#type: String,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesSetMessage {
    pub r#type: String,
    pub request_id: String,
    pub preferences: RemoteTerminalAgentPreferencesData,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesResultMessage {
    pub r#type: String,
    pub request_id: String,
    pub ok: bool,
    #[serde(default)]
    pub message: String,
    pub preferences: RemoteTerminalAgentPreferencesData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionReadyMessage {
    pub r#type: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOutputMessage {
    pub r#type: String,
    pub session_id: String,
    pub stream: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExitMessage {
    pub r#type: String,
    pub session_id: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionErrorMessage {
    pub r#type: String,
    pub session_id: String,
    pub message: String,
}

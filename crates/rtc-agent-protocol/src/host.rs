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

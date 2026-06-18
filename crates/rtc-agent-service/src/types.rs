use serde::Serialize;

pub const WINDOWS_SERVICE_NAME: &str = "RemoteTerminalCloudAgent";

pub const MACOS_SERVICE_LABEL: &str = "com.remote-terminal-cloud.agent";
pub const MACOS_PLIST_PATH: &str = "/Library/LaunchDaemons/com.remote-terminal-cloud.agent.plist";
pub const LINUX_SYSTEMD_SERVICE_NAME: &str = "remote-terminal-cloud-agent.service";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceActionResult {
    pub action: String,
    pub ok: bool,
    pub message: String,
}

use anyhow::Result;
use serde::Serialize;

pub const WINDOWS_SERVICE_NAME: &str = "RemoteTerminalCloudAgent";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceActionResult {
    pub action: String,
    pub ok: bool,
    pub message: String,
}

pub fn service_status() -> ServiceActionResult {
    ServiceActionResult {
        action: "status".into(),
        ok: true,
        message: "Rust native service integration is scaffolded; platform-specific implementation is pending.".into(),
    }
}

pub fn install_service(_install_root: &str, _token: Option<&str>) -> Result<ServiceActionResult> {
    Ok(ServiceActionResult {
        action: "install".into(),
        ok: true,
        message: "Service install scaffolded.".into(),
    })
}

pub fn uninstall_service(_install_root: &str) -> Result<ServiceActionResult> {
    Ok(ServiceActionResult {
        action: "uninstall".into(),
        ok: true,
        message: "Service uninstall scaffolded.".into(),
    })
}

pub fn start_service() -> Result<ServiceActionResult> {
    Ok(ServiceActionResult {
        action: "start".into(),
        ok: true,
        message: "Service start scaffolded.".into(),
    })
}

pub fn stop_service(_install_root: &str) -> Result<ServiceActionResult> {
    Ok(ServiceActionResult {
        action: "stop".into(),
        ok: true,
        message: "Service stop scaffolded.".into(),
    })
}

pub fn restart_service(_install_root: &str) -> Result<ServiceActionResult> {
    Ok(ServiceActionResult {
        action: "restart".into(),
        ok: true,
        message: "Service restart scaffolded.".into(),
    })
}

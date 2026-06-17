#[cfg(target_os = "windows")]
use std::path::Path;
#[cfg(target_os = "windows")]
use std::process::Command;

#[cfg(target_os = "windows")]
use anyhow::{Context, Result, bail};
#[cfg(target_os = "windows")]
use rtc_agent_config::{default_config_file_path, persist_registration_token};

#[cfg(target_os = "windows")]
use crate::{ServiceActionResult, WINDOWS_SERVICE_NAME};

#[cfg(target_os = "windows")]
pub fn service_status() -> ServiceActionResult {
    let output = Command::new("sc")
        .args(["query", WINDOWS_SERVICE_NAME])
        .output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let text = if !stdout.is_empty() { stdout } else { stderr };
            ServiceActionResult {
                action: "status".into(),
                ok: out.status.success(),
                message: text.to_string(),
            }
        }
        Err(e) => ServiceActionResult {
            action: "status".into(),
            ok: false,
            message: format!("failed to query service: {e}"),
        },
    }
}

#[cfg(target_os = "windows")]
pub fn install_service(install_root: &str, token: Option<&str>) -> Result<ServiceActionResult> {
    let root = install_root.trim();
    if root.is_empty() {
        bail!("windows service install requires an install_root");
    }

    let bin_path = format!(
        r#""{}" service-host"#,
        Path::new(root).join("rtc-agentd.exe").display()
    );
    let status = Command::new("sc")
        .args([
            "create",
            WINDOWS_SERVICE_NAME,
            "binPath=",
            &bin_path,
            "start=",
            "auto",
            "DisplayName=",
            "Remote Terminal Cloud Agent",
        ])
        .status()
        .context("failed to create service")?;
    if !status.success() {
        bail!("sc create failed");
    }
    if let Some(token) = token {
        let token_trimmed = token.trim();
        if !token_trimmed.is_empty() {
            persist_registration_token(&default_config_file_path(), token_trimmed)?;
            Command::new("sc")
                .args(["config", WINDOWS_SERVICE_NAME, "obj=", "LocalSystem"])
                .status()
                .context("failed to configure service")?;
        }
    }
    Command::new("sc")
        .args(["start", WINDOWS_SERVICE_NAME])
        .status()
        .context("failed to start service")?;
    Ok(ServiceActionResult {
        action: "install".into(),
        ok: true,
        message: format!("service '{WINDOWS_SERVICE_NAME}' installed and started from {root}"),
    })
}

#[cfg(target_os = "windows")]
pub fn uninstall_service() -> Result<ServiceActionResult> {
    Command::new("sc")
        .args(["stop", WINDOWS_SERVICE_NAME])
        .status()
        .ok();
    Command::new("sc")
        .args(["delete", WINDOWS_SERVICE_NAME])
        .status()
        .context("failed to delete service")?;
    Ok(ServiceActionResult {
        action: "uninstall".into(),
        ok: true,
        message: format!("service '{WINDOWS_SERVICE_NAME}' uninstalled"),
    })
}

#[cfg(target_os = "windows")]
pub fn start_service() -> Result<ServiceActionResult> {
    let status = Command::new("sc")
        .args(["start", WINDOWS_SERVICE_NAME])
        .status()
        .context("failed to start service")?;
    if status.success() {
        Ok(ServiceActionResult {
            action: "start".into(),
            ok: true,
            message: format!("service '{WINDOWS_SERVICE_NAME}' started"),
        })
    } else {
        bail!("sc start failed");
    }
}

#[cfg(target_os = "windows")]
pub fn stop_service() -> Result<ServiceActionResult> {
    let status = Command::new("sc")
        .args(["stop", WINDOWS_SERVICE_NAME])
        .status()
        .context("failed to stop service")?;
    if status.success() {
        Ok(ServiceActionResult {
            action: "stop".into(),
            ok: true,
            message: format!("service '{WINDOWS_SERVICE_NAME}' stopped"),
        })
    } else {
        bail!("sc stop failed");
    }
}

#[cfg(target_os = "windows")]
pub fn restart_service() -> Result<ServiceActionResult> {
    Command::new("sc")
        .args(["stop", WINDOWS_SERVICE_NAME])
        .status()
        .ok();
    std::thread::sleep(std::time::Duration::from_secs(2));
    Command::new("sc")
        .args(["start", WINDOWS_SERVICE_NAME])
        .status()
        .context("failed to restart service")?;
    Ok(ServiceActionResult {
        action: "restart".into(),
        ok: true,
        message: format!("service '{WINDOWS_SERVICE_NAME}' restarted"),
    })
}
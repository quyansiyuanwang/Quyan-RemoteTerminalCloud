#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
use std::process::Command;

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
use anyhow::{Context, Result, bail};

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
use crate::{LINUX_SYSTEMD_SERVICE_NAME, ServiceActionResult};

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn service_status() -> ServiceActionResult {
    if !command_exists("systemctl") {
        return ServiceActionResult {
            action: "status".into(),
            ok: false,
            message: "systemctl is not available on this host.".into(),
        };
    }

    match Command::new("systemctl")
        .args(["status", LINUX_SYSTEMD_SERVICE_NAME, "--no-pager", "--full"])
        .output()
    {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let text = if !stdout.trim().is_empty() { stdout } else { stderr };
            ServiceActionResult {
                action: "status".into(),
                ok: out.status.success(),
                message: text.trim().to_owned(),
            }
        }
        Err(err) => ServiceActionResult {
            action: "status".into(),
            ok: false,
            message: format!("failed to query systemd service: {err}"),
        },
    }
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn install_service(install_root: &str) -> Result<ServiceActionResult> {
    if !command_exists("systemctl") {
        bail!("systemctl is required to install the Linux service");
    }
    let root = install_root.trim();
    if root.is_empty() {
        bail!("linux service install requires an install_root");
    }

    let status = Command::new("systemctl")
        .args(["daemon-reload"])
        .status()
        .context("failed to reload systemd")?;
    if !status.success() {
        bail!("systemctl daemon-reload failed");
    }

    let status = Command::new("systemctl")
        .args(["enable", "--now", LINUX_SYSTEMD_SERVICE_NAME])
        .status()
        .context("failed to enable Linux service")?;
    if !status.success() {
        bail!("systemctl enable --now failed for {}", LINUX_SYSTEMD_SERVICE_NAME);
    }

    Ok(ServiceActionResult {
        action: "install".into(),
        ok: true,
        message: format!(
            "systemd service '{}' enabled and started for install root {}",
            LINUX_SYSTEMD_SERVICE_NAME, root
        ),
    })
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn uninstall_service() -> Result<ServiceActionResult> {
    if !command_exists("systemctl") {
        bail!("systemctl is required to uninstall the Linux service");
    }

    Command::new("systemctl").args(["disable", "--now", LINUX_SYSTEMD_SERVICE_NAME]).status().ok();

    Ok(ServiceActionResult {
        action: "uninstall".into(),
        ok: true,
        message: format!("systemd service '{}' disabled", LINUX_SYSTEMD_SERVICE_NAME),
    })
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn start_service() -> Result<ServiceActionResult> {
    run_systemctl("start", &["start", LINUX_SYSTEMD_SERVICE_NAME])
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn stop_service() -> Result<ServiceActionResult> {
    run_systemctl("stop", &["stop", LINUX_SYSTEMD_SERVICE_NAME])
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn restart_service() -> Result<ServiceActionResult> {
    run_systemctl("restart", &["restart", LINUX_SYSTEMD_SERVICE_NAME])
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn run_systemctl(action: &str, args: &[&str]) -> Result<ServiceActionResult> {
    if !command_exists("systemctl") {
        bail!("systemctl is not available on this host");
    }
    let status = Command::new("systemctl")
        .args(args)
        .status()
        .with_context(|| format!("failed to {action} Linux service"))?;
    if !status.success() {
        bail!("systemctl {} failed", args.join(" "));
    }
    Ok(ServiceActionResult {
        action: action.into(),
        ok: true,
        message: format!("systemd service '{}' {}ed", LINUX_SYSTEMD_SERVICE_NAME, action),
    })
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn command_exists(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

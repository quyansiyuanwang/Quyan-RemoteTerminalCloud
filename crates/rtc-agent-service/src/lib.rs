use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Result, Context, anyhow, bail};
use serde::Serialize;

pub const WINDOWS_SERVICE_NAME: &str = "RemoteTerminalCloudAgent";

#[allow(dead_code)]
const MACOS_SERVICE_LABEL: &str = "com.remote-terminal-cloud.agent";
#[allow(dead_code)]
const MACOS_PLIST_PATH: &str = "/Library/LaunchDaemons/com.remote-terminal-cloud.agent.plist";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceActionResult {
    pub action: String,
    pub ok: bool,
    pub message: String,
}

pub fn service_status() -> ServiceActionResult {
    #[cfg(target_os = "windows")]
    {
        windows_service_status()
    }
    #[cfg(target_os = "macos")]
    {
        macos_service_status()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        ServiceActionResult {
            action: "status".into(),
            ok: true,
            message: "Service management is not yet implemented for this platform.".into(),
        }
    }
}

pub fn install_service(install_root: &str, _token: Option<&str>) -> Result<ServiceActionResult> {
    #[cfg(target_os = "windows")]
    {
        windows_install_service(install_root, _token)
    }
    #[cfg(target_os = "macos")]
    {
        macos_install_service(install_root)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = install_root;
        Ok(ServiceActionResult {
            action: "install".into(),
            ok: true,
            message: "Service install is not yet implemented for this platform.".into(),
        })
    }
}

pub fn uninstall_service(_install_root: &str) -> Result<ServiceActionResult> {
    #[cfg(target_os = "windows")]
    {
        windows_uninstall_service(_install_root)
    }
    #[cfg(target_os = "macos")]
    {
        macos_uninstall_service()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(ServiceActionResult {
            action: "uninstall".into(),
            ok: true,
            message: "Service uninstall is not yet implemented for this platform.".into(),
        })
    }
}

pub fn start_service() -> Result<ServiceActionResult> {
    #[cfg(target_os = "windows")]
    {
        windows_start_service()
    }
    #[cfg(target_os = "macos")]
    {
        macos_start_service()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(ServiceActionResult {
            action: "start".into(),
            ok: true,
            message: "Service start is not yet implemented for this platform.".into(),
        })
    }
}

pub fn stop_service(_install_root: &str) -> Result<ServiceActionResult> {
    #[cfg(target_os = "windows")]
    {
        windows_stop_service(_install_root)
    }
    #[cfg(target_os = "macos")]
    {
        macos_stop_service()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(ServiceActionResult {
            action: "stop".into(),
            ok: true,
            message: "Service stop is not yet implemented for this platform.".into(),
        })
    }
}

pub fn restart_service(_install_root: &str) -> Result<ServiceActionResult> {
    #[cfg(target_os = "windows")]
    {
        windows_restart_service(_install_root)
    }
    #[cfg(target_os = "macos")]
    {
        macos_restart_service()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Ok(ServiceActionResult {
            action: "restart".into(),
            ok: true,
            message: "Service restart is not yet implemented for this platform.".into(),
        })
    }
}

// ── macOS launchd ──

#[cfg(target_os = "macos")]
fn macos_service_status() -> ServiceActionResult {
    let output = Command::new("launchctl")
        .args(["print", &format!("system/{MACOS_SERVICE_LABEL}")])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let is_running = stdout.contains("state = running");
            ServiceActionResult {
                action: "status".into(),
                ok: true,
                message: if is_running {
                    format!("launchd service '{MACOS_SERVICE_LABEL}' is running")
                } else {
                    format!("launchd service '{MACOS_SERVICE_LABEL}' is loaded but not running")
                },
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("Could not find service") || stderr.contains("service not found") {
                ServiceActionResult {
                    action: "status".into(),
                    ok: true,
                    message: format!("launchd service '{MACOS_SERVICE_LABEL}' is not installed (pending)"),
                }
            } else {
                ServiceActionResult {
                    action: "status".into(),
                    ok: false,
                    message: format!("launchd service status error: {}", stderr.trim()),
                }
            }
        }
        Err(e) => ServiceActionResult {
            action: "status".into(),
            ok: false,
            message: format!("failed to query launchd: {e}"),
        },
    }
}

#[cfg(target_os = "macos")]
fn macos_install_service(install_root: &str) -> Result<ServiceActionResult> {
    let plist_src = Path::new(install_root).join("com.remote-terminal-cloud.agent.plist");
    let plist_content = if plist_src.exists() {
        fs::read_to_string(&plist_src)
            .with_context(|| format!("read plist template from {}", plist_src.display()))?
    } else {
        generate_launchd_plist(install_root)?
    };
    if let Some(parent) = Path::new(MACOS_PLIST_PATH).parent() {
        fs::create_dir_all(parent).context("create LaunchDaemons directory")?;
    }
    fs::write(MACOS_PLIST_PATH, &plist_content)
        .with_context(|| format!("write {}", MACOS_PLIST_PATH))?;
    std::process::Command::new("launchctl")
        .args(["bootstrap", "system", MACOS_PLIST_PATH])
        .status()
        .context("failed to execute launchctl bootstrap")?;
    Ok(ServiceActionResult {
        action: "install".into(),
        ok: true,
        message: format!("launchd service '{MACOS_SERVICE_LABEL}' installed"),
    })
}

#[cfg(target_os = "macos")]
fn macos_uninstall_service() -> Result<ServiceActionResult> {
    let bootout = Command::new("launchctl")
        .args(["bootout", "system", MACOS_PLIST_PATH])
        .status();
    if let Err(e) = bootout {
        if Path::new(MACOS_PLIST_PATH).exists() {
            fs::remove_file(MACOS_PLIST_PATH)?;
        }
        return Err(anyhow!("failed to unload launchd service: {e}"));
    }
    if Path::new(MACOS_PLIST_PATH).exists() {
        fs::remove_file(MACOS_PLIST_PATH)?;
    }
    Ok(ServiceActionResult {
        action: "uninstall".into(),
        ok: true,
        message: format!("launchd service '{MACOS_SERVICE_LABEL}' uninstalled"),
    })
}

#[cfg(target_os = "macos")]
fn macos_start_service() -> Result<ServiceActionResult> {
    let status = Command::new("launchctl")
        .args(["kickstart", "-k", &format!("system/{MACOS_SERVICE_LABEL}")])
        .status()
        .context("failed to execute launchctl kickstart")?;
    if status.success() {
        Ok(ServiceActionResult {
            action: "start".into(),
            ok: true,
            message: format!("launchd service '{MACOS_SERVICE_LABEL}' started"),
        })
    } else {
        bail!("launchctl kickstart failed");
    }
}

#[cfg(target_os = "macos")]
fn macos_stop_service() -> Result<ServiceActionResult> {
    let status = Command::new("launchctl")
        .args(["bootout", "system", MACOS_PLIST_PATH])
        .status()
        .context("failed to execute launchctl bootout")?;
    if status.success() {
        Ok(ServiceActionResult {
            action: "stop".into(),
            ok: true,
            message: format!("launchd service '{MACOS_SERVICE_LABEL}' stopped"),
        })
    } else {
        bail!("launchctl bootout failed");
    }
}

#[cfg(target_os = "macos")]
fn macos_restart_service() -> Result<ServiceActionResult> {
    let status = Command::new("launchctl")
        .args(["kickstart", "-k", &format!("system/{MACOS_SERVICE_LABEL}")])
        .status()
        .context("failed to execute launchctl kickstart")?;
    if status.success() {
        Ok(ServiceActionResult {
            action: "restart".into(),
            ok: true,
            message: format!("launchd service '{MACOS_SERVICE_LABEL}' restarted"),
        })
    } else {
        bail!("launchctl kickstart failed");
    }
}

#[cfg(target_os = "macos")]
fn generate_launchd_plist(install_root: &str) -> Result<String> {
    let log_root = "/Library/Logs/RemoteTerminalCloudAgent";
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{MACOS_SERVICE_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{install_root}/bin/rtc-agent</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>RTC_CONFIG_FILE</key>
        <string>{install_root}/config.json</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>WorkingDirectory</key>
    <string>{install_root}</string>
    <key>StandardOutPath</key>
    <string>{log_root}/stdout.log</string>
    <key>StandardErrorPath</key>
    <string>{log_root}/stderr.log</string>
</dict>
</plist>
"#))
}

// ── Windows sc ──

#[cfg(target_os = "windows")]
fn windows_service_status() -> ServiceActionResult {
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
                ok: true,
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
fn windows_install_service(install_root: &str, token: Option<&str>) -> Result<ServiceActionResult> {
    let bin_path = format!(
        r#""{}" --service"#,
        Path::new(install_root).join("rtc-agentd.exe").display()
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
            Command::new("sc")
                .args([
                    "config",
                    WINDOWS_SERVICE_NAME,
                    "obj=",
                    "LocalSystem",
                ])
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
        message: format!("service '{WINDOWS_SERVICE_NAME}' installed and started"),
    })
}

#[cfg(target_os = "windows")]
fn windows_uninstall_service(install_root: &str) -> Result<ServiceActionResult> {
    let _ = install_root;
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
fn windows_start_service() -> Result<ServiceActionResult> {
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
fn windows_stop_service(install_root: &str) -> Result<ServiceActionResult> {
    let _ = install_root;
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
fn windows_restart_service(install_root: &str) -> Result<ServiceActionResult> {
    let _ = install_root;
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

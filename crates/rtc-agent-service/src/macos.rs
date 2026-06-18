#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::path::Path;
#[cfg(target_os = "macos")]
use std::process::Command;

#[cfg(target_os = "macos")]
use anyhow::{Context, Result, anyhow, bail};

#[cfg(target_os = "macos")]
use crate::{MACOS_PLIST_PATH, MACOS_SERVICE_LABEL, ServiceActionResult};

#[cfg(target_os = "macos")]
pub fn service_status() -> ServiceActionResult {
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
pub fn install_service(install_root: &str) -> Result<ServiceActionResult> {
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
pub fn uninstall_service() -> Result<ServiceActionResult> {
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
pub fn start_service() -> Result<ServiceActionResult> {
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
pub fn stop_service() -> Result<ServiceActionResult> {
    let status = Command::new("launchctl")
        .args(["disable", &format!("system/{MACOS_SERVICE_LABEL}")])
        .status()
        .context("failed to execute launchctl disable")?;
    if status.success() {
        Ok(ServiceActionResult {
            action: "stop".into(),
            ok: true,
            message: format!("launchd service '{MACOS_SERVICE_LABEL}' stopped (disabled)"),
        })
    } else {
        bail!("launchctl disable failed");
    }
}

#[cfg(target_os = "macos")]
pub fn restart_service() -> Result<ServiceActionResult> {
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
"#
    ))
}
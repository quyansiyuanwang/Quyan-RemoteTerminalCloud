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
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceState {
    Running,
    Stopped,
    Missing,
    Unknown(String),
}

#[cfg(target_os = "windows")]
pub fn query_service_state() -> ServiceState {
    let output = Command::new("sc").args(["query", WINDOWS_SERVICE_NAME]).output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let text = if !stdout.trim().is_empty() { stdout } else { stderr };
            parse_sc_query_output(&text)
        }
        Err(_) => ServiceState::Unknown("failed to query service".into()),
    }
}

/// Parses the text output from `sc query <service_name>` into a ServiceState.
/// Exposed for testing; the text comes either from stdout or stderr of `sc query`.
#[cfg(target_os = "windows")]
pub fn parse_sc_query_output(text: &str) -> ServiceState {
    if text.contains("FAILED 1060")
        || text.contains("service does not exist")
        || text.contains("not installed")
    {
        return ServiceState::Missing;
    }
    if text.contains("STOPPED") || text.contains("stopped") {
        return ServiceState::Stopped;
    }
    if text.contains("RUNNING") || text.contains("running") {
        return ServiceState::Running;
    }
    ServiceState::Unknown(text.trim().to_owned())
}

#[cfg(target_os = "windows")]
pub fn service_status() -> ServiceActionResult {
    let output = Command::new("sc").args(["query", WINDOWS_SERVICE_NAME]).output();
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

    let bin_path =
        format!(r#""{}" service-host"#, Path::new(root).join("rtc-agentd.exe").display());
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
    Command::new("sc").args(["stop", WINDOWS_SERVICE_NAME]).status().ok();
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
    Command::new("sc").args(["stop", WINDOWS_SERVICE_NAME]).status().ok();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_running_state() {
        let output = "SERVICE_NAME: RemoteTerminalCloudAgent\nTYPE               : 10  WIN32_OWN_PROCESS\nSTATE              : 4  RUNNING\n";
        assert_eq!(parse_sc_query_output(output), ServiceState::Running);
    }

    #[test]
    fn parse_stopped_state() {
        let output = "SERVICE_NAME: RemoteTerminalCloudAgent\nSTATE              : 1  STOPPED\n";
        assert_eq!(parse_sc_query_output(output), ServiceState::Stopped);
    }

    #[test]
    fn parse_missing_service() {
        let output = "FAILED 1060: The specified service does not exist as an installed service.";
        assert_eq!(parse_sc_query_output(output), ServiceState::Missing);
    }

    #[test]
    fn parse_missing_service_stderr() {
        let output = "The specified service does not exist as an installed service.";
        assert_eq!(parse_sc_query_output(output), ServiceState::Missing);
    }

    #[test]
    fn parse_pending_state_is_unknown() {
        let output = "STATE              : 3  STOP_PENDING\n";
        assert_eq!(
            parse_sc_query_output(output),
            ServiceState::Unknown("STATE              : 3  STOP_PENDING".into())
        );
    }

    // ── Edge case: start_pending ──

    #[test]
    fn parse_start_pending_is_unknown() {
        let output = "STATE              : 2  START_PENDING\n";
        assert_eq!(
            parse_sc_query_output(output),
            ServiceState::Unknown("STATE              : 2  START_PENDING".into())
        );
    }

    // ── Edge case: paused states ──

    #[test]
    fn parse_paused_is_unknown() {
        let output = "STATE              : 7  PAUSED\n";
        assert_eq!(
            parse_sc_query_output(output),
            ServiceState::Unknown("STATE              : 7  PAUSED".into())
        );
    }

    #[test]
    fn parse_pause_pending_is_unknown() {
        let output = "STATE              : 6  PAUSE_PENDING\n";
        assert_eq!(
            parse_sc_query_output(output),
            ServiceState::Unknown("STATE              : 6  PAUSE_PENDING".into())
        );
    }

    #[test]
    fn parse_continue_pending_is_unknown() {
        let output = "STATE              : 5  CONTINUE_PENDING\n";
        assert_eq!(
            parse_sc_query_output(output),
            ServiceState::Unknown("STATE              : 5  CONTINUE_PENDING".into())
        );
    }

    // ── Edge case: empty and unusual input ──

    #[test]
    fn parse_empty_string_is_unknown() {
        assert_eq!(parse_sc_query_output(""), ServiceState::Unknown("".into()));
    }

    #[test]
    fn parse_whitespace_only_is_unknown() {
        assert_eq!(parse_sc_query_output("   "), ServiceState::Unknown("".into()));
    }

    #[test]
    fn parse_newline_only_is_unknown() {
        assert_eq!(parse_sc_query_output("\n\n"), ServiceState::Unknown("".into()));
    }

    // ── Edge case: lowercase output variants ──

    #[test]
    fn parse_running_lowercase_in_full_output() {
        let output = "SERVICE_NAME: RemoteTerminalCloudAgent\nSTATE              : 4  running\n";
        assert_eq!(parse_sc_query_output(output), ServiceState::Running);
    }

    #[test]
    fn parse_stopped_lowercase() {
        let output = "STATE              : 1  stopped\n";
        assert_eq!(parse_sc_query_output(output), ServiceState::Stopped);
    }

    // ── Edge case: missing service text variants ──

    #[test]
    fn parse_missing_not_installed_text() {
        let output = "The service is not installed.";
        assert_eq!(parse_sc_query_output(output), ServiceState::Missing);
    }

    #[test]
    fn parse_missing_failed_1060_without_full_text() {
        // Some locales emit "FAILED 1060" without the full English message
        let output = "FAILED 1060";
        assert_eq!(parse_sc_query_output(output), ServiceState::Missing);
    }

    // ── Edge case: substring safety ──

    #[test]
    fn parse_does_not_false_positive_on_service_running_in_name() {
        // "RUNNING" appears only inside the service name, not the state field
        let output = "SERVICE_NAME: RunningService\nSTATE              : 1  STOPPED\n";
        assert_eq!(parse_sc_query_output(output), ServiceState::Stopped);
    }

    #[test]
    fn parse_does_not_false_positive_on_stopped_in_other_text() {
        let output = "SERVICE_NAME: StoppedService\nSTATE              : 4  RUNNING\n";
        assert_eq!(parse_sc_query_output(output), ServiceState::Running);
    }

    // ── Edge case: no state keyword at all ──

    #[test]
    fn parse_output_with_only_service_name_is_unknown() {
        let output = "SERVICE_NAME: RemoteTerminalCloudAgent\nTYPE: OWN_PROCESS\n";
        assert_eq!(
            parse_sc_query_output(output),
            ServiceState::Unknown(
                "SERVICE_NAME: RemoteTerminalCloudAgent\nTYPE: OWN_PROCESS".into()
            )
        );
    }

    // ── Edge case: BOM prefix ──

    #[test]
    fn parse_output_with_bom_prefix_is_unknown_not_missing() {
        // BOM characters should not trigger "FAILED 1060" or "missing" text
        let output = "\u{feff}STATE              : 4  RUNNING\n";
        assert_eq!(parse_sc_query_output(output), ServiceState::Running);
    }

    // ── Edge case: multiline error output ──

    #[test]
    fn parse_missing_multiline_stderr() {
        let output =
            "The specified service does not exist.\n\nTry running with administrator privileges.";
        assert_eq!(parse_sc_query_output(output), ServiceState::Missing);
    }
}

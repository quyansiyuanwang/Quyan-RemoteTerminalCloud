use std::path::PathBuf;

use anyhow::Result;
use hostname::get;
use rtc_agent_protocol::{
    AgentCapabilities, HostDiagnostics, HostSnapshot, PlatformId, ShellType, SshCheck,
};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ManagerPaths {
    pub config_dir: PathBuf,
    pub config_file_path: PathBuf,
    pub preferences_path: PathBuf,
    pub logs_dir: PathBuf,
}

pub fn collect_host_snapshot(
    agent_version: &str,
    enabled_shells: &[ShellType],
    default_log_path: String,
) -> Result<HostSnapshot> {
    let platform = current_platform();
    let available_shells = detect_available_shells(enabled_shells);
    let hostname = get().unwrap_or_default().to_string_lossy().to_string();

    Ok(HostSnapshot {
        hostname,
        platform: Some(platform),
        arch: std::env::consts::ARCH.to_owned(),
        agent_version: agent_version.to_owned(),
        capabilities: AgentCapabilities {
            ssh_forward: true,
            native_pty: true,
            self_update: true,
            proxy_aware: true,
            service_managed: true,
            session_recording: false,
        },
        diagnostics: HostDiagnostics {
            install_formats: install_formats_for(platform),
            service_manager: service_manager_for(platform).to_owned(),
            default_log_path,
            available_shells,
            ssh_check: ssh_check_for(platform),
            notes: notes_for(platform),
        },
    })
}

pub fn detect_available_shells(enabled_shells: &[ShellType]) -> Vec<ShellType> {
    let mut detected = vec![ShellType::SystemDefault];
    #[cfg(target_os = "windows")]
    {
        detected.push(ShellType::Cmd);
        if which::which("powershell.exe").is_ok() {
            detected.push(ShellType::Powershell);
        }
        if which::which("pwsh.exe").is_ok() {
            detected.push(ShellType::Pwsh);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        for candidate in [ShellType::Bash, ShellType::Zsh, ShellType::Sh, ShellType::Pwsh] {
            if which::which(candidate.as_str()).is_ok() {
                detected.push(candidate);
            }
        }
    }

    if enabled_shells.is_empty() {
        return detected;
    }

    detected.into_iter().filter(|item| enabled_shells.contains(item)).collect()
}

pub fn resolve_default_shell(configured: ShellType, available: &[ShellType]) -> ShellType {
    if available.contains(&configured) {
        return configured;
    }
    if available.contains(&ShellType::SystemDefault) {
        return ShellType::SystemDefault;
    }
    available.first().copied().unwrap_or(configured)
}

pub fn current_platform() -> PlatformId {
    #[cfg(target_os = "windows")]
    {
        PlatformId::Windows
    }
    #[cfg(target_os = "linux")]
    {
        PlatformId::Linux
    }
    #[cfg(target_os = "macos")]
    {
        PlatformId::Macos
    }
}

fn install_formats_for(platform: PlatformId) -> Vec<String> {
    match platform {
        PlatformId::Windows => vec!["exe".into()],
        PlatformId::Linux => vec!["deb".into(), "rpm".into(), "binary".into()],
        PlatformId::Macos => vec!["pkg".into(), "signed-helper".into()],
    }
}

fn service_manager_for(platform: PlatformId) -> &'static str {
    match platform {
        PlatformId::Windows => "Windows Service",
        PlatformId::Linux => "systemd",
        PlatformId::Macos => "launchd",
    }
}

fn ssh_check_for(platform: PlatformId) -> SshCheck {
    match platform {
        PlatformId::Windows => SshCheck {
            available: true,
            detail: "sshd service status probe will be implemented in Rust runtime.".into(),
        },
        PlatformId::Linux => SshCheck {
            available: true,
            detail: "sshd binary probe will be implemented in Rust runtime.".into(),
        },
        PlatformId::Macos => SshCheck {
            available: true,
            detail: "Remote Login probe will be implemented in Rust runtime.".into(),
        },
    }
}

fn notes_for(platform: PlatformId) -> Vec<String> {
    match platform {
        PlatformId::Windows => vec![
            "MVP expects local OpenSSH Server.".into(),
            "Validate UAC, Defender and localized paths.".into(),
        ],
        PlatformId::Linux => vec![
            "MVP expects local sshd.".into(),
            "Validate glibc, SELinux and filesystem constraints.".into(),
        ],
        PlatformId::Macos => vec![
            "MVP expects Remote Login/OpenSSH.".into(),
            "Validate notarization and Full Disk Access.".into(),
        ],
    }
}

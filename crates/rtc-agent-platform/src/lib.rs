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

#[cfg(test)]
mod tests {
    use super::*;

    // ── resolve_default_shell ──

    #[test]
    fn resolve_uses_configured_when_available() {
        let available = vec![ShellType::Bash, ShellType::Zsh, ShellType::SystemDefault];
        assert_eq!(resolve_default_shell(ShellType::Bash, &available), ShellType::Bash);
    }

    #[test]
    fn resolve_falls_back_to_system_default() {
        let available = vec![ShellType::SystemDefault, ShellType::Sh];
        assert_eq!(resolve_default_shell(ShellType::Bash, &available), ShellType::SystemDefault);
    }

    #[test]
    fn resolve_falls_back_to_first_available() {
        let available = vec![ShellType::Zsh];
        assert_eq!(resolve_default_shell(ShellType::Bash, &available), ShellType::Zsh);
    }

    #[test]
    fn resolve_returns_configured_when_none_available() {
        let available: Vec<ShellType> = vec![];
        assert_eq!(resolve_default_shell(ShellType::Bash, &available), ShellType::Bash);
    }

    #[test]
    fn resolve_configured_is_system_default_and_available() {
        let available = vec![ShellType::SystemDefault, ShellType::Bash];
        assert_eq!(
            resolve_default_shell(ShellType::SystemDefault, &available),
            ShellType::SystemDefault
        );
    }

    #[test]
    fn resolve_deduplicates_available_shells() {
        // Even with duplicates, resolve picks configured
        let available = vec![ShellType::Bash, ShellType::Bash, ShellType::Zsh];
        assert_eq!(resolve_default_shell(ShellType::Zsh, &available), ShellType::Zsh);
    }

    // ── current_platform ──

    #[test]
    fn current_platform_returns_expected() {
        let platform = current_platform();
        #[cfg(target_os = "windows")]
        assert_eq!(platform, PlatformId::Windows);
        #[cfg(target_os = "linux")]
        assert_eq!(platform, PlatformId::Linux);
        #[cfg(target_os = "macos")]
        assert_eq!(platform, PlatformId::Macos);
    }

    // ── install_formats_for ──

    #[test]
    fn install_formats_for_windows() {
        let formats = install_formats_for(PlatformId::Windows);
        assert_eq!(formats, vec!["exe"]);
    }

    #[test]
    fn install_formats_for_linux() {
        let formats = install_formats_for(PlatformId::Linux);
        assert_eq!(formats, vec!["deb", "rpm", "binary"]);
    }

    #[test]
    fn install_formats_for_macos() {
        let formats = install_formats_for(PlatformId::Macos);
        assert_eq!(formats, vec!["pkg", "signed-helper"]);
    }

    // ── service_manager_for ──

    #[test]
    fn service_manager_for_windows() {
        assert_eq!(service_manager_for(PlatformId::Windows), "Windows Service");
    }

    #[test]
    fn service_manager_for_linux() {
        assert_eq!(service_manager_for(PlatformId::Linux), "systemd");
    }

    #[test]
    fn service_manager_for_macos() {
        assert_eq!(service_manager_for(PlatformId::Macos), "launchd");
    }

    // ── ssh_check_for ──

    #[test]
    fn ssh_check_for_windows() {
        let check = ssh_check_for(PlatformId::Windows);
        assert!(check.available);
        assert!(check.detail.contains("sshd"));
    }

    #[test]
    fn ssh_check_for_linux() {
        let check = ssh_check_for(PlatformId::Linux);
        assert!(check.available);
        assert!(check.detail.contains("sshd"));
    }

    #[test]
    fn ssh_check_for_macos() {
        let check = ssh_check_for(PlatformId::Macos);
        assert!(check.available);
        assert!(check.detail.contains("Remote Login"));
    }

    // ── notes_for ──

    #[test]
    fn notes_for_windows() {
        let notes = notes_for(PlatformId::Windows);
        assert!(notes.iter().any(|n| n.contains("OpenSSH")));
    }

    #[test]
    fn notes_for_linux() {
        let notes = notes_for(PlatformId::Linux);
        assert!(notes.iter().any(|n| n.contains("sshd")));
    }

    #[test]
    fn notes_for_macos() {
        let notes = notes_for(PlatformId::Macos);
        assert!(notes.iter().any(|n| n.contains("Remote Login")));
    }

    // ── ManagerPaths ──

    #[test]
    fn manager_paths_is_debug_and_serialize() {
        let paths = ManagerPaths {
            config_dir: PathBuf::from("/cfg"),
            config_file_path: PathBuf::from("/cfg/config.json"),
            preferences_path: PathBuf::from("/cfg/prefs.json"),
            logs_dir: PathBuf::from("/logs"),
        };
        // Debug format is available
        let debug = format!("{paths:?}");
        assert!(debug.contains("/cfg"));
        // Serialize is available
        let json = serde_json::to_string(&paths).unwrap();
        assert!(json.contains("config_dir"));
    }
}

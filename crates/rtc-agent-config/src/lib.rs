use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use rtc_agent_protocol::ShellType;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const LOCAL_SERVER_BASE_URL: &str = "http://localhost:10001";
pub const RELEASE_SERVER_BASE_URL: &str = "https://api.qysyw.cn";

static DOTENV_ONCE: OnceLock<()> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FileConfig {
    pub registration_token: Option<String>,
    pub run_heartbeat: Option<bool>,
    pub run_tunnel: Option<bool>,
    #[serde(default)]
    pub default_shell_type: String,
    #[serde(default)]
    pub enabled_shell_types: Vec<String>,
    #[serde(default)]
    pub preferences_file_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfig {
    pub server_base_url: String,
    pub registration_token: Option<String>,
    pub run_heartbeat: bool,
    pub run_tunnel: bool,
    pub default_shell_type: ShellType,
    pub enabled_shell_types: Vec<ShellType>,
    pub preferences_file_path: PathBuf,
    pub config_file_path: PathBuf,
}

pub fn ensure_dotenv_loaded() {
    DOTENV_ONCE.get_or_init(|| {
        let should_load = env::var("RTC_LOAD_DOTENV")
            .ok()
            .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        if !should_load {
            return;
        }
        for candidate in dotenv_candidates() {
            if candidate.is_file() {
                let _ = dotenvy::from_path(candidate);
                break;
            }
        }
    });
}

fn dotenv_candidates() -> Vec<PathBuf> {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    vec![cwd.join(".env"), cwd.parent().unwrap_or(&cwd).join(".env")]
}

pub fn default_config_file_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            if !appdata.trim().is_empty() {
                return PathBuf::from(appdata)
                    .join("remote-terminal-cloud-agent")
                    .join("config.json");
            }
        }
        let roaming = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        return roaming.join("remote-terminal-cloud-agent").join("config.json");
    }
    #[cfg(target_os = "macos")]
    {
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        return base
            .join("Library")
            .join("Application Support")
            .join("remote-terminal-cloud-agent")
            .join("config.json");
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(config_home) = env::var("XDG_CONFIG_HOME") {
            if !config_home.trim().is_empty() {
                return PathBuf::from(config_home)
                    .join("remote-terminal-cloud-agent")
                    .join("config.json");
            }
        }
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("remote-terminal-cloud-agent").join("config.json")
    }
}

pub fn default_preferences_file_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = env::var("APPDATA") {
            if !appdata.trim().is_empty() {
                return PathBuf::from(appdata)
                    .join("remote-terminal-cloud-agent")
                    .join("preferences.json");
            }
        }
        let roaming = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        return roaming.join("remote-terminal-cloud-agent").join("preferences.json");
    }
    #[cfg(target_os = "macos")]
    {
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        return base
            .join("Library")
            .join("Application Support")
            .join("remote-terminal-cloud-agent")
            .join("preferences.json");
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(state_home) = env::var("XDG_STATE_HOME") {
            if !state_home.trim().is_empty() {
                return PathBuf::from(state_home)
                    .join("remote-terminal-cloud-agent")
                    .join("preferences.json");
            }
        }
        let base = dirs::state_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("remote-terminal-cloud-agent").join("preferences.json")
    }
}

pub fn read_runtime_config(server_base_url: &str) -> RuntimeConfig {
    ensure_dotenv_loaded();

    let config_file_path = env::var("RTC_CONFIG_FILE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_config_file_path);

    let file_config = read_config_file(&config_file_path);

    let registration_token = normalize_template_string(env::var("RTC_REGISTRATION_TOKEN").ok())
        .or(file_config.registration_token);

    let run_heartbeat = read_boolean_env("RTC_DISABLE_HEARTBEAT")
        .map(|disabled| !disabled)
        .or(file_config.run_heartbeat)
        .unwrap_or(true);
    let run_tunnel = read_boolean_env("RTC_DISABLE_TUNNEL")
        .map(|disabled| !disabled)
        .or(file_config.run_tunnel)
        .unwrap_or(true);

    let default_shell_type = env::var("RTC_DEFAULT_SHELL")
        .ok()
        .and_then(parse_shell_type)
        .or_else(|| parse_shell_type(file_config.default_shell_type.clone()))
        .unwrap_or(ShellType::SystemDefault);

    let enabled_shell_types = env::var("RTC_ENABLED_SHELLS")
        .ok()
        .map(|value| value.split(',').map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or(file_config.enabled_shell_types)
        .into_iter()
        .filter_map(parse_shell_type)
        .fold(Vec::<ShellType>::new(), |mut acc, item| {
            if !acc.contains(&item) {
                acc.push(item);
            }
            acc
        });

    let preferences_file_path = env::var("RTC_PREFERENCES_FILE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            if file_config.preferences_file_path.trim().is_empty() {
                None
            } else {
                Some(PathBuf::from(file_config.preferences_file_path))
            }
        })
        .unwrap_or_else(default_preferences_file_path);

    RuntimeConfig {
        server_base_url: env::var("RTC_SERVER_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| server_base_url.to_owned()),
        registration_token,
        run_heartbeat,
        run_tunnel,
        default_shell_type,
        enabled_shell_types,
        preferences_file_path,
        config_file_path,
    }
}

pub fn default_server_base_url() -> &'static str {
    option_env!("RTC_AGENT_SERVER_BASE_URL").unwrap_or(RELEASE_SERVER_BASE_URL)
}

pub fn read_config_file(path: &Path) -> FileConfig {
    let Ok(content) = fs::read_to_string(path) else {
        return FileConfig::default();
    };
    let Ok(mut config) = serde_json::from_str::<FileConfig>(&content) else {
        return FileConfig::default();
    };
    config.registration_token = normalize_template_string(config.registration_token);
    config.preferences_file_path = config.preferences_file_path.trim().to_owned();
    config
}

pub fn persist_registration_token(path: &Path, token: &str) -> Result<()> {
    let mut config = read_config_file(path);
    config.registration_token = normalize_template_string(Some(token.to_owned()));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let payload = serde_json::to_string_pretty(&config)?;
    fs::write(path, payload).with_context(|| format!("write {}", path.display()))
}

pub fn has_registration_token_env_override() -> bool {
    normalize_template_string(env::var("RTC_REGISTRATION_TOKEN").ok()).is_some()
}

// ── Agent runtime state (persisted across restarts) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentState {
    pub device_id: String,
    pub heartbeat_token: String,
    pub heartbeat_interval_seconds: i32,
    pub websocket_url: String,
    #[serde(default)]
    pub server_base_url: String,
    #[serde(default)]
    pub device_fingerprint: String,
    #[serde(default)]
    pub fingerprint_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFingerprintSources {
    pub machine_id: bool,
    pub board_serial: bool,
    pub platform_uuid: bool,
    pub fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFingerprint {
    pub device_fingerprint: String,
    pub fingerprint_version: String,
    pub sources: DeviceFingerprintSources,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedDeviceFingerprint {
    device_fingerprint: String,
    fingerprint_version: String,
    sources: DeviceFingerprintSources,
    updated_at: String,
}

/// Derives the state file path from the config file path (sibling `state.json`).
pub fn state_file_path(config_file_path: &Path) -> PathBuf {
    config_file_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("state.json")
}

pub fn device_fingerprint_file_path(config_file_path: &Path) -> PathBuf {
    config_file_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("device-fingerprint.json")
}

pub fn load_cached_device_fingerprint(config_file_path: &Path) -> Option<DeviceFingerprint> {
    let path = device_fingerprint_file_path(config_file_path);
    let content = fs::read_to_string(path).ok()?;
    let cached = serde_json::from_str::<CachedDeviceFingerprint>(&content).ok()?;
    Some(DeviceFingerprint {
        device_fingerprint: cached.device_fingerprint,
        fingerprint_version: cached.fingerprint_version,
        sources: cached.sources,
    })
}

pub fn save_device_fingerprint(
    config_file_path: &Path,
    fingerprint: &str,
    version: &str,
    sources: &DeviceFingerprintSources,
) -> Result<()> {
    let path = device_fingerprint_file_path(config_file_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let payload = CachedDeviceFingerprint {
        device_fingerprint: fingerprint.to_owned(),
        fingerprint_version: version.to_owned(),
        sources: sources.clone(),
        updated_at: "cached".to_owned(),
    };
    fs::write(&path, serde_json::to_string_pretty(&payload)?).with_context(|| format!("write {}", path.display()))
}

pub fn collect_device_fingerprint() -> Result<DeviceFingerprint> {
    Ok(build_device_fingerprint_from_material(collect_fingerprint_material()))
}

pub fn load_or_collect_device_fingerprint(config_file_path: &Path) -> Result<DeviceFingerprint> {
    match collect_device_fingerprint() {
        Ok(fingerprint) => {
            save_device_fingerprint(
                config_file_path,
                &fingerprint.device_fingerprint,
                &fingerprint.fingerprint_version,
                &fingerprint.sources,
            )?;
            Ok(fingerprint)
        }
        Err(err) => {
            if let Some(cached) = load_cached_device_fingerprint(config_file_path) {
                return Ok(cached);
            }
            Err(err)
        }
    }
}

pub fn load_agent_state(config_file_path: &Path) -> Option<AgentState> {
    let path = state_file_path(config_file_path);
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_agent_state(config_file_path: &Path, state: &AgentState) -> Result<()> {
    let path = state_file_path(config_file_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let payload = serde_json::to_string_pretty(state)?;
    fs::write(&path, payload).with_context(|| format!("write {}", path.display()))
}

pub fn clear_agent_state(config_file_path: &Path) {
    let path = state_file_path(config_file_path);
    let _ = fs::remove_file(path);
}

#[derive(Debug, Default)]
struct FingerprintMaterial {
    machine_id: Option<String>,
    board_serial: Option<String>,
    platform_uuid: Option<String>,
    fallback_seed: Option<String>,
}

fn build_device_fingerprint_from_material(material: FingerprintMaterial) -> DeviceFingerprint {
    let mut normalized_parts = Vec::new();

    if let Some(machine_id) = material.machine_id.as_deref() {
        normalized_parts.push(format!("machine-id={}", normalize_fingerprint_source(machine_id)));
    }
    if let Some(board_serial) = material.board_serial.as_deref() {
        normalized_parts.push(format!("board-serial={}", normalize_fingerprint_source(board_serial)));
    }
    if let Some(platform_uuid) = material.platform_uuid.as_deref() {
        normalized_parts.push(format!("platform-uuid={}", normalize_fingerprint_source(platform_uuid)));
    }

    let fallback = normalized_parts.is_empty();
    if fallback {
        normalized_parts.push(format!(
            "fallback={}",
            normalize_fingerprint_source(material.fallback_seed.as_deref().unwrap_or("unknown-device"))
        ));
    }

    let digest = Sha256::digest(normalized_parts.join("|").as_bytes());
    DeviceFingerprint {
        device_fingerprint: hex::encode(digest),
        fingerprint_version: if fallback { "v1-fallback" } else { "v1" }.to_owned(),
        sources: DeviceFingerprintSources {
            machine_id: material.machine_id.is_some(),
            board_serial: material.board_serial.is_some(),
            platform_uuid: material.platform_uuid.is_some(),
            fallback,
        },
    }
}

fn normalize_fingerprint_source(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' => ch,
            _ => '-',
        })
        .collect::<String>()
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn collect_fingerprint_material() -> FingerprintMaterial {
    let mut material = FingerprintMaterial::default();

    #[cfg(target_os = "windows")]
    {
        material.machine_id = read_windows_machine_guid();
    }

    #[cfg(target_os = "linux")]
    {
        material.machine_id = read_first_existing_file(&["/etc/machine-id", "/var/lib/dbus/machine-id"]);
    }

    #[cfg(target_os = "macos")]
    {
        material.platform_uuid = read_command_output("ioreg", &["-rd1", "-c", "IOPlatformExpertDevice"])
            .and_then(|output| extract_ioplatform_uuid(&output));
    }

    if material.machine_id.is_none() && material.platform_uuid.is_none() && material.board_serial.is_none() {
        material.fallback_seed = Some(
            env::var("COMPUTERNAME")
                .or_else(|_| env::var("HOSTNAME"))
                .unwrap_or_else(|_| "unknown-host".to_owned()),
        );
    }

    material
}

#[cfg(target_os = "windows")]
fn read_windows_machine_guid() -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm.open_subkey("SOFTWARE\\Microsoft\\Cryptography").ok()?;
    key.get_value::<String, _>("MachineGuid").ok()
}

#[cfg(target_os = "linux")]
fn read_first_existing_file(paths: &[&str]) -> Option<String> {
    paths.iter().find_map(|path| {
        fs::read_to_string(path)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    })
}

#[cfg(target_os = "macos")]
fn read_command_output(command: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(command).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

#[cfg(target_os = "macos")]
fn extract_ioplatform_uuid(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        if !line.contains("IOPlatformUUID") {
            return None;
        }
        let parts = line.split('"').collect::<Vec<_>>();
        if parts.len() >= 4 {
            Some(parts[3].to_owned())
        } else {
            None
        }
    })
}

pub fn normalize_template_string(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_owned();
    match value.as_str() {
        "" | "replace-with-real-token" => None,
        _ => Some(value),
    }
}

fn read_boolean_env(name: &str) -> Option<bool> {
    let value = env::var(name).ok()?;
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_shell_type(value: String) -> Option<ShellType> {
    match value.trim() {
        "system-default" => Some(ShellType::SystemDefault),
        "cmd" => Some(ShellType::Cmd),
        "powershell" => Some(ShellType::Powershell),
        "pwsh" => Some(ShellType::Pwsh),
        "bash" => Some(ShellType::Bash),
        "zsh" => Some(ShellType::Zsh),
        "sh" => Some(ShellType::Sh),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{FingerprintMaterial, build_device_fingerprint_from_material, normalize_fingerprint_source};

    #[test]
    fn fingerprint_is_stable_across_restart() {
        let first = build_device_fingerprint_from_material(FingerprintMaterial {
            machine_id: Some("ABC-123".into()),
            board_serial: None,
            platform_uuid: None,
            fallback_seed: None,
        });
        let second = build_device_fingerprint_from_material(FingerprintMaterial {
            machine_id: Some("abc-123".into()),
            board_serial: None,
            platform_uuid: None,
            fallback_seed: None,
        });
        assert_eq!(first.device_fingerprint, second.device_fingerprint);
        assert_eq!(first.fingerprint_version, "v1");
    }

    #[test]
    fn fallback_path_is_explicitly_marked() {
        let fingerprint = build_device_fingerprint_from_material(FingerprintMaterial {
            machine_id: None,
            board_serial: None,
            platform_uuid: None,
            fallback_seed: Some("Fallback Seed".into()),
        });
        assert_eq!(fingerprint.fingerprint_version, "v1-fallback");
        assert!(fingerprint.sources.fallback);
    }

    #[test]
    fn normalization_compacts_separators() {
        assert_eq!(normalize_fingerprint_source("  A_B:C  "), "a-b-c");
    }
}

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use rtc_agent_protocol::ShellType;
use serde::{Deserialize, Serialize};

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
}

/// Derives the state file path from the config file path (sibling `state.json`).
pub fn state_file_path(config_file_path: &Path) -> PathBuf {
    config_file_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("state.json")
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

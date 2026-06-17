use std::cmp;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use rtc_agent_config::{
    RuntimeConfig, default_config_file_path, default_preferences_file_path,
    default_server_base_url, persist_registration_token, read_runtime_config,
};
use rtc_agent_platform::ManagerPaths;
use rtc_agent_preferences::PreferencesStore;
use rtc_agent_protocol::ShellType;
use rtc_agent_runtime::{ApiErrorKind, describe_error};
use serde::Serialize;

use crate::{MAX_BACKOFF_INTERVAL, PreferencesSummary};

pub fn runtime_config() -> RuntimeConfig {
    read_runtime_config(default_server_base_url())
}

pub fn manager_paths() -> ManagerPaths {
    let config_file_path = default_config_file_path();
    let config_dir = config_file_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let preferences_path = default_preferences_file_path();
    let logs_dir = managed_logs_dir();
    ManagerPaths { config_dir, config_file_path, preferences_path, logs_dir }
}

pub fn managed_logs_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("ProgramData")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
        return base.join("RemoteTerminalCloudAgent").join("logs");
    }
    #[cfg(not(target_os = "windows"))]
    {
        default_preferences_file_path().parent().unwrap_or(Path::new(".")).join("logs")
    }
}

pub fn read_preferences_summary(path: &Path) -> PreferencesSummary {
    let store = PreferencesStore::new(path);
    let preferences = store.get_preferences();
    PreferencesSummary {
        default_working_directory: preferences.default_working_directory,
        shortcuts_count: preferences.shortcuts.len(),
        quick_commands_count: preferences.quick_commands.len(),
    }
}

pub fn prompt_and_persist_registration_token(path: &Path) -> Result<Option<String>> {
    println!(
        "[remote-terminal-cloud-agent] registration token is not configured. Enter token to save into {}",
        path.display()
    );
    print!("[remote-terminal-cloud-agent] token (press Enter to save, empty to skip): ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let token = input.trim().to_owned();
    if token.is_empty() {
        return Ok(None);
    }
    persist_registration_token(path, &token)?;
    println!("[remote-terminal-cloud-agent] token saved to {}", path.display());
    Ok(Some(token))
}

pub fn join_shells(items: &[ShellType]) -> String {
    if items.is_empty() {
        "none".into()
    } else {
        items.iter().map(|item| item.as_str()).collect::<Vec<_>>().join(", ")
    }
}

pub fn emit_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn grow_backoff(current: Duration) -> Duration {
    cmp::min(current.saturating_mul(2), MAX_BACKOFF_INTERVAL)
}

pub fn next_backoff_delay(current: Duration) -> Duration {
    let jitter_seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.subsec_nanos() as u64)
        .unwrap_or(0);
    let jitter = jitter_seed % 2;
    current.saturating_add(Duration::from_secs(jitter))
}

pub fn is_authentication_error(err: &anyhow::Error) -> bool {
    matches!(
        describe_error(err).map(|details| details.kind),
        Some(ApiErrorKind::InvalidToken | ApiErrorKind::Unauthorized)
    )
}

pub fn is_interactive_input_available() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

pub fn print_runtime_error(prefix: &str, err: &anyhow::Error) {
    if let Some(details) = describe_error(err) {
        eprintln!("{prefix}: {}", details.message);
        eprintln!("[remote-terminal-cloud-agent] suggestion: {}", details.suggestion);
    } else {
        eprintln!("{prefix}: {err}");
    }
}

pub fn user_label_for_error_kind(kind: &ApiErrorKind) -> &'static str {
    match kind {
        ApiErrorKind::InvalidToken => "invalid or expired token",
        ApiErrorKind::DeviceLimitReached => "device limit reached",
        ApiErrorKind::GatewayUnavailable => "backend gateway unavailable",
        ApiErrorKind::ProxyConfiguration => "proxy or gateway issue",
        ApiErrorKind::WebsocketUnavailable => "websocket connection unavailable",
        ApiErrorKind::Unauthorized => "backend rejected credentials",
        ApiErrorKind::ServerRejected => "backend rejected request",
        ApiErrorKind::Transport => "network transport issue",
        ApiErrorKind::Unexpected => "unexpected error",
    }
}
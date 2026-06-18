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
        base.join("RemoteTerminalCloudAgent").join("logs")
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- grow_backoff ---

    #[test]
    fn grow_backoff_doubles() {
        assert_eq!(grow_backoff(Duration::from_secs(2)), Duration::from_secs(4));
        assert_eq!(grow_backoff(Duration::from_secs(4)), Duration::from_secs(8));
        assert_eq!(grow_backoff(Duration::from_secs(8)), Duration::from_secs(16));
    }

    #[test]
    fn grow_backoff_capped_at_max() {
        let max = Duration::from_secs(60);
        assert_eq!(grow_backoff(Duration::from_secs(60)), max);
        assert_eq!(grow_backoff(Duration::from_secs(100)), max);
    }

    #[test]
    fn grow_backoff_zero_stays_zero() {
        assert_eq!(grow_backoff(Duration::ZERO), Duration::ZERO);
    }

    // --- next_backoff_delay ---

    #[test]
    fn next_backoff_delay_at_least_base() {
        let delay = next_backoff_delay(Duration::from_secs(10));
        assert!(delay >= Duration::from_secs(10));
        assert!(delay <= Duration::from_secs(11));
    }

    // --- join_shells ---

    #[test]
    fn join_shells_empty() {
        assert_eq!(join_shells(&[]), "none");
    }

    #[test]
    fn join_shells_single() {
        let shells = [ShellType::Bash];
        assert_eq!(join_shells(&shells), "bash");
    }

    #[test]
    fn join_shells_multiple() {
        let shells = [ShellType::Bash, ShellType::Zsh, ShellType::Powershell];
        let result = join_shells(&shells);
        assert!(result.contains("bash"));
        assert!(result.contains("zsh"));
        assert!(result.contains("powershell"));
        assert_eq!(result.matches(", ").count(), 2);
    }

    // --- is_authentication_error ---

    #[test]
    fn auth_error_invalid_token() {
        let err = anyhow::anyhow!(rtc_agent_runtime::ApiError {
            kind: rtc_agent_runtime::ApiErrorKind::InvalidToken,
            status: Some(401),
            code: None,
            message: "invalid token".into(),
            suggestion: "update token".into(),
        });
        assert!(is_authentication_error(&err));
    }

    #[test]
    fn auth_error_unauthorized() {
        let err = anyhow::anyhow!(rtc_agent_runtime::ApiError {
            kind: rtc_agent_runtime::ApiErrorKind::Unauthorized,
            status: Some(403),
            code: None,
            message: "unauthorized".into(),
            suggestion: "check credentials".into(),
        });
        assert!(is_authentication_error(&err));
    }

    #[test]
    fn transport_error_not_auth() {
        let err = anyhow::anyhow!(rtc_agent_runtime::ApiError {
            kind: rtc_agent_runtime::ApiErrorKind::Transport,
            status: Some(503),
            code: None,
            message: "service unavailable".into(),
            suggestion: "retry later".into(),
        });
        assert!(!is_authentication_error(&err));
    }

    #[test]
    fn generic_error_not_auth() {
        let err = anyhow::anyhow!("something completely different");
        assert!(!is_authentication_error(&err));
    }

    // --- normalize_template_string (replicating the function inline to test without dep) ---

    fn normalize(value: Option<&str>) -> Option<String> {
        let value = value?.trim().to_owned();
        match value.as_str() {
            "" | "replace-with-real-token" => None,
            _ => Some(value),
        }
    }

    #[test]
    fn normalize_none_is_none() {
        assert_eq!(normalize(None), None);
    }

    #[test]
    fn normalize_empty_is_none() {
        assert_eq!(normalize(Some("")), None);
    }

    #[test]
    fn normalize_whitespace_only_is_none() {
        assert_eq!(normalize(Some("  ")), None);
    }

    #[test]
    fn normalize_placeholder_token_is_none() {
        assert_eq!(normalize(Some("replace-with-real-token")), None);
    }

    #[test]
    fn normalize_valid_token_preserved() {
        assert_eq!(normalize(Some("abc123")), Some("abc123".into()));
    }

    #[test]
    fn normalize_valid_token_trimmed() {
        assert_eq!(normalize(Some("  abc-456  ")), Some("abc-456".into()));
    }

    #[test]
    fn normalize_placeholder_with_whitespace() {
        assert_eq!(normalize(Some("  replace-with-real-token  ")), None);
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

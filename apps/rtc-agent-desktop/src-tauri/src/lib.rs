use std::collections::VecDeque;
use std::env;
use std::fs;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow, bail};
use rtc_agent_config::{
    RuntimeConfig, default_config_file_path, default_server_base_url,
    has_registration_token_env_override, normalize_template_string, persist_registration_token,
    read_config_file, read_runtime_config,
};
use rtc_agent_platform::{
    collect_host_snapshot, resolve_default_shell,
};
use rtc_agent_preferences::PreferencesStore;
use rtc_agent_protocol::ShellType;
use rtc_agent_runtime::{ApiClient, describe_error, run_agent_tunnel_with_connect_hook};
use rtc_agent_service as service;
use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Listener, Manager, State, WindowEvent};
#[cfg(target_os = "windows")]
use winreg::RegKey;
#[cfg(target_os = "windows")]
use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const TRAY_ID: &str = "rtc-agent-tray";
const MENU_OPEN: &str = "open-manager";
const MENU_START_AGENT: &str = "start-agent";
const MENU_STOP_AGENT: &str = "stop-agent";
const MENU_RESTART_AGENT: &str = "restart-agent";
const MENU_SAVE_AUTOSTART: &str = "save-autostart";
const MENU_OPEN_CONFIG: &str = "open-config";
const MENU_OPEN_LOGS: &str = "open-logs";
const MENU_QUIT: &str = "quit";
const MAIN_WINDOW_LABEL: &str = "main";
const APP_RUN_REG_VALUE: &str = "RemoteTerminalCloudAgentDesktop";
const DESKTOP_STATE_FILE_NAME: &str = "desktop-state.json";
const MISSING_CONFIG_RETRY: Duration = Duration::from_secs(30);
const RUNTIME_RETRY_INTERVAL: Duration = Duration::from_secs(10);
const MAX_BACKOFF_INTERVAL: Duration = Duration::from_secs(90);
const HEARTBEAT_STALE_TIMEOUT: Duration = Duration::from_secs(75);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum DesktopRuntimePhase {
    Idle,
    WaitingConfig,
    Starting,
    Registering,
    Online,
    Reconnecting,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopRuntimeSnapshot {
    phase: DesktopRuntimePhase,
    connected: bool,
    registered_device_id: Option<String>,
    websocket_url: Option<String>,
    last_heartbeat_at: Option<String>,
    last_error: Option<String>,
    retry_attempt: u32,
}

#[derive(Debug, Clone, Copy)]
enum TaskSignal {
    Heartbeat,
    Tunnel,
    Watchdog,
}

impl Default for DesktopRuntimeSnapshot {
    fn default() -> Self {
        Self {
            phase: DesktopRuntimePhase::Idle,
            connected: false,
            registered_device_id: None,
            websocket_url: None,
            last_heartbeat_at: None,
            last_error: None,
            retry_attempt: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreferencesSummary {
    default_working_directory: String,
    shortcuts_count: usize,
    quick_commands_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusPayload {
    version: String,
    server_base_url: String,
    config_file: String,
    preferences_file: String,
    registration_token: String,
    registration_token_source: String,
    run_heartbeat: bool,
    run_tunnel: bool,
    configured_default_shell: String,
    effective_default_shell: String,
    available_shells: Vec<String>,
    ssh_available: bool,
    ssh_detail: String,
    platform: String,
    arch: String,
    preferences_summary: PreferencesSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveTokenResult {
    ok: bool,
    config_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentOverview {
    desired_running: bool,
    running: bool,
    connected: bool,
    pid: Option<u32>,
    autostart_enabled: bool,
    has_token: bool,
    token_source: String,
    status_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentLogEntry {
    stream: String,
    line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapPayload {
    status: StatusPayload,
    agent: AgentOverview,
    recent_logs: Vec<AgentLogEntry>,
    desktop_mode: String,
    onboarding_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct DesktopPersistedState {
    onboarding_completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopAgentActionResult {
    action: String,
    ok: bool,
    message: String,
    state: AgentOverview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutostartResult {
    ok: bool,
    enabled: bool,
    message: String,
}

#[derive(Debug)]
struct AgentSupervisor {
    task: Option<JoinHandle<()>>,
    desired_running: bool,
    virtual_pid: Option<u32>,
    runtime: DesktopRuntimeSnapshot,
}

impl AgentSupervisor {
    fn new() -> Self {
        Self {
            task: None,
            desired_running: false,
            virtual_pid: Some(std::process::id()),
            runtime: DesktopRuntimeSnapshot::default(),
        }
    }

    fn pid(&self) -> Option<u32> {
        self.virtual_pid
    }
}

#[derive(Debug)]
struct DesktopState {
    agent: Mutex<AgentSupervisor>,
    logs: Mutex<VecDeque<AgentLogEntry>>,
}

impl DesktopState {
    fn new() -> Self {
        Self { agent: Mutex::new(AgentSupervisor::new()), logs: Mutex::new(VecDeque::new()) }
    }
}

#[tauri::command]
async fn desktop_bootstrap(state: State<'_, Arc<DesktopState>>) -> Result<BootstrapPayload, String> {
    build_bootstrap_payload(state.as_ref()).await.map_err(|err| err.to_string())
}

#[tauri::command]
async fn desktop_logs(state: State<'_, Arc<DesktopState>>) -> Result<Vec<AgentLogEntry>, String> {
    Ok(snapshot_logs(state.as_ref()))
}

#[tauri::command]
async fn save_token(
    token: String,
    state: State<'_, Arc<DesktopState>>,
    app: AppHandle,
) -> Result<SaveTokenResult, String> {
    let token = token.trim().to_owned();
    if token.is_empty() {
        return Err("Token cannot be empty.".into());
    }

    persist_registration_token(&default_config_file_path(), &token).map_err(|err| err.to_string())?;
    let result = SaveTokenResult {
        ok: true,
        config_file: Some(default_config_file_path().display().to_string()),
    };

    mark_onboarding_completed().map_err(|err| err.to_string())?;
    ensure_agent_started(&app, &state, true).await.map_err(|err| err.to_string())?;
    emit_agent_state(&app, &state).await;

    Ok(result)
}

#[tauri::command]
async fn desktop_agent_action(
    action: String,
    state: State<'_, Arc<DesktopState>>,
    app: AppHandle,
) -> Result<DesktopAgentActionResult, String> {
    let normalized = action.trim().to_ascii_lowercase();
    let result = match normalized.as_str() {
        "start" => {
            ensure_agent_started(&app, &state, true).await.map_err(|err| err.to_string())?;
            desktop_agent_result(
                "start",
                "Agent process started. Registration is still in progress; check the log panel for `registered device` or backend errors.",
                &state,
            )
            .await
        }
        "stop" => {
            stop_agent(&state).await.map_err(|err| err.to_string())?;
            desktop_agent_result("stop", "Desktop background agent has been stopped.", &state).await
        }
        "restart" => {
            restart_agent(&app, &state, true).await.map_err(|err| err.to_string())?;
            desktop_agent_result(
                "restart",
                "Agent process restarted. Registration is still in progress; check the log panel for `registered device` or backend errors.",
                &state,
            )
            .await
        }
        "status" => {
            reconcile_agent_state(&state).await;
            desktop_agent_result("status", "Desktop background agent state refreshed.", &state)
                .await
        }
        other => return Err(format!("Unsupported desktop agent action: {other}")),
    };

    emit_agent_state(&app, &state).await;
    Ok(result)
}

#[tauri::command]
async fn set_autostart(
    enabled: bool,
    state: State<'_, Arc<DesktopState>>,
    app: AppHandle,
) -> Result<AutostartResult, String> {
    if enabled {
        enable_autostart().map_err(|err| err.to_string())?;
    } else {
        disable_autostart().map_err(|err| err.to_string())?;
    }

    emit_agent_state(&app, &state).await;

    Ok(AutostartResult {
        ok: true,
        enabled,
        message: if enabled {
            "Desktop manager will now start automatically when this user signs in.".into()
        } else {
            "Desktop manager autostart has been disabled for this user.".into()
        },
    })
}

async fn build_bootstrap_payload(state: &DesktopState) -> Result<BootstrapPayload> {
    reconcile_agent_state(state).await;
    let status = build_status_payload()?;
    let agent = build_agent_overview(state).await;
    Ok(BootstrapPayload {
        status,
        agent,
        recent_logs: snapshot_logs(state),
        desktop_mode: "tray-background-app".into(),
        onboarding_required: onboarding_required(),
    })
}

async fn desktop_agent_result(
    action: &str,
    message: &str,
    state: &DesktopState,
) -> DesktopAgentActionResult {
    DesktopAgentActionResult {
        action: action.into(),
        ok: true,
        message: message.into(),
        state: build_agent_overview(state).await,
    }
}

async fn ensure_agent_started(
    app: &AppHandle,
    state: &DesktopState,
    force_restart: bool,
) -> Result<()> {
    reconcile_agent_state(state).await;

    if !has_saved_registration_token() {
        bail!(
            "Registration token is missing. Save a token before starting the desktop background agent."
        );
    }

    let service_state = service::service_status();
    if looks_like_service_running(&service_state.message) {
        bail!(
            "Background service appears to be active. Stop the service before using desktop background mode to avoid duplicate agent connections."
        );
    }

    if force_restart {
        stop_agent(state).await?;
    }

    let mut agent = state.agent.lock().map_err(|_| anyhow!("agent state is unavailable"))?;
    if agent.task.is_some() {
        agent.desired_running = true;
        return Ok(());
    }
    agent.runtime.phase = DesktopRuntimePhase::Starting;
    agent.runtime.last_error = None;
    agent.runtime.retry_attempt = 0;

    let runtime_state = app.state::<Arc<DesktopState>>().inner().clone();
    let app_handle = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        run_agent_supervisor(app_handle, runtime_state).await;
    });
    agent.task = Some(task);
    agent.desired_running = true;
    drop(agent);

    let _ = app.emit(
        "desktop://agent-message",
        "Agent process started. Registration is still in progress; check the log panel for `registered device` or backend errors.",
    );
    Ok(())
}

async fn stop_agent(state: &DesktopState) -> Result<()> {
    let task_to_abort = {
        let mut agent = state.agent.lock().map_err(|_| anyhow!("agent state is unavailable"))?;
        agent.desired_running = false;
        agent.runtime.phase = DesktopRuntimePhase::Stopped;
        agent.runtime.connected = false;
        agent.runtime.last_error = None;
        agent.runtime.websocket_url = None;
        agent.task.take()
    };

    if let Some(task) = task_to_abort {
        task.abort();
    }

    Ok(())
}

async fn restart_agent(app: &AppHandle, state: &DesktopState, force_restart: bool) -> Result<()> {
    stop_agent(state).await?;
    ensure_agent_started(app, state, force_restart).await
}

async fn reconcile_agent_state(state: &DesktopState) {
    if let Ok(mut agent) = state.agent.lock() {
        if agent.task.is_none() && agent.desired_running && matches!(agent.runtime.phase, DesktopRuntimePhase::Idle) {
            agent.runtime.phase = DesktopRuntimePhase::Stopped;
        }
    }
}

async fn build_agent_overview(state: &DesktopState) -> AgentOverview {
    let (desired_running, running, connected, pid, runtime) = match state.agent.lock() {
        Ok(mut agent) => {
            let running = agent.task.is_some();
            if !running {
                agent.task = None;
            }
            (
                agent.desired_running,
                running,
                agent.runtime.connected,
                agent.pid(),
                agent.runtime.clone(),
            )
        }
        Err(_) => (false, false, false, None, DesktopRuntimeSnapshot::default()),
    };

    let (has_token, token_source) = token_state();
    AgentOverview {
        desired_running,
        running,
        connected,
        pid,
        autostart_enabled: is_autostart_enabled(),
        has_token,
        token_source,
        status_summary: runtime_status_summary(&runtime, running, has_token),
    }
}

async fn emit_agent_state(app: &AppHandle, state: &DesktopState) {
    let payload = build_agent_overview(state).await;
    let _ = app.emit("desktop://agent-state", payload);
}

fn open_path_in_file_manager(path: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = StdCommand::new("explorer");
        command.arg(path);
        apply_no_window(&mut command);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = StdCommand::new("open");
        command.arg(path);
        command
    };

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let mut command = {
        let mut command = StdCommand::new("xdg-open");
        command.arg(path);
        command
    };

    command.spawn().map(|_| ()).map_err(|err| anyhow!("failed to open `{path}`: {err}"))
}

fn build_status_payload() -> Result<StatusPayload> {
    let config = runtime_config();
    let logs_dir = managed_logs_dir();
    let snapshot = collect_host_snapshot(
        VERSION,
        &config.enabled_shell_types,
        logs_dir.display().to_string(),
    )?;
    let effective =
        resolve_default_shell(config.default_shell_type, &snapshot.diagnostics.available_shells);
    Ok(StatusPayload {
        version: VERSION.to_owned(),
        server_base_url: config.server_base_url.clone(),
        config_file: config.config_file_path.display().to_string(),
        preferences_file: config.preferences_file_path.display().to_string(),
        registration_token: if config.registration_token.is_some() {
            "configured".into()
        } else {
            "missing".into()
        },
        registration_token_source: if config.registration_token.is_some() {
            if has_registration_token_env_override() {
                "environment variable RTC_REGISTRATION_TOKEN".into()
            } else {
                "config file".into()
            }
        } else {
            "none".into()
        },
        run_heartbeat: config.run_heartbeat,
        run_tunnel: config.run_tunnel,
        configured_default_shell: config.default_shell_type.as_str().to_owned(),
        effective_default_shell: effective.as_str().to_owned(),
        available_shells: snapshot
            .diagnostics
            .available_shells
            .iter()
            .map(|item| item.as_str().to_owned())
            .collect(),
        ssh_available: snapshot.diagnostics.ssh_check.available,
        ssh_detail: snapshot.diagnostics.ssh_check.detail.clone(),
        platform: match snapshot.platform {
            Some(platform) => serde_json::to_value(platform)?.as_str().unwrap_or("unknown").to_owned(),
            None => "unknown".into(),
        },
        arch: snapshot.arch,
        preferences_summary: read_preferences_summary(&config.preferences_file_path),
    })
}

fn runtime_config() -> RuntimeConfig {
    read_runtime_config(default_server_base_url())
}

fn read_preferences_summary(path: &Path) -> PreferencesSummary {
    let store = PreferencesStore::new(path);
    let preferences = store.get_preferences();
    PreferencesSummary {
        default_working_directory: preferences.default_working_directory,
        shortcuts_count: preferences.shortcuts.len(),
        quick_commands_count: preferences.quick_commands.len(),
    }
}

async fn run_agent_supervisor(app: AppHandle, state: Arc<DesktopState>) {
    let mut retry_attempt = 0_u32;
    loop {
        if !desired_running(&state) {
            update_runtime_snapshot(&state, |runtime| {
                runtime.phase = DesktopRuntimePhase::Stopped;
                runtime.connected = false;
                runtime.websocket_url = None;
            });
            emit_agent_state(&app, &state).await;
            break;
        }

        let run_result = run_agent_once_in_process(&app, Arc::clone(&state)).await;
        if let Err(err) = run_result {
            retry_attempt = retry_attempt.saturating_add(1);
            let delay = compute_retry_delay(retry_attempt);
            update_runtime_snapshot(&state, |runtime| {
                runtime.phase = DesktopRuntimePhase::Reconnecting;
                runtime.connected = false;
                runtime.last_error = Some(user_facing_error(&err));
                runtime.retry_attempt = retry_attempt;
                runtime.websocket_url = None;
            });
            push_log_entry_and_emit(
                &app,
                &state,
                "stderr",
                format!(
                    "[remote-terminal-cloud-agent] runtime error: {}",
                    user_facing_error(&err)
                ),
            );
            push_log_entry_and_emit(
                &app,
                &state,
                "stdout",
                format!(
                    "[remote-terminal-cloud-agent] reconnect scheduled in {}s (attempt {}).",
                    delay.as_secs(),
                    retry_attempt
                ),
            );
            emit_agent_state(&app, &state).await;
            tokio::time::sleep(delay).await;
            continue;
        }
        retry_attempt = 0;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    if let Ok(mut agent) = state.agent.lock() {
        agent.task = None;
        if !agent.desired_running {
            agent.runtime.phase = DesktopRuntimePhase::Stopped;
        }
        agent.runtime.connected = false;
        agent.runtime.websocket_url = None;
    }
}

async fn run_agent_once_in_process(app: &AppHandle, state: Arc<DesktopState>) -> Result<()> {
    update_runtime_snapshot(&state, |runtime| {
        runtime.phase = DesktopRuntimePhase::Registering;
        runtime.connected = false;
        runtime.last_error = None;
        runtime.websocket_url = None;
    });
    emit_agent_state(app, &state).await;

    let mut config = runtime_config();
    let logs_dir = managed_logs_dir();
    let snapshot = collect_host_snapshot(
        VERSION,
        &config.enabled_shell_types,
        logs_dir.display().to_string(),
    )?;
    let effective_default_shell =
        resolve_default_shell(config.default_shell_type, &snapshot.diagnostics.available_shells);

    push_log_entry_and_emit(
        app,
        state.as_ref(),
        "stdout",
        format!(
            "[remote-terminal-cloud-agent] config file: {}",
            config.config_file_path.display()
        ),
    );
    push_log_entry_and_emit(
        app,
        state.as_ref(),
        "stdout",
        "[remote-terminal-cloud-agent] host snapshot".into(),
    );
    push_log_entry_and_emit(
        app,
        state.as_ref(),
        "stdout",
        serde_json::to_string_pretty(&snapshot).unwrap_or_else(|_| "{}".into()),
    );
    push_log_entry_and_emit(
        app,
        state.as_ref(),
        "stdout",
        format!(
            "[remote-terminal-cloud-agent] shell capabilities: {}",
            join_shells(&snapshot.diagnostics.available_shells)
        ),
    );

    if config.registration_token.is_none() {
        update_runtime_snapshot(&state, |runtime| {
            runtime.phase = DesktopRuntimePhase::WaitingConfig;
            runtime.connected = false;
            runtime.last_error = None;
        });
        push_log_entry_and_emit(
            app,
            state.as_ref(),
            "stdout",
            format!(
                "[remote-terminal-cloud-agent] waiting for configuration: update {} and the desktop agent will retry automatically.",
                config.config_file_path.display()
            ),
        );
        tokio::time::sleep(MISSING_CONFIG_RETRY).await;
        return Ok(());
    }

    let Some(registration_token) = config.registration_token.take() else {
        return Ok(());
    };

    let api_client = ApiClient::default();
    let session = match api_client
        .register_agent(&config.server_base_url, &registration_token, snapshot)
        .await
    {
        Ok(session) => session,
        Err(err) => {
            update_runtime_snapshot(&state, |runtime| {
                runtime.phase = DesktopRuntimePhase::Error;
                runtime.connected = false;
                runtime.last_error = Some(user_facing_error(&err));
            });
            emit_agent_state(app, &state).await;
            push_log_entry_and_emit(
                app,
                state.as_ref(),
                "stderr",
                format!("[remote-terminal-cloud-agent] registration failed: {}", user_facing_error(&err)),
            );
            return Err(err);
        }
    };
    push_log_entry_and_emit(
        app,
        state.as_ref(),
        "stdout",
        format!("[remote-terminal-cloud-agent] registered device {}", session.device_id),
    );
    update_runtime_snapshot(&state, |runtime| {
        runtime.phase = DesktopRuntimePhase::Online;
        runtime.connected = false;
        runtime.registered_device_id = Some(session.device_id.clone());
        runtime.retry_attempt = 0;
        runtime.last_error = None;
    });
    emit_agent_state(app, &state).await;

    if !config.run_heartbeat && !config.run_tunnel {
        push_log_entry_and_emit(
            app,
            state.as_ref(),
            "stdout",
            "[remote-terminal-cloud-agent] heartbeat and tunnel are both disabled; retrying later."
                .into(),
        );
        tokio::time::sleep(MISSING_CONFIG_RETRY).await;
        return Ok(());
    }

    let mut tasks = tokio::task::JoinSet::<(TaskSignal, Result<()>)>::new();

    if config.run_heartbeat {
        let api_client = api_client.clone();
        let server_base_url = config.server_base_url.clone();
        let enabled_shell_types = config.enabled_shell_types.clone();
        let logs_dir = logs_dir.clone();
        let app_handle = app.clone();
        let state_ref = Arc::clone(&state);
        let mut heartbeat_session = session.clone();
        tasks.spawn(async move {
            let result: Result<()> = async {
                loop {
                tokio::time::sleep(Duration::from_secs(
                    heartbeat_session.heartbeat_interval_seconds.max(1) as u64,
                ))
                .await;

                let heartbeat_snapshot = collect_host_snapshot(
                    VERSION,
                    &enabled_shell_types,
                    logs_dir.display().to_string(),
                )?;
                heartbeat_session = api_client
                    .send_heartbeat(&server_base_url, &heartbeat_session, heartbeat_snapshot)
                    .await?;
                update_runtime_snapshot(&state_ref, |runtime| {
                    runtime.last_heartbeat_at = Some(format_rfc3339_now());
                    runtime.retry_attempt = 0;
                    runtime.connected = true;
                    if runtime.websocket_url.is_some() {
                        runtime.phase = DesktopRuntimePhase::Online;
                    }
                });
                push_log_entry_and_emit(
                    &app_handle,
                    state_ref.as_ref(),
                    "stdout",
                    format!(
                        "[remote-terminal-cloud-agent] heartbeat ok for {}; next interval {}s",
                        heartbeat_session.device_id, heartbeat_session.heartbeat_interval_seconds
                    ),
                );
                emit_agent_state(&app_handle, &state_ref).await;
            }
            #[allow(unreachable_code)]
                Ok(())
            }
            .await;
            (TaskSignal::Heartbeat, result)
        });
    };

    if config.run_tunnel {
        let server_base_url = config.server_base_url.clone();
        let preferences_file_path = config.preferences_file_path.clone();
        let app_handle = app.clone();
        let state_ref = Arc::clone(&state);
        tasks.spawn(async move {
            let result = run_agent_tunnel_with_connect_hook(
                &server_base_url,
                session,
                effective_default_shell,
                &preferences_file_path,
                |websocket_url| {
                    update_runtime_snapshot(&state_ref, |runtime| {
                        runtime.connected = true;
                        runtime.phase = DesktopRuntimePhase::Online;
                        runtime.websocket_url = Some(websocket_url.to_owned());
                        runtime.last_error = None;
                    });
                    push_log_entry_and_emit(
                        &app_handle,
                        state_ref.as_ref(),
                        "stdout",
                        format!(
                            "[remote-terminal-cloud-agent] websocket connected: {}",
                            websocket_url
                        ),
                    );
                },
            )
            .await;
            (TaskSignal::Tunnel, result)
        });
    }

    if config.run_heartbeat {
        let app_handle = app.clone();
        let state_ref = Arc::clone(&state);
        tasks.spawn(async move {
            let result: Result<()> = async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    let snapshot = current_runtime_snapshot(&state_ref);
                    if !snapshot.connected {
                        continue;
                    }
                    let Some(last_heartbeat_at) = snapshot.last_heartbeat_at.as_deref() else {
                        continue;
                    };
                    let Some(age) = seconds_since_epoch_string(last_heartbeat_at) else {
                        continue;
                    };
                    if age > HEARTBEAT_STALE_TIMEOUT.as_secs() {
                        push_log_entry_and_emit(
                            &app_handle,
                            state_ref.as_ref(),
                            "stderr",
                            format!(
                                "[remote-terminal-cloud-agent] heartbeat watchdog detected stale connection: last heartbeat was {}s ago.",
                                age
                            ),
                        );
                        bail!(
                            "heartbeat watchdog detected stale connection after {} seconds",
                            age
                        );
                    }
                }
                #[allow(unreachable_code)]
                Ok(())
            }
            .await;
            (TaskSignal::Watchdog, result)
        });
    }

    if config.run_tunnel {
        match tasks.join_next().await {
            Some(Ok((signal, Ok(())))) => {
                tasks.abort_all();
                push_log_entry_and_emit(
                    app,
                    state.as_ref(),
                    "stderr",
                    format!(
                        "[remote-terminal-cloud-agent] {} exited unexpectedly; restarting agent connection.",
                        task_signal_label(signal)
                    ),
                );
                update_runtime_snapshot(&state, |runtime| {
                    runtime.connected = false;
                    runtime.websocket_url = None;
                    runtime.phase = DesktopRuntimePhase::Reconnecting;
                    runtime.last_error = Some(
                        format!(
                            "{} disconnected. Reconnecting to the backend.",
                            task_signal_label(signal)
                        ),
                    );
                });
                emit_agent_state(app, &state).await;
                bail!("{} disconnected", task_signal_label(signal))
            }
            Some(Ok((_, Err(err)))) => {
                tasks.abort_all();
                push_log_entry_and_emit(
                    app,
                    state.as_ref(),
                    "stderr",
                    format!(
                        "[remote-terminal-cloud-agent] connection anomaly detected; restarting agent connection: {}",
                        user_facing_error(&err)
                    ),
                );
                update_runtime_snapshot(&state, |runtime| {
                    runtime.connected = false;
                    runtime.websocket_url = None;
                    runtime.phase = DesktopRuntimePhase::Reconnecting;
                    runtime.last_error = Some(user_facing_error(&err));
                });
                emit_agent_state(app, &state).await;
                Err(err)
            }
            Some(Err(err)) => {
                tasks.abort_all();
                update_runtime_snapshot(&state, |runtime| {
                    runtime.connected = false;
                    runtime.websocket_url = None;
                    runtime.phase = DesktopRuntimePhase::Error;
                    runtime.last_error = Some(err.to_string());
                });
                emit_agent_state(app, &state).await;
                bail!("agent background task failed: {err}")
            }
            None => {
                bail!("agent runtime exited without active tasks")
            }
        }
    } else if config.run_heartbeat {
        match tasks.join_next().await {
            Some(Ok((_, result))) => result,
            Some(Err(err)) => Err(anyhow!("heartbeat task failed: {err}")),
            None => bail!("agent runtime exited without active tasks"),
        }
    } else {
        bail!("agent runtime exited without active tasks")
    }
}

fn push_log_entry_and_emit(app: &AppHandle, state: &DesktopState, stream: &str, line: String) {
    let entry = AgentLogEntry { stream: stream.into(), line };
    push_log_entry(state, entry.clone());
    let _ = app.emit("desktop://agent-log", entry);
}

fn join_shells(items: &[ShellType]) -> String {
    if items.is_empty() {
        "none".into()
    } else {
        items.iter().map(|item| item.as_str()).collect::<Vec<_>>().join(", ")
    }
}

fn user_facing_error(err: &anyhow::Error) -> String {
    if let Some(details) = describe_error(err) {
        format!("{} | suggestion: {}", details.message, details.suggestion)
    } else {
        err.to_string()
    }
}

fn desired_running(state: &DesktopState) -> bool {
    state.agent.lock().map(|agent| agent.desired_running).unwrap_or(false)
}

fn update_runtime_snapshot<F>(state: &DesktopState, update: F)
where
    F: FnOnce(&mut DesktopRuntimeSnapshot),
{
    if let Ok(mut agent) = state.agent.lock() {
        update(&mut agent.runtime);
    }
}

fn current_runtime_snapshot(state: &DesktopState) -> DesktopRuntimeSnapshot {
    state
        .agent
        .lock()
        .map(|agent| agent.runtime.clone())
        .unwrap_or_default()
}

fn task_signal_label(signal: TaskSignal) -> &'static str {
    match signal {
        TaskSignal::Heartbeat => "Heartbeat loop",
        TaskSignal::Tunnel => "WebSocket tunnel",
        TaskSignal::Watchdog => "Heartbeat watchdog",
    }
}

fn compute_retry_delay(retry_attempt: u32) -> Duration {
    let capped_attempt = retry_attempt.min(6);
    let scale = 1_u64 << capped_attempt;
    let secs = RUNTIME_RETRY_INTERVAL.as_secs().saturating_mul(scale);
    Duration::from_secs(secs.min(MAX_BACKOFF_INTERVAL.as_secs()))
}

fn runtime_status_summary(
    runtime: &DesktopRuntimeSnapshot,
    running: bool,
    has_token: bool,
) -> String {
    match runtime.phase {
        DesktopRuntimePhase::Online if runtime.connected => {
            if let Some(device_id) = runtime.registered_device_id.as_deref() {
                format!("Agent 已在线，设备 ID: {device_id}。心跳与隧道连接正常。")
            } else {
                "Agent 已在线，心跳与隧道连接正常。".into()
            }
        }
        DesktopRuntimePhase::Registering | DesktopRuntimePhase::Starting => {
            "Agent 正在注册并建立隧道连接，请查看下方日志。".into()
        }
        DesktopRuntimePhase::Reconnecting => format!(
            "Agent 连接已中断，正在自动重连（第 {} 次）。{}",
            runtime.retry_attempt.max(1),
            runtime.last_error.clone().unwrap_or_else(|| "请查看日志面板。".into())
        ),
        DesktopRuntimePhase::WaitingConfig => {
            "等待配置 Token。保存后桌面端会自动重试注册。".into()
        }
        DesktopRuntimePhase::Stopped => {
            if has_token {
                "后台 Agent 已停止，可随时重新启动。".into()
            } else {
                "Token 是启动后台 Agent 的前提条件。".into()
            }
        }
        DesktopRuntimePhase::Error => runtime
            .last_error
            .clone()
            .unwrap_or_else(|| "后台 Agent 遇到错误，请查看日志。".into()),
        DesktopRuntimePhase::Idle => {
            if running {
                "Agent 进程已启动，正在等待连接状态更新。".into()
            } else if has_token {
                "Ready to start in tray background mode.".into()
            } else {
                "Token is required before the background agent can start.".into()
            }
        }
        DesktopRuntimePhase::Online => {
            "Agent 已运行，正在等待最新连接状态。".into()
        }
    }
}

fn format_rfc3339_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs();
    format!("{now}")
}

fn seconds_since_epoch_string(value: &str) -> Option<u64> {
    let then = value.trim().parse::<u64>().ok()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs();
    Some(now.saturating_sub(then))
}

#[cfg(target_os = "windows")]
fn apply_no_window(command: &mut StdCommand) {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn apply_no_window(_command: &mut StdCommand) {}

fn managed_logs_dir() -> PathBuf {
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
        default_config_file_path().parent().unwrap_or(Path::new(".")).join("logs")
    }
}

fn snapshot_logs(state: &DesktopState) -> Vec<AgentLogEntry> {
    state
        .logs
        .lock()
        .map(|logs| logs.iter().cloned().collect())
        .unwrap_or_default()
}

fn push_log_entry(state: &DesktopState, entry: AgentLogEntry) {
    const MAX_LOG_ENTRIES: usize = 300;
    if let Ok(mut logs) = state.logs.lock() {
        logs.push_back(entry);
        while logs.len() > MAX_LOG_ENTRIES {
            let _ = logs.pop_front();
        }
    }
}

fn has_saved_registration_token() -> bool {
    let env_token = normalize_template_string(env::var("RTC_REGISTRATION_TOKEN").ok());
    if env_token.is_some() {
        return true;
    }
    let file_config = read_config_file(&default_config_file_path());
    file_config.registration_token.is_some()
}

fn onboarding_required() -> bool {
    let forced = env::var("RTC_AGENT_DESKTOP_FORCE_ONBOARDING")
        .ok()
        .map(|value| {
            matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false);
    if forced {
        return true;
    }
    if !has_saved_registration_token() {
        return true;
    }
    !load_desktop_persisted_state().onboarding_completed
}

fn desktop_state_file_path() -> PathBuf {
    default_config_file_path().parent().unwrap_or(Path::new(".")).join(DESKTOP_STATE_FILE_NAME)
}

fn load_desktop_persisted_state() -> DesktopPersistedState {
    let path = desktop_state_file_path();
    let Ok(content) = fs::read_to_string(path) else {
        return DesktopPersistedState::default();
    };
    serde_json::from_str::<DesktopPersistedState>(&content).unwrap_or_default()
}

fn save_desktop_persisted_state(state: &DesktopPersistedState) -> Result<()> {
    let path = desktop_state_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(&path, serde_json::to_string_pretty(state)?)
        .with_context(|| format!("write {}", path.display()))
}

fn mark_onboarding_completed() -> Result<()> {
    let mut state = load_desktop_persisted_state();
    state.onboarding_completed = true;
    save_desktop_persisted_state(&state)
}

fn token_state() -> (bool, String) {
    if normalize_template_string(env::var("RTC_REGISTRATION_TOKEN").ok()).is_some() {
        return (true, "environment variable RTC_REGISTRATION_TOKEN".into());
    }
    let config = read_config_file(&default_config_file_path());
    if config.registration_token.is_some() {
        return (true, "config file".into());
    }
    (false, "none".into())
}

fn looks_like_service_running(message: &str) -> bool {
    let text = message.to_ascii_lowercase();
    text.contains("running") && !text.contains("pending")
}

fn is_autostart_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        return windows_run_key()
            .ok()
            .and_then(|key| key.get_value::<String, _>(APP_RUN_REG_VALUE).ok())
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
    }
    #[cfg(target_os = "macos")]
    {
        return macos_autostart_plist_path().exists();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        false
    }
}

fn enable_autostart() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let exe = env::current_exe().context("resolve current desktop executable")?;
        let key = windows_run_key_write()?;
        key.set_value(APP_RUN_REG_VALUE, &format!("\"{}\"", exe.display()))
            .context("update autostart registry value")?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let exe = env::current_exe().context("resolve current desktop executable")?;
        let plist_path = macos_autostart_plist_path();
        if let Some(parent) = plist_path.parent() {
            fs::create_dir_all(parent).context("create LaunchAgents directory")?;
        }
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.remote-terminal-cloud.agent.desktop</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#,
            exe = exe.display()
        );
        fs::write(&plist_path, plist).context("write LaunchAgent plist")?;
        return Ok(());
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        bail!("autostart management is currently implemented for Windows and macOS only")
    }
}

fn disable_autostart() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let key = windows_run_key_write()?;
        match key.delete_value(APP_RUN_REG_VALUE) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(anyhow!(err).context("remove autostart registry value"));
            }
        }
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let plist_path = macos_autostart_plist_path();
        if plist_path.exists() {
            fs::remove_file(&plist_path).context("remove LaunchAgent plist")?;
        }
        return Ok(());
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        bail!("autostart management is currently implemented for Windows and macOS only")
    }
}

#[cfg(target_os = "macos")]
fn macos_autostart_plist_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".into());
    Path::new(&home).join("Library/LaunchAgents/com.remote-terminal-cloud.agent.desktop.plist")
}

#[cfg(target_os = "windows")]
fn windows_run_key() -> Result<RegKey> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            KEY_QUERY_VALUE,
        )
        .context("open Windows Run registry key")
}

#[cfg(target_os = "windows")]
fn windows_run_key_write() -> Result<RegKey> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            KEY_SET_VALUE | KEY_QUERY_VALUE,
        )
        .context("open Windows Run registry key for write")
}

fn show_main_window(app: &AppHandle) -> Result<()> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| anyhow!("main window is unavailable"))?;
    window.show()?;
    window.unminimize()?;
    window.set_focus()?;
    Ok(())
}

fn hide_main_window(app: &AppHandle) -> Result<()> {
    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| anyhow!("main window is unavailable"))?;
    window.hide()?;
    Ok(())
}

fn build_tray(app: &AppHandle) -> Result<()> {
    let open = MenuItem::with_id(app, MENU_OPEN, "Open Manager", true, None::<&str>)?;
    let start_agent =
        MenuItem::with_id(app, MENU_START_AGENT, "Start Background Agent", true, None::<&str>)?;
    let stop_agent =
        MenuItem::with_id(app, MENU_STOP_AGENT, "Stop Background Agent", true, None::<&str>)?;
    let restart_agent =
        MenuItem::with_id(app, MENU_RESTART_AGENT, "Restart Background Agent", true, None::<&str>)?;
    let autostart =
        MenuItem::with_id(app, MENU_SAVE_AUTOSTART, "Enable Autostart", true, None::<&str>)?;
    let open_config =
        MenuItem::with_id(app, MENU_OPEN_CONFIG, "Open Config Folder", true, None::<&str>)?;
    let open_logs = MenuItem::with_id(app, MENU_OPEN_LOGS, "Open Logs", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &PredefinedMenuItem::separator(app)?,
            &start_agent,
            &stop_agent,
            &restart_agent,
            &autostart,
            &PredefinedMenuItem::separator(app)?,
            &open_config,
            &open_logs,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let icon =
        app.default_window_icon().cloned().ok_or_else(|| anyhow!("default icon is unavailable"))?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .menu(&menu)
        .tooltip("Remote Terminal Cloud Agent")
        .show_menu_on_left_click(false)
        .build(app)?;
    Ok(())
}

fn install_runtime_hooks(app: &AppHandle) {
    let app_handle = app.clone();
    app.listen("desktop://show-window", move |_| {
        let _ = show_main_window(&app_handle);
    });
}

async fn handle_menu_event(app: AppHandle, state: Arc<DesktopState>, id: &str) {
    match id {
        MENU_OPEN => {
            let _ = show_main_window(&app);
        }
        MENU_START_AGENT => {
            let _ = ensure_agent_started(&app, &state, false).await;
            emit_agent_state(&app, &state).await;
        }
        MENU_STOP_AGENT => {
            let _ = stop_agent(&state).await;
            emit_agent_state(&app, &state).await;
        }
        MENU_RESTART_AGENT => {
            let _ = restart_agent(&app, &state, true).await;
            emit_agent_state(&app, &state).await;
        }
        MENU_SAVE_AUTOSTART => {
            let enabled = !is_autostart_enabled();
            let _ = if enabled { enable_autostart() } else { disable_autostart() };
            emit_agent_state(&app, &state).await;
        }
        MENU_OPEN_CONFIG => {
            let path = default_config_file_path()
                .parent()
                .unwrap_or(Path::new("."))
                .display()
                .to_string();
            let _ = open_path_in_file_manager(&path);
        }
        MENU_OPEN_LOGS => {
            let path = managed_logs_dir().display().to_string();
            let _ = open_path_in_file_manager(&path);
        }
        MENU_QUIT => {
            let _ = stop_agent(&state).await;
            app.exit(0);
        }
        _ => {}
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let desktop_state = Arc::new(DesktopState::new());
    let setup_state = Arc::clone(&desktop_state);
    let menu_state = Arc::clone(&desktop_state);
    let tray_state = Arc::clone(&desktop_state);
    let close_state = Arc::clone(&desktop_state);
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .manage(desktop_state)
        .setup(move |app| {
            build_tray(app.handle())?;
            install_runtime_hooks(app.handle());
            let should_hide_on_launch = !onboarding_required();
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                if should_hide_on_launch {
                    window.hide()?;
                } else {
                    window.show()?;
                    window.set_focus()?;
                }
            }
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = ensure_agent_started(&app_handle, &setup_state, false).await;
                emit_agent_state(&app_handle, &setup_state).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_bootstrap,
            desktop_logs,
            save_token,
            desktop_agent_action,
            set_autostart
        ])
        .on_menu_event(move |app, event| {
            let app_handle = app.clone();
            let state = Arc::clone(&menu_state);
            let id = event.id().0.clone();
            tauri::async_runtime::spawn(async move {
                handle_menu_event(app_handle, state, &id).await;
            });
        })
        .on_tray_icon_event(move |tray, event| {
            let app_handle = tray.app_handle().clone();
            let state = Arc::clone(&tray_state);
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                tauri::async_runtime::spawn(async move {
                    let _ = show_main_window(&app_handle);
                    emit_agent_state(&app_handle, &state).await;
                });
            }
        })
        .on_window_event(move |window, event| {
            if window.label() == MAIN_WINDOW_LABEL
                && matches!(event, WindowEvent::CloseRequested { .. })
            {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                }
                let app_handle = window.app_handle().clone();
                let state = Arc::clone(&close_state);
                tauri::async_runtime::spawn(async move {
                    let _ = hide_main_window(&app_handle);
                    emit_agent_state(&app_handle, &state).await;
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use std::collections::HashSet;
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow, bail};
use rtc_agent_config::{
    default_config_file_path, normalize_template_string, persist_registration_token,
    read_config_file,
};
use rtc_agent_service as service;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Listener, Manager, State, WindowEvent};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
#[cfg(target_os = "windows")]
use winreg::RegKey;
#[cfg(target_os = "windows")]
use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};

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
    child: Option<Child>,
    desired_running: bool,
}

impl AgentSupervisor {
    fn new() -> Self {
        Self { child: None, desired_running: false }
    }

    fn pid(&mut self) -> Option<u32> {
        self.child.as_mut().and_then(|child| child.id())
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
    let status = run_agent_json::<StatusPayload>(&["status", "--json"]).await?;
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
            "Windows Service appears to be active. Stop the service before using desktop background mode to avoid duplicate agent connections."
        );
    }

    if force_restart {
        stop_agent(state).await?;
    }

    let mut agent = state.agent.lock().map_err(|_| anyhow!("agent state is unavailable"))?;
    if agent.child.is_some() {
        agent.desired_running = true;
        return Ok(());
    }

    let binary = resolve_binary("rtc-agentd");
    let mut command = Command::new(&binary);
    command.arg("run");
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    apply_no_window(&mut command);
    let mut child =
        command.spawn().with_context(|| format!("failed to launch `{}`", binary.display()))?;
    let shared_state = app.state::<Arc<DesktopState>>().inner().clone();
    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(app.clone(), Arc::clone(&shared_state), "stdout", stdout);
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(app.clone(), shared_state, "stderr", stderr);
    }
    agent.child = Some(child);
    agent.desired_running = true;
    drop(agent);

    let _ = app.emit(
        "desktop://agent-message",
        "Agent process started. Registration is still in progress; check the log panel for `registered device` or backend errors.",
    );
    Ok(())
}

async fn stop_agent(state: &DesktopState) -> Result<()> {
    let mut child_to_kill = {
        let mut agent = state.agent.lock().map_err(|_| anyhow!("agent state is unavailable"))?;
        agent.desired_running = false;
        agent.child.take()
    };

    if let Some(mut child) = child_to_kill.take() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }

    Ok(())
}

async fn restart_agent(app: &AppHandle, state: &DesktopState, force_restart: bool) -> Result<()> {
    stop_agent(state).await?;
    ensure_agent_started(app, state, force_restart).await
}

async fn reconcile_agent_state(state: &DesktopState) {
    let mut stale_child = None;
    if let Ok(mut agent) = state.agent.lock() {
        if let Some(child) = agent.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_status)) => stale_child = agent.child.take(),
                Ok(None) => {}
                Err(_) => stale_child = agent.child.take(),
            }
        }
    }

    if let Some(mut child) = stale_child {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

async fn build_agent_overview(state: &DesktopState) -> AgentOverview {
    let (desired_running, running, pid) = match state.agent.lock() {
        Ok(mut agent) => {
            let running = match agent.child.as_mut() {
                Some(child) => child.try_wait().ok().flatten().is_none(),
                None => false,
            };
            if !running {
                agent.child = None;
            }
            (agent.desired_running, running, agent.pid())
        }
        Err(_) => (false, false, None),
    };

    let (has_token, token_source) = token_state();
    AgentOverview {
        desired_running,
        running,
        pid,
        autostart_enabled: is_autostart_enabled(),
        has_token,
        token_source,
        status_summary: if running {
            "Agent process is running. Registration and heartbeat results are shown in the log panel.".into()
        } else if has_token {
            "Ready to start in tray background mode.".into()
        } else {
            "Token is required before the background agent can start.".into()
        },
    }
}

async fn emit_agent_state(app: &AppHandle, state: &DesktopState) {
    let payload = build_agent_overview(state).await;
    let _ = app.emit("desktop://agent-state", payload);
}

async fn run_agent_json<T>(args: &[&str]) -> Result<T>
where
    T: DeserializeOwned,
{
    run_json_command("rtc-agentd", args).await
}

async fn run_json_command<T>(binary_name: &str, args: &[&str]) -> Result<T>
where
    T: DeserializeOwned,
{
    let candidate_names = binary_candidate_names(binary_name);
    let candidates = resolve_binary_candidates(&candidate_names);
    let binary = resolve_binary(binary_name);
    let mut command = Command::new(&binary);
    command.args(args);
    apply_no_window(&mut command);
    let output = command.output().await.with_context(|| {
            let attempted = candidates
                .iter()
                .take(8)
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("failed to spawn `{}`. attempted: {}", binary.display(), attempted)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        bail!(
            "`{}` failed with status {}. stdout: {} stderr: {}",
            binary.display(),
            output.status,
            stdout,
            stderr
        );
    }

    serde_json::from_slice::<T>(&output.stdout).with_context(|| {
        format!(
            "failed to decode JSON from `{}`: {}",
            binary.display(),
            String::from_utf8_lossy(&output.stdout).trim()
        )
    })
}

fn resolve_binary(binary_name: &str) -> PathBuf {
    let env_name = format!("{}_BIN", binary_name.replace('-', "_").to_ascii_uppercase());
    if let Some(path) = env::var_os(&env_name).filter(|value| !value.is_empty()) {
        return PathBuf::from(path);
    }

    let candidate_names = binary_candidate_names(binary_name);
    let candidates = resolve_binary_candidates(&candidate_names);

    for candidate in candidates {
        if candidate.is_file() {
            return candidate;
        }
    }

    PathBuf::from(candidate_names.into_iter().next().unwrap_or_else(|| binary_file_name(binary_name)))
}

fn resolve_binary_candidates(candidate_names: &[String]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(current_exe) = env::current_exe() {
        for file_name in candidate_names {
            push_exe_relative_candidates(&mut candidates, current_exe.parent(), file_name);
        }
    }

    if let Ok(cwd) = env::current_dir() {
        for file_name in candidate_names {
            push_exe_relative_candidates(&mut candidates, Some(cwd.as_path()), file_name);
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(workspace_root) = manifest_dir.ancestors().nth(3) {
        let target_dir = workspace_root.join("target");
        let build_bin_dir = workspace_root.join("build").join("bin").join("win32-x64");
        for file_name in candidate_names {
            candidates.push(build_bin_dir.join(file_name));
            candidates.push(target_dir.join("debug").join(file_name));
            candidates.push(target_dir.join("release").join(file_name));
            candidates.push(target_dir.join(file_name));
            candidates.push(target_dir.join("debug").join("deps").join(file_name));
            candidates.push(target_dir.join("release").join("deps").join(file_name));
            candidates.push(target_dir.join("deps").join(file_name));
        }
    }

    let mut seen = HashSet::new();
    candidates.into_iter().filter(|candidate| seen.insert(candidate.clone())).collect()
}

fn push_exe_relative_candidates(
    candidates: &mut Vec<PathBuf>,
    start: Option<&Path>,
    file_name: &str,
) {
    let Some(start) = start else {
        return;
    };

    let mut current = Some(start);
    for _ in 0..4 {
        let Some(dir) = current else {
            break;
        };
        candidates.push(dir.join(file_name));
        candidates.push(dir.join("resources").join(file_name));
        candidates.push(dir.join("resources").join("bin").join(file_name));
        candidates.push(dir.join("bin").join(file_name));
        current = dir.parent();
    }
}

fn binary_file_name(binary_name: &str) -> String {
    if cfg!(target_os = "windows") { format!("{binary_name}.exe") } else { binary_name.to_owned() }
}

fn binary_candidate_names(binary_name: &str) -> Vec<String> {
    let mut names = vec![binary_file_name(binary_name)];
    if let Some(sidecar_name) = sidecar_binary_file_name(binary_name) {
        names.push(sidecar_name);
    }
    match binary_name {
        "rtc-agentd" => {
            names.push(binary_file_name("rtc-agent"));
            if let Some(sidecar_name) = sidecar_binary_file_name("rtc-agent") {
                names.push(sidecar_name);
            }
        }
        "rtc-agent" => {
            names.push(binary_file_name("rtc-agentd"));
            if let Some(sidecar_name) = sidecar_binary_file_name("rtc-agentd") {
                names.push(sidecar_name);
            }
        }
        _ => {}
    }
    let mut seen = HashSet::new();
    names.into_iter().filter(|name| seen.insert(name.clone())).collect()
}

fn sidecar_binary_file_name(binary_name: &str) -> Option<String> {
    let target_triple = tauri_sidecar_target_triple()?;
    let extension = if cfg!(target_os = "windows") { ".exe" } else { "" };
    Some(format!("{binary_name}-{target_triple}{extension}"))
}

fn tauri_sidecar_target_triple() -> Option<&'static str> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return Some("x86_64-pc-windows-msvc");
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        return Some("aarch64-pc-windows-msvc");
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Some("x86_64-apple-darwin");
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return Some("aarch64-apple-darwin");
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Some("x86_64-unknown-linux-gnu");
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return Some("aarch64-unknown-linux-gnu");
    }
    #[allow(unreachable_code)]
    None
}

fn open_path_in_file_manager(path: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer");
        command.arg(path);
        apply_no_window(&mut command);
        command
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(path);
        command
    };

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };

    command.spawn().map(|_| ()).map_err(|err| anyhow!("failed to open `{path}`: {err}"))
}

#[cfg(target_os = "windows")]
fn apply_no_window(command: &mut Command) {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn apply_no_window(_command: &mut Command) {}

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

fn spawn_log_reader<R>(app: AppHandle, state: Arc<DesktopState>, stream: &'static str, reader: R)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let entry = AgentLogEntry {
                stream: stream.into(),
                line,
            };
            push_log_entry(&state, entry.clone());
            let _ = app.emit("desktop://agent-log", entry);
        }
    });
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
    #[cfg(not(target_os = "windows"))]
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
    #[cfg(not(target_os = "windows"))]
    {
        bail!("autostart management is currently implemented for Windows only")
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
    #[cfg(not(target_os = "windows"))]
    {
        bail!("autostart management is currently implemented for Windows only")
    }
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

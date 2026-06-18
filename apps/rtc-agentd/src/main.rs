mod path_ops;
mod support;

use std::path::Path;
use std::time::Duration;

#[cfg(target_os = "windows")]
use std::ffi::OsString;
#[cfg(target_os = "windows")]
use std::sync::mpsc;

use anyhow::{Context, Result, bail};
use clap::{ArgAction, Args, Parser, Subcommand};
use rtc_agent_config::{
    AgentState, clear_agent_state, default_server_base_url,
    has_registration_token_env_override, load_agent_state, load_or_collect_device_fingerprint,
    save_agent_state,
};
use rtc_agent_platform::{
    collect_host_snapshot, detect_available_shells, resolve_default_shell,
};
use rtc_agent_protocol::ShellType;
use rtc_agent_runtime::{
    ApiClient, ApiErrorDetails, ApiErrorKind, RegisteredAgentSession, describe_error, run_agent_tunnel,
};
use rtc_agent_service as service;
use serde::Serialize;
use tokio::task::JoinSet;
use tracing_subscriber::EnvFilter;

use crate::path_ops::{run_install_path, run_start, run_stop, run_uninstall_path};
use crate::support::{
    emit_json, grow_backoff, is_authentication_error, is_interactive_input_available,
    join_shells, managed_logs_dir, manager_paths, next_backoff_delay, print_runtime_error,
    prompt_and_persist_registration_token, read_preferences_summary, runtime_config,
    user_label_for_error_kind,
};

#[cfg(target_os = "windows")]
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MISSING_CONFIG_RETRY: Duration = Duration::from_secs(30);
const RUNTIME_RETRY_INTERVAL: Duration = Duration::from_secs(10);
const INITIAL_BACKOFF_INTERVAL: Duration = Duration::from_secs(2);
const MAX_BACKOFF_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Parser)]
#[command(name = "rtc-agent", version = VERSION, about = "Remote Terminal Cloud Agent")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Configure(JsonFlag),
    Conf(JsonFlag),
    Version(JsonFlag),
    Ver(JsonFlag),
    Paths(JsonFlag),
    Path(JsonFlag),
    Config(JsonFlag),
    Status(JsonFlag),
    Doctor(JsonFlag),
    Diag(JsonFlag),
    Diagnose(JsonFlag),
    Verify(JsonFlag),
    Probe(JsonFlag),
    Shells(JsonFlag),
    Shell(JsonFlag),
    Run,
    Once,
    Start,
    Stop,
    #[cfg(target_os = "windows")]
    #[command(name = "service-host", hide = true)]
    ServiceHost,
    #[command(name = "install-path")]
    InstallPath,
    #[command(name = "uninstall-path")]
    UninstallPath,
    Service(ServiceArgs),
}

#[derive(Args, Clone, Default)]
struct JsonFlag {
    #[arg(long, action = ArgAction::SetTrue)]
    json: bool,
}

#[derive(Args)]
struct ServiceArgs {
    action: Option<String>,
    value: Option<String>,
    #[arg(long, action = ArgAction::SetTrue)]
    json: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionResponse<'a> {
    version: &'a str,
    server_base_url: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PathsResponse {
    config_file: String,
    preferences_file: String,
    config_dir: String,
    logs_dir: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreferencesSummary {
    default_working_directory: String,
    shortcuts_count: usize,
    quick_commands_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VerifyResponse {
    ok: bool,
    server_base_url: String,
    config_file: String,
    registration_token_source: String,
    device_id: String,
    heartbeat_interval_seconds: i32,
    websocket_url: String,
    effective_default_shell: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VerifyErrorResponse {
    ok: bool,
    server_base_url: String,
    config_file: String,
    registration_token_source: String,
    effective_default_shell: String,
    error: VerifyErrorDetails,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VerifyErrorDetails {
    kind: String,
    status: Option<u16>,
    code: Option<i64>,
    message: String,
    suggestion: String,
}

fn main() -> Result<()> {
    // musl static binaries buffer stdout in non-TTY mode; clap --help/--version
    // calls exit() before any flush. Only needed on musl targets.
    #[cfg(target_env = "musl")]
    unsafe {
        #[allow(non_camel_case_types)]
        type libc_FILE = std::ffi::c_void;
        unsafe extern "C" {
            fn setvbuf(stream: *mut libc_FILE, buf: *mut u8, mode: i32, size: usize) -> i32;
            static mut stdout: *mut libc_FILE;
        }
        setvbuf(stdout, std::ptr::null_mut(), 2 /* _IONBF */, 0);
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Run);
    let result = match command {
        Command::Configure(flag) | Command::Conf(flag) => run_configure(flag.json),
        Command::Version(flag) | Command::Ver(flag) => run_version(flag.json),
        Command::Paths(flag) | Command::Path(flag) => run_paths(flag.json),
        Command::Config(flag) => run_config(flag.json),
        Command::Status(flag) => run_status(flag.json),
        Command::Doctor(flag) | Command::Diag(flag) | Command::Diagnose(flag) => {
            run_doctor(flag.json)
        }
        Command::Verify(flag) | Command::Probe(flag) => {
            let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
            runtime.block_on(run_verify(flag.json))
        }
        Command::Shells(flag) | Command::Shell(flag) => run_shells(flag.json),
        Command::Run => {
            warn_if_service_running();
            let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
            runtime.block_on(run_agent_forever())
        }
        Command::Once => {
            let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
            runtime.block_on(run_agent_once())
        }
        Command::Start => run_start(),
        Command::Stop => run_stop(),
        #[cfg(target_os = "windows")]
        Command::ServiceHost => run_windows_service_host(),
        Command::InstallPath => run_install_path(),
        Command::UninstallPath => run_uninstall_path(),
        Command::Service(args) => run_service(args),
    };
    use std::io::Write;
    let _ = std::io::stdout().flush();
    result
}

#[cfg(target_os = "windows")]
define_windows_service!(ffi_service_main, windows_service_main);

#[cfg(target_os = "windows")]
fn run_windows_service_host() -> Result<()> {
    service_dispatcher::start(service::WINDOWS_SERVICE_NAME, ffi_service_main)
        .context("failed to start Windows service dispatcher")?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_service_main(_arguments: Vec<OsString>) {
    if let Err(err) = run_windows_service_worker() {
        eprintln!("[remote-terminal-cloud-agent] Windows service failed: {err:#}");
    }
}

#[cfg(target_os = "windows")]
fn run_windows_service_worker() -> Result<()> {
    let (shutdown_tx, shutdown_rx) = mpsc::channel();
    let status_handle = service_control_handler::register(service::WINDOWS_SERVICE_NAME, move |control| {
        match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    })
    .context("failed to register Windows service control handler")?;

    set_windows_service_status(&status_handle, ServiceState::StartPending, ServiceControlAccept::empty(), 0)?;
    set_windows_service_status(
        &status_handle,
        ServiceState::Running,
        ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        0,
    )?;

    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let result = runtime.block_on(async {
        tokio::select! {
            result = run_agent_forever() => result,
            _ = async { let _ = shutdown_rx.recv(); } => Ok(()),
        }
    });

    set_windows_service_status(&status_handle, ServiceState::StopPending, ServiceControlAccept::empty(), 0)?;

    match result {
        Ok(()) => {
            set_windows_service_status(&status_handle, ServiceState::Stopped, ServiceControlAccept::empty(), 0)?;
            Ok(())
        }
        Err(err) => {
            set_windows_service_status(&status_handle, ServiceState::Stopped, ServiceControlAccept::empty(), 1)?;
            Err(err)
        }
    }
}

#[cfg(target_os = "windows")]
fn set_windows_service_status(
    status_handle: &service_control_handler::ServiceStatusHandle,
    current_state: ServiceState,
    controls_accepted: ServiceControlAccept,
    win32_exit_code: u32,
) -> Result<()> {
    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state,
            controls_accepted,
            exit_code: ServiceExitCode::Win32(win32_exit_code),
            checkpoint: 0,
            wait_hint: Duration::from_secs(10),
            process_id: None,
        })
        .context("failed to update Windows service status")?;
    Ok(())
}

fn run_configure(as_json: bool) -> Result<()> {
    let config = runtime_config();
    println!("[remote-terminal-cloud-agent] config file: {}", config.config_file_path.display());
    let token = prompt_and_persist_registration_token(&config.config_file_path)?;
    if as_json {
        emit_json(&serde_json::json!({
            "ok": true,
            "saved": token.is_some(),
            "configFile": config.config_file_path.display().to_string(),
        }))
    } else if token.is_some() {
        println!("[remote-terminal-cloud-agent] configuration updated successfully.");
        Ok(())
    } else {
        println!("[remote-terminal-cloud-agent] no token saved.");
        Ok(())
    }
}

fn run_version(as_json: bool) -> Result<()> {
    if as_json {
        emit_json(&VersionResponse { version: VERSION, server_base_url: default_server_base_url() })
    } else {
        println!("rtc-agent version {VERSION}");
        println!("server base URL: {}", default_server_base_url());
        Ok(())
    }
}

fn run_paths(as_json: bool) -> Result<()> {
    let paths = manager_paths();
    if as_json {
        emit_json(&PathsResponse {
            config_file: paths.config_file_path.display().to_string(),
            preferences_file: paths.preferences_path.display().to_string(),
            config_dir: paths.config_dir.display().to_string(),
            logs_dir: paths.logs_dir.display().to_string(),
        })
    } else {
        println!("config file: {}", paths.config_file_path.display());
        println!("preferences file: {}", paths.preferences_path.display());
        println!("config dir: {}", paths.config_dir.display());
        println!("logs dir: {}", paths.logs_dir.display());
        Ok(())
    }
}

fn run_config(as_json: bool) -> Result<()> {
    let config = runtime_config();
    let token_status = if config.registration_token.is_some() { "configured" } else { "missing" };
    let token_source = if config.registration_token.is_some() {
        if has_registration_token_env_override() {
            "environment variable RTC_REGISTRATION_TOKEN"
        } else {
            "config file"
        }
    } else {
        "none"
    };
    if as_json {
        emit_json(&serde_json::json!({
            "serverBaseUrl": config.server_base_url,
            "registrationToken": token_status,
            "registrationTokenSource": token_source,
            "configFile": config.config_file_path.display().to_string(),
            "preferencesFile": config.preferences_file_path.display().to_string(),
            "runHeartbeat": config.run_heartbeat,
            "runTunnel": config.run_tunnel,
            "defaultShell": config.default_shell_type.as_str(),
            "enabledShells": config.enabled_shell_types.iter().map(|item| item.as_str()).collect::<Vec<_>>(),
        }))
    } else {
        println!("server base URL: {}", config.server_base_url);
        println!("registration token: {}", token_status);
        println!("registration token source: {}", token_source);
        println!("config file: {}", config.config_file_path.display());
        println!("preferences file: {}", config.preferences_file_path.display());
        println!("run heartbeat: {}", config.run_heartbeat);
        println!("run tunnel: {}", config.run_tunnel);
        println!("default shell: {}", config.default_shell_type.as_str());
        println!("enabled shells: {}", join_shells(&config.enabled_shell_types));
        Ok(())
    }
}

fn run_status(as_json: bool) -> Result<()> {
    let config = runtime_config();
    let logs_dir = managed_logs_dir();
    let snapshot = collect_host_snapshot(
        VERSION,
        &config.enabled_shell_types,
        logs_dir.display().to_string(),
    )?;
    let effective =
        resolve_default_shell(config.default_shell_type, &snapshot.diagnostics.available_shells);
    let response = StatusResponse {
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
            Some(platform) => {
                serde_json::to_value(platform)?.as_str().unwrap_or("unknown").to_owned()
            }
            None => "unknown".into(),
        },
        arch: snapshot.arch.clone(),
        preferences_summary: read_preferences_summary(&config.preferences_file_path),
    };

    if as_json {
        emit_json(&response)
    } else {
        println!("agent version: {}", response.version);
        println!("platform: {}/{}", response.platform, response.arch);
        println!("server base URL: {}", response.server_base_url);
        println!("config file: {}", response.config_file);
        println!("registration token: {}", response.registration_token);
        println!("heartbeat enabled: {}", response.run_heartbeat);
        println!("tunnel enabled: {}", response.run_tunnel);
        println!("configured default shell: {}", response.configured_default_shell);
        println!("effective default shell: {}", response.effective_default_shell);
        println!("available shells: {}", response.available_shells.join(", "));
        println!("ssh available: {}", response.ssh_available);
        println!("ssh detail: {}", response.ssh_detail);
        println!(
            "preferences: cwd=`{}` shortcuts={} quickCommands={}",
            response.preferences_summary.default_working_directory,
            response.preferences_summary.shortcuts_count,
            response.preferences_summary.quick_commands_count
        );
        Ok(())
    }
}

fn run_doctor(as_json: bool) -> Result<()> {
    let config = runtime_config();
    let logs_dir = managed_logs_dir();
    let snapshot = collect_host_snapshot(
        VERSION,
        &config.enabled_shell_types,
        logs_dir.display().to_string(),
    )?;
    if as_json {
        emit_json(&serde_json::json!({
            "summary": {
                "agentVersion": VERSION,
                "serverBaseUrl": config.server_base_url,
                "configFile": config.config_file_path.display().to_string(),
                "preferencesFile": config.preferences_file_path.display().to_string(),
                "registrationTokenConfigured": config.registration_token.is_some(),
            },
            "snapshot": snapshot,
        }))
    } else {
        println!("Doctor summary");
        println!("- Agent version: {}", VERSION);
        println!("- Server base URL: {}", config.server_base_url);
        println!("- Config file: {}", config.config_file_path.display());
        println!("- Preferences file: {}", config.preferences_file_path.display());
        println!(
            "- Registration token: {}",
            if config.registration_token.is_some() { "configured" } else { "missing" }
        );
        println!(
            "- Available shells: {}",
            snapshot
                .diagnostics
                .available_shells
                .iter()
                .map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("- SSH check: {}", snapshot.diagnostics.ssh_check.detail);
        Ok(())
    }
}

fn run_shells(as_json: bool) -> Result<()> {
    let config = runtime_config();
    let available = detect_available_shells(&config.enabled_shell_types);
    let effective = resolve_default_shell(config.default_shell_type, &available);
    if as_json {
        emit_json(&serde_json::json!({
            "configuredDefaultShell": config.default_shell_type.as_str(),
            "effectiveDefaultShell": effective.as_str(),
            "enabledShells": config.enabled_shell_types.iter().map(|item| item.as_str()).collect::<Vec<_>>(),
            "availableShells": available.iter().map(|item| item.as_str()).collect::<Vec<_>>(),
        }))
    } else {
        println!("configured default shell: {}", config.default_shell_type.as_str());
        println!("effective default shell: {}", effective.as_str());
        println!("enabled shells: {}", join_shells(&config.enabled_shell_types));
        println!("detected available shells: {}", join_shells(&available));
        Ok(())
    }
}

fn run_service(args: ServiceArgs) -> Result<()> {
    let action = args.action.unwrap_or_else(|| "status".into());
    let value = args.value.unwrap_or_default();
    let install_root = detect_install_root();
    let result = match action.as_str() {
        "install" => service::install_service(
            &install_root,
            if value.trim().is_empty() { None } else { Some(value.as_str()) },
        )?,
        "uninstall" => service::uninstall_service(&install_root)?,
        "start" => service::start_service()?,
        "stop" => service::stop_service(&install_root)?,
        "restart" => service::restart_service(&install_root)?,
        "status" => service::service_status(),
        other => bail!("unknown service action: {other}"),
    };
    if args.json {
        emit_json(&result)
    } else {
        println!("{}", result.message);
        Ok(())
    }
}

fn detect_install_root() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.display().to_string())
        })
        .unwrap_or_else(|| ".".into())
}

async fn run_verify(as_json: bool) -> Result<()> {
    let config = runtime_config();
    let logs_dir = managed_logs_dir();
    let snapshot = collect_host_snapshot(
        VERSION,
        &config.enabled_shell_types,
        logs_dir.display().to_string(),
    )?;
    let effective_default_shell =
        resolve_default_shell(config.default_shell_type, &snapshot.diagnostics.available_shells);

    let Some(registration_token) = config.registration_token.clone() else {
        let message = "Registration token is missing. Run `rtc-agent configure` or set RTC_REGISTRATION_TOKEN first.";
        if as_json {
            return emit_json(&VerifyErrorResponse {
                ok: false,
                server_base_url: config.server_base_url.clone(),
                config_file: config.config_file_path.display().to_string(),
                registration_token_source: "none".into(),
                effective_default_shell: effective_default_shell.as_str().to_owned(),
                error: VerifyErrorDetails {
                    kind: "missing-token".into(),
                    status: None,
                    code: None,
                    message: message.into(),
                    suggestion:
                        "Open `rtc-agent configure`, paste a valid token, and try verify again."
                            .into(),
                },
            });
        }
        bail!(message);
    };

    let token_source = if has_registration_token_env_override() {
        "environment variable RTC_REGISTRATION_TOKEN".to_owned()
    } else {
        "config file".to_owned()
    };

    let api_client = ApiClient::default();
    let device_fingerprint = load_or_collect_device_fingerprint(&config.config_file_path)?;
    let session = match api_client
        .register_agent(
            &config.server_base_url,
            &registration_token,
            &device_fingerprint.device_fingerprint,
            &device_fingerprint.fingerprint_version,
            snapshot,
        )
        .await
    {
        Ok(session) => session,
        Err(err) => {
            return emit_verify_error(
                as_json,
                &config.server_base_url,
                &config.config_file_path,
                &token_source,
                effective_default_shell,
                err,
            );
        }
    };
    let websocket_url =
        match rtc_agent_runtime::verify_websocket_connectivity(&config.server_base_url, &session)
            .await
        {
            Ok(url) => url,
            Err(err) => {
                return emit_verify_error(
                    as_json,
                    &config.server_base_url,
                    &config.config_file_path,
                    &token_source,
                    effective_default_shell,
                    err,
                );
            }
        };

    let response = VerifyResponse {
        ok: true,
        server_base_url: config.server_base_url.clone(),
        config_file: config.config_file_path.display().to_string(),
        registration_token_source: token_source,
        device_id: session.device_id,
        heartbeat_interval_seconds: session.heartbeat_interval_seconds,
        websocket_url,
        effective_default_shell: effective_default_shell.as_str().to_owned(),
    };

    if as_json {
        emit_json(&response)
    } else {
        println!("verify ok: true");
        println!("server base URL: {}", response.server_base_url);
        println!("config file: {}", response.config_file);
        println!("registration token source: {}", response.registration_token_source);
        println!("device id: {}", response.device_id);
        println!("heartbeat interval seconds: {}", response.heartbeat_interval_seconds);
        println!("websocket URL: {}", response.websocket_url);
        println!("effective default shell: {}", response.effective_default_shell);
        Ok(())
    }
}

async fn run_agent_forever() -> Result<()> {
    let mut retry_attempt = 0_u32;
    loop {
        if let Err(err) = run_agent_once().await {
            retry_attempt = retry_attempt.saturating_add(1);
            let delay = compute_retry_delay(retry_attempt);
            print_runtime_error("[remote-terminal-cloud-agent] runtime error", &err);
            eprintln!(
                "[remote-terminal-cloud-agent] the agent will retry automatically in {} seconds (attempt {}).",
                delay.as_secs(),
                retry_attempt
            );
            tokio::time::sleep(delay).await;
        } else {
            retry_attempt = 0;
        }
    }
}

fn compute_retry_delay(retry_attempt: u32) -> Duration {
    let capped = retry_attempt.min(5);
    let secs = RUNTIME_RETRY_INTERVAL.as_secs().saturating_mul(1_u64 << capped);
    Duration::from_secs(secs.min(MAX_BACKOFF_INTERVAL.as_secs()))
}

/// Warns if the system service is currently running, since the foreground
/// agent will conflict with it.
fn warn_if_service_running() {
    let status = service::service_status();
    let text = status.message.to_ascii_lowercase();
    if status.ok && (text.contains("running") || text.contains("loaded")) {
        eprintln!(
            "[remote-terminal-cloud-agent] warning: the background service appears to be running.\n\
             [remote-terminal-cloud-agent] running the agent in the foreground while the service\n\
             [remote-terminal-cloud-agent] is active may cause duplicate connections.\n\
             [remote-terminal-cloud-agent] stop it first with: rtc-agent service stop"
        );
    }
}

async fn run_agent_once() -> Result<()> {
    let mut config = runtime_config();
    let logs_dir = managed_logs_dir();
    let snapshot = collect_host_snapshot(
        VERSION,
        &config.enabled_shell_types,
        logs_dir.display().to_string(),
    )?;
    let effective_default_shell =
        resolve_default_shell(config.default_shell_type, &snapshot.diagnostics.available_shells);

    println!("[remote-terminal-cloud-agent] config file: {}", config.config_file_path.display());
    println!("[remote-terminal-cloud-agent] host snapshot");
    emit_json(&snapshot)?;
    println!(
        "[remote-terminal-cloud-agent] shell capabilities: {}",
        join_shells(&snapshot.diagnostics.available_shells)
    );

    if effective_default_shell != config.default_shell_type {
        println!(
            "[remote-terminal-cloud-agent] RTC_DEFAULT_SHELL={} is unavailable; fallback to {}.",
            config.default_shell_type.as_str(),
            effective_default_shell.as_str()
        );
    }
    if snapshot.diagnostics.available_shells.is_empty() {
        println!(
            "[remote-terminal-cloud-agent] no shells available after detection/config filtering."
        );
    }
    if !snapshot.diagnostics.ssh_check.available {
        println!("[remote-terminal-cloud-agent] SSH precheck failed.");
    }

    if config.registration_token.is_none() && is_interactive_input_available() {
        config.registration_token =
            prompt_and_persist_registration_token(&config.config_file_path)?;
    }

    let Some(registration_token) = config.registration_token.clone() else {
        if is_interactive_input_available() {
            println!(
                "[remote-terminal-cloud-agent] registration token is still empty. Update {} or set RTC_REGISTRATION_TOKEN, then the agent will retry automatically.",
                config.config_file_path.display()
            );
        } else {
            println!(
                "[remote-terminal-cloud-agent] waiting for configuration: set RTC_REGISTRATION_TOKEN in {} or environment, then the service will retry automatically.",
                config.config_file_path.display()
            );
        }
        tokio::time::sleep(MISSING_CONFIG_RETRY).await;
        return Ok(());
    };

    let api_client = ApiClient::default();
    let device_fingerprint = load_or_collect_device_fingerprint(&config.config_file_path)?;
    let now_unix_seconds = rtc_agent_config::current_unix_timestamp();

    let build_agent_state = |session: &RegisteredAgentSession| AgentState {
        device_id: session.device_id.clone(),
        heartbeat_token: session.heartbeat_token.clone(),
        heartbeat_interval_seconds: session.heartbeat_interval_seconds,
        websocket_url: session.websocket_url.clone(),
        server_base_url: config.server_base_url.clone(),
        device_fingerprint: device_fingerprint.device_fingerprint.clone(),
        fingerprint_version: device_fingerprint.fingerprint_version.clone(),
        state_version: rtc_agent_config::AGENT_STATE_VERSION,
        saved_at_unix_seconds: rtc_agent_config::current_unix_timestamp(),
    };

    let register_fresh_session = async || -> Result<Option<RegisteredAgentSession>> {
        let session = match api_client
            .register_agent(
                &config.server_base_url,
                &registration_token,
                &device_fingerprint.device_fingerprint,
                &device_fingerprint.fingerprint_version,
                snapshot.clone(),
            )
            .await
        {
            Ok(session) => session,
            Err(err) => {
                print_runtime_error("[remote-terminal-cloud-agent] registration failed", &err);
                clear_agent_state(&config.config_file_path);
                if is_authentication_error(&err) {
                    eprintln!(
                        "[remote-terminal-cloud-agent] registration token was rejected; keep the current token in {} or update RTC_REGISTRATION_TOKEN, then retry.",
                        config.config_file_path.display()
                    );
                    tokio::time::sleep(MISSING_CONFIG_RETRY).await;
                    return Ok(None);
                }
                return Err(err);
            }
        };
        println!(
            "[remote-terminal-cloud-agent] registered device {} for fingerprint {}",
            session.device_id,
            &device_fingerprint.device_fingerprint[..12.min(device_fingerprint.device_fingerprint.len())]
        );
        if let Err(err) = save_agent_state(&config.config_file_path, &build_agent_state(&session)) {
            eprintln!("[remote-terminal-cloud-agent] warning: could not persist state: {err}");
        }
        Ok(Some(session))
    };

    // Try to resume from persisted state, but only after a successful heartbeat validation.
    let session = if let Some(saved) = load_agent_state(&config.config_file_path) {
        let is_fresh = rtc_agent_config::is_agent_state_fresh(&saved, now_unix_seconds);
        let same_server = saved.server_base_url.trim().is_empty()
            || saved.server_base_url == config.server_base_url;
        let same_fingerprint = saved.device_fingerprint.trim().is_empty()
            || (saved.device_fingerprint == device_fingerprint.device_fingerprint
                && (saved.fingerprint_version.trim().is_empty()
                    || saved.fingerprint_version == device_fingerprint.fingerprint_version));

        if is_fresh && same_server && same_fingerprint {
            println!(
                "[remote-terminal-cloud-agent] validating persisted session for device {}",
                saved.device_id
            );
            let persisted_session = RegisteredAgentSession {
                device_id: saved.device_id,
                heartbeat_token: saved.heartbeat_token,
                heartbeat_interval_seconds: saved.heartbeat_interval_seconds,
                websocket_url: saved.websocket_url,
            };

            match api_client
                .send_heartbeat(&config.server_base_url, &persisted_session, snapshot.clone())
                .await
            {
                Ok(validated_session) => {
                    println!(
                        "[remote-terminal-cloud-agent] resumed persisted session for device {}",
                        validated_session.device_id
                    );
                    if let Err(err) =
                        save_agent_state(&config.config_file_path, &build_agent_state(&validated_session))
                    {
                        eprintln!(
                            "[remote-terminal-cloud-agent] warning: could not persist validated state: {err}"
                        );
                    }
                    validated_session
                }
                Err(err) => {
                    print_runtime_error(
                        "[remote-terminal-cloud-agent] persisted session heartbeat check failed; re-registering",
                        &err,
                    );
                    clear_agent_state(&config.config_file_path);
                    match register_fresh_session().await? {
                        Some(session) => session,
                        None => return Ok(()),
                    }
                }
            }
        } else if !is_fresh {
            println!(
                "[remote-terminal-cloud-agent] ignoring persisted session because state cache expired; re-registering."
            );
            clear_agent_state(&config.config_file_path);
            match register_fresh_session().await? {
                Some(session) => session,
                None => return Ok(()),
            }
        } else {
            println!(
                "[remote-terminal-cloud-agent] ignoring persisted session because environment changed; re-registering."
            );
            clear_agent_state(&config.config_file_path);
            match register_fresh_session().await? {
                Some(session) => session,
                None => return Ok(()),
            }
        }
    } else {
        match register_fresh_session().await? {
            Some(session) => session,
            None => return Ok(()),
        }
    };

    if !config.run_heartbeat && !config.run_tunnel {
        println!(
            "[remote-terminal-cloud-agent] heartbeat and tunnel are both disabled; retrying later."
        );
        tokio::time::sleep(MISSING_CONFIG_RETRY).await;
        return Ok(());
    }

    let mut tasks = JoinSet::<Result<()>>::new();

    if config.run_heartbeat {
        let api_client = api_client.clone();
        let server_base_url = config.server_base_url.clone();
        let enabled_shell_types = config.enabled_shell_types.clone();
        let logs_dir = logs_dir.clone();
        let config_file_path = config.config_file_path.clone();
        let mut heartbeat_session = session.clone();
        tasks.spawn(async move {
            let mut failure_backoff = INITIAL_BACKOFF_INTERVAL;
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
                match api_client
                    .send_heartbeat(&server_base_url, &heartbeat_session, heartbeat_snapshot)
                    .await
                {
                    Ok(next_session) => {
                        heartbeat_session = next_session;
                        failure_backoff = INITIAL_BACKOFF_INTERVAL;
                    }
                    Err(err) => {
                        print_runtime_error("[remote-terminal-cloud-agent] heartbeat failed", &err);
                        if is_authentication_error(&err) {
                            clear_agent_state(&config_file_path);
                            return Err(err);
                        }
                        let sleep_for = next_backoff_delay(failure_backoff);
                        eprintln!(
                            "[remote-terminal-cloud-agent] heartbeat retrying in {} seconds.",
                            sleep_for.as_secs()
                        );
                        tokio::time::sleep(sleep_for).await;
                        failure_backoff = grow_backoff(failure_backoff);
                        continue;
                    }
                }
                // Persist updated heartbeat state (interval / websocket_url may change).
                let _ = save_agent_state(&config_file_path, &AgentState {
                    device_id: heartbeat_session.device_id.clone(),
                    heartbeat_token: heartbeat_session.heartbeat_token.clone(),
                    heartbeat_interval_seconds: heartbeat_session.heartbeat_interval_seconds,
                    websocket_url: heartbeat_session.websocket_url.clone(),
                    server_base_url: server_base_url.clone(),
                    device_fingerprint: device_fingerprint.device_fingerprint.clone(),
                    fingerprint_version: device_fingerprint.fingerprint_version.clone(),
                    state_version: rtc_agent_config::AGENT_STATE_VERSION,
                    saved_at_unix_seconds: rtc_agent_config::current_unix_timestamp(),
                });
                println!(
                    "[remote-terminal-cloud-agent] heartbeat ok for {}; next interval {}s",
                    heartbeat_session.device_id, heartbeat_session.heartbeat_interval_seconds
                );
            }
        });
    } else {
        println!("[remote-terminal-cloud-agent] heartbeat disabled by RTC_DISABLE_HEARTBEAT=1");
    }

    if config.run_tunnel {
        let server_base_url = config.server_base_url.clone();
        let preferences_file_path = config.preferences_file_path.clone();
        let config_file_path = config.config_file_path.clone();
        let tunnel_session = session.clone();
        tasks.spawn(async move {
            let mut failure_backoff = INITIAL_BACKOFF_INTERVAL;
            loop {
                let current_session = tunnel_session.clone();
                match run_agent_tunnel(
                    &server_base_url,
                    current_session,
                    effective_default_shell,
                    &preferences_file_path,
                )
                .await
                {
                    Ok(()) => {
                        failure_backoff = INITIAL_BACKOFF_INTERVAL;
                        let sleep_for = next_backoff_delay(failure_backoff);
                        eprintln!(
                            "[remote-terminal-cloud-agent] tunnel closed; reconnecting in {} seconds.",
                            sleep_for.as_secs()
                        );
                        tokio::time::sleep(sleep_for).await;
                        failure_backoff = grow_backoff(failure_backoff);
                    }
                    Err(err) => {
                        print_runtime_error("[remote-terminal-cloud-agent] tunnel failed", &err);
                        if is_authentication_error(&err) {
                            clear_agent_state(&config_file_path);
                            return Err(err);
                        }
                        let sleep_for = next_backoff_delay(failure_backoff);
                        eprintln!(
                            "[remote-terminal-cloud-agent] tunnel retrying in {} seconds.",
                            sleep_for.as_secs()
                        );
                        tokio::time::sleep(sleep_for).await;
                        failure_backoff = grow_backoff(failure_backoff);
                    }
                }
            }
        });
    } else {
        println!("[remote-terminal-cloud-agent] tunnel disabled by RTC_DISABLE_TUNNEL=1");
    }

    loop {
        let task_result = match tasks.join_next().await {
            Some(result) => result?,
            None => bail!("agent runtime exited without active tasks"),
        };
        if let Err(err) = task_result {
            clear_agent_state(&config.config_file_path);
            return Err(err);
        }
    }
}

fn emit_verify_error(
    as_json: bool,
    server_base_url: &str,
    config_file: &Path,
    registration_token_source: &str,
    effective_default_shell: ShellType,
    err: anyhow::Error,
) -> Result<()> {
    if let Some(details) = describe_error(&err) {
        if as_json {
            return emit_json(&VerifyErrorResponse {
                ok: false,
                server_base_url: server_base_url.to_owned(),
                config_file: config_file.display().to_string(),
                registration_token_source: registration_token_source.to_owned(),
                effective_default_shell: effective_default_shell.as_str().to_owned(),
                error: VerifyErrorDetails::from(details),
            });
        }

        eprintln!("verify ok: false");
        eprintln!("server base URL: {server_base_url}");
        eprintln!("config file: {}", config_file.display());
        eprintln!("registration token source: {registration_token_source}");
        eprintln!("effective default shell: {}", effective_default_shell.as_str());
        eprintln!("reason: {}", user_label_for_error_kind(&details.kind));
        eprintln!("message: {}", details.message);
        eprintln!("suggestion: {}", details.suggestion);
        return Ok(());
    }

    if as_json {
        return emit_json(&VerifyErrorResponse {
            ok: false,
            server_base_url: server_base_url.to_owned(),
            config_file: config_file.display().to_string(),
            registration_token_source: registration_token_source.to_owned(),
            effective_default_shell: effective_default_shell.as_str().to_owned(),
            error: VerifyErrorDetails {
                kind: "unexpected".into(),
                status: None,
                code: None,
                message: err.to_string(),
                suggestion:
                    "Retry once. If it still fails, capture the JSON output and inspect the backend and local network path.".into(),
            },
        });
    }

    eprintln!("verify ok: false");
    eprintln!("server base URL: {server_base_url}");
    eprintln!("config file: {}", config_file.display());
    eprintln!("registration token source: {registration_token_source}");
    eprintln!("effective default shell: {}", effective_default_shell.as_str());
    eprintln!("reason: unexpected error");
    eprintln!("message: {err}");
    eprintln!(
        "suggestion: Retry once. If it still fails, inspect the backend and local network path."
    );
    Ok(())
}

impl From<ApiErrorDetails> for VerifyErrorDetails {
    fn from(value: ApiErrorDetails) -> Self {
        Self {
            kind: match value.kind {
                ApiErrorKind::InvalidToken => "invalid-token",
                ApiErrorKind::DeviceLimitReached => "device-limit-reached",
                ApiErrorKind::GatewayUnavailable => "gateway-unavailable",
                ApiErrorKind::ProxyConfiguration => "proxy-configuration",
                ApiErrorKind::WebsocketUnavailable => "websocket-unavailable",
                ApiErrorKind::Unauthorized => "unauthorized",
                ApiErrorKind::ServerRejected => "server-rejected",
                ApiErrorKind::Transport => "transport",
                ApiErrorKind::Unexpected => "unexpected",
            }
            .into(),
            status: value.status,
            code: value.code,
            message: value.message,
            suggestion: value.suggestion,
        }
    }
}

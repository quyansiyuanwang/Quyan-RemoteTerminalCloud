use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use clap::{ArgAction, Args, Parser, Subcommand};
use rtc_agent_config::{
    default_config_file_path, default_preferences_file_path, persist_registration_token,
};
use rtc_agent_service as service;
use serde::Serialize;

#[derive(Parser)]
#[command(name = "rtc-agent-installer", version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Windows(WindowsArgs),
}

#[derive(Args)]
struct WindowsArgs {
    action: Option<String>,
    install_root: Option<String>,
    token: Option<String>,
    #[arg(long, action = ArgAction::SetTrue)]
    json: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallerPaths {
    config_file: String,
    preferences_file: String,
    config_dir: String,
    logs_dir: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let result = match cli.command {
        Some(Command::Windows(args)) => run_windows(args),
        None => print_help(false),
    };
    use std::io::Write;
    let _ = std::io::stdout().flush();
    result
}

fn run_windows(args: WindowsArgs) -> Result<()> {
    let action = args.action.unwrap_or_else(|| "help".into());
    let install_root = args
        .install_root
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(default_windows_install_root);
    match action.as_str() {
        "help" | "--help" | "-h" => print_help(args.json),
        "init-config" => {
            let path = default_config_file_path();
            if !path.exists() {
                persist_registration_token(&path, "replace-with-real-token")?;
            }
            if args.json {
                let config_dir = path
                    .parent()
                    .map(|item| item.display().to_string())
                    .unwrap_or_else(|| ".".into());
                println!(
                    "{}",
                    serde_json::to_string_pretty(&InstallerPaths {
                        config_file: path.display().to_string(),
                        preferences_file: default_preferences_file_path().display().to_string(),
                        config_dir,
                        logs_dir: managed_logs_dir()
                    })?
                );
            }
            Ok(())
        }
        "save-token" => {
            let token = args.token.or(args.install_root).unwrap_or_default();
            if token.trim().is_empty() {
                bail!("windows save-token requires a token value");
            }
            persist_registration_token(Path::new(&default_config_file_path()), token.trim())?;
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "ok": true,
                        "configFile": default_config_file_path().display().to_string()
                    }))?
                );
            }
            Ok(())
        }
        "start" | "start-service" => emit_service_result(service::start_service()?, args.json),
        "stop" | "stop-service" => {
            stop_service_with_cleanup(&install_root, args.json)
        }
        "restart" | "restart-service" => emit_service_result(
            service::restart_service(&install_root)?,
            args.json,
        ),
        "install" | "install-service" => emit_service_result(
            service::install_service(
                &install_root,
                args.token.as_deref(),
            )?,
            args.json,
        ),
        "uninstall" | "uninstall-service" => emit_service_result(
            service::uninstall_service(&install_root)?,
            args.json,
        ),
        "open-config-dir" => {
            let path = default_config_file_path();
            if let Some(parent) = path.parent() {
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &serde_json::json!({ "path": parent.display().to_string() })
                        )?
                    );
                } else {
                    println!("{}", parent.display());
                }
            }
            Ok(())
        }
        "open-logs" => {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &serde_json::json!({ "path": managed_logs_dir() })
                    )?
                );
            } else {
                println!("{}", managed_logs_dir());
            }
            Ok(())
        }
        "paths" => {
            let path = default_config_file_path();
            let config_dir =
                path.parent().map(|item| item.display().to_string()).unwrap_or_else(|| ".".into());
            let payload = InstallerPaths {
                config_file: path.display().to_string(),
                preferences_file: default_preferences_file_path().display().to_string(),
                config_dir,
                logs_dir: managed_logs_dir(),
            };
            if args.json {
                println!("{}", serde_json::to_string_pretty(&payload)?);
            } else {
                println!("config file: {}", payload.config_file);
                println!("preferences file: {}", payload.preferences_file);
                println!("config dir: {}", payload.config_dir);
                println!("logs dir: {}", payload.logs_dir);
            }
            Ok(())
        }
        "status" => emit_service_result(service::service_status(), args.json),
        _ => bail!("unknown windows installer action: {}", action),
    }
}

fn print_help(as_json: bool) -> Result<()> {
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "defaultInstallRoot": default_windows_install_root(),
                "commands": [
                    "windows help",
                    "windows init-config",
                    "windows save-token <token>",
                    "windows paths",
                    "windows status",
                    "windows start",
                    "windows stop",
                    "windows restart",
                    "windows install",
                    "windows uninstall",
                    "windows open-config-dir",
                    "windows open-logs"
                ]
            }))?
        );
    } else {
        println!("Remote Terminal Cloud Agent Windows installer helper");
        println!("Default install root: {}", default_windows_install_root());
        println!();
        println!("Usage:");
        println!("  rtc-agent-installer");
        println!("  rtc-agent-installer windows help");
        println!("  rtc-agent-installer windows init-config [--json]");
        println!("  rtc-agent-installer windows save-token <token> [--json]");
        println!("  rtc-agent-installer windows paths [--json]");
        println!("  rtc-agent-installer windows status [--json]");
        println!("  rtc-agent-installer windows start [--json]");
        println!("  rtc-agent-installer windows stop [--json]");
        println!("  rtc-agent-installer windows restart [--json]");
        println!("  rtc-agent-installer windows install [install_root] [token] [--json]");
        println!("  rtc-agent-installer windows uninstall [install_root] [--json]");
        println!("  rtc-agent-installer windows open-config-dir [--json]");
        println!("  rtc-agent-installer windows open-logs [--json]");
        println!();
        println!("Examples:");
        println!("  rtc-agent-installer windows install");
        println!("  rtc-agent-installer windows install . <token>");
    }
    Ok(())
}

fn default_windows_install_root() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.display().to_string()))
        .unwrap_or_else(|| ".".into())
}

fn managed_logs_dir() -> String {
    #[cfg(target_os = "windows")]
    {
        return std::env::var("ProgramData")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| r"C:\ProgramData".into())
            + r"\RemoteTerminalCloudAgent\logs";
    }
    #[cfg(not(target_os = "windows"))]
    {
        "./logs".into()
    }
}

fn emit_service_result(result: service::ServiceActionResult, as_json: bool) -> Result<()> {
    if as_json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", result.message);
    }
    Ok(())
}

fn stop_service_with_cleanup(install_root: &str, as_json: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        // Step 1: stop the native service
        let _ = Command::new("sc")
            .args(["stop", service::WINDOWS_SERVICE_NAME])
            .status();

        // Step 2: wait for service to reach Stopped / Missing (30s timeout)
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            match service::query_service_state() {
                service::ServiceState::Stopped | service::ServiceState::Missing => break,
                _ => std::thread::sleep(Duration::from_millis(500)),
            }
        }

        // Step 3: kill any lingering agent processes under the install root
        let root_upper = install_root.trim().to_ascii_uppercase();
        if !root_upper.is_empty() {
            for process_name in &["rtc-agent.exe", "rtc-agentd.exe"] {
                if let Ok(output) = Command::new("tasklist")
                    .args(["/FI", &format!("IMAGENAME eq {process_name}"), "/FO", "CSV", "/NH"])
                    .output()
                {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    // tasklist /FO CSV /NH output: "image.exe","pid","session","session#","mem"
                    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
                        let fields: Vec<&str> = line.trim_matches('"').split("\",\"").collect();
                        if fields.len() >= 2 {
                            if let Ok(pid) = fields[1].trim().parse::<u32>() {
                                // check executable path via wmic to avoid killing unrelated processes
                                if let Ok(wmic_output) = Command::new("wmic")
                                    .args([
                                        "process",
                                        "where",
                                        &format!("processid={pid}"),
                                        "get",
                                        "executablepath",
                                        "/format:value",
                                    ])
                                    .output()
                                {
                                    let wmic_text = String::from_utf8_lossy(&wmic_output.stdout);
                                    if wmic_text.to_ascii_uppercase().contains(&root_upper) {
                                        let _ = Command::new("taskkill")
                                            .args(["/F", "/PID", &pid.to_string()])
                                            .status();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Step 4: wait again for service to fully stop (15s timeout)
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            match service::query_service_state() {
                service::ServiceState::Stopped | service::ServiceState::Missing => break,
                _ => std::thread::sleep(Duration::from_millis(500)),
            }
        }

        let status = service::service_status();
        return emit_service_result(status, as_json);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let result = service::stop_service(install_root)?;
        emit_service_result(result, as_json)
    }
}

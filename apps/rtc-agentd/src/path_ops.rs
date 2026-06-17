use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::support::manager_paths;
use rtc_agent_service as service;

fn pid_file_path() -> PathBuf {
    manager_paths().config_dir.join("rtc-agent.pid")
}

pub fn run_start() -> Result<()> {
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        match service::start_service() {
            Ok(result) => {
                println!("{}", result.message);
                return Ok(());
            }
            Err(err) => {
                eprintln!(
                    "[remote-terminal-cloud-agent] service start unavailable, falling back to legacy background mode: {err}"
                );
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        match service::start_service() {
            Ok(result) => {
                println!("{}", result.message);
                return Ok(());
            }
            Err(err) => {
                eprintln!(
                    "[remote-terminal-cloud-agent] service start unavailable, falling back to legacy background mode: {err}"
                );
            }
        }
    }

    let pid_path = pid_file_path();
    if let Some(parent) = pid_path.parent() {
        std::fs::create_dir_all(parent).context("create config dir")?;
    }
    if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
        let pid: u32 = pid_str.trim().parse().unwrap_or(0);
        if pid > 0 && process_is_running(pid) {
            println!("[remote-terminal-cloud-agent] already running (pid {pid})");
            return Ok(());
        }
    }

    let exe = std::env::current_exe().context("cannot resolve current executable path")?;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        let child = std::process::Command::new(&exe)
            .arg("run")
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn()
            .context("failed to spawn background process")?;
        let pid = child.id();
        std::fs::write(&pid_path, pid.to_string()).context("write pid file")?;
        println!("[remote-terminal-cloud-agent] started in background (pid {pid})");
    }

    #[cfg(not(target_os = "windows"))]
    {
        let logs_dir = manager_paths().logs_dir;
        std::fs::create_dir_all(&logs_dir).ok();
        let stdout_log = logs_dir.join("rtc-agent.log");
        let stdout_file = std::fs::OpenOptions::new()
            .create(true).append(true).open(&stdout_log)
            .context("open stdout log")?;
        let stderr_file = stdout_file.try_clone().context("clone log fd")?;
        let child = std::process::Command::new(&exe)
            .arg("run")
            .stdout(stdout_file)
            .stderr(stderr_file)
            .spawn()
            .context("failed to spawn background process")?;
        let pid = child.id();
        std::fs::write(&pid_path, pid.to_string()).context("write pid file")?;
        println!(
            "[remote-terminal-cloud-agent] started in background (pid {pid}), logs: {}",
            stdout_log.display()
        );
    }

    Ok(())
}

pub fn run_stop() -> Result<()> {
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        match service::stop_service("") {
            Ok(result) => {
                println!("{}", result.message);
                return Ok(());
            }
            Err(err) => {
                eprintln!(
                    "[remote-terminal-cloud-agent] service stop unavailable, falling back to legacy PID mode: {err}"
                );
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        match service::stop_service("") {
            Ok(result) => {
                println!("{}", result.message);
                return Ok(());
            }
            Err(err) => {
                eprintln!(
                    "[remote-terminal-cloud-agent] service stop unavailable, falling back to legacy PID mode: {err}"
                );
            }
        }
    }

    let pid_path = pid_file_path();
    let pid_str = std::fs::read_to_string(&pid_path)
        .context("no pid file found — is the agent running? (use `rtc-agent start`)")?;
    let pid: u32 = pid_str.trim().parse().context("invalid pid file")?;

    #[cfg(target_os = "windows")]
    {
        let status = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status()
            .context("taskkill failed")?;
        if !status.success() {
            bail!("taskkill returned non-zero for pid {pid}");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let status = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
        match status {
            Ok(s) if s.success() => {}
            Ok(_) => {
                println!("[remote-terminal-cloud-agent] process {pid} not found, cleaning up pid file");
            }
            Err(e) => bail!("kill failed: {e}"),
        }
    }

    std::fs::remove_file(&pid_path).ok();
    println!("[remote-terminal-cloud-agent] stopped (pid {pid})");
    Ok(())
}

fn process_is_running(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output();
        match out {
            Ok(o) => String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()),
            Err(_) => false,
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

pub fn run_uninstall_path() -> Result<()> {
    let exe = std::env::current_exe().context("cannot resolve current executable path")?;
    let bin_dir = exe.parent().context("executable has no parent directory")?;

    #[cfg(target_os = "windows")]
    {
        windows_unregister_path(bin_dir)?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        unix_unregister_path(bin_dir)?;
    }
    Ok(())
}

pub fn run_install_path() -> Result<()> {
    let exe = std::env::current_exe().context("cannot resolve current executable path")?;
    let bin_dir = exe.parent().context("executable has no parent directory")?;

    #[cfg(target_os = "windows")]
    {
        windows_register_path(bin_dir)?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        unix_register_path(bin_dir)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_unregister_path(bin_dir: &std::path::Path) -> Result<()> {
    let bin_str = bin_dir.to_string_lossy();
    let output = std::process::Command::new("reg")
        .args(["query", r"HKCU\Environment", "/v", "Path"])
        .output()
        .context("reg query failed")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let current_path = stdout.lines()
        .find(|l| l.trim_start().starts_with("Path"))
        .map(|l| {
            let parts: Vec<&str> = l.splitn(4, "    ").filter(|s| !s.is_empty()).collect();
            parts.last().copied().unwrap_or("").to_owned()
        })
        .unwrap_or_default();

    if !current_path.to_lowercase().contains(&bin_str.to_lowercase()) {
        println!("[remote-terminal-cloud-agent] PATH does not contain {bin_str}, nothing to remove");
        return Ok(());
    }

    let new_path: String = current_path.split(';')
        .filter(|seg| !seg.eq_ignore_ascii_case(&bin_str))
        .collect::<Vec<_>>()
        .join(";");

    let status = std::process::Command::new("reg")
        .args(["add", r"HKCU\Environment", "/v", "Path", "/t", "REG_EXPAND_SZ", "/d", &new_path, "/f"])
        .status()
        .context("reg add failed")?;
    if !status.success() {
        bail!("reg add returned non-zero");
    }
    println!("[remote-terminal-cloud-agent] removed {bin_str} from HKCU\\Environment\\Path");
    println!("  Note: open a new terminal window for the change to take effect.");
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_register_path(bin_dir: &std::path::Path) -> Result<()> {
    let bin_str = bin_dir.to_string_lossy();
    let output = std::process::Command::new("reg")
        .args(["query", r"HKCU\Environment", "/v", "Path"])
        .output()
        .context("reg query failed")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let current_path = stdout.lines()
        .find(|l| l.trim_start().starts_with("Path"))
        .map(|l| {
            let parts: Vec<&str> = l.splitn(4, "    ").filter(|s| !s.is_empty()).collect();
            parts.last().copied().unwrap_or("").to_owned()
        })
        .unwrap_or_default();

    if current_path.to_lowercase().contains(&bin_str.to_lowercase()) {
        println!("[remote-terminal-cloud-agent] PATH already contains {bin_str}");
        return Ok(());
    }

    let new_path = if current_path.is_empty() {
        bin_str.to_string()
    } else {
        format!("{current_path};{bin_str}")
    };

    let status = std::process::Command::new("reg")
        .args(["add", r"HKCU\Environment", "/v", "Path", "/t", "REG_EXPAND_SZ", "/d", &new_path, "/f"])
        .status()
        .context("reg add failed")?;
    if !status.success() {
        bail!("reg add returned non-zero");
    }
    println!("[remote-terminal-cloud-agent] added {bin_str} to HKCU\\Environment\\Path");
    println!("  Note: open a new terminal window for PATH to take effect.");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn unix_unregister_path(bin_dir: &std::path::Path) -> Result<()> {
    let bin_str = bin_dir.to_string_lossy();
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        bail!("$HOME is not set");
    }
    let candidates: &[&str] = &[".zshrc", ".bashrc", ".bash_profile", ".profile"];
    for rc in candidates {
        let path = PathBuf::from(&home).join(rc);
        if !path.exists() { continue; }
        let contents = std::fs::read_to_string(&path).unwrap_or_default();
        if !contents.contains(bin_str.as_ref()) { continue; }
        let new_contents: String = contents.lines()
            .filter(|l| !l.contains(bin_str.as_ref()))
            .map(|l| format!("{l}\n"))
            .collect();
        std::fs::write(&path, new_contents).with_context(|| format!("write ~/{rc}"))?;
        println!("[remote-terminal-cloud-agent] removed PATH entry from ~/{rc}");
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn unix_register_path(bin_dir: &std::path::Path) -> Result<()> {
    let bin_str = bin_dir.to_string_lossy();
    let export_line = format!("\nexport PATH=\"{bin_str}:$PATH\"  # rtc-agent\n");

    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        bail!("$HOME is not set");
    }

    let candidates: &[&str] = &[".zshrc", ".bashrc", ".bash_profile"];
    let mut wrote_any = false;
    for rc in candidates {
        let path = PathBuf::from(&home).join(rc);
        if path.exists() {
            let contents = std::fs::read_to_string(&path).unwrap_or_default();
            if contents.contains(bin_str.as_ref()) {
                println!("[remote-terminal-cloud-agent] {rc} already contains {bin_str}");
                wrote_any = true;
                continue;
            }
            std::fs::OpenOptions::new()
                .append(true).open(&path)
                .and_then(|mut f| { use std::io::Write; f.write_all(export_line.as_bytes()) })
                .with_context(|| format!("write to ~/{rc}"))?;
            println!("[remote-terminal-cloud-agent] appended PATH entry to ~/{rc}");
            wrote_any = true;
        }
    }

    let profile = PathBuf::from(&home).join(".profile");
    let profile_contents = std::fs::read_to_string(&profile).unwrap_or_default();
    if !profile_contents.contains(bin_str.as_ref()) {
        std::fs::OpenOptions::new()
            .create(true).append(true).open(&profile)
            .and_then(|mut f| { use std::io::Write; f.write_all(export_line.as_bytes()) })
            .context("write to ~/.profile")?;
        println!("[remote-terminal-cloud-agent] appended PATH entry to ~/.profile");
        wrote_any = true;
    }

    if wrote_any {
        println!("  Run: source ~/.bashrc  (or open a new shell) for PATH to take effect.");
        println!("  You can then use: rtc-agent start  /  rtc-agent stop  /  rtc-agent status");
    }
    Ok(())
}
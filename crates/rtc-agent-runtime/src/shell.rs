use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context, Result, anyhow};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use rtc_agent_protocol::{
    DirectoryBrowseResultMessage, DirectoryEntry, SessionErrorMessage, SessionExitMessage,
    SessionOutputMessage, SessionReadyMessage, ShellType,
};
use tokio::sync::mpsc::UnboundedSender;
use tracing::warn;

pub struct ShellSessionManager {
    state: Arc<ShellSessionManagerState>,
}

struct ShellSessionManagerState {
    default_shell_type: ShellType,
    outbound: UnboundedSender<String>,
    sessions: Mutex<HashMap<String, Arc<ShellSession>>>,
}

struct ShellSession {
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Mutex<Box<dyn Child + Send>>,
    closed: AtomicBool,
}

struct ShellLaunch {
    executable: String,
    args: Vec<String>,
    shell_type: ShellType,
}

pub fn browse_directories(target_path: &str) -> Result<DirectoryBrowseResultMessage> {
    let normalized = target_path.trim();
    if normalized.is_empty() {
        return root_browse_result();
    }

    let resolved = fs::canonicalize(normalized)
        .or_else(|_| PathBuf::from(normalized).canonicalize())
        .with_context(|| format!("resolve directory `{normalized}`"))?;
    let metadata = fs::metadata(&resolved)
        .with_context(|| format!("stat directory `{}`", resolved.display()))?;
    if !metadata.is_dir() {
        return Err(anyhow!("selected path is not a directory"));
    }

    let mut items = fs::read_dir(&resolved)
        .with_context(|| format!("read directory `{}`", resolved.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = entry.metadata().ok()?;
            if !metadata.is_dir() {
                return None;
            }
            Some(DirectoryEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                path: path.display().to_string(),
            })
        })
        .collect::<Vec<_>>();
    items.sort_by_key(|entry| entry.name.to_ascii_lowercase());

    let parent = resolved.parent().and_then(|path| {
        let current = resolved.display().to_string();
        let candidate = path.display().to_string();
        if candidate == current { None } else { Some(candidate) }
    });

    Ok(DirectoryBrowseResultMessage {
        r#type: "directory-browse-result".into(),
        request_id: String::new(),
        ok: true,
        message: String::new(),
        current_path: resolved.display().to_string(),
        parent_path: parent.unwrap_or_default(),
        items,
    })
}

impl ShellSessionManager {
    pub fn new(default_shell_type: ShellType, outbound: UnboundedSender<String>) -> Self {
        Self {
            state: Arc::new(ShellSessionManagerState {
                default_shell_type,
                outbound,
                sessions: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn start_session(
        &self,
        session_id: String,
        requested_shell: ShellType,
        working_directory: String,
    ) {
        if self.state.sessions.lock().expect("sessions").contains_key(&session_id) {
            self.emit_session_error(&session_id, "Session already exists.");
            return;
        }

        let launch = match resolve_shell_launch(requested_shell, self.state.default_shell_type) {
            Ok(value) => value,
            Err(err) => {
                self.emit_session_error(&session_id, &err.to_string());
                return;
            }
        };

        let (cwd, warning) = resolve_working_directory(&working_directory);
        let pty_system = native_pty_system();
        let pair = match pty_system.openpty(PtySize {
            rows: 30,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            Ok(value) => value,
            Err(err) => {
                self.emit_session_error(&session_id, &err.to_string());
                return;
            }
        };

        let mut builder = CommandBuilder::new(&launch.executable);
        builder.args(&launch.args);
        builder.cwd(&cwd);
        for (key, value) in build_shell_env() {
            builder.env(key, value);
        }

        tracing::info!(
            session_id = session_id,
            shell = launch.shell_type.as_str(),
            executable = launch.executable,
            cwd = %cwd.display(),
            "starting shell session"
        );

        let mut child = match pair.slave.spawn_command(builder) {
            Ok(value) => value,
            Err(err) => {
                self.emit_session_error(&session_id, &err.to_string());
                return;
            }
        };

        let reader = match pair.master.try_clone_reader() {
            Ok(value) => value,
            Err(err) => {
                let _ = child.kill();
                self.emit_session_error(&session_id, &err.to_string());
                return;
            }
        };

        let writer = match pair.master.take_writer() {
            Ok(value) => value,
            Err(err) => {
                let _ = child.kill();
                self.emit_session_error(&session_id, &err.to_string());
                return;
            }
        };

        let session = Arc::new(ShellSession {
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            child: Mutex::new(child),
            closed: AtomicBool::new(false),
        });

        self.state
            .sessions
            .lock()
            .expect("sessions")
            .insert(session_id.clone(), Arc::clone(&session));

        if let Some(message) = warning {
            self.emit_session_output(&session_id, "stderr", format!("{message}\n"));
        }
        self.emit_ready(&session_id);
        self.spawn_output_pump(session_id.clone(), Arc::clone(&session), reader);
        self.spawn_waiter(session_id, session);
    }

    pub fn write_input(&self, session_id: &str, data: &str) {
        let Some(session) = self.state.sessions.lock().expect("sessions").get(session_id).cloned()
        else {
            return;
        };

        let mut writer = session.writer.lock().expect("writer");
        if let Err(err) = writer.write_all(data.as_bytes()) {
            self.emit_session_error(session_id, &err.to_string());
            return;
        }
        let _ = writer.flush();
    }

    pub fn resize_session(&self, session_id: &str, cols: i32, rows: i32) {
        let Some(session) = self.state.sessions.lock().expect("sessions").get(session_id).cloned()
        else {
            return;
        };

        let cols = cols.max(1) as u16;
        let rows = rows.max(1) as u16;
        let master = session.master.lock().expect("master");
        if let Err(err) = master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }) {
            self.emit_session_error(session_id, &err.to_string());
        }
    }

    pub fn stop_session(&self, session_id: &str) {
        let Some(session) = self.state.sessions.lock().expect("sessions").get(session_id).cloned()
        else {
            return;
        };
        session.shutdown();
    }

    pub fn stop_all(&self) {
        let sessions =
            self.state.sessions.lock().expect("sessions").values().cloned().collect::<Vec<_>>();
        for session in sessions {
            session.shutdown();
        }
    }

    fn spawn_output_pump(
        &self,
        session_id: String,
        session: Arc<ShellSession>,
        mut reader: Box<dyn Read + Send>,
    ) {
        let manager = self.clone();
        thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => return,
                    Ok(size) => {
                        let data = String::from_utf8_lossy(&buffer[..size]).to_string();
                        manager.emit_session_output(&session_id, "stdout", data);
                    }
                    Err(err) => {
                        if !session.closed.load(Ordering::SeqCst) {
                            manager.emit_session_error(&session_id, &err.to_string());
                        }
                        return;
                    }
                }
            }
        });
    }

    fn spawn_waiter(&self, session_id: String, session: Arc<ShellSession>) {
        let manager = self.clone();
        thread::spawn(move || {
            let wait_result = session.child.lock().expect("child").wait();
            session.closed.store(true, Ordering::SeqCst);
            manager.state.sessions.lock().expect("sessions").remove(&session_id);
            match wait_result {
                Ok(status) => {
                    manager.emit_exit(&session_id, Some(status.exit_code() as i32));
                }
                Err(err) => {
                    manager.emit_session_error(&session_id, &err.to_string());
                    manager.emit_exit(&session_id, None);
                }
            }
        });
    }

    fn emit_ready(&self, session_id: &str) {
        self.send_json(&SessionReadyMessage {
            r#type: "session-ready".into(),
            session_id: session_id.to_owned(),
        });
    }

    fn emit_session_output(&self, session_id: &str, stream: &str, data: String) {
        self.send_json(&SessionOutputMessage {
            r#type: "session-output".into(),
            session_id: session_id.to_owned(),
            stream: stream.to_owned(),
            data,
        });
    }

    fn emit_session_error(&self, session_id: &str, message: &str) {
        self.send_json(&SessionErrorMessage {
            r#type: "session-error".into(),
            session_id: session_id.to_owned(),
            message: message.to_owned(),
        });
    }

    fn emit_exit(&self, session_id: &str, exit_code: Option<i32>) {
        self.send_json(&SessionExitMessage {
            r#type: "session-exit".into(),
            session_id: session_id.to_owned(),
            exit_code,
        });
    }

    fn send_json<T: serde::Serialize>(&self, payload: &T) {
        match serde_json::to_string(payload) {
            Ok(json) => {
                if self.state.outbound.send(json).is_err() {
                    warn!("session outbound channel closed");
                }
            }
            Err(err) => warn!(error = %err, "failed to serialize session payload"),
        }
    }
}

impl Clone for ShellSessionManager {
    fn clone(&self) -> Self {
        Self { state: Arc::clone(&self.state) }
    }
}

impl ShellSession {
    fn shutdown(&self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

fn build_shell_env() -> Vec<(String, String)> {
    let mut env = std::env::vars().collect::<Vec<_>>();

    #[cfg(target_os = "macos")]
    merge_shell_env(&mut env);

    env.push(("TERM".into(), env_or_default("TERM", "xterm-256color")));
    env.push(("COLORTERM".into(), env_or_default("COLORTERM", "truecolor")));
    env.push(("TERM_PROGRAM".into(), env_or_default("TERM_PROGRAM", "remote-terminal-cloud")));
    env.push(("TERM_PROGRAM_VERSION".into(), env_or_default("TERM_PROGRAM_VERSION", "agent")));
    if cfg!(target_os = "windows") {
        env.push(("ConEmuANSI".into(), env_or_default("ConEmuANSI", "ON")));
    }
    env
}

#[cfg(target_os = "macos")]
fn merge_shell_env(env: &mut Vec<(String, String)>) {
    let shell_env = macos_user_shell_env();
    for (key, value) in shell_env {
        if let Some(pos) = env.iter().position(|(k, _)| k == key) {
            env[pos] = (key.clone(), value.clone());
        } else {
            env.push((key.clone(), value.clone()));
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_user_shell_env() -> &'static [(String, String)] {
    static SHELL_ENV: LazyLock<Vec<(String, String)>> = LazyLock::new(load_macos_user_shell_env);
    &SHELL_ENV
}

#[cfg(target_os = "macos")]
fn load_macos_user_shell_env() -> Vec<(String, String)> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let Ok(output) = std::process::Command::new(&shell)
        .args(["-l", "-c", "env"])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let Ok(stdout) = String::from_utf8(output.stdout) else {
        return Vec::new();
    };

    let skip_keys =
        ["SHLVL", "ZSH_EVAL_CONTEXT", "BASH_REMATCH", "BASH_SUBSHELL", "_", "PWD", "OLDPWD"];
    let mut result = Vec::new();
    for line in stdout.lines() {
        if let Some(pos) = line.find('=') {
            let key = &line[..pos];
            if !key.is_empty() && !skip_keys.contains(&key) {
                result.push((key.to_string(), line[pos + 1..].to_string()));
            }
        }
    }
    result
}

fn env_or_default(key: &str, fallback: &str) -> String {
    let value = std::env::var(key).unwrap_or_default();
    let trimmed = value.trim();
    if trimmed.is_empty() { fallback.to_owned() } else { trimmed.to_owned() }
}

fn resolve_working_directory(working_directory: &str) -> (PathBuf, Option<String>) {
    let normalized = working_directory.trim();
    if normalized.is_empty() {
        return (current_dir_or_dot(), None);
    }

    match fs::metadata(normalized) {
        Ok(metadata) if metadata.is_dir() => (PathBuf::from(normalized), None),
        Ok(_) => (
            current_dir_or_dot(),
            Some(format!(
                "Unable to use working directory \"{normalized}\", fallback to default directory: selected path is not a directory."
            )),
        ),
        Err(err) => (
            current_dir_or_dot(),
            Some(format!(
                "Unable to use working directory \"{normalized}\", fallback to default directory: {err}"
            )),
        ),
    }
}

fn current_dir_or_dot() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn resolve_shell_launch(requested: ShellType, default_shell: ShellType) -> Result<ShellLaunch> {
    let normalized = if requested == ShellType::SystemDefault { default_shell } else { requested };

    #[cfg(target_os = "windows")]
    {
        return match normalized {
            ShellType::SystemDefault | ShellType::Cmd => {
                let executable = std::env::var("ComSpec")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "cmd.exe".into());
                Ok(ShellLaunch {
                    executable,
                    args: vec!["/d".into(), "/k".into(), "chcp 65001>nul".into()],
                    shell_type: ShellType::Cmd,
                })
            }
            ShellType::Powershell => Ok(ShellLaunch {
                executable: "powershell.exe".into(),
                args: vec![
                    "-NoLogo".into(),
                    "-NoExit".into(),
                    "-Command".into(),
                    "[Console]::InputEncoding=[System.Text.UTF8Encoding]::new($false); [Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); $OutputEncoding=[Console]::OutputEncoding; chcp 65001 > $null".into(),
                ],
                shell_type: ShellType::Powershell,
            }),
            ShellType::Pwsh => Ok(ShellLaunch {
                executable: "pwsh.exe".into(),
                args: vec![
                    "-NoLogo".into(),
                    "-NoExit".into(),
                    "-Command".into(),
                    "[Console]::InputEncoding=[System.Text.UTF8Encoding]::new($false); [Console]::OutputEncoding=[System.Text.UTF8Encoding]::new($false); $OutputEncoding=[Console]::OutputEncoding; chcp 65001 > $null".into(),
                ],
                shell_type: ShellType::Pwsh,
            }),
            _ => Err(anyhow!("shell is not supported on Windows")),
        };
    }

    #[cfg(not(target_os = "windows"))]
    {
        return match normalized {
            ShellType::SystemDefault => {
                let executable = std::env::var("SHELL")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "/bin/bash".into());
                Ok(ShellLaunch {
                    executable,
                    args: vec!["-i".into()],
                    shell_type: ShellType::SystemDefault,
                })
            }
            ShellType::Bash | ShellType::Zsh | ShellType::Sh => Ok(ShellLaunch {
                executable: normalized.as_str().into(),
                args: vec!["-i".into()],
                shell_type: normalized,
            }),
            ShellType::Pwsh => Ok(ShellLaunch {
                executable: "pwsh".into(),
                args: vec!["-NoLogo".into()],
                shell_type: ShellType::Pwsh,
            }),
            _ => Err(anyhow!("shell is not supported on this platform")),
        };
    }
}

fn root_browse_result() -> Result<DirectoryBrowseResultMessage> {
    #[cfg(not(target_os = "windows"))]
    {
        let mut items = fs::read_dir(Path::new("/"))
            .context("read directory `/`")?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                let metadata = entry.metadata().ok()?;
                if !metadata.is_dir() {
                    return None;
                }
                Some(DirectoryEntry {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path: path.display().to_string(),
                })
            })
            .collect::<Vec<_>>();
        items.sort_by_key(|entry| entry.name.to_ascii_lowercase());

        return Ok(DirectoryBrowseResultMessage {
            r#type: "directory-browse-result".into(),
            request_id: String::new(),
            ok: true,
            message: String::new(),
            current_path: "/".into(),
            parent_path: String::new(),
            items,
        });
    }

    #[cfg(target_os = "windows")]
    {
        let mut items = Vec::new();
        for drive in b'A'..=b'Z' {
            let root = format!("{}:\\", drive as char);
            if Path::new(&root).exists() {
                items.push(DirectoryEntry {
                    name: root.trim_end_matches('\\').to_owned(),
                    path: root,
                });
            }
        }

        return Ok(DirectoryBrowseResultMessage {
            r#type: "directory-browse-result".into(),
            request_id: String::new(),
            ok: true,
            message: String::new(),
            current_path: String::new(),
            parent_path: String::new(),
            items,
        });
    }
}

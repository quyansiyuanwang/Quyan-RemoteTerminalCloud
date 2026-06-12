use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use rtc_agent_preferences::PreferencesStore;
use rtc_agent_protocol::{
    AgentHeartbeatRequest, AgentHeartbeatResponse, AgentRegistrationRequest,
    AgentRegistrationResponse, HostSnapshot, PreferencesGetMessage, PreferencesResultMessage,
    PreferencesSetMessage, RemoteTerminalAgentPreferencesData,
};
use rtc_agent_runtime::{ApiClient, run_agent_tunnel};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, oneshot};
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[tokio::test]
async fn runtime_can_register_heartbeat_and_sync_preferences_over_websocket() -> Result<()> {
    let preference_file = unique_preferences_file();
    let preferences_store = PreferencesStore::new(&preference_file);
    let _ = preferences_store.set_preferences(RemoteTerminalAgentPreferencesData {
        default_working_directory: "D:/Initial".into(),
        shortcuts: Vec::new(),
        quick_commands: Vec::new(),
    })?;

    let requests_seen = Arc::new(AtomicUsize::new(0));
    let (server_ready_tx, server_ready_rx) = oneshot::channel::<ServerReady>();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let state = Arc::new(ServerState {
        requests_seen: Arc::clone(&requests_seen),
        shutdown: Mutex::new(Some(shutdown_tx)),
    });

    tokio::spawn(run_mock_backend(Arc::clone(&state), server_ready_tx));
    let server = server_ready_rx.await.context("wait server ready")?;
    let server_base_url = format!("http://{}", server.http_addr);

    let api_client = ApiClient::with_http(
        Client::builder().timeout(std::time::Duration::from_secs(15)).no_proxy().build()?,
    );

    let session =
        api_client.register_agent(&server_base_url, "demo-token", sample_snapshot()).await?;
    assert_eq!(session.device_id, "device-123");
    assert_eq!(session.heartbeat_interval_seconds, 1);

    let session = api_client.send_heartbeat(&server_base_url, &session, sample_snapshot()).await?;
    assert_eq!(session.heartbeat_interval_seconds, 2);

    let tunnel_base_url = server_base_url.clone();
    let tunnel_preferences_file = preference_file.clone();
    let tunnel_task = tokio::spawn(async move {
        run_agent_tunnel(
            &tunnel_base_url,
            session,
            rtc_agent_protocol::ShellType::SystemDefault,
            &tunnel_preferences_file,
        )
        .await
    });

    shutdown_rx.await.context("wait websocket assertions")?;
    tunnel_task.await.context("join tunnel task")??;

    let saved = PreferencesStore::new(&preference_file).get_preferences();
    assert_eq!(saved.default_working_directory, "D:/Updated");
    assert!(requests_seen.load(Ordering::SeqCst) >= 2);
    Ok(())
}

struct ServerState {
    requests_seen: Arc<AtomicUsize>,
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
}

#[derive(Clone, Copy)]
struct ServerReady {
    http_addr: SocketAddr,
}

async fn run_mock_backend(
    state: Arc<ServerState>,
    ready: oneshot::Sender<ServerReady>,
) -> Result<()> {
    let http_listener = TcpListener::bind("127.0.0.1:0").await?;
    let ws_listener = TcpListener::bind("127.0.0.1:0").await?;
    let http_addr = http_listener.local_addr()?;
    let ws_addr = ws_listener.local_addr()?;
    let _ = ready.send(ServerReady { http_addr });

    let ws_state = Arc::clone(&state);
    tokio::spawn(async move {
        loop {
            let (stream, _) = match ws_listener.accept().await {
                Ok(value) => value,
                Err(_) => return,
            };
            let state = Arc::clone(&ws_state);
            tokio::spawn(async move {
                let _ = handle_websocket(state, stream).await;
            });
        }
    });

    loop {
        let (stream, _) = http_listener.accept().await?;
        let state = Arc::clone(&state);
        let ws_addr = ws_addr;
        tokio::spawn(async move {
            let _ = handle_http_connection(state, stream, ws_addr).await;
        });
    }
}

async fn handle_http_connection(
    state: Arc<ServerState>,
    mut stream: tokio::net::TcpStream,
    ws_addr: SocketAddr,
) -> Result<()> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 2048];
    let header_end = loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(());
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
        if buffer.len() > 64 * 1024 {
            bail!("request headers too large");
        }
    };

    let header_text = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let request_line = header_text.lines().next().unwrap_or_default().to_owned();
    let headers = parse_headers(&header_text);
    let content_length =
        headers.get("content-length").and_then(|value| value.parse::<usize>().ok()).unwrap_or(0);

    let mut body = buffer[header_end + 4..].to_vec();
    while body.len() < content_length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    state.requests_seen.fetch_add(1, Ordering::SeqCst);

    let (status_line, payload) = if request_line.starts_with("POST /remote-terminal/agent/register")
    {
        let _: AgentRegistrationRequest = serde_json::from_slice(&body)?;
        (
            "HTTP/1.1 200 OK",
            serde_json::to_vec(&AgentRegistrationResponse {
                device_id: "device-123".into(),
                heartbeat_interval_seconds: 1,
                heartbeat_token: "heartbeat-abc".into(),
                websocket_url: format!("ws://{}/remote-terminal/ws", ws_addr),
                accepted_at: "2026-06-11T00:00:00Z".into(),
            })?,
        )
    } else if request_line.starts_with("POST /remote-terminal/agent/heartbeat") {
        let _: AgentHeartbeatRequest = serde_json::from_slice(&body)?;
        (
            "HTTP/1.1 200 OK",
            serde_json::to_vec(&AgentHeartbeatResponse {
                ok: true,
                next_heartbeat_interval_seconds: 2,
                websocket_url: String::new(),
                server_time: "2026-06-11T00:00:01Z".into(),
            })?,
        )
    } else {
        ("HTTP/1.1 404 Not Found", br#"{"error":"not found"}"#.to_vec())
    };

    let response = format!(
        "{status_line}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        payload.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.write_all(&payload).await?;
    stream.flush().await?;
    Ok(())
}

async fn handle_websocket(state: Arc<ServerState>, stream: tokio::net::TcpStream) -> Result<()> {
    let mut websocket = accept_async(stream).await?;
    websocket
        .send(Message::Text(
            serde_json::to_string(&PreferencesGetMessage {
                r#type: "preferences-get".into(),
                request_id: "req-get".into(),
            })?
            .into(),
        ))
        .await?;

    let first = websocket.next().await.context("preferences result")??;
    let first_text = message_text(first)?;
    let first_result: PreferencesResultMessage = serde_json::from_str(&first_text)?;
    assert_eq!(first_result.request_id, "req-get");
    assert!(first_result.ok);
    assert_eq!(first_result.preferences.default_working_directory, "D:/Initial");

    websocket
        .send(Message::Text(
            serde_json::to_string(&PreferencesSetMessage {
                r#type: "preferences-set".into(),
                request_id: "req-set".into(),
                preferences: RemoteTerminalAgentPreferencesData {
                    default_working_directory: "D:/Updated".into(),
                    shortcuts: Vec::new(),
                    quick_commands: Vec::new(),
                },
            })?
            .into(),
        ))
        .await?;

    let second = websocket.next().await.context("preferences set result")??;
    let second_text = message_text(second)?;
    let second_result: PreferencesResultMessage = serde_json::from_str(&second_text)?;
    assert_eq!(second_result.request_id, "req-set");
    assert!(second_result.ok);
    assert_eq!(second_result.preferences.default_working_directory, "D:/Updated");

    websocket.close(None).await?;
    if let Some(shutdown) = state.shutdown.lock().await.take() {
        let _ = shutdown.send(());
    }
    Ok(())
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_headers(headers: &str) -> HashMap<String, String> {
    headers
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        })
        .collect()
}

fn sample_snapshot() -> HostSnapshot {
    serde_json::from_value(json!({
        "hostname": "mock-host",
        "platform": "windows",
        "arch": "x86_64",
        "agentVersion": "0.3.2",
        "capabilities": {
            "sshForward": true,
            "nativePty": true,
            "selfUpdate": true,
            "proxyAware": true,
            "serviceManaged": true,
            "sessionRecording": false
        },
        "diagnostics": {
            "installFormats": ["exe"],
            "serviceManager": "Windows Service",
            "defaultLogPath": "C:/ProgramData/RemoteTerminalCloudAgent/logs",
            "availableShells": ["system-default", "powershell"],
            "sshCheck": {
                "available": true,
                "detail": "mock"
            },
            "notes": []
        }
    }))
    .expect("snapshot json")
}

fn unique_preferences_file() -> PathBuf {
    let unique = format!(
        "rtc-agent-runtime-preferences-{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    );
    std::env::temp_dir().join(unique)
}

fn message_text(message: Message) -> Result<String> {
    match message {
        Message::Text(text) => Ok(text.to_string()),
        Message::Binary(data) => Ok(String::from_utf8(data.to_vec())?),
        other => Err(anyhow::anyhow!("unexpected websocket message: {other:?}")),
    }
}

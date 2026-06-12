mod shell;

use std::path::Path;

use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use rtc_agent_preferences::PreferencesStore;
use rtc_agent_protocol::{
    AgentHeartbeatRequest, AgentHeartbeatResponse, AgentRegistrationRequest,
    AgentRegistrationResponse, DirectoryBrowseRequestMessage, DirectoryBrowseResultMessage,
    HostSnapshot, PreferencesGetMessage, PreferencesResultMessage, PreferencesSetMessage,
    SessionInputMessage, SessionResizeMessage, SessionStartMessage, SessionStopMessage, ShellType,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn};
use url::Url;

use crate::shell::{ShellSessionManager, browse_directories};

#[derive(Debug, Clone, Serialize)]
pub struct RegisteredAgentSession {
    pub device_id: String,
    pub heartbeat_token: String,
    pub heartbeat_interval_seconds: i32,
    pub websocket_url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ApiErrorKind {
    InvalidToken,
    DeviceLimitReached,
    GatewayUnavailable,
    ProxyConfiguration,
    WebsocketUnavailable,
    Unauthorized,
    ServerRejected,
    Transport,
    Unexpected,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorDetails {
    pub kind: ApiErrorKind,
    pub status: Option<u16>,
    pub code: Option<i64>,
    pub message: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Error, Serialize)]
#[error("{message}")]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub kind: ApiErrorKind,
    pub status: Option<u16>,
    pub code: Option<i64>,
    pub message: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackendErrorBody {
    code: Option<i64>,
    message: Option<String>,
}

#[derive(Clone)]
pub struct ApiClient {
    http: Client,
}

impl Default for ApiClient {
    fn default() -> Self {
        Self::with_http(
            Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("reqwest client"),
        )
    }
}

impl ApiClient {
    pub fn with_http(http: Client) -> Self {
        Self { http }
    }

    pub async fn register_agent(
        &self,
        server_base_url: &str,
        registration_token: &str,
        snapshot: HostSnapshot,
    ) -> Result<RegisteredAgentSession> {
        let url =
            format!("{}/remote-terminal/agent/register", server_base_url.trim_end_matches('/'));
        let response = self
            .http
            .post(url)
            .json(&AgentRegistrationRequest {
                registration_token: registration_token.to_owned(),
                snapshot,
            })
            .send()
            .await
            .map_err(|err| map_transport_error("register request", err))?;

        let status = response.status();
        if !response.status().is_success() {
            return Err(decode_api_error("register request", status.as_u16(), response)
                .await
                .into());
        }

        let payload: AgentRegistrationResponse = response.json().await?;
        Ok(RegisteredAgentSession {
            device_id: payload.device_id,
            heartbeat_token: payload.heartbeat_token,
            heartbeat_interval_seconds: payload.heartbeat_interval_seconds,
            websocket_url: payload.websocket_url.trim().to_owned(),
        })
    }

    pub async fn send_heartbeat(
        &self,
        server_base_url: &str,
        session: &RegisteredAgentSession,
        snapshot: HostSnapshot,
    ) -> Result<RegisteredAgentSession> {
        let url =
            format!("{}/remote-terminal/agent/heartbeat", server_base_url.trim_end_matches('/'));
        let response = self
            .http
            .post(url)
            .json(&AgentHeartbeatRequest {
                device_id: session.device_id.clone(),
                heartbeat_token: session.heartbeat_token.clone(),
                snapshot,
            })
            .send()
            .await
            .map_err(|err| map_transport_error("heartbeat request", err))?;

        let status = response.status();
        if !response.status().is_success() {
            return Err(decode_api_error("heartbeat request", status.as_u16(), response)
                .await
                .into());
        }

        let payload: AgentHeartbeatResponse = response.json().await?;
        let mut next = session.clone();
        next.heartbeat_interval_seconds = payload.next_heartbeat_interval_seconds;
        if !payload.websocket_url.trim().is_empty() {
            next.websocket_url = payload.websocket_url.trim().to_owned();
        }
        Ok(next)
    }
}

pub async fn verify_websocket_connectivity(
    server_base_url: &str,
    session: &RegisteredAgentSession,
) -> Result<String> {
    let (stream, url) = connect_agent_websocket(server_base_url, session).await?;
    drop(stream);
    info!("websocket endpoint verified: {}", url);
    Ok(url)
}

pub async fn run_agent_tunnel(
    server_base_url: &str,
    session: RegisteredAgentSession,
    default_shell_type: ShellType,
    preferences_file_path: &Path,
) -> Result<()> {
    run_agent_tunnel_with_connect_hook(
        server_base_url,
        session,
        default_shell_type,
        preferences_file_path,
        |_| {},
    )
    .await
}

pub async fn run_agent_tunnel_with_connect_hook<F>(
    server_base_url: &str,
    session: RegisteredAgentSession,
    default_shell_type: ShellType,
    preferences_file_path: &Path,
    on_connected: F,
) -> Result<()>
where
    F: FnOnce(&str),
{
    let (websocket, websocket_url) = connect_agent_websocket(server_base_url, &session).await?;
    info!("tunnel connected for {}", session.device_id);
    info!("websocket endpoint: {}", websocket_url);
    on_connected(&websocket_url);

    let (mut write_half, mut read_half) = websocket.split();
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<String>();
    let shell_manager = ShellSessionManager::new(default_shell_type, outbound_tx.clone());
    let preferences_store = PreferencesStore::new(preferences_file_path);

    let result: Result<()> = async {
        loop {
            tokio::select! {
                outbound = outbound_rx.recv() => {
                    let Some(payload) = outbound else {
                        bail!("websocket outbound channel closed");
                    };
                    write_half.send(Message::Text(payload.into())).await.context("send websocket message")?;
                }
                incoming = read_half.next() => {
                    let Some(frame) = incoming else {
                        bail!("websocket stream ended without a close frame");
                    };
                    let frame = frame.context("receive websocket message")?;
                    match frame {
                        Message::Text(text) => {
                            handle_server_message(&text, &shell_manager, &preferences_store, &outbound_tx)?;
                        }
                        Message::Binary(data) => {
                            let text = String::from_utf8_lossy(&data).to_string();
                            handle_server_message(&text, &shell_manager, &preferences_store, &outbound_tx)?;
                        }
                        Message::Close(Some(frame)) => {
                            bail!(
                                "websocket closed by server (code {}, reason `{}`)",
                                frame.code,
                                frame.reason
                            );
                        }
                        Message::Close(None) => {
                            bail!("websocket closed by server");
                        }
                        Message::Ping(payload) => {
                            write_half.send(Message::Pong(payload)).await.context("respond to ping")?;
                        }
                        Message::Pong(_) => {}
                        Message::Frame(_) => {}
                    }
                }
            }
        }
        #[allow(unreachable_code)]
        Ok(())
    }
    .await;

    shell_manager.stop_all();
    result
}

pub fn build_websocket_url(
    server_base_url: &str,
    session: &RegisteredAgentSession,
) -> Result<String> {
    let mut parsed = Url::parse(server_base_url)?;
    parsed
        .set_scheme(match parsed.scheme() {
            "http" => "ws",
            "https" => "wss",
            "ws" => "ws",
            "wss" => "wss",
            other => bail!("unsupported server base URL scheme: {}", other),
        })
        .map_err(|_| anyhow::anyhow!("invalid websocket scheme"))?;
    parsed.set_path("/remote-terminal/ws");
    parsed
        .query_pairs_mut()
        .append_pair("role", "agent")
        .append_pair("deviceId", &session.device_id)
        .append_pair("heartbeatToken", &session.heartbeat_token);
    Ok(parsed.to_string())
}

async fn connect_agent_websocket(
    server_base_url: &str,
    session: &RegisteredAgentSession,
) -> Result<(
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    String,
)> {
    let websocket_url = if session.websocket_url.trim().is_empty() {
        build_websocket_url(server_base_url, session)?
    } else {
        session.websocket_url.clone()
    };
    let (stream, _response) = connect_async(websocket_url.as_str())
        .await
        .map_err(|_err| {
            ApiError {
                kind: ApiErrorKind::WebsocketUnavailable,
                status: None,
                code: None,
                message: format!("Unable to connect to websocket endpoint `{websocket_url}`."),
                suggestion: "Check whether the server is reachable, whether your proxy allows websocket traffic, and whether the backend returned a usable websocket URL.".into(),
            }
        })?;
    Ok((stream, websocket_url))
}

fn handle_server_message(
    payload: &str,
    shell_manager: &ShellSessionManager,
    preferences_store: &PreferencesStore,
    outbound_tx: &mpsc::UnboundedSender<String>,
) -> Result<()> {
    let raw: serde_json::Value =
        serde_json::from_str(payload).context("decode websocket payload")?;
    let message_type = raw.get("type").and_then(|value| value.as_str()).unwrap_or_default();

    match message_type {
        "session-start" => {
            let message: SessionStartMessage =
                serde_json::from_value(raw).context("decode session-start")?;
            shell_manager.start_session(
                message.session_id,
                message.shell_type,
                message.working_directory,
            );
        }
        "session-input" => {
            let message: SessionInputMessage =
                serde_json::from_value(raw).context("decode session-input")?;
            shell_manager.write_input(&message.session_id, &message.data);
        }
        "session-resize" => {
            let message: SessionResizeMessage =
                serde_json::from_value(raw).context("decode session-resize")?;
            shell_manager.resize_session(&message.session_id, message.cols, message.rows);
        }
        "session-stop" => {
            let message: SessionStopMessage =
                serde_json::from_value(raw).context("decode session-stop")?;
            shell_manager.stop_session(&message.session_id);
        }
        "directory-browse" => {
            let message: DirectoryBrowseRequestMessage =
                serde_json::from_value(raw).context("decode directory-browse")?;
            let mut result = match browse_directories(&message.path) {
                Ok(result) => result,
                Err(err) => DirectoryBrowseResultMessage {
                    r#type: "directory-browse-result".into(),
                    request_id: String::new(),
                    ok: false,
                    message: err.to_string(),
                    current_path: message.path.trim().to_owned(),
                    parent_path: String::new(),
                    items: Vec::new(),
                },
            };
            result.request_id = message.request_id;
            send_json(outbound_tx, &result)?;
        }
        "preferences-get" => {
            let message: PreferencesGetMessage =
                serde_json::from_value(raw).context("decode preferences-get")?;
            send_json(
                outbound_tx,
                &PreferencesResultMessage {
                    r#type: "preferences-result".into(),
                    request_id: message.request_id,
                    ok: true,
                    message: String::new(),
                    preferences: preferences_store.get_preferences(),
                },
            )?;
        }
        "preferences-set" => {
            let message: PreferencesSetMessage =
                serde_json::from_value(raw).context("decode preferences-set")?;
            let result = match preferences_store.set_preferences(message.preferences) {
                Ok(preferences) => PreferencesResultMessage {
                    r#type: "preferences-result".into(),
                    request_id: message.request_id,
                    ok: true,
                    message: String::new(),
                    preferences,
                },
                Err(err) => PreferencesResultMessage {
                    r#type: "preferences-result".into(),
                    request_id: message.request_id,
                    ok: false,
                    message: err.to_string(),
                    preferences: preferences_store.get_preferences(),
                },
            };
            send_json(outbound_tx, &result)?;
        }
        "" => warn!("received websocket payload without type"),
        other => warn!(message_type = other, "unsupported websocket message"),
    }

    Ok(())
}

fn send_json<T: Serialize>(outbound_tx: &mpsc::UnboundedSender<String>, payload: &T) -> Result<()> {
    let encoded = serde_json::to_string(payload)?;
    outbound_tx.send(encoded).map_err(|_| anyhow::anyhow!("websocket outbound channel closed"))
}

pub fn describe_error(error: &anyhow::Error) -> Option<ApiErrorDetails> {
    error.downcast_ref::<ApiError>().map(|err| ApiErrorDetails {
        kind: err.kind.clone(),
        status: err.status,
        code: err.code,
        message: err.message.clone(),
        suggestion: err.suggestion.clone(),
    })
}

fn map_transport_error(context: &str, err: reqwest::Error) -> anyhow::Error {
    let message = if err.is_timeout() {
        format!("{context} timed out while contacting the backend.")
    } else if err.is_connect() {
        format!("{context} could not reach the backend.")
    } else if err.is_request() {
        format!("{context} could not be sent to the backend.")
    } else {
        format!("{context} failed before the backend returned a response.")
    };

    ApiError {
        kind: classify_transport_error(&err),
        status: err.status().map(|status| status.as_u16()),
        code: None,
        message,
        suggestion: transport_suggestion(&err),
    }
    .into()
}

async fn decode_api_error(context: &str, status: u16, response: reqwest::Response) -> ApiError {
    let body_text = response.text().await.unwrap_or_default();
    let parsed = parse_backend_error_body(&body_text);
    let backend_message = parsed
        .message
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback_backend_message(status, &body_text));
    let kind = classify_backend_error(status, parsed.code, &backend_message);
    let message = match kind {
        ApiErrorKind::InvalidToken => "Registration token is invalid or has expired.".to_owned(),
        ApiErrorKind::DeviceLimitReached => {
            "Registration token is valid, but the entitlement has reached its device limit."
                .to_owned()
        }
        ApiErrorKind::GatewayUnavailable => {
            "Backend gateway is temporarily unavailable.".to_owned()
        }
        ApiErrorKind::Unauthorized => {
            "Backend rejected the agent credentials for this request.".to_owned()
        }
        ApiErrorKind::ProxyConfiguration => {
            "A local proxy or gateway rejected the backend request.".to_owned()
        }
        ApiErrorKind::ServerRejected => {
            format!("{context} was rejected by the backend: {backend_message}")
        }
        ApiErrorKind::Transport => {
            format!("{context} failed while contacting the backend: {backend_message}")
        }
        ApiErrorKind::WebsocketUnavailable => {
            "Backend returned a websocket endpoint that could not be used.".to_owned()
        }
        ApiErrorKind::Unexpected => {
            format!("{context} failed unexpectedly: {backend_message}")
        }
    };

    ApiError {
        kind,
        status: Some(status),
        code: parsed.code,
        message,
        suggestion: suggestion_for_kind(status, parsed.code, &backend_message),
    }
}

fn parse_backend_error_body(body_text: &str) -> BackendErrorBody {
    let trimmed = body_text.trim();
    if trimmed.is_empty() {
        return BackendErrorBody { code: None, message: None };
    }

    if let Ok(parsed) = serde_json::from_str::<BackendErrorBody>(trimmed) {
        return parsed;
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return BackendErrorBody {
            code: value.get("code").and_then(Value::as_i64),
            message: value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| value.get("error").and_then(Value::as_str).map(str::to_owned)),
        };
    }

    BackendErrorBody { code: None, message: Some(trimmed.to_owned()) }
}

fn classify_backend_error(status: u16, code: Option<i64>, message: &str) -> ApiErrorKind {
    let normalized = message.to_ascii_lowercase();
    if normalized.contains("invalid or expired registration token")
        || normalized.contains("invalid registration token")
        || normalized.contains("token expired")
    {
        return ApiErrorKind::InvalidToken;
    }
    if normalized.contains("device limit reached") || normalized.contains("entitlement") {
        return ApiErrorKind::DeviceLimitReached;
    }
    if status == 401 || status == 403 {
        return ApiErrorKind::Unauthorized;
    }
    if status == 502 || status == 503 || status == 504 {
        return ApiErrorKind::GatewayUnavailable;
    }
    if status == 407 || normalized.contains("proxy") {
        return ApiErrorKind::ProxyConfiguration;
    }
    if status >= 400 && status < 500 {
        return ApiErrorKind::ServerRejected;
    }
    if status >= 500 {
        return ApiErrorKind::Transport;
    }
    if code == Some(1002) {
        return ApiErrorKind::ServerRejected;
    }
    ApiErrorKind::Unexpected
}

fn suggestion_for_kind(status: u16, _code: Option<i64>, message: &str) -> String {
    match classify_backend_error(status, _code, message) {
        ApiErrorKind::InvalidToken => {
            "Run `rtc-agent configure` and save a fresh token, or set RTC_REGISTRATION_TOKEN to a valid value.".into()
        }
        ApiErrorKind::DeviceLimitReached => {
            "Release an existing device from the entitlement, or increase the device limit before trying again.".into()
        }
        ApiErrorKind::GatewayUnavailable => {
            "Retry in a moment. If you use a proxy, confirm it allows HTTPS and websocket traffic to the backend.".into()
        }
        ApiErrorKind::ProxyConfiguration => {
            "Check HTTP_PROXY / HTTPS_PROXY and confirm the proxy allows requests to the backend.".into()
        }
        ApiErrorKind::Unauthorized => {
            "Verify the backend environment, service entitlement, and token binding for this agent.".into()
        }
        ApiErrorKind::ServerRejected => {
            "Inspect the backend response details and server-side policy for this token or request.".into()
        }
        ApiErrorKind::Transport => {
            "Confirm the backend is online and reachable from this machine, then retry.".into()
        }
        ApiErrorKind::WebsocketUnavailable => {
            "Verify that the backend websocket endpoint is enabled and that your network path does not block websocket upgrades.".into()
        }
        ApiErrorKind::Unexpected => {
            "Retry once. If it keeps failing, capture `rtc-agent verify --json` output for diagnosis.".into()
        }
    }
}

fn classify_transport_error(err: &reqwest::Error) -> ApiErrorKind {
    if err.is_connect() {
        return ApiErrorKind::Transport;
    }
    if err.is_timeout() {
        return ApiErrorKind::Transport;
    }
    let rendered = err.to_string().to_ascii_lowercase();
    if rendered.contains("proxy") {
        return ApiErrorKind::ProxyConfiguration;
    }
    ApiErrorKind::Transport
}

fn transport_suggestion(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        return "The backend did not respond in time. Check connectivity, proxy latency, and server health.".into();
    }
    let rendered = err.to_string().to_ascii_lowercase();
    if rendered.contains("proxy") {
        return "Check HTTP_PROXY / HTTPS_PROXY and confirm the proxy can reach the backend and websocket endpoint.".into();
    }
    "Confirm the backend URL is correct and reachable from this machine, then retry.".into()
}

fn fallback_backend_message(status: u16, body_text: &str) -> String {
    let trimmed = body_text.trim();
    if trimmed.is_empty() {
        return format!("HTTP {status}");
    }
    trimmed.to_owned()
}

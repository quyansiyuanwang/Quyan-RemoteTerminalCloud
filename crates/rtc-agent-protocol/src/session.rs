use serde::{Deserialize, Serialize};

use crate::{DirectoryEntry, ShellType};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartMessage {
    pub r#type: String,
    pub session_id: String,
    pub mode: String,
    pub shell_type: ShellType,
    #[serde(default)]
    pub working_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInputMessage {
    pub r#type: String,
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResizeMessage {
    pub r#type: String,
    pub session_id: String,
    pub cols: i32,
    pub rows: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStopMessage {
    pub r#type: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryBrowseRequestMessage {
    pub r#type: String,
    pub request_id: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryBrowseResultMessage {
    pub r#type: String,
    pub request_id: String,
    pub ok: bool,
    #[serde(default)]
    pub message: String,
    pub current_path: String,
    #[serde(default)]
    pub parent_path: String,
    #[serde(default)]
    pub items: Vec<DirectoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionReadyMessage {
    pub r#type: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOutputMessage {
    pub r#type: String,
    pub session_id: String,
    pub stream: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExitMessage {
    pub r#type: String,
    pub session_id: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionErrorMessage {
    pub r#type: String,
    pub session_id: String,
    pub message: String,
}

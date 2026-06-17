use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RemoteTerminalShortcutModifier {
    Ctrl,
    Alt,
    Shift,
    Meta,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RemoteTerminalShortcutKind {
    Sequence,
    Key,
}

impl Default for RemoteTerminalShortcutKind {
    fn default() -> Self {
        Self::Sequence
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTerminalShortcutData {
    pub id: String,
    pub label: String,
    pub kind: RemoteTerminalShortcutKind,
    #[serde(default)]
    pub sequence: Vec<String>,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub modifiers: Vec<RemoteTerminalShortcutModifier>,
    #[serde(default)]
    pub preset: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteTerminalQuickCommandData {
    pub id: String,
    pub label: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTerminalAgentPreferencesData {
    #[serde(default)]
    pub default_working_directory: String,
    #[serde(default)]
    pub shortcuts: Vec<RemoteTerminalShortcutData>,
    #[serde(default)]
    pub quick_commands: Vec<RemoteTerminalQuickCommandData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesGetMessage {
    pub r#type: String,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesSetMessage {
    pub r#type: String,
    pub request_id: String,
    pub preferences: RemoteTerminalAgentPreferencesData,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesResultMessage {
    pub r#type: String,
    pub request_id: String,
    pub ok: bool,
    #[serde(default)]
    pub message: String,
    pub preferences: RemoteTerminalAgentPreferencesData,
}
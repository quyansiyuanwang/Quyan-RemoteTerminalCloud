use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rtc_agent_protocol::{
    RemoteTerminalAgentPreferencesData, RemoteTerminalQuickCommandData, RemoteTerminalShortcutData,
    RemoteTerminalShortcutKind, RemoteTerminalShortcutModifier,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedPreferencesFile {
    version: i32,
    #[serde(default)]
    default_working_directory: String,
    #[serde(default)]
    shortcuts: Vec<RemoteTerminalShortcutData>,
    #[serde(default)]
    quick_commands: Vec<RemoteTerminalQuickCommandData>,
}

#[derive(Clone)]
pub struct PreferencesStore {
    file_path: PathBuf,
    cache: Arc<Mutex<Option<RemoteTerminalAgentPreferencesData>>>,
}

impl PreferencesStore {
    pub fn new(file_path: impl Into<PathBuf>) -> Self {
        Self { file_path: file_path.into(), cache: Arc::new(Mutex::new(None)) }
    }

    pub fn get_preferences(&self) -> RemoteTerminalAgentPreferencesData {
        let mut cache = self.cache.lock().expect("preferences cache");
        if cache.is_none() {
            *cache = Some(load_preferences(&self.file_path));
        }
        cache.clone().unwrap_or_default()
    }

    pub fn set_preferences(
        &self,
        preferences: RemoteTerminalAgentPreferencesData,
    ) -> Result<RemoteTerminalAgentPreferencesData> {
        let sanitized = sanitize_preferences(preferences);
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        let payload = PersistedPreferencesFile {
            version: 1,
            default_working_directory: sanitized.default_working_directory.clone(),
            shortcuts: sanitized.shortcuts.clone(),
            quick_commands: sanitized.quick_commands.clone(),
        };
        fs::write(&self.file_path, serde_json::to_string_pretty(&payload)?)
            .with_context(|| format!("write {}", self.file_path.display()))?;
        let mut cache = self.cache.lock().expect("preferences cache");
        *cache = Some(sanitized.clone());
        Ok(sanitized)
    }
}

fn load_preferences(path: &Path) -> RemoteTerminalAgentPreferencesData {
    let Ok(content) = fs::read_to_string(path) else {
        return RemoteTerminalAgentPreferencesData::default();
    };
    let Ok(payload) = serde_json::from_str::<PersistedPreferencesFile>(&content) else {
        return RemoteTerminalAgentPreferencesData::default();
    };
    sanitize_preferences(RemoteTerminalAgentPreferencesData {
        default_working_directory: payload.default_working_directory,
        shortcuts: payload.shortcuts,
        quick_commands: payload.quick_commands,
    })
}

fn sanitize_preferences(
    preferences: RemoteTerminalAgentPreferencesData,
) -> RemoteTerminalAgentPreferencesData {
    RemoteTerminalAgentPreferencesData {
        default_working_directory: preferences.default_working_directory.trim().to_owned(),
        shortcuts: sanitize_shortcuts(preferences.shortcuts),
        quick_commands: sanitize_quick_commands(preferences.quick_commands),
    }
}

fn sanitize_shortcuts(items: Vec<RemoteTerminalShortcutData>) -> Vec<RemoteTerminalShortcutData> {
    items
        .into_iter()
        .filter_map(|item| {
            let id = item.id.trim().to_owned();
            if id.is_empty() {
                return None;
            }
            let kind = match item.kind {
                RemoteTerminalShortcutKind::Key => RemoteTerminalShortcutKind::Key,
                _ => RemoteTerminalShortcutKind::Sequence,
            };
            let sequence = item
                .sequence
                .into_iter()
                .filter(|entry| !entry.trim().is_empty())
                .collect::<Vec<_>>();
            let key = item.key.trim().to_owned();
            if matches!(kind, RemoteTerminalShortcutKind::Key) && key.is_empty() {
                return None;
            }
            if matches!(kind, RemoteTerminalShortcutKind::Sequence) && sequence.is_empty() {
                return None;
            }
            let modifiers = item
                .modifiers
                .into_iter()
                .filter(|modifier| {
                    matches!(
                        modifier,
                        RemoteTerminalShortcutModifier::Ctrl
                            | RemoteTerminalShortcutModifier::Alt
                            | RemoteTerminalShortcutModifier::Shift
                            | RemoteTerminalShortcutModifier::Meta
                    )
                })
                .collect::<Vec<_>>();
            Some(RemoteTerminalShortcutData {
                id,
                label: item.label,
                kind,
                sequence,
                key,
                modifiers,
                preset: item.preset,
            })
        })
        .collect()
}

fn sanitize_quick_commands(
    items: Vec<RemoteTerminalQuickCommandData>,
) -> Vec<RemoteTerminalQuickCommandData> {
    items
        .into_iter()
        .filter_map(|item| {
            let id = item.id.trim().to_owned();
            let label = item.label.trim().to_owned();
            let command = item.command.trim().to_owned();
            if id.is_empty() || label.is_empty() || command.is_empty() {
                return None;
            }
            Some(RemoteTerminalQuickCommandData { id, label, command })
        })
        .collect()
}

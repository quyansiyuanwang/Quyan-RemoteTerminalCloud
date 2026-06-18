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

#[cfg(test)]
mod tests {
    use super::*;

    // ── sanitize_shortcuts ──

    #[test]
    fn sanitize_empty_shortcuts() {
        let result = sanitize_shortcuts(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn sanitize_shortcut_empty_id_is_filtered() {
        let items = vec![RemoteTerminalShortcutData {
            id: "".into(),
            label: "test".into(),
            kind: RemoteTerminalShortcutKind::Key,
            sequence: vec![],
            key: "a".into(),
            modifiers: vec![],
            preset: false,
        }];
        assert!(sanitize_shortcuts(items).is_empty());
    }

    #[test]
    fn sanitize_shortcut_whitespace_id_is_filtered() {
        let items = vec![RemoteTerminalShortcutData { id: "  ".into(), ..Default::default() }];
        assert!(sanitize_shortcuts(items).is_empty());
    }

    #[test]
    fn sanitize_shortcut_key_kind_without_key_is_filtered() {
        let items = vec![RemoteTerminalShortcutData {
            id: "s1".into(),
            kind: RemoteTerminalShortcutKind::Key,
            key: "".into(),
            ..Default::default()
        }];
        assert!(sanitize_shortcuts(items).is_empty());
    }

    #[test]
    fn sanitize_shortcut_sequence_kind_without_sequence_is_filtered() {
        let items = vec![RemoteTerminalShortcutData {
            id: "s1".into(),
            kind: RemoteTerminalShortcutKind::Sequence,
            sequence: vec![],
            ..Default::default()
        }];
        assert!(sanitize_shortcuts(items).is_empty());
    }

    #[test]
    fn sanitize_shortcut_valid_key_passes() {
        let items = vec![RemoteTerminalShortcutData {
            id: "s1".into(),
            label: "Copy".into(),
            kind: RemoteTerminalShortcutKind::Key,
            key: "c".into(),
            modifiers: vec![RemoteTerminalShortcutModifier::Ctrl],
            ..Default::default()
        }];
        let result = sanitize_shortcuts(items);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "s1");
        assert_eq!(result[0].modifiers, vec![RemoteTerminalShortcutModifier::Ctrl]);
    }

    #[test]
    fn sanitize_shortcut_valid_sequence_passes() {
        let items = vec![RemoteTerminalShortcutData {
            id: "s1".into(),
            label: "Escape".into(),
            kind: RemoteTerminalShortcutKind::Sequence,
            sequence: vec!["\x1b".into(), "c".into()],
            ..Default::default()
        }];
        let result = sanitize_shortcuts(items);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].sequence.len(), 2);
    }

    #[test]
    fn sanitize_shortcut_filters_unknown_modifiers() {
        // All defined modifiers are valid: Ctrl, Alt, Shift, Meta
        let items = vec![RemoteTerminalShortcutData {
            id: "s1".into(),
            kind: RemoteTerminalShortcutKind::Key,
            key: "x".into(),
            modifiers: vec![
                RemoteTerminalShortcutModifier::Ctrl,
                RemoteTerminalShortcutModifier::Alt,
                RemoteTerminalShortcutModifier::Shift,
                RemoteTerminalShortcutModifier::Meta,
            ],
            ..Default::default()
        }];
        let result = sanitize_shortcuts(items);
        assert_eq!(result[0].modifiers.len(), 4);
    }

    #[test]
    fn sanitize_shortcut_filters_empty_sequence_entries() {
        let items = vec![RemoteTerminalShortcutData {
            id: "s1".into(),
            kind: RemoteTerminalShortcutKind::Sequence,
            sequence: vec!["valid".into(), "".into(), "also-valid".into()],
            ..Default::default()
        }];
        let result = sanitize_shortcuts(items);
        assert_eq!(result[0].sequence.len(), 2);
    }

    #[test]
    fn sanitize_shortcut_trims_id() {
        let items = vec![RemoteTerminalShortcutData {
            id: "  sid-1  ".into(),
            kind: RemoteTerminalShortcutKind::Key,
            key: "a".into(),
            ..Default::default()
        }];
        let result = sanitize_shortcuts(items);
        assert_eq!(result[0].id, "sid-1");
    }

    // ── sanitize_quick_commands ──

    #[test]
    fn sanitize_empty_quick_commands() {
        assert!(sanitize_quick_commands(vec![]).is_empty());
    }

    #[test]
    fn sanitize_quick_command_empty_id_is_filtered() {
        let items = vec![RemoteTerminalQuickCommandData {
            id: "".into(),
            label: "ls".into(),
            command: "ls -la".into(),
        }];
        assert!(sanitize_quick_commands(items).is_empty());
    }

    #[test]
    fn sanitize_quick_command_empty_label_is_filtered() {
        let items = vec![RemoteTerminalQuickCommandData {
            id: "c1".into(),
            label: "".into(),
            command: "ls".into(),
        }];
        assert!(sanitize_quick_commands(items).is_empty());
    }

    #[test]
    fn sanitize_quick_command_empty_command_is_filtered() {
        let items = vec![RemoteTerminalQuickCommandData {
            id: "c1".into(),
            label: "List".into(),
            command: "".into(),
        }];
        assert!(sanitize_quick_commands(items).is_empty());
    }

    #[test]
    fn sanitize_quick_command_whitespace_only_is_filtered() {
        let items = vec![RemoteTerminalQuickCommandData {
            id: "  ".into(),
            label: "  ".into(),
            command: "  ".into(),
        }];
        assert!(sanitize_quick_commands(items).is_empty());
    }

    #[test]
    fn sanitize_quick_command_valid_passes() {
        let items = vec![RemoteTerminalQuickCommandData {
            id: "c1".into(),
            label: "List files".into(),
            command: "ls -la".into(),
        }];
        let result = sanitize_quick_commands(items);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].command, "ls -la");
    }

    #[test]
    fn sanitize_quick_command_trims_all_fields() {
        let items = vec![RemoteTerminalQuickCommandData {
            id: "  c1  ".into(),
            label: "  List  ".into(),
            command: "  ls  ".into(),
        }];
        let result = sanitize_quick_commands(items);
        assert_eq!(result[0].id, "c1");
        assert_eq!(result[0].label, "List");
        assert_eq!(result[0].command, "ls");
    }

    #[test]
    fn sanitize_quick_command_mixed_valid_invalid() {
        let items = vec![
            RemoteTerminalQuickCommandData {
                id: "c1".into(),
                label: "Valid".into(),
                command: "echo ok".into(),
            },
            RemoteTerminalQuickCommandData {
                id: "".into(),
                label: "Invalid".into(),
                command: "echo fail".into(),
            },
            RemoteTerminalQuickCommandData {
                id: "c2".into(),
                label: "".into(),
                command: "echo fail2".into(),
            },
            RemoteTerminalQuickCommandData {
                id: "c3".into(),
                label: "Valid2".into(),
                command: "echo ok2".into(),
            },
        ];
        let result = sanitize_quick_commands(items);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "c1");
        assert_eq!(result[1].id, "c3");
    }

    // ── sanitize_preferences ──

    #[test]
    fn sanitize_preferences_trims_working_directory() {
        let prefs = RemoteTerminalAgentPreferencesData {
            default_working_directory: "  /home/user  ".into(),
            ..Default::default()
        };
        let result = sanitize_preferences(prefs);
        assert_eq!(result.default_working_directory, "/home/user");
    }

    // ── PreferencesStore ──

    #[test]
    fn preferences_store_default_when_file_missing() {
        let store = PreferencesStore::new("/nonexistent/path/prefs.json");
        let prefs = store.get_preferences();
        assert_eq!(prefs.default_working_directory, "");
        assert!(prefs.shortcuts.is_empty());
    }

    #[test]
    fn preferences_store_set_and_get_roundtrip() {
        let dir = std::env::temp_dir().join("rtc-agent-prefs-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("preferences.json");
        let _ = std::fs::remove_file(&path);

        let store = PreferencesStore::new(&path);
        let prefs = RemoteTerminalAgentPreferencesData {
            default_working_directory: "/tmp".into(),
            shortcuts: vec![RemoteTerminalShortcutData {
                id: "s1".into(),
                label: "Copy".into(),
                kind: RemoteTerminalShortcutKind::Key,
                key: "c".into(),
                modifiers: vec![RemoteTerminalShortcutModifier::Ctrl],
                ..Default::default()
            }],
            quick_commands: vec![],
        };

        let saved = store.set_preferences(prefs).expect("set should succeed");
        assert_eq!(saved.default_working_directory, "/tmp");

        let retrieved = store.get_preferences();
        assert_eq!(retrieved.default_working_directory, "/tmp");
        assert_eq!(retrieved.shortcuts.len(), 1);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn preferences_store_handles_corrupted_file() {
        let dir = std::env::temp_dir().join("rtc-agent-prefs-test-corrupt");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("preferences.json");
        std::fs::write(&path, "not valid json").expect("write corrupt file");

        let store = PreferencesStore::new(&path);
        let prefs = store.get_preferences();
        assert_eq!(prefs.default_working_directory, "");
        assert!(prefs.shortcuts.is_empty());

        let _ = std::fs::remove_file(&path);
    }
}

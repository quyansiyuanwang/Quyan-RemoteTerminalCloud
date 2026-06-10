package agent

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"sync"

	"github.com/remote-terminal-cloud/agent/internal/protocol"
)

type persistedPreferencesFile struct {
	Version                 int                                       `json:"version"`
	DefaultWorkingDirectory string                                    `json:"defaultWorkingDirectory,omitempty"`
	Shortcuts               []protocol.RemoteTerminalShortcutData     `json:"shortcuts,omitempty"`
	QuickCommands           []protocol.RemoteTerminalQuickCommandData `json:"quickCommands,omitempty"`
}

type PreferencesStore struct {
	filePath string
	mu       sync.Mutex
	cache    *protocol.RemoteTerminalAgentPreferencesData
}

func NewPreferencesStore(filePath string) *PreferencesStore {
	return &PreferencesStore{filePath: filePath}
}

func (s *PreferencesStore) GetPreferences() protocol.RemoteTerminalAgentPreferencesData {
	s.mu.Lock()
	defer s.mu.Unlock()

	if s.cache == nil {
		loaded := s.loadFromDisk()
		s.cache = &loaded
	}

	return clonePreferences(*s.cache)
}

func (s *PreferencesStore) SetPreferences(preferences protocol.RemoteTerminalAgentPreferencesData) (protocol.RemoteTerminalAgentPreferencesData, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	sanitized := sanitizePreferences(preferences)
	if err := os.MkdirAll(filepath.Dir(s.filePath), 0o755); err != nil {
		return protocol.RemoteTerminalAgentPreferencesData{}, err
	}

	payload := persistedPreferencesFile{
		Version:                 1,
		DefaultWorkingDirectory: sanitized.DefaultWorkingDirectory,
		Shortcuts:               sanitized.Shortcuts,
		QuickCommands:           sanitized.QuickCommands,
	}
	content, err := json.MarshalIndent(payload, "", "  ")
	if err != nil {
		return protocol.RemoteTerminalAgentPreferencesData{}, err
	}

	if err := os.WriteFile(s.filePath, content, 0o644); err != nil {
		return protocol.RemoteTerminalAgentPreferencesData{}, err
	}

	s.cache = &sanitized
	return clonePreferences(sanitized), nil
}

func (s *PreferencesStore) loadFromDisk() protocol.RemoteTerminalAgentPreferencesData {
	content, err := os.ReadFile(s.filePath)
	if err != nil {
		return protocol.RemoteTerminalAgentPreferencesData{
			Shortcuts:     []protocol.RemoteTerminalShortcutData{},
			QuickCommands: []protocol.RemoteTerminalQuickCommandData{},
		}
	}

	var payload persistedPreferencesFile
	if err := json.Unmarshal(content, &payload); err != nil {
		return protocol.RemoteTerminalAgentPreferencesData{
			Shortcuts:     []protocol.RemoteTerminalShortcutData{},
			QuickCommands: []protocol.RemoteTerminalQuickCommandData{},
		}
	}

	return sanitizePreferences(protocol.RemoteTerminalAgentPreferencesData{
		DefaultWorkingDirectory: payload.DefaultWorkingDirectory,
		Shortcuts:               payload.Shortcuts,
		QuickCommands:           payload.QuickCommands,
	})
}

func clonePreferences(preferences protocol.RemoteTerminalAgentPreferencesData) protocol.RemoteTerminalAgentPreferencesData {
	shortcuts := make([]protocol.RemoteTerminalShortcutData, 0, len(preferences.Shortcuts))
	for _, shortcut := range preferences.Shortcuts {
		sequence := append([]string(nil), shortcut.Sequence...)
		modifiers := append([]protocol.RemoteTerminalShortcutModifier(nil), shortcut.Modifiers...)
		shortcuts = append(shortcuts, protocol.RemoteTerminalShortcutData{
			ID:        shortcut.ID,
			Label:     shortcut.Label,
			Kind:      shortcut.Kind,
			Sequence:  sequence,
			Key:       shortcut.Key,
			Modifiers: modifiers,
			Preset:    shortcut.Preset,
		})
	}

	quickCommands := make([]protocol.RemoteTerminalQuickCommandData, 0, len(preferences.QuickCommands))
	quickCommands = append(quickCommands, preferences.QuickCommands...)

	return protocol.RemoteTerminalAgentPreferencesData{
		DefaultWorkingDirectory: preferences.DefaultWorkingDirectory,
		Shortcuts:               shortcuts,
		QuickCommands:           quickCommands,
	}
}

func sanitizePreferences(preferences protocol.RemoteTerminalAgentPreferencesData) protocol.RemoteTerminalAgentPreferencesData {
	result := protocol.RemoteTerminalAgentPreferencesData{
		DefaultWorkingDirectory: strings.TrimSpace(preferences.DefaultWorkingDirectory),
		Shortcuts:               sanitizeShortcuts(preferences.Shortcuts),
		QuickCommands:           sanitizeQuickCommands(preferences.QuickCommands),
	}
	return result
}

func sanitizeShortcuts(items []protocol.RemoteTerminalShortcutData) []protocol.RemoteTerminalShortcutData {
	result := make([]protocol.RemoteTerminalShortcutData, 0, len(items))
	for _, item := range items {
		id := strings.TrimSpace(item.ID)
		if id == "" {
			continue
		}

		label := item.Label
		kind := item.Kind
		if kind != protocol.ShortcutKindKey {
			kind = protocol.ShortcutKindSequence
		}

		sequence := make([]string, 0, len(item.Sequence))
		for _, entry := range item.Sequence {
			if strings.TrimSpace(entry) != "" {
				sequence = append(sequence, entry)
			}
		}

		key := strings.TrimSpace(item.Key)
		if kind == protocol.ShortcutKindKey && key == "" {
			continue
		}
		if kind == protocol.ShortcutKindSequence && len(sequence) == 0 {
			continue
		}

		modifiers := sanitizeModifiers(item.Modifiers)
		result = append(result, protocol.RemoteTerminalShortcutData{
			ID:        id,
			Label:     label,
			Kind:      kind,
			Sequence:  sequence,
			Key:       key,
			Modifiers: modifiers,
			Preset:    item.Preset,
		})
	}
	return result
}

func sanitizeModifiers(items []protocol.RemoteTerminalShortcutModifier) []protocol.RemoteTerminalShortcutModifier {
	if len(items) == 0 {
		return nil
	}
	result := make([]protocol.RemoteTerminalShortcutModifier, 0, len(items))
	for _, item := range items {
		switch item {
		case protocol.ShortcutModifierCtrl, protocol.ShortcutModifierAlt, protocol.ShortcutModifierShift, protocol.ShortcutModifierMeta:
			result = append(result, item)
		}
	}
	if len(result) == 0 {
		return nil
	}
	return result
}

func sanitizeQuickCommands(items []protocol.RemoteTerminalQuickCommandData) []protocol.RemoteTerminalQuickCommandData {
	result := make([]protocol.RemoteTerminalQuickCommandData, 0, len(items))
	for _, item := range items {
		id := strings.TrimSpace(item.ID)
		label := strings.TrimSpace(item.Label)
		command := item.Command
		if id == "" || label == "" || strings.TrimSpace(command) == "" {
			continue
		}
		result = append(result, protocol.RemoteTerminalQuickCommandData{
			ID:      id,
			Label:   label,
			Command: command,
		})
	}
	return result
}

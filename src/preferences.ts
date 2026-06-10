import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import type {
  RemoteTerminalAgentPreferencesData,
  RemoteTerminalQuickCommandData,
  RemoteTerminalShortcutData,
  RemoteTerminalShortcutModifier,
} from "@rtc/protocol";

interface PersistedPreferencesFile {
  version: number;
  defaultWorkingDirectory?: string;
  shortcuts?: RemoteTerminalShortcutData[];
  quickCommands?: RemoteTerminalQuickCommandData[];
}

const PREFERENCES_FILE_VERSION = 1;

function cloneShortcut(shortcut: RemoteTerminalShortcutData): RemoteTerminalShortcutData {
  return {
    ...shortcut,
    sequence: [...shortcut.sequence],
    modifiers: shortcut.modifiers ? [...shortcut.modifiers] : undefined,
  };
}

function cloneQuickCommand(command: RemoteTerminalQuickCommandData): RemoteTerminalQuickCommandData {
  return {
    ...command,
  };
}

function clonePreferences(preferences: RemoteTerminalAgentPreferencesData): RemoteTerminalAgentPreferencesData {
  return {
    defaultWorkingDirectory: preferences.defaultWorkingDirectory,
    shortcuts: preferences.shortcuts.map(cloneShortcut),
    quickCommands: preferences.quickCommands.map(cloneQuickCommand),
  };
}

function sanitizeModifiers(value: unknown): RemoteTerminalShortcutModifier[] | undefined {
  if (!Array.isArray(value)) return undefined;

  const modifiers = value.filter(
    (item): item is RemoteTerminalShortcutModifier =>
      item === "ctrl" || item === "alt" || item === "shift" || item === "meta",
  );

  return modifiers.length > 0 ? modifiers : undefined;
}

function sanitizeShortcuts(value: unknown): RemoteTerminalShortcutData[] {
  if (!Array.isArray(value)) return [];

  const shortcuts: RemoteTerminalShortcutData[] = [];

  for (const item of value) {
    if (!item || typeof item !== "object") continue;

    const candidate = item as Partial<RemoteTerminalShortcutData>;
    const id = typeof candidate.id === "string" ? candidate.id.trim() : "";
    const label = typeof candidate.label === "string" ? candidate.label : "";
    const kind = candidate.kind === "key" ? "key" : "sequence";
    const sequence = Array.isArray(candidate.sequence)
      ? candidate.sequence.filter((entry: unknown): entry is string => typeof entry === "string")
      : [];
    const key = typeof candidate.key === "string" ? candidate.key.trim() : undefined;
    const modifiers = sanitizeModifiers(candidate.modifiers);
    const preset = candidate.preset === true;

    if (!id) continue;
    if (kind === "key" && !key) continue;
    if (kind === "sequence" && sequence.length === 0) continue;

    shortcuts.push({
      id,
      label,
      kind,
      sequence,
      key,
      modifiers,
      preset,
    });
  }

  return shortcuts;
}

function sanitizeQuickCommands(value: unknown): RemoteTerminalQuickCommandData[] {
  if (!Array.isArray(value)) return [];

  const commands: RemoteTerminalQuickCommandData[] = [];

  for (const item of value) {
    if (!item || typeof item !== "object") continue;

    const candidate = item as Partial<RemoteTerminalQuickCommandData>;
    const id = typeof candidate.id === "string" ? candidate.id.trim() : "";
    const label = typeof candidate.label === "string" ? candidate.label.trim() : "";
    const command = typeof candidate.command === "string" ? candidate.command : "";

    if (!id || !label || !command.trim()) continue;

    commands.push({
      id,
      label,
      command,
    });
  }

  return commands;
}

function sanitizePreferences(value: unknown): RemoteTerminalAgentPreferencesData {
  const candidate: Partial<PersistedPreferencesFile> =
    value && typeof value === "object" ? (value as PersistedPreferencesFile) : {};
  const defaultWorkingDirectory =
    typeof candidate.defaultWorkingDirectory === "string" ? candidate.defaultWorkingDirectory.trim() : "";

  return {
    defaultWorkingDirectory: defaultWorkingDirectory || undefined,
    shortcuts: sanitizeShortcuts(candidate.shortcuts),
    quickCommands: sanitizeQuickCommands(candidate.quickCommands),
  };
}

export class AgentPreferencesStore {
  private cache: RemoteTerminalAgentPreferencesData | null = null;

  public constructor(private readonly filePath: string) {}

  public getPreferences(): RemoteTerminalAgentPreferencesData {
    if (!this.cache) {
      this.cache = this.loadFromDisk();
    }

    return clonePreferences(this.cache);
  }

  public setPreferences(preferences: RemoteTerminalAgentPreferencesData): RemoteTerminalAgentPreferencesData {
    const sanitized = sanitizePreferences(preferences);
    mkdirSync(path.dirname(this.filePath), { recursive: true });
    writeFileSync(
      this.filePath,
      JSON.stringify(
        {
          version: PREFERENCES_FILE_VERSION,
          defaultWorkingDirectory: sanitized.defaultWorkingDirectory,
          shortcuts: sanitized.shortcuts,
          quickCommands: sanitized.quickCommands,
        } satisfies PersistedPreferencesFile,
        null,
        2,
      ),
      "utf8",
    );
    this.cache = sanitized;
    return clonePreferences(sanitized);
  }

  private loadFromDisk(): RemoteTerminalAgentPreferencesData {
    try {
      const raw = readFileSync(this.filePath, "utf8");
      return sanitizePreferences(JSON.parse(raw));
    } catch {
      return {
        defaultWorkingDirectory: undefined,
        shortcuts: [],
        quickCommands: [],
      };
    }
  }
}
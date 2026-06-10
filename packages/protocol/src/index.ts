export type PlatformId = "windows" | "linux" | "macos";
export type ShellType = "system-default" | "cmd" | "powershell" | "pwsh" | "bash" | "zsh" | "sh";

export interface AgentCapabilities {
  sshForward: boolean;
  nativePty: boolean;
  selfUpdate: boolean;
  proxyAware: boolean;
  serviceManaged: boolean;
  sessionRecording: boolean;
}

export interface HostDiagnostics {
  installFormats: string[];
  serviceManager: string;
  defaultLogPath: string;
  availableShells: ShellType[];
  sshCheck: {
    available: boolean;
    detail: string;
  };
  notes: string[];
}

export interface HostSnapshot {
  hostname: string;
  platform: PlatformId;
  arch: string;
  agentVersion: string;
  capabilities: AgentCapabilities;
  diagnostics: HostDiagnostics;
}

export interface AgentRegistrationRequest {
  registrationToken: string;
  snapshot: HostSnapshot;
}

export interface AgentRegistrationResponse {
  deviceId: string;
  heartbeatIntervalSeconds: number;
  heartbeatToken: string;
  acceptedAt: string;
}

export interface AgentHeartbeatRequest {
  deviceId: string;
  heartbeatToken: string;
  snapshot: HostSnapshot;
}

export interface AgentHeartbeatResponse {
  ok: true;
  nextHeartbeatIntervalSeconds: number;
  serverTime: string;
}

export interface DeviceSummary {
  deviceId: string;
  hostname: string;
  platform: HostSnapshot["platform"];
  arch: string;
  availableShells: ShellType[];
  lastSeenAt: string;
  registeredAt: string;
  online: boolean;
}

export interface DeviceListResponse {
  items: DeviceSummary[];
}

export type SessionMode = "shell";

export interface SessionCreateRequest {
  deviceId: string;
  mode: SessionMode;
  shellType: ShellType;
  workingDirectory?: string;
}

export interface SessionCreateResponse {
  sessionId: string;
  deviceId: string;
  mode: SessionMode;
  shellType: ShellType;
  browserToken: string;
  websocketUrl: string;
  createdAt: string;
}

export interface SessionSummary {
  sessionId: string;
  deviceId: string;
  mode: SessionMode;
  shellType: ShellType;
  status: "pending" | "connected" | "closed";
  createdAt: string;
}

export interface DirectoryEntry {
  name: string;
  path: string;
}

export type RemoteTerminalShortcutModifier = "ctrl" | "alt" | "shift" | "meta";
export type RemoteTerminalShortcutKind = "sequence" | "key";

export interface RemoteTerminalShortcutData {
  id: string;
  label: string;
  kind: RemoteTerminalShortcutKind;
  sequence: string[];
  key?: string;
  modifiers?: RemoteTerminalShortcutModifier[];
  preset?: boolean;
}

export interface RemoteTerminalQuickCommandData {
  id: string;
  label: string;
  command: string;
}

export interface RemoteTerminalAgentPreferencesData {
  defaultWorkingDirectory?: string;
  shortcuts: RemoteTerminalShortcutData[];
  quickCommands: RemoteTerminalQuickCommandData[];
}

export interface DirectoryBrowseRequestMessage {
  type: "directory-browse";
  requestId: string;
  path?: string;
}

export interface DirectoryBrowseResultMessage {
  type: "directory-browse-result";
  requestId: string;
  ok: boolean;
  message?: string;
  currentPath: string;
  parentPath?: string;
  items: DirectoryEntry[];
}

export interface PreferencesGetMessage {
  type: "preferences-get";
  requestId: string;
}

export interface PreferencesSetMessage {
  type: "preferences-set";
  requestId: string;
  preferences: RemoteTerminalAgentPreferencesData;
}

export interface PreferencesResultMessage {
  type: "preferences-result";
  requestId: string;
  ok: boolean;
  message?: string;
  preferences: RemoteTerminalAgentPreferencesData;
}

export interface SessionListResponse {
  items: SessionSummary[];
}

export interface SessionStartMessage {
  type: "session-start";
  sessionId: string;
  mode: SessionMode;
  shellType: ShellType;
  workingDirectory?: string;
}

export interface SessionInputMessage {
  type: "session-input";
  sessionId: string;
  data: string;
}

export interface SessionResizeMessage {
  type: "session-resize";
  sessionId: string;
  cols: number;
  rows: number;
}

export interface SessionStopMessage {
  type: "session-stop";
  sessionId: string;
}

export interface SessionReadyMessage {
  type: "session-ready";
  sessionId: string;
}

export interface SessionOutputMessage {
  type: "session-output";
  sessionId: string;
  stream: "stdout" | "stderr";
  data: string;
}

export interface SessionExitMessage {
  type: "session-exit";
  sessionId: string;
  exitCode: number | null;
}

export interface SessionErrorMessage {
  type: "session-error";
  sessionId: string;
  message: string;
}

export interface BrowserConnectedMessage {
  type: "browser-connected";
  sessionId: string;
}

export type ServerToAgentMessage =
  | SessionStartMessage
  | SessionInputMessage
  | SessionResizeMessage
  | SessionStopMessage
  | DirectoryBrowseRequestMessage
  | PreferencesGetMessage
  | PreferencesSetMessage;

export type AgentToServerMessage =
  | SessionReadyMessage
  | SessionOutputMessage
  | SessionExitMessage
  | SessionErrorMessage
  | DirectoryBrowseResultMessage
  | PreferencesResultMessage;

export type ServerToBrowserMessage =
  | BrowserConnectedMessage
  | SessionReadyMessage
  | SessionOutputMessage
  | SessionExitMessage
  | SessionErrorMessage;

export type BrowserToServerMessage = SessionInputMessage | SessionResizeMessage | SessionStopMessage;

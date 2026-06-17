export type StatusPayload = {
  version: string;
  serverBaseUrl: string;
  registrationToken: string;
  registrationTokenSource: string;
  runHeartbeat: boolean;
  runTunnel: boolean;
  configuredDefaultShell: string;
  effectiveDefaultShell: string;
  availableShells: string[];
  sshAvailable: boolean;
  sshDetail: string;
  platform: string;
  arch: string;
};

export type AgentOverview = {
  desiredRunning: boolean;
  running: boolean;
  connected: boolean;
  pid?: number | null;
  autostartEnabled: boolean;
  hasToken: boolean;
  tokenSource: string;
  statusSummary: string;
};

export type AgentLogEntry = {
  stream: string;
  line: string;
};

export type BootstrapPayload = {
  status: StatusPayload;
  agent: AgentOverview;
  recentLogs: AgentLogEntry[];
  desktopMode: string;
  onboardingRequired: boolean;
};

export type ActionPayload = {
  action?: string;
  ok: boolean;
  message?: string;
  configFile?: string;
};

export type AgentActionPayload = {
  action: string;
  ok: boolean;
  message: string;
  state: AgentOverview;
};

export type AutostartPayload = {
  ok: boolean;
  enabled: boolean;
  message: string;
};

export type RuntimeStateTone = "neutral" | "success" | "warning" | "danger" | "primary";

export type RuntimeState = {
  label: string;
  tone: RuntimeStateTone;
  detail: string;
};

export type HealthItem = {
  key: string;
  label: string;
  value: string;
  tone: RuntimeStateTone;
};
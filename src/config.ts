import { config as loadDotenv } from "dotenv";
import { existsSync, readFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { isSupportedShellType, type SupportedShellType } from "./shells";

interface AgentFileConfig {
  serverBaseUrl?: string;
  registrationToken?: string | null;
  runHeartbeat?: boolean;
  runTunnel?: boolean;
  defaultShellType?: SupportedShellType;
  enabledShellTypes?: SupportedShellType[] | null;
  preferencesFilePath?: string;
}

export interface AgentRuntimeConfig {
  serverBaseUrl: string;
  registrationToken: string | null;
  runHeartbeat: boolean;
  runTunnel: boolean;
  defaultShellType: SupportedShellType;
  enabledShellTypes: SupportedShellType[] | null;
  preferencesFilePath: string;
  configFilePath: string;
}

let dotenvLoaded = false;

function ensureDotenvLoaded(): void {
  if (dotenvLoaded) {
    return;
  }

  const envCandidates = [
    path.resolve(process.cwd(), ".env"),
    path.resolve(process.cwd(), "..", ".env"),
  ];

  for (const envPath of envCandidates) {
    if (!existsSync(envPath)) {
      continue;
    }

    loadDotenv({ path: envPath, override: false });
    break;
  }

  dotenvLoaded = true;
}

function readString(name: string): string | null {
  const value = process.env[name]?.trim();
  return value ? value : null;
}

function normalizeTemplateString(value: string | null): string | null {
  if (!value) {
    return null;
  }

  if (value === "https://your-domain.example.com") {
    return null;
  }

  if (value === "replace-with-real-token") {
    return null;
  }

  return value;
}

function readBooleanEnv(name: string): boolean | null {
  const value = readString(name);
  if (value === null) {
    return null;
  }

  if (["1", "true", "yes", "on"].includes(value.toLowerCase())) {
    return true;
  }

  if (["0", "false", "no", "off"].includes(value.toLowerCase())) {
    return false;
  }

  return null;
}

function parseShellList(name: string): SupportedShellType[] | null {
  const raw = readString(name);
  if (!raw) {
    return null;
  }

  const values = raw
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean);

  const uniqueShells: SupportedShellType[] = [];
  const seen = new Set<SupportedShellType>();
  for (const value of values) {
    if (!isSupportedShellType(value) || seen.has(value)) {
      continue;
    }

    seen.add(value);
    uniqueShells.push(value);
  }

  return uniqueShells;
}

function getDefaultPreferencesFilePath(): string {
  if (process.platform === "win32") {
    const appData = process.env.APPDATA?.trim() || path.join(os.homedir(), "AppData", "Roaming");
    return path.join(appData, "remote-terminal-cloud-agent", "preferences.json");
  }

  if (process.platform === "darwin") {
    return path.join(os.homedir(), "Library", "Application Support", "remote-terminal-cloud-agent", "preferences.json");
  }

  const stateHome = process.env.XDG_STATE_HOME?.trim() || path.join(os.homedir(), ".local", "state");
  return path.join(stateHome, "remote-terminal-cloud-agent", "preferences.json");
}

function getDefaultConfigFilePath(): string {
  if (process.platform === "win32") {
    const appData = process.env.APPDATA?.trim() || path.join(os.homedir(), "AppData", "Roaming");
    return path.join(appData, "remote-terminal-cloud-agent", "config.json");
  }

  if (process.platform === "darwin") {
    return path.join(os.homedir(), "Library", "Application Support", "remote-terminal-cloud-agent", "config.json");
  }

  const configHome = process.env.XDG_CONFIG_HOME?.trim() || path.join(os.homedir(), ".config");
  return path.join(configHome, "remote-terminal-cloud-agent", "config.json");
}

function normalizeShellList(values: unknown): SupportedShellType[] | null {
  if (!Array.isArray(values)) {
    return null;
  }

  const uniqueShells: SupportedShellType[] = [];
  const seen = new Set<SupportedShellType>();
  for (const value of values) {
    if (typeof value !== "string") {
      continue;
    }

    const normalized = value.trim();
    if (!isSupportedShellType(normalized) || seen.has(normalized)) {
      continue;
    }

    seen.add(normalized);
    uniqueShells.push(normalized);
  }

  return uniqueShells;
}

function readConfigFile(configFilePath: string): AgentFileConfig {
  if (!existsSync(configFilePath)) {
    return {};
  }

  try {
    const raw = readFileSync(configFilePath, "utf8");
    const parsed = JSON.parse(raw) as Record<string, unknown>;

    const defaultShellType =
      typeof parsed.defaultShellType === "string" && isSupportedShellType(parsed.defaultShellType)
        ? parsed.defaultShellType
        : undefined;

    const serverBaseUrl = typeof parsed.serverBaseUrl === "string" ? normalizeTemplateString(parsed.serverBaseUrl.trim()) : undefined;
    const registrationToken =
      typeof parsed.registrationToken === "string"
        ? normalizeTemplateString(parsed.registrationToken.trim())
        : parsed.registrationToken === null
          ? null
          : undefined;
    const preferencesFilePath =
      typeof parsed.preferencesFilePath === "string" ? parsed.preferencesFilePath.trim() : undefined;

    return {
      serverBaseUrl: serverBaseUrl || undefined,
      registrationToken,
      runHeartbeat: typeof parsed.runHeartbeat === "boolean" ? parsed.runHeartbeat : undefined,
      runTunnel: typeof parsed.runTunnel === "boolean" ? parsed.runTunnel : undefined,
      defaultShellType,
      enabledShellTypes: normalizeShellList(parsed.enabledShellTypes),
      preferencesFilePath: preferencesFilePath || undefined,
    };
  } catch (error) {
    console.warn(
      `[remote-terminal-cloud-agent] failed to parse config file ${configFilePath}: ${error instanceof Error ? error.message : String(error)}`,
    );
    return {};
  }
}

export function getAgentRuntimeConfig(): AgentRuntimeConfig {
  ensureDotenvLoaded();

  const configFilePath = readString("RTC_CONFIG_FILE") ?? getDefaultConfigFilePath();
  const fileConfig = readConfigFile(configFilePath);
  const configuredDefaultShell = readString("RTC_DEFAULT_SHELL");
  const enabledShellTypes = parseShellList("RTC_ENABLED_SHELLS");
  const heartbeatEnabled = readBooleanEnv("RTC_DISABLE_HEARTBEAT");
  const tunnelEnabled = readBooleanEnv("RTC_DISABLE_TUNNEL");

  const defaultShellCandidate = configuredDefaultShell ?? fileConfig.defaultShellType;

  return {
    serverBaseUrl: normalizeTemplateString(readString("RTC_SERVER_BASE_URL")) ?? fileConfig.serverBaseUrl ?? "http://127.0.0.1:10001",
    registrationToken: normalizeTemplateString(readString("RTC_REGISTRATION_TOKEN")) ?? fileConfig.registrationToken ?? null,
    runHeartbeat: heartbeatEnabled === null ? (fileConfig.runHeartbeat ?? true) : !heartbeatEnabled,
    runTunnel: tunnelEnabled === null ? (fileConfig.runTunnel ?? true) : !tunnelEnabled,
    defaultShellType: isSupportedShellType(defaultShellCandidate) ? defaultShellCandidate : "system-default",
    enabledShellTypes: enabledShellTypes ?? fileConfig.enabledShellTypes ?? null,
    preferencesFilePath: readString("RTC_PREFERENCES_FILE") ?? fileConfig.preferencesFilePath ?? getDefaultPreferencesFilePath(),
    configFilePath,
  };
}


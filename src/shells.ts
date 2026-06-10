import type { ShellType } from "@rtc/protocol";
import { runCommand } from "./process";

export const SUPPORTED_SHELL_TYPES = [
  "system-default",
  "cmd",
  "powershell",
  "pwsh",
  "bash",
  "zsh",
  "sh",
] as const satisfies readonly ShellType[];

export type SupportedShellType = (typeof SUPPORTED_SHELL_TYPES)[number];

const shellTypeSet = new Set<ShellType>(SUPPORTED_SHELL_TYPES);

export function isSupportedShellType(value: string | null | undefined): value is SupportedShellType {
  return value ? shellTypeSet.has(value as ShellType) : false;
}

async function commandExists(command: string): Promise<boolean> {
  if (process.platform === "win32") {
    const result = await runCommand("where.exe", [command]);
    return result.ok;
  }

  const result = await runCommand("sh", ["-lc", `command -v ${command} >/dev/null 2>&1`]);
  return result.ok;
}

export async function detectAvailableShells(): Promise<SupportedShellType[]> {
  const availableShells: SupportedShellType[] = ["system-default"];

  if (process.platform === "win32") {
    const candidates: SupportedShellType[] = ["cmd", "powershell", "pwsh"];
    for (const candidate of candidates) {
      if (candidate === "cmd") {
        availableShells.push(candidate);
        continue;
      }

      if (await commandExists(candidate === "powershell" ? "powershell.exe" : "pwsh.exe")) {
        availableShells.push(candidate);
      }
    }

    return availableShells;
  }

  const candidates: Array<{ command: string; shellType: SupportedShellType }> = [
    { command: "bash", shellType: "bash" },
    { command: "zsh", shellType: "zsh" },
    { command: "sh", shellType: "sh" },
    { command: "pwsh", shellType: "pwsh" },
  ];

  for (const candidate of candidates) {
    if (await commandExists(candidate.command)) {
      availableShells.push(candidate.shellType);
    }
  }

  return availableShells;
}

export async function detectConfiguredAvailableShells(
  enabledShellTypes: SupportedShellType[] | null,
): Promise<SupportedShellType[]> {
  const detectedShells = await detectAvailableShells();
  if (!enabledShellTypes) {
    return detectedShells;
  }

  const enabledShellSet = new Set(enabledShellTypes);
  return detectedShells.filter((shellType) => enabledShellSet.has(shellType));
}

export function resolveEffectiveDefaultShell(
  configuredDefaultShell: SupportedShellType,
  availableShells: SupportedShellType[],
): SupportedShellType {
  if (availableShells.includes(configuredDefaultShell)) {
    return configuredDefaultShell;
  }

  if (availableShells.includes("system-default")) {
    return "system-default";
  }

  return availableShells[0] ?? configuredDefaultShell;
}

export function resolveShellLaunch(
  requestedShellType: ShellType,
  defaultShellType: SupportedShellType,
): { executable: string; args: string[]; shellType: SupportedShellType; encoding?: string | null } {
  const normalizedShellType = requestedShellType === "system-default" ? defaultShellType : requestedShellType;

  const utf8Bootstrap = [
    "[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)",
    "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)",
    "$OutputEncoding = [Console]::OutputEncoding",
    "chcp 65001 > $null",
  ].join("; ");

  if (process.platform === "win32") {
    switch (normalizedShellType) {
      case "system-default":
      case "cmd":
        return {
          executable: process.env.ComSpec ?? "cmd.exe",
          args: ["/d", "/k", "chcp 65001>nul"],
          shellType: "cmd",
          encoding: "utf-8",
        };
      case "powershell":
        return {
          executable: "powershell.exe",
          args: ["-NoLogo", "-NoExit", "-Command", utf8Bootstrap],
          shellType: "powershell",
          encoding: "utf-8",
        };
      case "pwsh":
        return {
          executable: "pwsh.exe",
          args: ["-NoLogo", "-NoExit", "-Command", utf8Bootstrap],
          shellType: "pwsh",
          encoding: "utf-8",
        };
      default:
        throw new Error(`Shell ${normalizedShellType} is not supported on Windows.`);
    }
  }

  switch (normalizedShellType) {
    case "system-default":
      return {
        executable: process.env.SHELL ?? "/bin/bash",
        args: ["-i"],
        shellType: "system-default",
      };
    case "bash":
      return { executable: "bash", args: ["-i"], shellType: "bash" };
    case "zsh":
      return { executable: "zsh", args: ["-i"], shellType: "zsh" };
    case "sh":
      return { executable: "sh", args: ["-i"], shellType: "sh" };
    case "pwsh":
      return { executable: "pwsh", args: ["-NoLogo"], shellType: "pwsh" };
    default:
      throw new Error(`Shell ${normalizedShellType} is not supported on this platform.`);
  }
}
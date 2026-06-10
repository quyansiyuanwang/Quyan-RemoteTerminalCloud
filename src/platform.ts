import os from "node:os";
import type {
  AgentCapabilities,
  HostSnapshot,
  PlatformId,
} from "@rtc/protocol";
import { runCommand } from "./process";
import { detectConfiguredAvailableShells, type SupportedShellType } from "./shells";

interface PlatformProfile {
  platform: PlatformId;
  installFormats: string[];
  serviceManager: string;
  defaultLogPath: string;
  capabilities: AgentCapabilities;
  notes: string[];
  probeSsh(): Promise<{ available: boolean; detail: string }>;
}

function getBaseCapabilities(): AgentCapabilities {
  return {
    sshForward: true,
    nativePty: false,
    selfUpdate: true,
    proxyAware: true,
    serviceManaged: true,
    sessionRecording: false,
  };
}

function createWindowsProfile(): PlatformProfile {
  return {
    platform: "windows",
    installFormats: ["msi", "exe"],
    serviceManager: "Windows Service",
    defaultLogPath: "%ProgramData%/remote-terminal-cloud-agent/logs",
    capabilities: getBaseCapabilities(),
    notes: [
      "MVP expects local OpenSSH Server.",
      "Phase 2 adds PowerShell/cmd native PTY.",
      "Validate UAC, Defender and localized paths.",
    ],
    async probeSsh() {
      const result = await runCommand("powershell", [
        "-NoProfile",
        "-Command",
        "try { (Get-Service sshd -ErrorAction Stop).Status } catch { 'missing' }",
      ]);

      if (!result.ok && !result.stdout) {
        return { available: false, detail: result.stderr || "Unable to inspect sshd service." };
      }

      const detail = result.stdout || "missing";
      return {
        available: detail.toLowerCase() !== "missing",
        detail: `sshd service status: ${detail}`,
      };
    },
  };
}

function createLinuxProfile(): PlatformProfile {
  return {
    platform: "linux",
    installFormats: ["deb", "rpm", "binary"],
    serviceManager: "systemd",
    defaultLogPath: "/var/log/remote-terminal-cloud-agent/",
    capabilities: getBaseCapabilities(),
    notes: [
      "MVP expects local sshd.",
      "Phase 2 adds bash/sh native PTY.",
      "Validate glibc, SELinux and filesystem constraints.",
    ],
    async probeSsh() {
      const result = await runCommand("sh", ["-lc", "command -v sshd >/dev/null && echo present || echo missing"]);
      const detail = result.stdout || "missing";
      return {
        available: detail === "present",
        detail: detail === "present" ? "sshd binary detected." : "sshd binary missing.",
      };
    },
  };
}

function createMacOsProfile(): PlatformProfile {
  return {
    platform: "macos",
    installFormats: ["pkg", "signed-helper"],
    serviceManager: "launchd",
    defaultLogPath: "/Library/Logs/remote-terminal-cloud-agent/",
    capabilities: getBaseCapabilities(),
    notes: [
      "MVP expects Remote Login/OpenSSH.",
      "Phase 2 adds zsh/sh native PTY.",
      "Validate notarization, Full Disk Access and ARM64/Intel differences.",
    ],
    async probeSsh() {
      const result = await runCommand("sh", [
        "-lc",
        "systemsetup -getremotelogin 2>/dev/null | tr -d '\\r' || echo unavailable",
      ]);
      const detail = result.stdout || "unavailable";
      const normalized = detail.toLowerCase();
      return {
        available: normalized.includes("on"),
        detail: `Remote Login status: ${detail}`,
      };
    },
  };
}

function getPlatformProfile(): PlatformProfile {
  switch (process.platform) {
    case "win32":
      return createWindowsProfile();
    case "linux":
      return createLinuxProfile();
    case "darwin":
      return createMacOsProfile();
    default:
      throw new Error(`Unsupported platform: ${process.platform}`);
  }
}

export async function collectHostSnapshot(
  agentVersion: string,
  enabledShellTypes: SupportedShellType[] | null,
): Promise<HostSnapshot> {
  const profile = getPlatformProfile();
  const sshCheck = await profile.probeSsh();
  const availableShells = await detectConfiguredAvailableShells(enabledShellTypes);

  return {
    hostname: os.hostname(),
    platform: profile.platform,
    arch: os.arch(),
    agentVersion,
    capabilities: profile.capabilities,
    diagnostics: {
      installFormats: profile.installFormats,
      serviceManager: profile.serviceManager,
      defaultLogPath: profile.defaultLogPath,
      availableShells,
      sshCheck,
      notes: profile.notes,
    },
  };
}

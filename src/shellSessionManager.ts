import type { IPty } from "node-pty";
import { spawn } from "node-pty";
import { accessSync, constants as fsConstants, readdirSync, statSync } from "node:fs";
import path from "node:path";
import type { ShellType } from "@rtc/protocol";
import { resolveShellLaunch, type SupportedShellType } from "./shells";

interface ShellSession {
  process: IPty;
}

export interface ShellSessionCallbacks {
  onReady(): void;
  onOutput(stream: "stdout" | "stderr", data: string): void;
  onExit(exitCode: number | null): void;
  onError(message: string): void;
}

export interface BrowseDirectoryResult {
  currentPath: string;
  parentPath?: string;
  items: Array<{
    name: string;
    path: string;
  }>;
}

function resolveWorkingDirectory(workingDirectory?: string): { cwd: string; warning?: string } {
  const normalizedDirectory = workingDirectory?.trim();
  if (!normalizedDirectory) {
    return { cwd: process.cwd() };
  }

  try {
    accessSync(normalizedDirectory, fsConstants.R_OK);
    return { cwd: normalizedDirectory };
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unknown error";
    return {
      cwd: process.cwd(),
      warning: `Unable to use working directory \"${normalizedDirectory}\", fallback to default directory: ${message}`,
    };
  }
}

function getRootBrowseResult(): BrowseDirectoryResult {
  if (process.platform !== "win32") {
    return {
      currentPath: "/",
      items: readdirSync("/", { withFileTypes: true })
        .filter((item) => item.isDirectory())
        .map((item) => ({
          name: item.name,
          path: path.posix.join("/", item.name),
        }))
        .sort((left, right) => left.name.localeCompare(right.name)),
    };
  }

  const items: BrowseDirectoryResult["items"] = [];
  for (let index = 65; index <= 90; index += 1) {
    const drive = `${String.fromCharCode(index)}:\\`;
    try {
      accessSync(drive, fsConstants.R_OK);
      items.push({
        name: drive.replace(/\\$/, ""),
        path: drive,
      });
    } catch {
      continue;
    }
  }

  return {
    currentPath: "",
    items,
  };
}

function normalizeBrowsePath(targetPath?: string): string | undefined {
  const normalized = targetPath?.trim();
  if (!normalized) return undefined;

  return path.resolve(normalized);
}

export class ShellSessionManager {
  private readonly sessions = new Map<string, ShellSession>();

  public constructor(private readonly defaultShellType: SupportedShellType) {}

  public startSession(
    sessionId: string,
    shellType: ShellType,
    callbacks: ShellSessionCallbacks,
    workingDirectory?: string,
  ): void {
    if (this.sessions.has(sessionId)) {
      callbacks.onError("Session already exists.");
      return;
    }

    try {
      const isWindows = process.platform === "win32";
      const shellLaunch = resolveShellLaunch(shellType, this.defaultShellType);
      const { cwd, warning } = resolveWorkingDirectory(workingDirectory);
      const child = spawn(shellLaunch.executable, shellLaunch.args, {
        name: "xterm-256color",
        cols: 120,
        rows: 30,
        cwd,
        env: {
          ...process.env,
          TERM: process.env.TERM ?? "xterm-256color",
          COLORTERM: process.env.COLORTERM ?? "truecolor",
          TERM_PROGRAM: process.env.TERM_PROGRAM ?? "remote-terminal-cloud",
          TERM_PROGRAM_VERSION: process.env.TERM_PROGRAM_VERSION ?? "agent",
          ...(isWindows
            ? {
                ConEmuANSI: process.env.ConEmuANSI ?? "ON",
              }
            : {}),
        },
      });

      child.onData((data) => {
        callbacks.onOutput("stdout", data);
      });

      if (warning) {
        queueMicrotask(() => {
          callbacks.onOutput("stderr", `${warning}\n`);
        });
      }

      queueMicrotask(() => {
        callbacks.onReady();
      });

      child.onExit(({ exitCode }) => {
        callbacks.onExit(exitCode);
        this.sessions.delete(sessionId);
      });

      this.sessions.set(sessionId, { process: child });
    } catch (error) {
      callbacks.onError(error instanceof Error ? error.message : "Failed to start shell session.");
    }
  }

  public writeInput(sessionId: string, data: string): void {
    const session = this.sessions.get(sessionId);
    if (!session) {
      return;
    }

    session.process.write(data);
  }

  public resizeSession(sessionId: string, cols: number, rows: number): void {
    const session = this.sessions.get(sessionId);
    if (!session) {
      return;
    }

    session.process.resize(Math.max(1, cols), Math.max(1, rows));
  }

  public stopSession(sessionId: string): void {
    const session = this.sessions.get(sessionId);
    if (!session) {
      return;
    }

    session.process.kill();
    this.sessions.delete(sessionId);
  }

  public browseDirectories(targetPath?: string): BrowseDirectoryResult {
    const normalizedPath = normalizeBrowsePath(targetPath);
    if (!normalizedPath) {
      return getRootBrowseResult();
    }

    accessSync(normalizedPath, fsConstants.R_OK);
    const stats = statSync(normalizedPath);
    if (!stats.isDirectory()) {
      throw new Error("Selected path is not a directory.");
    }

    const items = readdirSync(normalizedPath, { withFileTypes: true })
      .filter((item) => item.isDirectory())
      .map((item) => ({
        name: item.name,
        path: path.join(normalizedPath, item.name),
      }))
      .sort((left, right) => left.name.localeCompare(right.name));

    const parentPath = path.dirname(normalizedPath);

    return {
      currentPath: normalizedPath,
      parentPath: parentPath !== normalizedPath ? parentPath : undefined,
      items,
    };
  }
}

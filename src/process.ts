import { execFile } from "node:child_process";

export interface CommandResult {
  ok: boolean;
  stdout: string;
  stderr: string;
  exitCode: number | null;
}

export async function runCommand(
  file: string,
  args: string[],
  timeout = 3_000,
): Promise<CommandResult> {
  return new Promise((resolve) => {
    execFile(file, args, { timeout, windowsHide: true }, (error, stdout, stderr) => {
      if (!error) {
        resolve({ ok: true, stdout: stdout.trim(), stderr: stderr.trim(), exitCode: 0 });
        return;
      }

      const exitCode = typeof error.code === "number" ? error.code : null;
      resolve({ ok: false, stdout: stdout.trim(), stderr: stderr.trim(), exitCode });
    });
  });
}

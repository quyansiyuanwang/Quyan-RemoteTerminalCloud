import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..");
const packageJson = JSON.parse(readFileSync(path.join(projectRoot, "package.json"), "utf8"));
const version = packageJson.version ?? "0.0.0";

const targetPlatform = process.env.RTC_TARGET_PLATFORM ?? os.platform();
const targetArch = process.env.RTC_TARGET_ARCH ?? os.arch();
const releaseRoot = path.join(projectRoot, "release");
const bundleRoot = path.join(releaseRoot, `remote-terminal-cloud-agent-${version}`);
const platformOutputRoot = path.join(releaseRoot, "artifacts", `${targetPlatform}-${targetArch}`);
const stageRoot = path.join(platformOutputRoot, `remote-terminal-cloud-agent-${version}`);
const runtimeRoot = path.join(stageRoot, "runtime");
const archiveBaseName = `remote-terminal-cloud-agent-${version}-${targetPlatform}-${targetArch}`;

function getExecutable(command) {
  if (process.platform !== "win32") {
    return command;
  }

  if (command === "pnpm") {
    return "pnpm.cmd";
  }

  if (command === "powershell") {
    return "powershell.exe";
  }

  return command;
}

function createCommandInvocation(command, args) {
  if (process.platform === "win32" && command === "pnpm") {
    const escaped = [command, ...args].map((part) => {
      if (/\s|"/.test(part)) {
        return `"${part.replaceAll('"', '\\"')}"`;
      }

      return part;
    });

    return {
      file: "cmd.exe",
      args: ["/d", "/s", "/c", escaped.join(" ")],
    };
  }

  return {
    file: getExecutable(command),
    args,
  };
}

function runCommand(command, args, cwd = projectRoot) {
  const invocation = createCommandInvocation(command, args);
  const result = spawnSync(invocation.file, invocation.args, {
    cwd,
    env: process.env,
    stdio: "inherit",
  });

  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with exit code ${result.status ?? "unknown"}`);
  }
}

function ensureReleaseBundle() {
  if (existsSync(bundleRoot)) {
    return;
  }

  runCommand("pnpm", ["build:bundle"]);
}

function copyNodeRuntime() {
  const nodeExecutableName = targetPlatform === "win32" ? "node.exe" : "node";
  const nodeSource = process.execPath;
  const nodeTarget = path.join(runtimeRoot, nodeExecutableName);

  if (!existsSync(nodeSource)) {
    throw new Error(`Node runtime executable not found: ${nodeSource}`);
  }

  mkdirSync(runtimeRoot, { recursive: true });
  cpSync(nodeSource, nodeTarget, { force: true });
}

function writeManifest() {
  const manifest = {
    generatedAt: new Date().toISOString(),
    version,
    targetPlatform,
    targetArch,
    nodeRuntimeExecutable: targetPlatform === "win32" ? "runtime/node.exe" : "runtime/node",
    startCommand: targetPlatform === "win32"
      ? ".\\runtime\\node.exe .\\dist\\index.js"
      : "./runtime/node ./dist/index.js",
  };

  writeFileSync(path.join(stageRoot, "ARTIFACT-INFO.json"), JSON.stringify(manifest, null, 2));
}

function writeReadme() {
  const installHint = targetPlatform === "win32"
    ? "Use packaging/windows/install-service.ps1 for service installation, or build MSI from the Windows staging flow."
    : targetPlatform === "darwin"
      ? "Use packaging/macos/install-service.sh for launchd installation."
      : "Use packaging/linux/install-service.sh for systemd installation.";

  writeFileSync(
    path.join(stageRoot, "README.txt"),
    [
      "Remote Terminal Cloud Agent platform artifact",
      "",
      `Version: ${version}`,
      `Platform: ${targetPlatform}`,
      `Architecture: ${targetArch}`,
      "",
      "This artifact contains:",
      "- dist/ compiled agent output",
      "- node_modules/ production dependencies",
      "- runtime/ local Node runtime executable",
      "- packaging/ platform service installation templates",
      "",
      installHint,
    ].join("\n"),
  );
}

function createArchive() {
  mkdirSync(platformOutputRoot, { recursive: true });

  if (targetPlatform === "win32") {
    const zipPath = path.join(platformOutputRoot, `${archiveBaseName}.zip`);
    if (existsSync(zipPath)) {
      rmSync(zipPath, { force: true });
    }

    runCommand(
      "powershell",
      [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        `Compress-Archive -Path '${stageRoot.replace(/'/g, "''")}\\*' -DestinationPath '${zipPath.replace(/'/g, "''")}' -Force`,
      ],
    );
    return;
  }

  const archivePath = path.join(platformOutputRoot, `${archiveBaseName}.tar.gz`);
  if (existsSync(archivePath)) {
    rmSync(archivePath, { force: true });
  }

  runCommand("tar", ["-czf", archivePath, "-C", platformOutputRoot, path.basename(stageRoot)]);
}

ensureReleaseBundle();

rmSync(platformOutputRoot, { force: true, recursive: true });
mkdirSync(stageRoot, { recursive: true });

cpSync(bundleRoot, stageRoot, { recursive: true });
copyNodeRuntime();
writeManifest();
writeReadme();
createArchive();

console.log(`[remote-terminal-cloud-agent] platform artifact created at ${platformOutputRoot}`);
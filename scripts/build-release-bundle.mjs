import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "..");
const releaseRoot = path.join(projectRoot, "release");
const distRoot = path.join(projectRoot, "dist");
const packagingRoot = path.join(projectRoot, "packaging");
const packageJsonPath = path.join(projectRoot, "package.json");
const windowsPackagingRoot = path.join(packagingRoot, "windows");
const protocolDistRoot = path.join(projectRoot, "packages", "protocol", "dist");

function getExecutable(command) {
  if (process.platform !== "win32") {
    return command;
  }

  if (command === "pnpm") {
    return "pnpm.cmd";
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

function runCommand(command, args) {
  const invocation = createCommandInvocation(command, args);
  const result = spawnSync(invocation.file, invocation.args, {
    cwd: projectRoot,
    env: process.env,
    stdio: "inherit",
  });

  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with exit code ${result.status ?? "unknown"}`);
  }
}

if (!existsSync(distRoot)) {
  throw new Error("dist directory not found. Run the agent build before creating a release bundle.");
}

if (!existsSync(protocolDistRoot)) {
  throw new Error("packages/protocol/dist directory not found. Run the protocol build before creating a release bundle.");
}

const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
const releaseVersion = packageJson.version ?? "0.0.0";

rmSync(releaseRoot, { force: true, recursive: true });
mkdirSync(releaseRoot, { recursive: true });

const bundleRoot = path.join(releaseRoot, `remote-terminal-cloud-agent-${releaseVersion}`);
mkdirSync(bundleRoot, { recursive: true });

runCommand("pnpm", ["--filter", "@rtc/agent", "deploy", "--legacy", "--prod", bundleRoot]);

cpSync(distRoot, path.join(bundleRoot, "dist"), { recursive: true });
cpSync(path.join(projectRoot, "src"), path.join(bundleRoot, "src"), { recursive: true });
cpSync(packagingRoot, path.join(bundleRoot, "packaging"), { recursive: true });
cpSync(protocolDistRoot, path.join(bundleRoot, "packages", "protocol", "dist"), { recursive: true });

writeFileSync(
  path.join(bundleRoot, "package.json"),
  JSON.stringify(
    {
      ...JSON.parse(readFileSync(path.join(bundleRoot, "package.json"), "utf8")),
      scripts: {
        start: packageJson.scripts?.start,
      },
    },
    null,
    2,
  ),
);

writeFileSync(
  path.join(bundleRoot, "README.txt"),
  [
    "Remote Terminal Cloud Agent release bundle",
    "",
    `Version: ${releaseVersion}`,
    "",
    "This bundle contains the compiled agent plus production dependencies for downstream packaging.",
    "It is still a packaging foundation, not a finished MSI/PKG/DEB/RPM installer.",
    "Use the files under packaging/ as templates for service installation and downstream platform packaging.",
  ].join("\n"),
);

const platformDirs = ["windows", "linux", "macos"];
for (const platformDir of platformDirs) {
  mkdirSync(path.join(bundleRoot, "artifacts", platformDir), { recursive: true });
}

const windowsArtifactsRoot = path.join(bundleRoot, "artifacts", "windows");
cpSync(windowsPackagingRoot, path.join(windowsArtifactsRoot, "packaging"), { recursive: true });

writeFileSync(
  path.join(windowsArtifactsRoot, "BUILDING-MSIS.txt"),
  [
    "Windows MSI packaging handoff",
    "",
    "1. Build this release bundle first.",
    "2. Run packaging\\windows\\wix\\prepare-msi-stage.ps1 against the bundle root.",
    "3. The staging script copies dist/, copies packaging/windows/, copies the Windows Node runtime into runtime/, downloads WinSW into service/, and writes MSI-INPUTS.json.",
    "4. Run packaging\\windows\\wix\\build-msi.ps1 using artifacts\\windows\\msi-build-root as AgentBuildRoot.",
  ].join("\n"),
);

console.log(`[remote-terminal-cloud-agent] release bundle created at ${bundleRoot}`);
import { getAgentRuntimeConfig } from "./config";
import { registerAgent, sendHeartbeat } from "./api";
import { collectHostSnapshot } from "./platform";
import { resolveEffectiveDefaultShell } from "./shells";
import { runAgentTunnel } from "./tunnel";

const agentVersion = "0.1.0";
const missingConfigRetryMs = 30_000;
const runtimeRetryMs = 10_000;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function runAgentOnce(): Promise<void> {
  const config = getAgentRuntimeConfig();
  const snapshot = await collectHostSnapshot(agentVersion, config.enabledShellTypes);
  const effectiveDefaultShell = resolveEffectiveDefaultShell(
    config.defaultShellType,
    snapshot.diagnostics.availableShells,
  );

  console.log(`[remote-terminal-cloud-agent] config file: ${config.configFilePath}`);
  console.log("[remote-terminal-cloud-agent] host snapshot");
  console.log(JSON.stringify(snapshot, null, 2));
  console.log(
    `[remote-terminal-cloud-agent] shell capabilities: ${snapshot.diagnostics.availableShells.join(", ") || "none"}`,
  );

  if (effectiveDefaultShell !== config.defaultShellType) {
    console.warn(
      `[remote-terminal-cloud-agent] RTC_DEFAULT_SHELL=${config.defaultShellType} is unavailable; fallback to ${effectiveDefaultShell}.`,
    );
  }

  if (snapshot.diagnostics.availableShells.length === 0) {
    console.warn("[remote-terminal-cloud-agent] no shells available after detection/config filtering.");
  }

  if (!snapshot.diagnostics.sshCheck.available) {
    console.warn("[remote-terminal-cloud-agent] SSH precheck failed.");
  }

  if (!config.registrationToken) {
    console.warn(
      `[remote-terminal-cloud-agent] waiting for configuration: set RTC_REGISTRATION_TOKEN in ${config.configFilePath} or environment, then the service will retry automatically.`,
    );
    await sleep(missingConfigRetryMs);
    return;
  }

  let session = await registerAgent(config.serverBaseUrl, config.registrationToken, snapshot);
  console.log(`[remote-terminal-cloud-agent] registered device ${session.deviceId}`);

  const tasks: Promise<unknown>[] = [];

  if (!config.runHeartbeat) {
    console.log("[remote-terminal-cloud-agent] heartbeat disabled by RTC_DISABLE_HEARTBEAT=1");
  } else {
    tasks.push(
      (async () => {
        while (true) {
          await sleep(session.heartbeatIntervalSeconds * 1_000);
          const heartbeatSnapshot = await collectHostSnapshot(agentVersion, config.enabledShellTypes);
          session = await sendHeartbeat(config.serverBaseUrl, session, heartbeatSnapshot);
          console.log(
            `[remote-terminal-cloud-agent] heartbeat ok for ${session.deviceId}; next interval ${session.heartbeatIntervalSeconds}s`,
          );
        }
      })(),
    );
  }

  if (!config.runTunnel) {
    console.log("[remote-terminal-cloud-agent] tunnel disabled by RTC_DISABLE_TUNNEL=1");
  } else {
    tasks.push(runAgentTunnel(config.serverBaseUrl, session, effectiveDefaultShell, config.preferencesFilePath));
  }

  if (tasks.length === 0) {
    console.warn("[remote-terminal-cloud-agent] heartbeat and tunnel are both disabled; retrying later.");
    await sleep(missingConfigRetryMs);
    return;
  }

  await Promise.all(tasks);
}

async function main(): Promise<never> {
  while (true) {
    try {
      await runAgentOnce();
    } catch (error: unknown) {
      console.error("[remote-terminal-cloud-agent] runtime error; retrying", error);
      await sleep(runtimeRetryMs);
    }
  }
}

main().catch((error: unknown) => {
  console.error("[remote-terminal-cloud-agent] fatal bootstrap error", error);
  process.exitCode = 1;
});
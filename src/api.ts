import type {
  AgentHeartbeatRequest,
  AgentHeartbeatResponse,
  AgentRegistrationRequest,
  AgentRegistrationResponse,
  HostSnapshot,
} from "@rtc/protocol";

export interface RegisteredAgentSession {
  deviceId: string;
  heartbeatToken: string;
  heartbeatIntervalSeconds: number;
}

async function postJson<TRequest, TResponse>(
  url: string,
  body: TRequest,
): Promise<TResponse> {
  const response = await fetch(url, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(`Request failed (${response.status}): ${text}`);
  }

  return (await response.json()) as TResponse;
}

export async function registerAgent(
  serverBaseUrl: string,
  registrationToken: string,
  snapshot: HostSnapshot,
): Promise<RegisteredAgentSession> {
  const payload: AgentRegistrationRequest = {
    registrationToken,
    snapshot,
  };

  const response = await postJson<AgentRegistrationRequest, AgentRegistrationResponse>(
    `${serverBaseUrl}/remote-terminal/agent/register`,
    payload,
  );

  return {
    deviceId: response.deviceId,
    heartbeatToken: response.heartbeatToken,
    heartbeatIntervalSeconds: response.heartbeatIntervalSeconds,
  };
}

export async function sendHeartbeat(
  serverBaseUrl: string,
  session: RegisteredAgentSession,
  snapshot: HostSnapshot,
): Promise<RegisteredAgentSession> {
  const payload: AgentHeartbeatRequest = {
    deviceId: session.deviceId,
    heartbeatToken: session.heartbeatToken,
    snapshot,
  };

  const response = await postJson<AgentHeartbeatRequest, AgentHeartbeatResponse>(
    `${serverBaseUrl}/remote-terminal/agent/heartbeat`,
    payload,
  );

  return {
    ...session,
    heartbeatIntervalSeconds: response.nextHeartbeatIntervalSeconds,
  };
}

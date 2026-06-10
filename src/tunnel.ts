import type {
  AgentToServerMessage,
  RemoteTerminalAgentPreferencesData,
  ShellType,
  ServerToAgentMessage,
} from "@rtc/protocol";
import WebSocket from "ws";
import type { RegisteredAgentSession } from "./api";
import { AgentPreferencesStore } from "./preferences";
import { ShellSessionManager } from "./shellSessionManager";

function sendJson(socket: WebSocket, payload: AgentToServerMessage): void {
  socket.send(JSON.stringify(payload));
}

export async function runAgentTunnel(
  serverBaseUrl: string,
  session: RegisteredAgentSession,
  defaultShellType: ShellType,
  preferencesFilePath: string,
): Promise<never> {
  const webSocketUrl = `${serverBaseUrl.replace(/^http/i, "ws")}/remote-terminal/ws?role=agent&deviceId=${session.deviceId}&heartbeatToken=${session.heartbeatToken}`;
  const shellManager = new ShellSessionManager(defaultShellType);
  const preferencesStore = new AgentPreferencesStore(preferencesFilePath);

  return await new Promise<never>((_resolve, reject) => {
    const socket = new WebSocket(webSocketUrl);

    socket.on("open", () => {
      console.log(`[remote-terminal-cloud-agent] tunnel connected for ${session.deviceId}`);
    });

    socket.on("message", (buffer) => {
      const message = JSON.parse(buffer.toString()) as ServerToAgentMessage;
      if (message.type === "session-start") {
        shellManager.startSession(message.sessionId, message.shellType, {
          onReady() {
            sendJson(socket, {
              type: "session-ready",
              sessionId: message.sessionId,
            });
          },
          onOutput(stream, data) {
            sendJson(socket, {
              type: "session-output",
              sessionId: message.sessionId,
              stream,
              data,
            });
          },
          onExit(exitCode) {
            sendJson(socket, {
              type: "session-exit",
              sessionId: message.sessionId,
              exitCode,
            });
          },
          onError(errorMessage) {
            sendJson(socket, {
              type: "session-error",
              sessionId: message.sessionId,
              message: errorMessage,
            });
          },
        }, message.workingDirectory);
        return;
      }

      if (message.type === "session-input") {
        shellManager.writeInput(message.sessionId, message.data);
        return;
      }

      if (message.type === "session-resize") {
        shellManager.resizeSession(message.sessionId, message.cols, message.rows);
        return;
      }

      if (message.type === "session-stop") {
        shellManager.stopSession(message.sessionId);
        return;
      }

      if (message.type === "directory-browse") {
        try {
          const result = shellManager.browseDirectories(message.path);
          sendJson(socket, {
            type: "directory-browse-result",
            requestId: message.requestId,
            ok: true,
            currentPath: result.currentPath,
            parentPath: result.parentPath,
            items: result.items,
          });
        } catch (error) {
          sendJson(socket, {
            type: "directory-browse-result",
            requestId: message.requestId,
            ok: false,
            message: error instanceof Error ? error.message : "Unable to browse directory.",
            currentPath: message.path?.trim() ?? "",
            items: [],
          });
        }

        return;
      }

      if (message.type === "preferences-get") {
        sendJson(socket, {
          type: "preferences-result",
          requestId: message.requestId,
          ok: true,
          preferences: preferencesStore.getPreferences(),
        });
        return;
      }

      if (message.type === "preferences-set") {
        try {
          const preferences: RemoteTerminalAgentPreferencesData = preferencesStore.setPreferences(message.preferences);
          sendJson(socket, {
            type: "preferences-result",
            requestId: message.requestId,
            ok: true,
            preferences,
          });
        } catch (error) {
          sendJson(socket, {
            type: "preferences-result",
            requestId: message.requestId,
            ok: false,
            message: error instanceof Error ? error.message : "Unable to save preferences.",
            preferences: preferencesStore.getPreferences(),
          });
        }
      }
    });

    socket.on("close", () => {
      reject(new Error("Agent tunnel closed."));
    });

    socket.on("error", (error) => {
      reject(error);
    });
  });
}

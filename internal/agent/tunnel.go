package agent

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"

	"github.com/gorilla/websocket"

	"github.com/remote-terminal-cloud/agent/internal/protocol"
)

type rawServerMessage struct {
	Type string `json:"type"`
}

func sendJSON(conn *websocket.Conn, payload any) error {
	return conn.WriteJSON(payload)
}

func RunAgentTunnel(ctx context.Context, serverBaseURL string, session RegisteredAgentSession, defaultShellType protocol.ShellType, preferencesFilePath string) error {
	conn, websocketURL, err := connectAgentWebSocket(ctx, serverBaseURL, session)
	if err != nil {
		return err
	}
	defer conn.Close()

	fmt.Printf("[remote-terminal-cloud-agent] tunnel connected for %s\n", session.DeviceID)
	fmt.Printf("[remote-terminal-cloud-agent] websocket endpoint: %s\n", websocketURL)

	shellManager := NewShellSessionManager(defaultShellType)
	preferencesStore := NewPreferencesStore(preferencesFilePath)

	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}

		_, data, err := conn.ReadMessage()
		if err != nil {
			return err
		}

		var raw rawServerMessage
		if err := json.Unmarshal(data, &raw); err != nil {
			return err
		}

		switch raw.Type {
		case "session-start":
			var message protocol.SessionStartMessage
			if err := json.Unmarshal(data, &message); err != nil {
				return err
			}
			shellManager.StartSession(message.SessionID, message.ShellType, ShellSessionCallbacks{
				OnReady: func() {
					_ = sendJSON(conn, protocol.SessionReadyMessage{
						Type:      "session-ready",
						SessionID: message.SessionID,
					})
				},
				OnOutput: func(stream string, output string) {
					_ = sendJSON(conn, protocol.SessionOutputMessage{
						Type:      "session-output",
						SessionID: message.SessionID,
						Stream:    stream,
						Data:      output,
					})
				},
				OnExit: func(exitCode *int) {
					_ = sendJSON(conn, protocol.SessionExitMessage{
						Type:      "session-exit",
						SessionID: message.SessionID,
						ExitCode:  exitCode,
					})
				},
				OnError: func(messageText string) {
					_ = sendJSON(conn, protocol.SessionErrorMessage{
						Type:      "session-error",
						SessionID: message.SessionID,
						Message:   messageText,
					})
				},
			}, message.WorkingDirectory)

		case "session-input":
			var message protocol.SessionInputMessage
			if err := json.Unmarshal(data, &message); err != nil {
				return err
			}
			shellManager.WriteInput(message.SessionID, message.Data)

		case "session-resize":
			var message protocol.SessionResizeMessage
			if err := json.Unmarshal(data, &message); err != nil {
				return err
			}
			shellManager.ResizeSession(message.SessionID, message.Cols, message.Rows)

		case "session-stop":
			var message protocol.SessionStopMessage
			if err := json.Unmarshal(data, &message); err != nil {
				return err
			}
			shellManager.StopSession(message.SessionID)

		case "directory-browse":
			var message protocol.DirectoryBrowseRequestMessage
			if err := json.Unmarshal(data, &message); err != nil {
				return err
			}
			result, browseErr := shellManager.BrowseDirectories(message.Path)
			if browseErr != nil {
				_ = sendJSON(conn, protocol.DirectoryBrowseResultMessage{
					Type:        "directory-browse-result",
					RequestID:   message.RequestID,
					OK:          false,
					Message:     browseErr.Error(),
					CurrentPath: strings.TrimSpace(message.Path),
					Items:       []protocol.DirectoryEntry{},
				})
				continue
			}
			_ = sendJSON(conn, protocol.DirectoryBrowseResultMessage{
				Type:        "directory-browse-result",
				RequestID:   message.RequestID,
				OK:          true,
				CurrentPath: result.CurrentPath,
				ParentPath:  result.ParentPath,
				Items:       result.Items,
			})

		case "preferences-get":
			var message protocol.PreferencesGetMessage
			if err := json.Unmarshal(data, &message); err != nil {
				return err
			}
			_ = sendJSON(conn, protocol.PreferencesResultMessage{
				Type:        "preferences-result",
				RequestID:   message.RequestID,
				OK:          true,
				Preferences: preferencesStore.GetPreferences(),
			})

		case "preferences-set":
			var message protocol.PreferencesSetMessage
			if err := json.Unmarshal(data, &message); err != nil {
				return err
			}
			preferences, setErr := preferencesStore.SetPreferences(message.Preferences)
			if setErr != nil {
				_ = sendJSON(conn, protocol.PreferencesResultMessage{
					Type:        "preferences-result",
					RequestID:   message.RequestID,
					OK:          false,
					Message:     setErr.Error(),
					Preferences: preferencesStore.GetPreferences(),
				})
				continue
			}
			_ = sendJSON(conn, protocol.PreferencesResultMessage{
				Type:        "preferences-result",
				RequestID:   message.RequestID,
				OK:          true,
				Preferences: preferences,
			})
		}
	}
}

func connectAgentWebSocket(ctx context.Context, serverBaseURL string, session RegisteredAgentSession) (*websocket.Conn, string, error) {
	headers := http.Header{}
	candidates, err := buildAgentWebSocketCandidates(serverBaseURL, session)
	if err != nil {
		return nil, "", err
	}

	var attempts []string
	for _, websocketURL := range candidates {
		conn, response, dialErr := websocket.DefaultDialer.DialContext(ctx, websocketURL, headers)
		if dialErr == nil {
			return conn, websocketURL, nil
		}

		attempts = append(attempts, formatWebSocketDialError(websocketURL, response, dialErr))
		if response != nil && response.StatusCode == http.StatusNotFound {
			continue
		}
		return nil, "", errors.New(strings.Join(attempts, "; "))
	}

	return nil, "", errors.New(strings.Join(attempts, "; "))
}

func buildAgentWebSocketCandidates(serverBaseURL string, session RegisteredAgentSession) ([]string, error) {
	if websocketURL := strings.TrimSpace(session.WebSocketURL); websocketURL != "" {
		return []string{websocketURL}, nil
	}

	websocketURL, err := buildAgentWebSocketURL(serverBaseURL, session, "/remote-terminal/ws")
	if err != nil {
		return nil, err
	}
	return []string{websocketURL}, nil
}

func buildAgentWebSocketURL(serverBaseURL string, session RegisteredAgentSession, websocketPath string) (string, error) {
	parsed, err := url.Parse(serverBaseURL)
	if err != nil {
		return "", err
	}

	switch parsed.Scheme {
	case "http":
		parsed.Scheme = "ws"
	case "https":
		parsed.Scheme = "wss"
	case "ws", "wss":
	default:
		return "", fmt.Errorf("unsupported server base URL scheme: %s", parsed.Scheme)
	}

	parsed.Path = strings.TrimRight(parsed.Path, "/") + websocketPath
	query := parsed.Query()
	query.Set("role", "agent")
	query.Set("deviceId", session.DeviceID)
	query.Set("heartbeatToken", session.HeartbeatToken)
	parsed.RawQuery = query.Encode()
	return parsed.String(), nil
}

func formatWebSocketDialError(websocketURL string, response *http.Response, dialErr error) string {
	if response == nil {
		return fmt.Sprintf("websocket dial %s failed: %v", websocketURL, dialErr)
	}

	bodyPreview := ""
	if response.Body != nil {
		defer response.Body.Close()
		if data, err := io.ReadAll(io.LimitReader(response.Body, 512)); err == nil {
			bodyPreview = strings.TrimSpace(string(data))
		}
	}

	if bodyPreview == "" {
		return fmt.Sprintf("websocket dial %s failed: status %d", websocketURL, response.StatusCode)
	}

	return fmt.Sprintf("websocket dial %s failed: status %d body %s", websocketURL, response.StatusCode, bodyPreview)
}

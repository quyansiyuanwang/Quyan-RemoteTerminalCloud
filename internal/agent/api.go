package agent

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"github.com/remote-terminal-cloud/agent/internal/protocol"
)

type RegisteredAgentSession struct {
	DeviceID                 string
	HeartbeatToken           string
	HeartbeatIntervalSeconds int
	WebSocketURL             string
}

type APIClient struct {
	httpClient *http.Client
}

func NewAPIClient() *APIClient {
	return &APIClient{
		httpClient: &http.Client{
			Timeout: 15 * time.Second,
		},
	}
}

func (c *APIClient) postJSON(ctx context.Context, url string, requestBody any, responseBody any) error {
	payload, err := json.Marshal(requestBody)
	if err != nil {
		return err
	}

	request, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(payload))
	if err != nil {
		return err
	}
	request.Header.Set("content-type", "application/json")

	response, err := c.httpClient.Do(request)
	if err != nil {
		return err
	}
	defer response.Body.Close()

	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		body, _ := io.ReadAll(response.Body)
		return fmt.Errorf("request failed (%d): %s", response.StatusCode, strings.TrimSpace(string(body)))
	}

	return json.NewDecoder(response.Body).Decode(responseBody)
}

func (c *APIClient) RegisterAgent(ctx context.Context, serverBaseURL string, registrationToken string, snapshot protocol.HostSnapshot) (RegisteredAgentSession, error) {
	requestBody := protocol.AgentRegistrationRequest{
		RegistrationToken: registrationToken,
		Snapshot:          snapshot,
	}

	var responseBody protocol.AgentRegistrationResponse
	if err := c.postJSON(ctx, strings.TrimRight(serverBaseURL, "/")+"/remote-terminal/agent/register", requestBody, &responseBody); err != nil {
		return RegisteredAgentSession{}, err
	}

	return RegisteredAgentSession{
		DeviceID:                 responseBody.DeviceID,
		HeartbeatToken:           responseBody.HeartbeatToken,
		HeartbeatIntervalSeconds: responseBody.HeartbeatIntervalSeconds,
		WebSocketURL:             strings.TrimSpace(responseBody.WebSocketURL),
	}, nil
}

func (c *APIClient) SendHeartbeat(ctx context.Context, serverBaseURL string, session RegisteredAgentSession, snapshot protocol.HostSnapshot) (RegisteredAgentSession, error) {
	requestBody := protocol.AgentHeartbeatRequest{
		DeviceID:       session.DeviceID,
		HeartbeatToken: session.HeartbeatToken,
		Snapshot:       snapshot,
	}

	var responseBody protocol.AgentHeartbeatResponse
	if err := c.postJSON(ctx, strings.TrimRight(serverBaseURL, "/")+"/remote-terminal/agent/heartbeat", requestBody, &responseBody); err != nil {
		return RegisteredAgentSession{}, err
	}

	session.HeartbeatIntervalSeconds = responseBody.NextHeartbeatIntervalSeconds
	if value := strings.TrimSpace(responseBody.WebSocketURL); value != "" {
		session.WebSocketURL = value
	}
	return session, nil
}

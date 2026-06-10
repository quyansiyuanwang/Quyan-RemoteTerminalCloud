package agent

import (
	"bytes"
	"context"
	"os/exec"
	"strings"
	"time"
)

type CommandResult struct {
	OK       bool
	Stdout   string
	Stderr   string
	ExitCode int
}

func runCommand(file string, args []string, timeout time.Duration) CommandResult {
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	defer cancel()

	cmd := exec.CommandContext(ctx, file, args...)
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	err := cmd.Run()
	result := CommandResult{
		OK:       err == nil,
		Stdout:   strings.TrimSpace(stdout.String()),
		Stderr:   strings.TrimSpace(stderr.String()),
		ExitCode: 0,
	}

	if err == nil {
		return result
	}

	if exitError, ok := err.(*exec.ExitError); ok {
		result.ExitCode = exitError.ExitCode()
	} else {
		result.ExitCode = -1
	}

	return result
}

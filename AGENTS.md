# AGENTS

## Project Overview

This repository contains the Go-based `Remote Terminal Cloud Agent`.

- Main runtime entry: `cmd/rtc-agent`
- Windows desktop manager: `cmd/rtc-agent-manager`
- Release/build pipeline: `cmd/rtc-release`
- Core runtime logic: `internal/agent`
- Packaging assets: `packaging/windows`, `packaging/linux`, `packaging/macos`

The old TypeScript / pnpm agent workspace has already been removed. Assume this is a pure Go project.

## Common Commands

- Build all packages: `go build ./...`
- Run tests: `go test ./...`
- Run agent locally: `go run ./cmd/rtc-agent`
- Show CLI help: `go run ./cmd/rtc-agent help`
- Build release binaries: `go run ./cmd/rtc-release build`
- Build release bundle: `go run ./cmd/rtc-release bundle`
- Build platform artifact: `go run ./cmd/rtc-release artifact`

## Product Direction

Current priority is to make the Windows experience feel like a real product instead of a developer tool.

- Prefer native product UX over PowerShell-driven UX
- `rtc-agent-manager.exe` is the main Windows management entry
- Keep the background agent/service architecture intact
- Avoid reintroducing server URL configuration in user-facing flows
- Registration token setup should be simple, guided, and persistent

## Windows Notes

- The service name is `RemoteTerminalCloudAgent`
- Default user config path is `%APPDATA%\remote-terminal-cloud-agent\config.json`
- Installer upgrades must stop the existing service before overwriting binaries
- Start Menu shortcuts should point to the native manager wherever possible
- Avoid visible console windows for the manager in release builds

## Editing Guidelines

- Prefer small, focused changes that preserve the current architecture
- Use existing helpers in `internal/agent` before adding duplicate logic
- Keep cross-platform builds working; add Windows-specific files with build tags when needed
- Treat `build/` and `release/` as disposable output directories
- Do not commit generated binaries or installer artifacts unless explicitly requested
- Do not add new business logic or install/runtime logic to PowerShell or bash scripts when it can live in Go
- Packaging scripts should be thin wrappers around Go binaries, not the source of truth for product behavior

## Validation Expectations

After meaningful code changes, run:

- `go test ./...`

When touching the Windows manager or release flow, also run:

- `go build ./cmd/rtc-agent-manager`
- `go run ./cmd/rtc-release build`

## Documentation

If behavior changes for end users, update the relevant docs:

- Root `README.md` for product usage
- `packaging/windows/README.md` for Windows install/manage behavior

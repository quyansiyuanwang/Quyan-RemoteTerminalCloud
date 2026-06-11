# AGENTS

## Project Overview

This repository is now a `Rust workspace + Tauri` implementation of `Remote Terminal Cloud Agent`.

- Workspace root: `Cargo.toml`
- Main apps: `apps/rtc-agentd`, `apps/rtc-agent-installer`, `apps/rtc-agent-desktop`
- Shared crates: `crates/rtc-agent-*`
- Build/release entry: `cargo xtask`
- Packaging assets: `packaging/windows`, `packaging/linux`, `packaging/macos`

Go is no longer part of the main product implementation.

## Common Commands

- Check Rust workspace: `cargo check`
- Run Rust agent CLI: `cargo run -p rtc-agentd -- status --json`
- Run Rust installer CLI: `cargo run -p rtc-agent-installer -- windows status --json`
- Run Rust release entry: `cargo xtask build`
- Build desktop frontend: `npm install` then `npm run build` in `apps/rtc-agent-desktop`

## Product Direction

- `rtc-agent-desktop` is the primary product entry on desktop platforms
- `rtc-agentd` remains the background runtime and CLI surface
- `rtc-agent-installer` handles config, install, service compatibility, and path helpers
- `cargo xtask` is the single release/build orchestration entry
- Prefer native product UX over script-driven UX

## Windows Notes

- The service name is `RemoteTerminalCloudAgent`
- Default user config path is `%APPDATA%\remote-terminal-cloud-agent\config.json`
- Installer upgrades must stop the existing service before overwriting binaries
- Start Menu shortcuts should point to the desktop manager
- Avoid visible console windows for user-facing desktop flows

## Editing Guidelines

- Prefer Rust-first changes for product behavior
- Keep scripts thin wrappers around Rust entrypoints
- Keep cross-platform builds working; gate platform-specific behavior cleanly
- Treat `build/` and `release/` as disposable output directories
- Do not commit generated binaries or installer artifacts unless explicitly requested

## Validation Expectations

After meaningful code changes, run the checks that match the touched stack:

- `cargo check`

When touching desktop or release flow, also run:

- `cargo run -p rtc-agentd -- version --json`
- `cargo xtask build`
- `cargo xtask bundle`
- `npm run build` in `apps/rtc-agent-desktop`

## Documentation

If behavior changes for end users, update the relevant docs:

- Root `README.md` for product usage
- `packaging/windows/README.md` for Windows install/manage behavior
- `docs/AGENT_DEPLOYMENT_FOUNDATION.md` for deployment/release flow

# Agent Deployment Foundation

This document describes the current deployment foundation for `Products/remote-terminal-cloud/`.

## Current status

The repository now includes:

- config-file based runtime configuration
- Windows desktop-first NSIS installer authoring
- Windows packaging helpers that assemble the desktop app and sidecars
- native Windows Service hosting in `rtc-agentd`
- Linux `systemd` unit, install/uninstall scripts, and `deb` packaging builder
- macOS `launchd` plist, install/uninstall scripts, and `pkg` packaging builder
- a Rust release-bundle builder that assembles `bin/`, Rust source snapshots, and `packaging/`
- a GitHub Actions multi-platform build workflow for Linux/macOS/Windows artifacts

The repository still does **not** include finished:

- `rpm` packaging
- code signing / notarization
- auto-update delivery

## Release bundle

Run from `Products/remote-terminal-cloud/`:

- `cargo xtask bundle`

Output:

- `release/remote-terminal-cloud-agent-<version>/`

Bundle contents:

- `bin/` — compiled Rust binaries for agent, installer, and desktop app
- `apps/` / `crates/` / `xtask/` — Rust source snapshot for inspection and debugging
- `packaging/` — platform service and installer templates
- `artifacts/windows/` — Windows NSIS packaging handoff files
- `artifacts/<platform>/` — downstream installer placeholders for each platform

Windows packaging flow:

1. Create the release bundle.
2. Run `cargo xtask package` for the default Windows desktop package.

Default Windows staging is desktop-first:

- bundles `rtc-agent-desktop.exe` as the main app
- initializes onboarding through the desktop UI after install
- does not require WinSW/service payloads in the default package

Optional managed-service flows:

- Linux service commands route to the packaged `remote-terminal-cloud-agent.service` via `systemctl`
- Windows service install uses the native `rtc-agentd.exe service-host` entrypoint
- compatibility `rtc-agentd start` / `rtc-agentd stop` commands prefer the platform service manager before falling back to legacy PID behavior

## CI artifacts

Workflow file:

- `.github/workflows/build-multi-platform.yml`

Current CI outputs:

- Linux: `release/artifacts/linux-x64/*.tar.gz` and `*.deb`
- macOS: `release/artifacts/darwin-arm64/*.tar.gz` and `*.pkg`
- Windows: `release/artifacts/win32-x64/*.zip`
- Windows NSIS: `release/artifacts/windows-installers/nsis/*.exe`
- GitHub Release on `v*` tags: all archived assets above plus `SHA256SUMS.txt`

Release automation rules:

- tag pattern: `v*`
- tag/version validation: `github.ref_name` must equal `v${VERSION}`
- prerelease detection: any tag containing `-` is published as a prerelease

## Runtime configuration

The agent reads configuration in this order:

1. environment variables
2. JSON config file
3. built-in defaults

Supported JSON keys:

- `registrationToken`
- `runHeartbeat`
- `runTunnel`
- `defaultShellType`
- `enabledShellTypes`
- `preferencesFilePath`

Built-in server targets:

- local development default: `http://localhost:10001`
- packaged release binaries: `https://api.qysyw.cn`

Default config file paths:

- Windows: `%APPDATA%\remote-terminal-cloud-agent\config.json`
- macOS: `~/Library/Application Support/remote-terminal-cloud-agent/config.json`
- Linux: `~/.config/remote-terminal-cloud-agent/config.json`

Override config path with:

- `RTC_CONFIG_FILE`

## Recommended next steps

1. harden Windows service install/update flow and remove remaining legacy wrapper references
2. add macOS signing and notarization pipeline
3. add release publishing provenance
4. extend Linux packaging from `deb` to `rpm`
5. add code signing for Windows desktop binaries

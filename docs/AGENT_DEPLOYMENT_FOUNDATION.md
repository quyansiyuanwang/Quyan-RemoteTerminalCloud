# Agent Deployment Foundation

This document describes the current deployment foundation for `Products/remote-terminal-cloud/`.

## Current status

The repository now includes:

- config-file based runtime configuration
- Windows desktop-first NSIS and WiX installer authoring
- Windows staging builders that assemble `bin/` and `packaging/windows/`, with optional service payload support
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

- `bin/` — compiled Rust binaries for agent, installer, desktop app, and compatibility manager alias
- `apps/` / `crates/` / `xtask/` — Rust source snapshot for inspection and debugging
- `packaging/` — platform service and installer templates
- `artifacts/windows/` — Windows NSIS/MSI packaging handoff files
- `artifacts/<platform>/` — downstream installer placeholders for each platform

Windows packaging flow:

1. Create the release bundle.
2. Run `cargo xtask windows-nsis-stage --force` for the EXE installer build root.
3. Run `cargo xtask windows-nsis-build` against `artifacts/windows/installer-build-root/`.
4. Run `cargo xtask windows-msi-stage --force` for the MSI build root.
5. Run `cargo xtask windows-msi-build --accept-eula` against `artifacts/windows/msi-build-root/`.

Default Windows staging is desktop-first:

- bundles `rtc-agent-desktop.exe` as the main app
- initializes onboarding through the desktop UI after install
- does not require WinSW/service payloads unless `--include-service` is explicitly requested
- auto-detects `makensis.exe` from `PATH`, standard install roots, and common package-manager layouts before requiring `--nsis-exe`

## CI artifacts

Workflow file:

- `.github/workflows/build-multi-platform.yml`

Current CI outputs:

- Linux: `release/artifacts/linux-x64/*.tar.gz` and `*.deb`
- macOS: `release/artifacts/darwin-arm64/*.tar.gz` and `*.pkg`
- Windows: `release/artifacts/win32-x64/*.zip`
- Windows NSIS: `release/artifacts/windows-installers/nsis/*.exe`
- Windows MSI: `release/artifacts/windows-installers/msi/*.msi`
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

1. implement native Rust service backends beyond current compatibility scaffolding
2. add macOS signing and notarization pipeline
3. add release publishing provenance
4. extend Linux packaging from `deb` to `rpm`
5. add code signing for Windows MSI and desktop binaries

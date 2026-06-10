# Agent Deployment Foundation

This document describes the current deployment foundation for `Products/remote-terminal-cloud/`.

## Current status

The repository now includes:

- config-file based runtime configuration
- Windows service installation scripts via WinSW skeleton
- Windows WiX MSI authoring skeleton
- Windows MSI staging builder that assembles `bin/`, `service/`, and `packaging/windows/`
- Linux `systemd` unit, install/uninstall scripts, and `deb` packaging builder
- macOS `launchd` plist, install/uninstall scripts, and `pkg` packaging builder
- a release-bundle builder that assembles `bin/`, source snapshots, and `packaging/`
- a GitHub Actions multi-platform build workflow for Linux/macOS/Windows artifacts

The repository still does **not** include finished:

- `rpm` packaging
- code signing / notarization
- auto-update delivery

## Release bundle

Run from `Products/remote-terminal-cloud/`:

- `go run ./cmd/rtc-release bundle`

Output:

- `release/remote-terminal-cloud-agent-<version>/`

Bundle contents:

- `bin/` — compiled Go agent binary
- `cmd/` / `internal/` — source snapshot for inspection/debugging
- `packaging/` — platform service templates
- `artifacts/windows/` — Windows MSI/service packaging handoff files
- `artifacts/<platform>/` — downstream installer placeholders for each platform

Windows MSI flow:

1. Create the release bundle.
2. Run `packaging/windows/wix/prepare-msi-stage.ps1` with the bundle root.
3. The script downloads WinSW, creates `service/RemoteTerminalCloudAgentService.exe`, copies `RemoteTerminalCloudAgentService.xml`, and copies `bin/rtc-agent.exe`.
4. Run `packaging/windows/wix/build-msi.ps1` against the generated `artifacts/windows/msi-build-root/`.
5. For WiX 7 CLI, pass `-AcceptEula` or accept the EULA separately before building.

## CI artifacts

Workflow file:

- `.github/workflows/build-multi-platform.yml`

Current CI outputs:

- Linux: `release/artifacts/linux-x64/*.tar.gz` and `*.deb`
- macOS: `release/artifacts/darwin-arm64/*.tar.gz` and `*.pkg`
- Windows: `release/artifacts/win32-x64/*.zip`
- Windows MSI: `release/remote-terminal-cloud-agent-<version>/artifacts/windows/msi-build-root/artifacts/windows/out/*.msi`
- GitHub Release on `v*` tags: all archived assets above plus `SHA256SUMS.txt`

Release automation rules:

- tag pattern: `v*`
- tag/version validation: `github.ref_name` must equal `v${VERSION}`
- prerelease detection: any tag containing `-` is published as a prerelease

The CI pipeline now publishes GitHub Releases automatically for version tags, but it still does not add signing, notarization, or external distribution publishing.

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

- local development runs (`go run ./cmd/rtc-agent`): `http://localhost:10001`
- packaged release binaries (`go run ./cmd/rtc-release build|bundle|artifact`): `https://api.qysyw.cn`

Default config file paths:

- Windows: `%APPDATA%\remote-terminal-cloud-agent\config.json`
- macOS: `~/Library/Application Support/remote-terminal-cloud-agent/config.json`
- Linux: `~/.config/remote-terminal-cloud-agent/config.json`

Override config path with:

- `RTC_CONFIG_FILE`

## Recommended next steps

1. add Linux post-install scripts for service user creation
2. add macOS signing and notarization pipeline
3. add release publishing, checksums, and provenance
4. extend Linux packaging from `deb` to `rpm`
5. add code signing for Windows MSI and service binaries

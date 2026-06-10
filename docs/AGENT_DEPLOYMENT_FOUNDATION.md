# Agent Deployment Foundation

This document describes the current deployment foundation for `Products/remote-terminal-cloud/`.

## Current status

The repository now includes:

- config-file based runtime configuration
- Windows service installation scripts via WinSW skeleton
- Windows WiX MSI authoring skeleton
- Windows MSI staging builder that assembles `dist/`, `runtime/`, `service/`, and `packaging/windows/`
- Linux `systemd` unit and install/uninstall script templates
- macOS `launchd` plist and install/uninstall script templates
- a release-bundle builder that assembles `dist/` and `packaging/`

The repository still does **not** include finished:

- fully built `msi` packaging output
- `pkg` packaging
- `deb`/`rpm` packaging
- code signing / notarization
- auto-update delivery

## Release bundle

Run from `Products/remote-terminal-cloud/`:

- `pnpm build:bundle`

Output:

- `release/remote-terminal-cloud-agent-<version>/`

Bundle contents:

- `dist/` — compiled agent
- `src/` — source snapshot for inspection/debugging
- `packaging/` — platform service templates
- `artifacts/windows/` — Windows MSI/service packaging handoff files
- `artifacts/<platform>/` — downstream installer placeholders for each platform

Windows MSI flow:

1. Create the release bundle.
2. Run `packaging/windows/wix/prepare-msi-stage.ps1` with the bundle root plus a Windows Node runtime directory.
3. The script downloads WinSW, creates `service/RemoteTerminalCloudAgentService.exe`, copies `RemoteTerminalCloudAgentService.xml`, and copies the Node runtime into `runtime/`.
4. Run `packaging/windows/wix/build-msi.ps1` against the generated `artifacts/windows/msi-build-root/`.
5. For WiX 7 CLI, pass `-AcceptEula` or accept the EULA separately before building.

## Runtime configuration

The agent reads configuration in this order:

1. environment variables
2. JSON config file
3. built-in defaults

Supported JSON keys:

- `serverBaseUrl`
- `registrationToken`
- `runHeartbeat`
- `runTunnel`
- `defaultShellType`
- `enabledShellTypes`
- `preferencesFilePath`

Default config file paths:

- Windows: `%APPDATA%\remote-terminal-cloud-agent\config.json`
- macOS: `~/Library/Application Support/remote-terminal-cloud-agent/config.json`
- Linux: `~/.config/remote-terminal-cloud-agent/config.json`

Override config path with:

- `RTC_CONFIG_FILE`

## Recommended next steps

1. bundle a Windows Node runtime and WinSW binary into release assembly
2. harden the validated WiX MSI flow on a Windows release runner
3. add Linux post-install scripts for service user creation
4. add macOS signing and notarization pipeline
5. add CI matrix for Windows/macOS/Linux release artifacts

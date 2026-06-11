# Windows packaging

This directory contains the Windows packaging scaffolding for the desktop-first product shape.

## Default product model

- Primary Windows app: `rtc-agent-desktop.exe`
- Start Menu entry: `Remote Terminal Cloud Agent`
- Config path: `%APPDATA%\remote-terminal-cloud-agent\config.json`
- Logs path: `%APPDATA%\remote-terminal-cloud-agent\logs`

The default Windows install path is now:

- install the desktop manager
- launch it after install
- let the desktop app handle first-run token onboarding, tray management, and background agent lifecycle

The agent server address is built into release binaries as `https://api.qysyw.cn`. The default `config.json` template only leaves `registrationToken` empty.

## Optional service mode

Windows service mode is no longer the default packaging path.

- `WinSW` remains available for compatibility and migration
- `RemoteTerminalCloudAgentService.xml` remains in the repo as an optional payload
- `cargo xtask windows-*-stage` defaults to desktop-first staging without service wrapper files
- pass `--include-service` only when you explicitly want the legacy service payload in a staged build root

## Files

- `apps/rtc-agent-desktop/` — Tauri desktop manager
- `apps/rtc-agent-installer/` — Rust Windows install/admin helper
- `bin/rtc-agent-installer.exe` — native Windows install/admin helper used by NSIS, WiX, and shortcuts
- `bin/rtc-agent-manager.exe` — compatibility manager binary mapped to the desktop app
- `RemoteTerminalCloudAgentService.xml` — optional WinSW service definition for compatibility mode
- `wix/RemoteTerminalCloudAgent.wxs` — WiX authoring for desktop-first MSI
- `nsis/agent.nsi` — NSIS authoring for desktop-first EXE installer
- `cargo xtask` — preferred staging and installer build entry point

## Default staged layout

Desktop-first `windows-msi-stage` and `windows-nsis-stage` build roots require:

- `bin/rtc-agent.exe`
- `bin/rtc-agent-desktop.exe`
- `bin/rtc-agent-installer.exe`
- `bin/rtc-agent-manager.exe`
- `packaging/windows/`
- `artifacts/windows/out/`

Service wrapper files are optional and only appear when staging with `--include-service`.

## Build Windows installers

Recommended order:

1. `cargo xtask bundle`
2. `cargo xtask windows-nsis-stage --force`
3. `cargo xtask windows-nsis-build`
4. `cargo xtask windows-msi-stage --force`
5. `cargo xtask windows-msi-build --accept-eula`

### NSIS

```bash
cargo xtask windows-nsis-stage \
  --bundle-root "D:\path\to\remote-terminal-cloud-agent-0.2.0" \
  --force

cargo xtask windows-nsis-build \
  --build-root "D:\path\to\remote-terminal-cloud-agent-0.2.0\artifacts\windows\installer-build-root"
```

`windows-nsis-build` will first try `PATH`, then probe common `NSIS` install roots including standard `Program Files`, `WinGet`, `Chocolatey`, `Scoop`, and `NSIS_HOME` / `NSIS_ROOT`. If none are found, pass `--nsis-exe <path>`.

### MSI

```bash
cargo xtask windows-msi-stage \
  --bundle-root "D:\path\to\remote-terminal-cloud-agent-0.2.0" \
  --force

cargo xtask windows-msi-build \
  --build-root "D:\path\to\remote-terminal-cloud-agent-0.2.0\artifacts\windows\msi-build-root" \
  --output-dir "D:\path\to\remote-terminal-cloud-agent-0.2.0\artifacts\windows\msi-build-root\artifacts\windows\out" \
  --accept-eula
```

If you are using WiX 7 CLI, `--accept-eula` is recommended so the Rust wrapper forwards the required EULA flag automatically.

### Outputs

- NSIS default output: `artifacts/windows/installer-build-root/artifacts/windows/out/`
- MSI default output: `artifacts/windows/msi-build-root/artifacts/windows/out/`

## User-facing behavior

Both NSIS and MSI align to the same default behavior:

- install `rtc-agent-desktop.exe`
- create Start Menu shortcuts for launch, token configuration, config folder, and logs
- initialize config if missing
- launch the desktop app after install

The preferred user path is to manage the product from the desktop window and tray, not by browsing into `bin\` manually.

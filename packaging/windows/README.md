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

- `RemoteTerminalCloudAgentService.xml` remains only as legacy compatibility material
- the default package does not include a service wrapper

## Files

- `apps/rtc-agent-desktop/` — Tauri desktop manager
- `apps/rtc-agent-installer/` — Rust Windows install/admin helper
- `bin/rtc-agent-installer.exe` — native Windows install/admin helper used by NSIS and shortcuts
- `RemoteTerminalCloudAgentService.xml` — optional WinSW service definition for compatibility mode
- `nsis/agent.nsi` — NSIS authoring for desktop-first EXE installer
- `cargo xtask` — preferred staging and installer build entry point

## Build Windows installer

```bash
cargo xtask package
```

This produces the desktop-first installer under `release/artifacts/windows-installers/tauri/nsis/`.

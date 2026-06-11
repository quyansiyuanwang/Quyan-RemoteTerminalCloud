# Windows packaging

This directory contains the Windows packaging scaffolding for the agent.

## Service model

- Service wrapper: `WinSW`
- Service id: `RemoteTerminalCloudAgent`
- Config path: `%ProgramData%\RemoteTerminalCloudAgent\config.json`
- Logs path: `%ProgramData%\RemoteTerminalCloudAgent\logs`

The default `config.json` template only leaves `registrationToken` empty. The agent server address is now built into the binary: local development runs connect to `http://localhost:10001`, while packaged release builds connect to `https://api.qysyw.cn`. Until a token is configured, the service will stay installed and keep retrying instead of terminating immediately.

When the agent is launched manually from an interactive terminal and no token is configured yet, it will prompt for the registration token once and save it into the configured `config.json`. The background Windows service remains non-interactive and continues retrying as before.

Core Windows install and management logic now lives in Go binaries. Packaging scripts in this folder are wrappers for staging and installer generation, not the source of truth for install behavior.

## Files

- `bin/rtc-agent-installer.exe` — native Windows install/admin helper used by NSIS, WiX, and shortcuts
- `bin/rtc-agent-manager.exe` — native Windows manager entry point installed with the product
- `RemoteTerminalCloudAgentService.xml` — WinSW service definition
- `download-winsw.ps1` — fetches a WinSW executable for packaging or staging
- `wix/RemoteTerminalCloudAgent.wxs` — WiX v4 MSI authoring skeleton
- `wix/prepare-msi-stage.ps1` — assembles a real WiX build root from the release bundle
- `wix/build-msi.ps1` — MSI build helper

## WiX build-root layout

- `bin/rtc-agent.exe` — compiled agent binary
- `bin/rtc-agent-installer.exe` — compiled native installer helper
- `service/RemoteTerminalCloudAgentService.exe` — WinSW binary
- `service/RemoteTerminalCloudAgentService.xml` — WinSW config
- `packaging/windows/` — staging templates and build wrappers
- `artifacts/windows/out/` — default MSI output directory

The WiX authoring expects `AgentBuildRoot` to follow this layout exactly.

During upgrade installs, both NSIS and WiX now invoke the native installer helper to stop the existing `RemoteTerminalCloudAgent` service and wait for `rtc-agent.exe` / `RemoteTerminalCloudAgentService.exe` to fully exit before replacing files. This avoids the common "Error opening file for writing" failure during overwrite installs.

The Windows installers also create Start Menu shortcuts so end users can manage the agent without browsing into the install directory manually.

The primary `Agent Manager` Start Menu entry now launches the native `rtc-agent-manager.exe` window. Token configuration is handled inside the window, so users no longer need to open PowerShell for everyday management.

## Build a real MSI

1. Build the agent bundle.
2. Run `wix/prepare-msi-stage.ps1` against the bundle root.
3. The staging script will:
   - copy `bin/rtc-agent.exe`
   - copy `packaging/windows/`
   - download WinSW into `service/RemoteTerminalCloudAgentService.exe`
   - copy `RemoteTerminalCloudAgentService.xml` into `service/`
4. Build MSI with WiX v4 using `wix/build-msi.ps1`.

If you are using WiX 7 CLI, `build-msi.ps1 -AcceptEula` will forward the required EULA flag automatically. This flag is not needed for WiX 6.

Example:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\wix\prepare-msi-stage.ps1 `
  -AgentBundleRoot "D:\path\to\remote-terminal-cloud-agent-0.1.0" `
  -Force

powershell -ExecutionPolicy Bypass -File packaging\windows\wix\build-msi.ps1 `
  -AgentBuildRoot "D:\path\to\remote-terminal-cloud-agent-0.1.0\artifacts\windows\msi-build-root" `
  -OutputDir "D:\path\to\remote-terminal-cloud-agent-0.1.0\artifacts\windows\msi-build-root\artifacts\windows\out"
```

# Windows packaging

This directory contains the first-pass Windows service and MSI scaffolding for the agent.

## Service model

- Service wrapper: `WinSW`
- Service id: `RemoteTerminalCloudAgent`
- Config path: `%ProgramData%\RemoteTerminalCloudAgent\config.json`
- Logs path: `%ProgramData%\RemoteTerminalCloudAgent\logs`

## Files

- `install-service.ps1` — installs and starts the Windows service via WinSW
- `uninstall-service.ps1` — stops and removes the Windows service via WinSW
- `RemoteTerminalCloudAgentService.xml` — WinSW service definition
- `download-winsw.ps1` — fetches a WinSW executable for packaging or staging
- `wix/RemoteTerminalCloudAgent.wxs` — WiX v4 MSI authoring skeleton
- `wix/prepare-msi-stage.ps1` — assembles a real WiX build root from the release bundle
- `wix/build-msi.ps1` — MSI build helper

## WiX build-root layout

- `dist/` — compiled agent
- `runtime/` — extracted Windows Node runtime directory
- `runtime/node.exe` — bundled Node runtime entry used by the service
- `service/RemoteTerminalCloudAgentService.exe` — WinSW binary
- `service/RemoteTerminalCloudAgentService.xml` — WinSW config
- `packaging/windows/` — support scripts used by install/uninstall
- `artifacts/windows/out/` — default MSI output directory

The WiX authoring expects `AgentBuildRoot` to follow this layout exactly.

## Runtime structure

`runtime/` should contain the full extracted Windows Node runtime that matches the target architecture.

Minimum expected file:

- `runtime/node.exe`

Recommended approach:

1. Download an official Windows Node.js zip for the target architecture.
2. Extract it.
3. Pass the extracted directory to `wix/prepare-msi-stage.ps1` with `-NodeRuntimeRoot`.

The staging script copies that directory into `runtime/` automatically.

## Build a real MSI

1. Build the agent bundle.
2. Run `wix/prepare-msi-stage.ps1` against the bundle root.
3. The staging script will:
   - copy `dist/`
   - copy `packaging/windows/`
   - copy the Windows Node runtime into `runtime/`
   - download WinSW into `service/RemoteTerminalCloudAgentService.exe`
   - copy `RemoteTerminalCloudAgentService.xml` into `service/`
4. Build MSI with WiX v4 using `wix/build-msi.ps1`.

If you are using WiX 7 CLI, `build-msi.ps1 -AcceptEula` will forward the required EULA flag automatically. This flag is not needed for WiX 6.

Example:

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\wix\prepare-msi-stage.ps1 `
  -AgentBundleRoot "D:\path\to\remote-terminal-cloud-agent-0.1.0" `
  -NodeRuntimeRoot "D:\path\to\node-v22.x-win-x64" `
  -Force

powershell -ExecutionPolicy Bypass -File packaging\windows\wix\build-msi.ps1 `
  -AgentBuildRoot "D:\path\to\remote-terminal-cloud-agent-0.1.0\artifacts\windows\msi-build-root" `
  -OutputDir "D:\path\to\remote-terminal-cloud-agent-0.1.0\artifacts\windows\msi-build-root\artifacts\windows\out"
```

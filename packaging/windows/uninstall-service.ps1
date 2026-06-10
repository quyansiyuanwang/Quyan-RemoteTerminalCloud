$ServiceName = "RemoteTerminalCloudAgent"
if (Test-Path (Join-Path $PSScriptRoot "service\RemoteTerminalCloudAgentService.exe")) {
  $InstallRoot = $PSScriptRoot
} else {
  $InstallRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
}
$WinSWExe = Join-Path $InstallRoot "service\RemoteTerminalCloudAgentService.exe"

$ErrorActionPreference = "Stop"

if (-not (Test-Path $WinSWExe)) {
	throw "WinSW executable not found at $WinSWExe"
}

Write-Host "Uninstalling Windows service $ServiceName"

Push-Location (Split-Path -Parent $WinSWExe)
try {
	& $WinSWExe stop | Out-Null
} catch {
}

& $WinSWExe uninstall
Pop-Location

Write-Host "Service removed via WinSW."
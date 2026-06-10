$ServiceName = "RemoteTerminalCloudAgent"
$PackageRoot = Split-Path -Parent $PSScriptRoot
$InstallRoot = Split-Path -Parent $PackageRoot
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
$ServiceName = "RemoteTerminalCloudAgent"
$ScriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
if (Test-Path (Join-Path $ScriptDir "service\RemoteTerminalCloudAgentService.exe")) {
  $InstallRoot = $ScriptDir
} elseif (Test-Path (Join-Path $ScriptDir "..\..\service\RemoteTerminalCloudAgentService.exe")) {
  $InstallRoot = (Resolve-Path (Join-Path $ScriptDir "..\..")).Path
} else {
  throw "Cannot locate install root from $ScriptDir"
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
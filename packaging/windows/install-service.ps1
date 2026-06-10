$ServiceName = "RemoteTerminalCloudAgent"
$DisplayName = "Remote Terminal Cloud Agent"
# $PSScriptRoot can be empty when invoked via nsExec in NSIS; fall back to MyInvocation.
$ScriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
if (Test-Path (Join-Path $ScriptDir "runtime\node.exe")) {
  $InstallRoot = $ScriptDir
} elseif (Test-Path (Join-Path $ScriptDir "..\..\runtime\node.exe")) {
  $InstallRoot = (Resolve-Path (Join-Path $ScriptDir "..\..")).Path
} else {
  throw "Cannot locate install root from $ScriptDir"
}
$NodeExe = Join-Path $InstallRoot "runtime\node.exe"
$AgentEntry = Join-Path $InstallRoot "dist\index.js"
$WinSWExe = Join-Path $InstallRoot "service\RemoteTerminalCloudAgentService.exe"
$WinSWXml = Join-Path $InstallRoot "service\RemoteTerminalCloudAgentService.xml"
$ConfigDir = Join-Path $env:ProgramData "RemoteTerminalCloudAgent"
$ConfigFile = Join-Path $ConfigDir "config.json"
$LogsDir = Join-Path $ConfigDir "logs"

$ErrorActionPreference = "Stop"

if (-not (Test-Path $ConfigDir)) {
  New-Item -ItemType Directory -Path $ConfigDir | Out-Null
}

if (-not (Test-Path $LogsDir)) {
  New-Item -ItemType Directory -Path $LogsDir | Out-Null
}

if (-not (Test-Path $ConfigFile)) {
  Copy-Item (Join-Path $PSScriptRoot "agent.config.json") $ConfigFile
}

if (-not (Test-Path $NodeExe)) {
  throw "Node runtime not found at $NodeExe"
}

if (-not (Test-Path $AgentEntry)) {
  throw "Agent entry not found at $AgentEntry"
}

if (-not (Test-Path $WinSWExe)) {
  throw "WinSW executable not found at $WinSWExe"
}

if (-not (Test-Path $WinSWXml)) {
  throw "WinSW config not found at $WinSWXml"
}

Write-Host "Installing Windows service $ServiceName"
Write-Host "Install root: $InstallRoot"
Write-Host "Config file: $ConfigFile"
Write-Host "Log directory: $LogsDir"

Push-Location (Split-Path -Parent $WinSWExe)
try {
  & $WinSWExe stop | Out-Null
} catch {
}

try {
  & $WinSWExe uninstall | Out-Null
} catch {
}

& $WinSWExe install
& $WinSWExe start
Pop-Location

Write-Host "Service installed via WinSW."
param(
  [ValidateSet("menu", "status", "start", "stop", "restart", "configure", "edit-config", "open-config-dir", "open-logs", "help")]
  [string]$Action = "menu"
)

$ErrorActionPreference = "Stop"
$ServiceName = "RemoteTerminalCloudAgent"
$ScriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }

function Resolve-InstallRoot {
  if (Test-Path (Join-Path $ScriptDir "bin\rtc-agent.exe")) {
    return $ScriptDir
  }
  if (Test-Path (Join-Path $ScriptDir "..\..\bin\rtc-agent.exe")) {
    return (Resolve-Path (Join-Path $ScriptDir "..\..")).Path
  }
  throw "Cannot locate install root from $ScriptDir"
}

function Get-ConfigPaths {
  $configDir = Join-Path $env:APPDATA "remote-terminal-cloud-agent"
  return [pscustomobject]@{
    InstallRoot = Resolve-InstallRoot
    ConfigDir = $configDir
    ConfigFile = Join-Path $configDir "config.json"
    PreferencesFile = Join-Path $configDir "preferences.json"
    LogsDir = Join-Path $env:ProgramData "RemoteTerminalCloudAgent\logs"
  }
}

function Show-Help {
  Write-Host "Remote Terminal Cloud Agent Manager"
  Write-Host ""
  Write-Host "Usage:"
  Write-Host "  powershell -ExecutionPolicy Bypass -File manage-agent.ps1 <action>"
  Write-Host ""
  Write-Host "Actions:"
  Write-Host "  menu            Interactive menu"
  Write-Host "  status          Show current service and config status"
  Write-Host "  start           Start the Windows service"
  Write-Host "  stop            Stop the Windows service"
  Write-Host "  restart         Restart the Windows service"
  Write-Host "  configure       Run rtc-agent conf"
  Write-Host "  edit-config     Open config.json in Notepad"
  Write-Host "  open-config-dir Open the config directory"
  Write-Host "  open-logs       Open the log directory"
  Write-Host "  help            Show this help"
}

function Get-ServiceStatusText {
  $service = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
  if ($null -eq $service) {
    return "Not installed"
  }
  return [string]$service.Status
}

function Show-Status {
  $paths = Get-ConfigPaths
  $agentExe = Join-Path $paths.InstallRoot "bin\rtc-agent.exe"

  Write-Host "Remote Terminal Cloud Agent"
  Write-Host "Service status : $(Get-ServiceStatusText)"
  Write-Host "Install root   : $($paths.InstallRoot)"
  Write-Host "Config file    : $($paths.ConfigFile)"
  Write-Host "Logs dir       : $($paths.LogsDir)"
  Write-Host ""

  if (Test-Path $agentExe) {
    & $agentExe config
  } else {
    Write-Host "Agent executable not found: $agentExe"
  }
}

function Ensure-ServiceExists {
  $service = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
  if ($null -eq $service) {
    throw "Service $ServiceName is not installed."
  }
}

function Start-AgentService {
  Ensure-ServiceExists
  Start-Service -Name $ServiceName
  Write-Host "Service started."
}

function Stop-AgentService {
  Ensure-ServiceExists
  Stop-Service -Name $ServiceName
  Write-Host "Service stopped."
}

function Restart-AgentService {
  Ensure-ServiceExists
  Restart-Service -Name $ServiceName
  Write-Host "Service restarted."
}

function Run-Configure {
  $paths = Get-ConfigPaths
  $agentExe = Join-Path $paths.InstallRoot "bin\rtc-agent.exe"
  if (-not (Test-Path $agentExe)) {
    throw "Agent executable not found: $agentExe"
  }
  & $agentExe conf
}

function Edit-Config {
  $paths = Get-ConfigPaths
  New-Item -ItemType Directory -Force -Path $paths.ConfigDir | Out-Null
  if (-not (Test-Path $paths.ConfigFile)) {
    '{}' | Set-Content -Path $paths.ConfigFile -Encoding UTF8
  }
  Start-Process notepad.exe $paths.ConfigFile
}

function Open-ConfigDir {
  $paths = Get-ConfigPaths
  New-Item -ItemType Directory -Force -Path $paths.ConfigDir | Out-Null
  Start-Process explorer.exe $paths.ConfigDir
}

function Open-Logs {
  $paths = Get-ConfigPaths
  New-Item -ItemType Directory -Force -Path $paths.LogsDir | Out-Null
  Start-Process explorer.exe $paths.LogsDir
}

function Show-Menu {
  while ($true) {
    Write-Host ""
    Write-Host "Remote Terminal Cloud Agent Manager"
    Write-Host "1. Show status"
    Write-Host "2. Start service"
    Write-Host "3. Stop service"
    Write-Host "4. Restart service"
    Write-Host "5. Configure token"
    Write-Host "6. Edit config file"
    Write-Host "7. Open config directory"
    Write-Host "8. Open logs directory"
    Write-Host "9. Help"
    Write-Host "0. Exit"
    $choice = Read-Host "Select"

    switch ($choice) {
      "1" { Show-Status }
      "2" { Start-AgentService }
      "3" { Stop-AgentService }
      "4" { Restart-AgentService }
      "5" { Run-Configure }
      "6" { Edit-Config }
      "7" { Open-ConfigDir }
      "8" { Open-Logs }
      "9" { Show-Help }
      "0" { return }
      default { Write-Host "Unknown selection." }
    }
  }
}

switch ($Action) {
  "menu" { Show-Menu }
  "status" { Show-Status }
  "start" { Start-AgentService }
  "stop" { Stop-AgentService }
  "restart" { Restart-AgentService }
  "configure" { Run-Configure }
  "edit-config" { Edit-Config }
  "open-config-dir" { Open-ConfigDir }
  "open-logs" { Open-Logs }
  "help" { Show-Help }
}

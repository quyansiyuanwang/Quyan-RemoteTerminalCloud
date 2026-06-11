param(
  [string]$InstallRoot = ""
)

$ServiceName = "RemoteTerminalCloudAgent"
$ErrorActionPreference = "Stop"

function Resolve-InstallRoot([string]$ExplicitRoot) {
  if ($ExplicitRoot) {
    if (Test-Path $ExplicitRoot) {
      return (Resolve-Path $ExplicitRoot).Path
    }
    return $ExplicitRoot
  }

  $ScriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
  if (Test-Path (Join-Path $ScriptDir "service\RemoteTerminalCloudAgentService.exe")) {
    return $ScriptDir
  }
  if (Test-Path (Join-Path $ScriptDir "..\..\service\RemoteTerminalCloudAgentService.exe")) {
    return (Resolve-Path (Join-Path $ScriptDir "..\..")).Path
  }

  throw "Cannot locate install root from $ScriptDir"
}

function Get-ServiceState([string]$Name) {
  $service = Get-Service -Name $Name -ErrorAction SilentlyContinue
  if ($null -eq $service) {
    return "Missing"
  }
  return [string]$service.Status
}

function Wait-ForServiceState([string]$Name, [string[]]$DesiredStates, [int]$TimeoutSeconds = 30) {
  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  do {
    $state = Get-ServiceState -Name $Name
    if ($DesiredStates -contains $state) {
      return $state
    }
    Start-Sleep -Milliseconds 500
  } while ((Get-Date) -lt $deadline)

  throw "Service $Name did not reach state: $($DesiredStates -join ', ')"
}

function Stop-ManagedProcesses([string]$Root) {
  if (-not (Test-Path $Root)) {
    return
  }

  $rootPrefix = [System.IO.Path]::GetFullPath($Root).TrimEnd('\') + '\'
  $processNames = @("rtc-agent", "RemoteTerminalCloudAgentService")
  $processes = Get-Process -ErrorAction SilentlyContinue | Where-Object { $processNames -contains $_.ProcessName }

  foreach ($process in $processes) {
    $processPath = $null
    try {
      $processPath = $process.Path
    } catch {
      $processPath = $null
    }

    if (-not $processPath) {
      continue
    }

    $fullProcessPath = [System.IO.Path]::GetFullPath($processPath)
    if ($fullProcessPath.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
      try {
        Stop-Process -Id $process.Id -Force -ErrorAction Stop
      } catch {
      }
    }
  }
}

$ResolvedInstallRoot = Resolve-InstallRoot -ExplicitRoot $InstallRoot
$WinSWExe = Join-Path $ResolvedInstallRoot "service\RemoteTerminalCloudAgentService.exe"

Write-Host "Stopping Windows service $ServiceName"

if (Test-Path $WinSWExe) {
  Push-Location (Split-Path -Parent $WinSWExe)
  try {
    & $WinSWExe stop | Out-Null
  } catch {
  } finally {
    Pop-Location
  }
}

try {
  Stop-Service -Name $ServiceName -ErrorAction Stop
} catch {
}

try {
  Wait-ForServiceState -Name $ServiceName -DesiredStates @("Stopped", "Missing") | Out-Null
} catch {
}

Stop-ManagedProcesses -Root $ResolvedInstallRoot
Wait-ForServiceState -Name $ServiceName -DesiredStates @("Stopped", "Missing") | Out-Null

Write-Host "Service stop sequence completed."

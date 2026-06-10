param(
  [Parameter(Mandatory = $false)]
  [string]$Version = "v2.12.0",
  [Parameter(Mandatory = $false)]
  [string]$TargetExe = (Join-Path (Join-Path $PSScriptRoot "winsw") "RemoteTerminalCloudAgentService.exe"),
  [Parameter(Mandatory = $false)]
  [switch]$Force
)

$ErrorActionPreference = "Stop"

$TargetDir = Split-Path -Parent $TargetExe
$DownloadUrl = "https://github.com/winsw/winsw/releases/download/$Version/WinSW-x64.exe"

if (-not (Test-Path $TargetDir)) {
  New-Item -ItemType Directory -Path $TargetDir | Out-Null
}

if ((Test-Path $TargetExe) -and -not $Force) {
  Write-Host "WinSW already exists at $TargetExe"
  exit 0
}

Invoke-WebRequest -Uri $DownloadUrl -OutFile $TargetExe
Write-Host "Downloaded WinSW to $TargetExe"
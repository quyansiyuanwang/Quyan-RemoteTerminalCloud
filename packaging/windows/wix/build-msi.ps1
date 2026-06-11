param(
  [Parameter(Mandatory = $true)]
  [string]$AgentBuildRoot,
  [Parameter(Mandatory = $false)]
  [string]$OutputDir = (Join-Path $PSScriptRoot "out"),
  [Parameter(Mandatory = $false)]
  [switch]$AcceptEula
)

$ErrorActionPreference = "Stop"

$AgentBuildRoot = [System.IO.Path]::GetFullPath($AgentBuildRoot)
$OutputDir = [System.IO.Path]::GetFullPath($OutputDir)

$WixFile = Join-Path $PSScriptRoot "RemoteTerminalCloudAgent.wxs"
$WixCommand = Get-Command wix.exe -ErrorAction SilentlyContinue

foreach ($RequiredPath in @(
  (Join-Path $AgentBuildRoot "bin\rtc-agent.exe"),
  (Join-Path $AgentBuildRoot "bin\rtc-agent-manager.exe"),
  (Join-Path $AgentBuildRoot "packaging\windows\install-service.ps1"),
  (Join-Path $AgentBuildRoot "packaging\windows\uninstall-service.ps1"),
  (Join-Path $AgentBuildRoot "packaging\windows\stop-service.ps1"),
  (Join-Path $AgentBuildRoot "packaging\windows\manage-agent.ps1"),
  (Join-Path $AgentBuildRoot "packaging\windows\write-config.ps1"),
  (Join-Path $AgentBuildRoot "packaging\windows\init-config.ps1"),
  (Join-Path $AgentBuildRoot "service\RemoteTerminalCloudAgentService.exe"),
  (Join-Path $AgentBuildRoot "service\RemoteTerminalCloudAgentService.xml")
)) {
  if (-not (Test-Path $RequiredPath)) {
    throw "Required MSI build input missing: $RequiredPath"
  }
}

if (-not $WixCommand) {
  throw "wix.exe not found. Install WiX Toolset CLI and ensure it is on PATH."
}

if (-not (Test-Path $AgentBuildRoot)) {
  throw "AgentBuildRoot not found: $AgentBuildRoot"
}

if (-not (Test-Path $OutputDir)) {
  New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

# Resolve WiX major version so we can install the matching extension
$WixVersionOutput = & $WixCommand.Source --version 2>$null
$WixVersionText = [string]::Join("`n", $WixVersionOutput)
$WixMajorVersion = 0
if ($WixVersionText -match "(\d+)\.(\d+)\.(\d+)") {
  $WixMajorVersion = [int]$Matches[1]
  $WixFullVersion = "$($Matches[1]).$($Matches[2]).$($Matches[3])"
}
Write-Host "Detected WiX version: $WixFullVersion (major: $WixMajorVersion)"

# Install the UI extension pinned to the same major.minor.patch as wix.exe itself.
# Using the exact version avoids the wixextN folder mismatch error.
& $WixCommand.Source extension add "WixToolset.UI.wixext/$WixFullVersion" --global 2>&1 | Write-Host
Write-Host "wix extension add exited $LASTEXITCODE"

$MsiPath = Join-Path $OutputDir "RemoteTerminalCloudAgent.msi"

$WixArguments = @(
  "build"
)

if ($AcceptEula) {
  $WixVersionOutput = & $WixCommand.Source --version 2>$null
  $WixVersionText = [string]::Join("`n", $WixVersionOutput)
  $WixMajorVersion = 0

  if ($WixVersionText -match "(\d+)\.") {
    $WixMajorVersion = [int]$Matches[1]
  }

  if ($WixMajorVersion -ge 7) {
    $WixArguments += @("--acceptEula", "yes")
  }
}

$WixArguments += @(
  "-ext", "WixToolset.UI.wixext",
  "-d", "AgentBuildRoot=$AgentBuildRoot",
  $WixFile,
  "-out", $MsiPath
)

& $WixCommand.Source @WixArguments

if ($LASTEXITCODE -ne 0) {
  throw "wix build failed with exit code $LASTEXITCODE"
}

Write-Host "Built MSI at $MsiPath"

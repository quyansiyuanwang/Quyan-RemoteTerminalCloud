param(
  [Parameter(Mandatory = $true)]  [string]$AgentBuildRoot,
  [Parameter(Mandatory = $false)] [string]$OutputDir = (Join-Path $AgentBuildRoot "artifacts\windows\out"),
  [Parameter(Mandatory = $false)] [string]$Version = ""
)

$ErrorActionPreference = "Stop"
$AgentBuildRoot = [System.IO.Path]::GetFullPath($AgentBuildRoot)
$OutputDir      = [System.IO.Path]::GetFullPath($OutputDir)

# Resolve version from VERSION file if not provided
if (-not $Version) {
  $versionFile = Join-Path $AgentBuildRoot "VERSION"
  if (Test-Path $versionFile) {
    $Version = (Get-Content $versionFile -Raw).Trim()
  }
}
if (-not $Version) { $Version = "0.0.0" }

foreach ($RequiredPath in @(
  (Join-Path $AgentBuildRoot "bin\rtc-agent.exe"),
  (Join-Path $AgentBuildRoot "service\RemoteTerminalCloudAgentService.exe"),
  (Join-Path $AgentBuildRoot "service\RemoteTerminalCloudAgentService.xml"),
  (Join-Path $AgentBuildRoot "packaging\windows\install-service.ps1"),
  (Join-Path $AgentBuildRoot "packaging\windows\uninstall-service.ps1"),
  (Join-Path $AgentBuildRoot "packaging\windows\stop-service.ps1"),
  (Join-Path $AgentBuildRoot "packaging\windows\write-config.ps1"),
  (Join-Path $AgentBuildRoot "packaging\windows\agent.config.json")
)) {
  if (-not (Test-Path $RequiredPath)) { throw "Required input missing: $RequiredPath" }
}

$Makensis = Get-Command makensis.exe -ErrorAction SilentlyContinue
if (-not $Makensis) {
  # Chocolatey installs NSIS here; PATH may not be refreshed in the current shell
  $chocoPath = "C:\Program Files (x86)\NSIS\makensis.exe"
  if (Test-Path $chocoPath) { $Makensis = @{ Source = $chocoPath } }
}
if (-not $Makensis) { throw "makensis.exe not found. Install NSIS and ensure it is on PATH." }

if (-not (Test-Path $OutputDir)) { New-Item -ItemType Directory -Path $OutputDir | Out-Null }

$NsiScript = Join-Path $PSScriptRoot "agent.nsi"

& $Makensis.Source `
  "/DAGENT_BUILD_ROOT=$AgentBuildRoot" `
  "/DAGENT_VERSION=$Version" `
  $NsiScript

if ($LASTEXITCODE -ne 0) { throw "makensis failed with exit code $LASTEXITCODE" }

Write-Host "Built installer at $OutputDir"

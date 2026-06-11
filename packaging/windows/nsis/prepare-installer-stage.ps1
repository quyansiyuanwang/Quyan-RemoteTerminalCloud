param(
  [Parameter(Mandatory = $true)]  [string]$AgentBundleRoot,
  [Parameter(Mandatory = $false)] [string]$StageRoot,
  [Parameter(Mandatory = $false)] [string]$WinSWVersion = "v2.12.0",
  [Parameter(Mandatory = $false)] [switch]$Force
)

$ErrorActionPreference = "Stop"

function Resolve-AbsolutePath { param([string]$PathValue)
  return [System.IO.Path]::GetFullPath($PathValue)
}

$AgentBundleRoot = Resolve-AbsolutePath $AgentBundleRoot
if (-not $StageRoot) { $StageRoot = Join-Path $AgentBundleRoot "artifacts\windows\installer-build-root" }
$StageRoot = Resolve-AbsolutePath $StageRoot

foreach ($p in @(
  (Join-Path $AgentBundleRoot "bin\rtc-agent.exe"),
  (Join-Path $AgentBundleRoot "bin\rtc-agent-manager.exe"),
  (Join-Path $AgentBundleRoot "packaging\windows\install-service.ps1"),
  (Join-Path $AgentBundleRoot "packaging\windows\uninstall-service.ps1"),
  (Join-Path $AgentBundleRoot "packaging\windows\stop-service.ps1"),
  (Join-Path $AgentBundleRoot "packaging\windows\manage-agent.ps1"),
  (Join-Path $AgentBundleRoot "packaging\windows\write-config.ps1"),
  (Join-Path $AgentBundleRoot "packaging\windows\init-config.ps1"),
  (Join-Path $AgentBundleRoot "packaging\windows\agent.config.json"),
  (Join-Path $AgentBundleRoot "packaging\windows\RemoteTerminalCloudAgentService.xml"),
  (Join-Path $AgentBundleRoot "packaging\windows\download-winsw.ps1"),
  (Join-Path $AgentBundleRoot "packaging\windows\nsis\agent.nsi"),
  (Join-Path $AgentBundleRoot "packaging\windows\nsis\build-installer.ps1")
)) {
  if (-not (Test-Path $p)) { throw "Required bundle input missing: $p" }
}

if ((Test-Path $StageRoot) -and -not $Force) {
  throw "StageRoot already exists: $StageRoot. Use -Force to replace."
}
if (Test-Path $StageRoot) { Remove-Item $StageRoot -Recurse -Force }

foreach ($d in @(
  (Join-Path $StageRoot "bin"),
  (Join-Path $StageRoot "packaging\windows\nsis"),
  (Join-Path $StageRoot "service"),
  (Join-Path $StageRoot "artifacts\windows\out")
)) { New-Item -ItemType Directory -Path $d -Force | Out-Null }

Copy-Item (Join-Path $AgentBundleRoot "bin\rtc-agent.exe")                  (Join-Path $StageRoot "bin\rtc-agent.exe") -Force
Copy-Item (Join-Path $AgentBundleRoot "bin\rtc-agent-manager.exe")          (Join-Path $StageRoot "bin\rtc-agent-manager.exe") -Force
Copy-Item (Join-Path $AgentBundleRoot "packaging\windows\*")                (Join-Path $StageRoot "packaging\windows") -Recurse -Force
Copy-Item (Join-Path $AgentBundleRoot "packaging\windows\RemoteTerminalCloudAgentService.xml") `
                                                                        (Join-Path $StageRoot "service\RemoteTerminalCloudAgentService.xml") -Force

$versionFile = Join-Path $AgentBundleRoot "VERSION"
if (Test-Path $versionFile) { Copy-Item $versionFile (Join-Path $StageRoot "VERSION") -Force }

$StageWinSWExe = Join-Path $StageRoot "service\RemoteTerminalCloudAgentService.exe"
& (Join-Path $AgentBundleRoot "packaging\windows\download-winsw.ps1") -Version $WinSWVersion -TargetExe $StageWinSWExe -Force:$Force

Write-Host "Prepared installer build root at $StageRoot"
Write-Host "Next: run packaging\windows\nsis\build-installer.ps1 -AgentBuildRoot `"$StageRoot`""

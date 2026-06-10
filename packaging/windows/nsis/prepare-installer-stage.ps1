param(
  [Parameter(Mandatory = $true)]  [string]$AgentBundleRoot,
  [Parameter(Mandatory = $false)] [string]$StageRoot,
  [Parameter(Mandatory = $false)] [string]$NodeRuntimeRoot,
  [Parameter(Mandatory = $false)] [string]$WinSWVersion = "v2.12.0",
  [Parameter(Mandatory = $false)] [switch]$Force
)

$ErrorActionPreference = "Stop"

function Resolve-AbsolutePath { param([string]$PathValue)
  return [System.IO.Path]::GetFullPath($PathValue)
}

function Resolve-NodeRuntimeRoot { param([string]$PathValue)
  if ($PathValue) {
    $r = Resolve-AbsolutePath $PathValue
    if ((Test-Path $r -PathType Leaf) -and ([System.IO.Path]::GetFileName($r).ToLowerInvariant() -eq "node.exe")) {
      return Split-Path -Parent $r
    }
    return $r
  }
  $cmd = Get-Command node.exe -ErrorAction SilentlyContinue
  if (-not $cmd) { throw "Cannot find node.exe. Provide -NodeRuntimeRoot." }
  return Split-Path -Parent $cmd.Source
}

$AgentBundleRoot  = Resolve-AbsolutePath $AgentBundleRoot
$NodeRuntimeRoot  = Resolve-NodeRuntimeRoot $NodeRuntimeRoot
if (-not $StageRoot) { $StageRoot = Join-Path $AgentBundleRoot "artifacts\windows\installer-build-root" }
$StageRoot = Resolve-AbsolutePath $StageRoot

foreach ($p in @(
  (Join-Path $AgentBundleRoot "dist"),
  (Join-Path $AgentBundleRoot "packaging\windows\install-service.ps1"),
  (Join-Path $AgentBundleRoot "packaging\windows\uninstall-service.ps1"),
  (Join-Path $AgentBundleRoot "packaging\windows\write-config.ps1"),
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
  (Join-Path $StageRoot "dist"),
  (Join-Path $StageRoot "packaging\windows\nsis"),
  (Join-Path $StageRoot "runtime"),
  (Join-Path $StageRoot "service"),
  (Join-Path $StageRoot "artifacts\windows\out")
)) { New-Item -ItemType Directory -Path $d -Force | Out-Null }

Copy-Item (Join-Path $AgentBundleRoot "dist\*")                         (Join-Path $StageRoot "dist")                       -Recurse -Force
Copy-Item (Join-Path $AgentBundleRoot "packaging\windows\*")            (Join-Path $StageRoot "packaging\windows")          -Recurse -Force
Copy-Item (Join-Path $NodeRuntimeRoot "node.exe")                       (Join-Path $StageRoot "runtime\node.exe")           -Force
Copy-Item (Join-Path $AgentBundleRoot "packaging\windows\RemoteTerminalCloudAgentService.xml") `
                                                                        (Join-Path $StageRoot "service\RemoteTerminalCloudAgentService.xml") -Force

# Copy package.json for version resolution
$pkgJson = Join-Path $AgentBundleRoot "package.json"
if (Test-Path $pkgJson) { Copy-Item $pkgJson (Join-Path $StageRoot "package.json") -Force }

$StageWinSWExe = Join-Path $StageRoot "service\RemoteTerminalCloudAgentService.exe"
& (Join-Path $AgentBundleRoot "packaging\windows\download-winsw.ps1") -Version $WinSWVersion -TargetExe $StageWinSWExe -Force:$Force

Write-Host "Prepared installer build root at $StageRoot"
Write-Host "Next: run packaging\windows\nsis\build-installer.ps1 -AgentBuildRoot `"$StageRoot`""

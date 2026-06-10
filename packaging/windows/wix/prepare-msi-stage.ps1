param(
  [Parameter(Mandatory = $true)]
  [string]$AgentBundleRoot,
  [Parameter(Mandatory = $false)]
  [string]$StageRoot,
  [Parameter(Mandatory = $false)]
  [string]$NodeRuntimeRoot,
  [Parameter(Mandatory = $false)]
  [string]$WinSWVersion = "v2.12.0",
  [Parameter(Mandatory = $false)]
  [switch]$Force
)

$ErrorActionPreference = "Stop"

function Resolve-AbsolutePath {
  param(
    [Parameter(Mandatory = $true)]
    [string]$PathValue
  )

  return [System.IO.Path]::GetFullPath($PathValue)
}

function Resolve-NodeRuntimeRoot {
  param(
    [Parameter(Mandatory = $false)]
    [string]$PathValue
  )

  if ($PathValue) {
    $Resolved = Resolve-AbsolutePath -PathValue $PathValue
    if (Test-Path $Resolved -PathType Leaf) {
      if ([System.IO.Path]::GetFileName($Resolved).ToLowerInvariant() -ne "node.exe") {
        throw "Node runtime file path must point to node.exe: $Resolved"
      }

      return Split-Path -Parent $Resolved
    }

    return $Resolved
  }

  $NodeCommand = Get-Command node.exe -ErrorAction SilentlyContinue
  if (-not $NodeCommand) {
    throw "Unable to resolve node.exe automatically. Provide -NodeRuntimeRoot pointing to an extracted Windows Node runtime."
  }

  return Split-Path -Parent $NodeCommand.Source
}

$AgentBundleRoot = Resolve-AbsolutePath -PathValue $AgentBundleRoot

if (-not (Test-Path $AgentBundleRoot -PathType Container)) {
  throw "AgentBundleRoot not found: $AgentBundleRoot"
}

if (-not $StageRoot) {
  $StageRoot = Join-Path $AgentBundleRoot "artifacts\windows\msi-build-root"
}

$StageRoot = Resolve-AbsolutePath -PathValue $StageRoot
$NodeRuntimeRoot = Resolve-NodeRuntimeRoot -PathValue $NodeRuntimeRoot

$BundleDistRoot = Join-Path $AgentBundleRoot "dist"
$BundlePackagingWindowsRoot = Join-Path $AgentBundleRoot "packaging\windows"
$BundleWixRoot = Join-Path $BundlePackagingWindowsRoot "wix"
$BundleInstallScript = Join-Path $BundlePackagingWindowsRoot "install-service.ps1"
$BundleUninstallScript = Join-Path $BundlePackagingWindowsRoot "uninstall-service.ps1"
$BundleServiceXml = Join-Path $BundlePackagingWindowsRoot "RemoteTerminalCloudAgentService.xml"
$BundleWixFile = Join-Path $BundleWixRoot "RemoteTerminalCloudAgent.wxs"
$BundleWinSWDownloader = Join-Path $BundlePackagingWindowsRoot "download-winsw.ps1"

foreach ($RequiredPath in @(
  $BundleDistRoot,
  $BundlePackagingWindowsRoot,
  $BundleWixRoot,
  $BundleInstallScript,
  $BundleUninstallScript,
  $BundleServiceXml,
  $BundleWixFile,
  $BundleWinSWDownloader
)) {
  if (-not (Test-Path $RequiredPath)) {
    throw "Required bundle input missing: $RequiredPath"
  }
}

$NodeExe = Join-Path $NodeRuntimeRoot "node.exe"
if (-not (Test-Path $NodeExe -PathType Leaf)) {
  throw "node.exe not found under NodeRuntimeRoot: $NodeExe"
}

if ((Test-Path $StageRoot) -and -not $Force) {
  throw "StageRoot already exists: $StageRoot. Re-run with -Force to replace it."
}

if (Test-Path $StageRoot) {
  Remove-Item -Path $StageRoot -Recurse -Force
}

New-Item -ItemType Directory -Path $StageRoot | Out-Null

$StageDistRoot = Join-Path $StageRoot "dist"
$StagePackagingWindowsRoot = Join-Path $StageRoot "packaging\windows"
$StageRuntimeRoot = Join-Path $StageRoot "runtime"
$StageServiceRoot = Join-Path $StageRoot "service"
$StageArtifactsRoot = Join-Path $StageRoot "artifacts\windows"
$StageOutputRoot = Join-Path $StageArtifactsRoot "out"

foreach ($DirectoryPath in @(
  $StageDistRoot,
  $StagePackagingWindowsRoot,
  $StageRuntimeRoot,
  $StageServiceRoot,
  $StageArtifactsRoot,
  $StageOutputRoot
)) {
  New-Item -ItemType Directory -Path $DirectoryPath -Force | Out-Null
}

Copy-Item -Path (Join-Path $BundleDistRoot "*") -Destination $StageDistRoot -Recurse -Force
Copy-Item -Path (Join-Path $BundlePackagingWindowsRoot "*") -Destination $StagePackagingWindowsRoot -Recurse -Force
Copy-Item -Path $NodeExe -Destination (Join-Path $StageRuntimeRoot "node.exe") -Force
Copy-Item -Path $BundleServiceXml -Destination (Join-Path $StageServiceRoot "RemoteTerminalCloudAgentService.xml") -Force

$StageWinSWExe = Join-Path $StageServiceRoot "RemoteTerminalCloudAgentService.exe"
& $BundleWinSWDownloader -Version $WinSWVersion -TargetExe $StageWinSWExe -Force:$Force

$ManifestPath = Join-Path $StageRoot "MSI-INPUTS.json"
$Manifest = [ordered]@{
  generatedAt = (Get-Date).ToString("o")
  agentBundleRoot = $AgentBundleRoot
  nodeRuntimeRoot = $NodeRuntimeRoot
  winSWVersion = $WinSWVersion
  stageRoot = $StageRoot
  requiredLayout = [ordered]@{
    dist = "compiled agent entry and dependencies"
    packaging = [ordered]@{
      windows = "install/uninstall scripts and WiX authoring files"
    }
    runtime = [ordered]@{
      nodeExe = "runtime\\node.exe"
      note = "copy the full extracted Windows Node runtime directory here"
    }
    service = [ordered]@{
      winSWExe = "service\\RemoteTerminalCloudAgentService.exe"
      winSWXml = "service\\RemoteTerminalCloudAgentService.xml"
    }
    artifacts = [ordered]@{
      windows = [ordered]@{
        out = "default MSI output directory"
      }
    }
  }
  wixBuildExample = "powershell -ExecutionPolicy Bypass -File packaging\\windows\\wix\\build-msi.ps1 -AgentBuildRoot `"$StageRoot`" -OutputDir `"$StageOutputRoot`""
}

$Manifest | ConvertTo-Json -Depth 8 | Set-Content -Path $ManifestPath -Encoding UTF8

Set-Content -Path (Join-Path $StageRuntimeRoot "README.txt") -Encoding UTF8 -Value @(
  "This folder intentionally contains only the minimum required runtime payload for MSI packaging.",
  "Bundled file:",
  "- node.exe"
)

Set-Content -Path (Join-Path $StageServiceRoot "README.txt") -Encoding UTF8 -Value @(
  "This folder contains the minimum service wrapper payload for MSI packaging.",
  "Bundled files:",
  "- RemoteTerminalCloudAgentService.exe",
  "- RemoteTerminalCloudAgentService.xml"
)

Set-Content -Path (Join-Path $StageRoot "README.txt") -Encoding UTF8 -Value @(
  "Remote Terminal Cloud Agent - Windows MSI build root",
  "",
  "This directory is ready to be used as AgentBuildRoot for WiX.",
  "",
  "Build command:",
  "powershell -ExecutionPolicy Bypass -File packaging\\windows\\wix\\build-msi.ps1 -AgentBuildRoot `"$StageRoot`" -OutputDir `"$StageOutputRoot`"",
  "",
  "Key paths:",
  "- dist\\",
  "- packaging\\windows\\",
  "- runtime\\node.exe",
  "- service\\RemoteTerminalCloudAgentService.exe",
  "- service\\RemoteTerminalCloudAgentService.xml"
)

Write-Host "Prepared Windows MSI build root at $StageRoot"
Write-Host "Node runtime source: $NodeRuntimeRoot"
Write-Host "WinSW executable: $StageWinSWExe"
Write-Host "Next: run packaging\\windows\\wix\\build-msi.ps1 against this build root."
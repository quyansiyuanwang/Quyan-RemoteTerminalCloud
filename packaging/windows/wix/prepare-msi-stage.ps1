param(
  [Parameter(Mandatory = $true)]
  [string]$AgentBundleRoot,
  [Parameter(Mandatory = $false)]
  [string]$StageRoot,
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

$AgentBundleRoot = Resolve-AbsolutePath -PathValue $AgentBundleRoot

if (-not (Test-Path $AgentBundleRoot -PathType Container)) {
  throw "AgentBundleRoot not found: $AgentBundleRoot"
}

if (-not $StageRoot) {
  $StageRoot = Join-Path $AgentBundleRoot "artifacts\windows\msi-build-root"
}

$StageRoot = Resolve-AbsolutePath -PathValue $StageRoot

$BundleBinRoot = Join-Path $AgentBundleRoot "bin"
$BundlePackagingWindowsRoot = Join-Path $AgentBundleRoot "packaging\windows"
$BundleWixRoot = Join-Path $BundlePackagingWindowsRoot "wix"
$BundleInstallScript = Join-Path $BundlePackagingWindowsRoot "install-service.ps1"
$BundleUninstallScript = Join-Path $BundlePackagingWindowsRoot "uninstall-service.ps1"
$BundleStopScript = Join-Path $BundlePackagingWindowsRoot "stop-service.ps1"
$BundleManageScript = Join-Path $BundlePackagingWindowsRoot "manage-agent.ps1"
$BundleManageUIScript = Join-Path $BundlePackagingWindowsRoot "manage-agent-ui.ps1"
$BundleLaunchManager = Join-Path $BundlePackagingWindowsRoot "launch-manager.vbs"
$BundleInitScript = Join-Path $BundlePackagingWindowsRoot "init-config.ps1"
$BundleServiceXml = Join-Path $BundlePackagingWindowsRoot "RemoteTerminalCloudAgentService.xml"
$BundleWixFile = Join-Path $BundleWixRoot "RemoteTerminalCloudAgent.wxs"
$BundleWinSWDownloader = Join-Path $BundlePackagingWindowsRoot "download-winsw.ps1"

foreach ($RequiredPath in @(
  (Join-Path $BundleBinRoot "rtc-agent.exe"),
  (Join-Path $BundleBinRoot "rtc-agent-manager.exe"),
  $BundlePackagingWindowsRoot,
  $BundleWixRoot,
  $BundleInstallScript,
  $BundleUninstallScript,
  $BundleStopScript,
  $BundleManageScript,
  $BundleManageUIScript,
  $BundleLaunchManager,
  $BundleInitScript,
  $BundleServiceXml,
  $BundleWixFile,
  $BundleWinSWDownloader
)) {
  if (-not (Test-Path $RequiredPath)) {
    throw "Required bundle input missing: $RequiredPath"
  }
}

if ((Test-Path $StageRoot) -and -not $Force) {
  throw "StageRoot already exists: $StageRoot. Re-run with -Force to replace it."
}

if (Test-Path $StageRoot) {
  Remove-Item -Path $StageRoot -Recurse -Force
}

New-Item -ItemType Directory -Path $StageRoot | Out-Null

$StageBinRoot = Join-Path $StageRoot "bin"
$StagePackagingWindowsRoot = Join-Path $StageRoot "packaging\windows"
$StageServiceRoot = Join-Path $StageRoot "service"
$StageArtifactsRoot = Join-Path $StageRoot "artifacts\windows"
$StageOutputRoot = Join-Path $StageArtifactsRoot "out"

foreach ($DirectoryPath in @(
  $StageBinRoot,
  $StagePackagingWindowsRoot,
  $StageServiceRoot,
  $StageArtifactsRoot,
  $StageOutputRoot
)) {
  New-Item -ItemType Directory -Path $DirectoryPath -Force | Out-Null
}

Copy-Item -Path (Join-Path $BundleBinRoot "rtc-agent.exe") -Destination (Join-Path $StageBinRoot "rtc-agent.exe") -Force
Copy-Item -Path (Join-Path $BundleBinRoot "rtc-agent-manager.exe") -Destination (Join-Path $StageBinRoot "rtc-agent-manager.exe") -Force
Copy-Item -Path (Join-Path $BundlePackagingWindowsRoot "*") -Destination $StagePackagingWindowsRoot -Recurse -Force
Copy-Item -Path $BundleServiceXml -Destination (Join-Path $StageServiceRoot "RemoteTerminalCloudAgentService.xml") -Force

$StageWinSWExe = Join-Path $StageServiceRoot "RemoteTerminalCloudAgentService.exe"
& $BundleWinSWDownloader -Version $WinSWVersion -TargetExe $StageWinSWExe -Force:$Force

$ManifestPath = Join-Path $StageRoot "MSI-INPUTS.json"
$Manifest = [ordered]@{
  generatedAt = (Get-Date).ToString("o")
  agentBundleRoot = $AgentBundleRoot
  winSWVersion = $WinSWVersion
  stageRoot = $StageRoot
  requiredLayout = [ordered]@{
    bin = [ordered]@{
      agentExe = "bin\\rtc-agent.exe"
    }
    packaging = [ordered]@{
      windows = "install/uninstall scripts and WiX authoring files"
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
  "- bin\\rtc-agent.exe",
  "- packaging\\windows\\",
  "- service\\RemoteTerminalCloudAgentService.exe",
  "- service\\RemoteTerminalCloudAgentService.xml"
)

Write-Host "Prepared Windows MSI build root at $StageRoot"
Write-Host "WinSW executable: $StageWinSWExe"
Write-Host "Next: run packaging\\windows\\wix\\build-msi.ps1 against this build root."

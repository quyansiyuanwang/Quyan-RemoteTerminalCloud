param(
  [Parameter(Mandatory = $false)]
  [string]$Version = "v2.12.0",
  [Parameter(Mandatory = $false)]
  [string]$TargetExe = (Join-Path (Join-Path $PSScriptRoot "winsw") "RemoteTerminalCloudAgentService.exe"),
  [Parameter(Mandatory = $false)]
  [switch]$Force
)

$ErrorActionPreference = "Stop"

$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$Arguments = @("xtask", "windows-download-winsw", "--target-exe", $TargetExe, "--winsw-version", $Version)

if ($Force) {
  $Arguments += "--force"
}

Push-Location $RepoRoot
try {
  & cargo @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "cargo xtask windows-download-winsw failed with exit code $LASTEXITCODE"
  }
} finally {
  Pop-Location
}

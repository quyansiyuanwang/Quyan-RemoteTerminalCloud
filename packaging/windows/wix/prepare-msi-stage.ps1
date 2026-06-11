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

$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\..\.."))
$Arguments = @("xtask", "windows-msi-stage", "--bundle-root", $AgentBundleRoot, "--winsw-version", $WinSWVersion)

if ($StageRoot) {
  $Arguments += @("--stage-root", $StageRoot)
}
if ($Force) {
  $Arguments += "--force"
}

Push-Location $RepoRoot
try {
  & cargo @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "cargo xtask windows-msi-stage failed with exit code $LASTEXITCODE"
  }
} finally {
  Pop-Location
}

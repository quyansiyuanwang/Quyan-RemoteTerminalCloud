param(
  [Parameter(Mandatory = $true)]
  [string]$AgentBundleRoot,
  [Parameter(Mandatory = $false)]
  [string]$StageRoot,
  [Parameter(Mandatory = $false)]
  [switch]$Force
)

$ErrorActionPreference = "Stop"

$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\..\.."))
$Arguments = @("xtask", "windows-nsis-stage", "--bundle-root", $AgentBundleRoot)

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
    throw "cargo xtask windows-nsis-stage failed with exit code $LASTEXITCODE"
  }
} finally {
  Pop-Location
}

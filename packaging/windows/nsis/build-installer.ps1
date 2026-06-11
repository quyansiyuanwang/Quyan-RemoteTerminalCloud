param(
  [Parameter(Mandatory = $true)]
  [string]$AgentBuildRoot,
  [Parameter(Mandatory = $false)]
  [string]$OutputDir,
  [Parameter(Mandatory = $false)]
  [string]$Version = ""
)

$ErrorActionPreference = "Stop"

$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\..\.."))
$Arguments = @("xtask", "windows-nsis-build", "--build-root", $AgentBuildRoot)

if ($OutputDir) {
  $Arguments += @("--output-dir", $OutputDir)
}
if ($Version) {
  $Arguments += @("--version", $Version)
}

Push-Location $RepoRoot
try {
  & cargo @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "cargo xtask windows-nsis-build failed with exit code $LASTEXITCODE"
  }
} finally {
  Pop-Location
}

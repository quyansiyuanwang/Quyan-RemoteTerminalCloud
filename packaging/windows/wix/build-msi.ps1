param(
  [Parameter(Mandatory = $true)]
  [string]$AgentBuildRoot,
  [Parameter(Mandatory = $false)]
  [string]$OutputDir,
  [Parameter(Mandatory = $false)]
  [switch]$AcceptEula
)

$ErrorActionPreference = "Stop"

$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\..\.."))
$Arguments = @("xtask", "windows-msi-build", "--build-root", $AgentBuildRoot)

if ($OutputDir) {
  $Arguments += @("--output-dir", $OutputDir)
}
if ($AcceptEula) {
  $Arguments += "--accept-eula"
}

Push-Location $RepoRoot
try {
  & cargo @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "cargo xtask windows-msi-build failed with exit code $LASTEXITCODE"
  }
} finally {
  Pop-Location
}

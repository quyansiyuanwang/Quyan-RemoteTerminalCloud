# Accepts params from NSIS (explicit args) or MSI CustomActionData (env var fallback).
param(
  [string]$RegToken = ""
)

# MSI fallback: CustomActionData carries the registration token.
if (-not $RegToken -and $env:CustomActionData) {
  $RegToken = $env:CustomActionData.Trim()
}

if (-not $RegToken) { exit 0 }

$configPath = Join-Path $env:ProgramData 'RemoteTerminalCloudAgent\config.json'
if (-not (Test-Path $configPath)) { exit 0 }

$config = Get-Content $configPath -Raw | ConvertFrom-Json
$config.registrationToken = $RegToken
$config | ConvertTo-Json -Depth 5 | Set-Content $configPath -Encoding UTF8

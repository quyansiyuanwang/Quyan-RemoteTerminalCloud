# Accepts params from NSIS (explicit args) or MSI CustomActionData (env var fallback).
param(
  [string]$ServerUrl = "",
  [string]$RegToken = ""
)

# MSI fallback: CustomActionData as "url|token"
if (-not $ServerUrl -and $env:CustomActionData) {
  $parts = $env:CustomActionData -split '\|', 2
  $ServerUrl = $parts[0].Trim()
  $RegToken  = if ($parts.Count -gt 1) { $parts[1].Trim() } else { '' }
}

if (-not $ServerUrl) { exit 0 }

$configPath = Join-Path $env:ProgramData 'RemoteTerminalCloudAgent\config.json'
if (-not (Test-Path $configPath)) { exit 0 }

$config = Get-Content $configPath -Raw | ConvertFrom-Json
$config.serverBaseUrl = $ServerUrl
if ($RegToken) { $config.registrationToken = $RegToken }
$config | ConvertTo-Json -Depth 5 | Set-Content $configPath -Encoding UTF8

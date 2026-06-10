# Invoked as a deferred MSI CustomAction. User input arrives via $env:CustomActionData
# as "serverUrl|registrationToken" (token may be empty).
$parts = $env:CustomActionData -split '\|', 2
$serverUrl = $parts[0].Trim()
$token = if ($parts.Count -gt 1) { $parts[1].Trim() } else { '' }

if (-not $serverUrl) { exit 0 }

$configPath = Join-Path $env:ProgramData 'RemoteTerminalCloudAgent\config.json'
if (-not (Test-Path $configPath)) { exit 0 }

$config = Get-Content $configPath -Raw | ConvertFrom-Json
$config.serverBaseUrl = $serverUrl
if ($token) { $config.registrationToken = $token }
$config | ConvertTo-Json -Depth 5 | Set-Content $configPath -Encoding UTF8

# Called by NSIS after files are installed to initialize ProgramData config.
# $PSScriptRoot may be empty under nsExec; use MyInvocation fallback.
$ScriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
$ConfigDir = Join-Path $env:ProgramData "RemoteTerminalCloudAgent"
New-Item -ItemType Directory -Force -Path $ConfigDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $ConfigDir "logs") | Out-Null
$dest = Join-Path $ConfigDir "config.json"
if (-not (Test-Path $dest)) {
  Copy-Item (Join-Path $ScriptDir "agent.config.json") $dest -Force
}

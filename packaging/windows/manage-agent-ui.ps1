Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$ErrorActionPreference = "Stop"
$ServiceName = "RemoteTerminalCloudAgent"
$ScriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }

function Resolve-InstallRoot {
  if (Test-Path (Join-Path $ScriptDir "bin\rtc-agent.exe")) {
    return $ScriptDir
  }
  if (Test-Path (Join-Path $ScriptDir "..\..\bin\rtc-agent.exe")) {
    return (Resolve-Path (Join-Path $ScriptDir "..\..")).Path
  }
  throw "Cannot locate install root from $ScriptDir"
}

function Get-ConfigPaths {
  $configDir = Join-Path $env:APPDATA "remote-terminal-cloud-agent"
  return [pscustomobject]@{
    InstallRoot = Resolve-InstallRoot
    ConfigDir = $configDir
    ConfigFile = Join-Path $configDir "config.json"
    PreferencesFile = Join-Path $configDir "preferences.json"
    LogsDir = Join-Path $env:ProgramData "RemoteTerminalCloudAgent\logs"
    AgentExe = Join-Path (Resolve-InstallRoot) "bin\rtc-agent.exe"
  }
}

function Get-ServiceStatusText {
  $service = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
  if ($null -eq $service) {
    return "Not installed"
  }
  return [string]$service.Status
}

function Invoke-AgentCommand([string[]]$Arguments) {
  $paths = Get-ConfigPaths
  if (-not (Test-Path $paths.AgentExe)) {
    throw "Agent executable not found: $($paths.AgentExe)"
  }

  $output = & $paths.AgentExe @Arguments 2>&1 | Out-String
  return $output.Trim()
}

function Ensure-ServiceExists {
  $service = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
  if ($null -eq $service) {
    throw "Service $ServiceName is not installed."
  }
}

function New-Button($text, $x, $y, $w, $h) {
  $button = New-Object System.Windows.Forms.Button
  $button.Text = $text
  $button.Location = New-Object System.Drawing.Point($x, $y)
  $button.Size = New-Object System.Drawing.Size($w, $h)
  return $button
}

function Refresh-Status {
  param($statusLabel, $detailsBox)

  $paths = Get-ConfigPaths
  $statusLabel.Text = "Service Status: $(Get-ServiceStatusText)"

  try {
    $details = Invoke-AgentCommand @("status")
  } catch {
    $details = $_.Exception.Message
  }

  $detailsBox.Text = @(
    "Install Root: $($paths.InstallRoot)"
    "Config File: $($paths.ConfigFile)"
    "Logs Dir: $($paths.LogsDir)"
    ""
    $details
  ) -join [Environment]::NewLine
}

$paths = Get-ConfigPaths

$form = New-Object System.Windows.Forms.Form
$form.Text = "Remote Terminal Cloud Agent"
$form.StartPosition = "CenterScreen"
$form.Size = New-Object System.Drawing.Size(760, 560)
$form.MinimumSize = New-Object System.Drawing.Size(760, 560)
$form.BackColor = [System.Drawing.Color]::FromArgb(246, 248, 252)

$title = New-Object System.Windows.Forms.Label
$title.Text = "Remote Terminal Cloud Agent"
$title.Font = New-Object System.Drawing.Font("Segoe UI", 16, [System.Drawing.FontStyle]::Bold)
$title.Location = New-Object System.Drawing.Point(24, 20)
$title.Size = New-Object System.Drawing.Size(420, 34)
$form.Controls.Add($title)

$subtitle = New-Object System.Windows.Forms.Label
$subtitle.Text = "Start, stop, configure, and inspect the local agent without using the install directory."
$subtitle.Font = New-Object System.Drawing.Font("Segoe UI", 9)
$subtitle.ForeColor = [System.Drawing.Color]::FromArgb(80, 88, 102)
$subtitle.Location = New-Object System.Drawing.Point(24, 56)
$subtitle.Size = New-Object System.Drawing.Size(680, 22)
$form.Controls.Add($subtitle)

$statusLabel = New-Object System.Windows.Forms.Label
$statusLabel.Font = New-Object System.Drawing.Font("Segoe UI", 10, [System.Drawing.FontStyle]::Bold)
$statusLabel.Location = New-Object System.Drawing.Point(24, 96)
$statusLabel.Size = New-Object System.Drawing.Size(300, 24)
$form.Controls.Add($statusLabel)

$configureButton = New-Button "Configure Token" 24 136 150 36
$startButton = New-Button "Start Service" 188 136 130 36
$stopButton = New-Button "Stop Service" 332 136 130 36
$restartButton = New-Button "Restart Service" 476 136 130 36
$refreshButton = New-Button "Refresh" 620 136 100 36

$editConfigButton = New-Button "Edit Config" 24 186 130 34
$openConfigDirButton = New-Button "Open Config Folder" 168 186 150 34
$openLogsButton = New-Button "Open Logs" 332 186 130 34
$showHelpButton = New-Button "CLI Help" 476 186 130 34

$detailsBox = New-Object System.Windows.Forms.TextBox
$detailsBox.Location = New-Object System.Drawing.Point(24, 238)
$detailsBox.Size = New-Object System.Drawing.Size(696, 262)
$detailsBox.Multiline = $true
$detailsBox.ScrollBars = "Vertical"
$detailsBox.ReadOnly = $true
$detailsBox.Font = New-Object System.Drawing.Font("Consolas", 9)
$detailsBox.BackColor = [System.Drawing.Color]::White

foreach ($control in @(
  $configureButton, $startButton, $stopButton, $restartButton, $refreshButton,
  $editConfigButton, $openConfigDirButton, $openLogsButton, $showHelpButton, $detailsBox
)) {
  $form.Controls.Add($control)
}

$showMessage = {
  param($text, $caption)
  [System.Windows.Forms.MessageBox]::Show($form, $text, $caption, [System.Windows.Forms.MessageBoxButtons]::OK, [System.Windows.Forms.MessageBoxIcon]::Information) | Out-Null
}

$showError = {
  param($text)
  [System.Windows.Forms.MessageBox]::Show($form, $text, "Remote Terminal Cloud Agent", [System.Windows.Forms.MessageBoxButtons]::OK, [System.Windows.Forms.MessageBoxIcon]::Error) | Out-Null
}

$configureButton.Add_Click({
  try {
    Start-Process powershell.exe -ArgumentList @("-NoExit", "-ExecutionPolicy", "Bypass", "-File", (Join-Path $ScriptDir "manage-agent.ps1"), "configure")
    & $showMessage "A configuration window has been opened for token input." "Configure Token"
  } catch {
    & $showError $_.Exception.Message
  }
})

$startButton.Add_Click({
  try {
    Ensure-ServiceExists
    Start-Service -Name $ServiceName
    Refresh-Status -statusLabel $statusLabel -detailsBox $detailsBox
  } catch {
    & $showError $_.Exception.Message
  }
})

$stopButton.Add_Click({
  try {
    Ensure-ServiceExists
    Stop-Service -Name $ServiceName
    Refresh-Status -statusLabel $statusLabel -detailsBox $detailsBox
  } catch {
    & $showError $_.Exception.Message
  }
})

$restartButton.Add_Click({
  try {
    Ensure-ServiceExists
    Restart-Service -Name $ServiceName
    Refresh-Status -statusLabel $statusLabel -detailsBox $detailsBox
  } catch {
    & $showError $_.Exception.Message
  }
})

$refreshButton.Add_Click({
  Refresh-Status -statusLabel $statusLabel -detailsBox $detailsBox
})

$editConfigButton.Add_Click({
  try {
    New-Item -ItemType Directory -Force -Path $paths.ConfigDir | Out-Null
    if (-not (Test-Path $paths.ConfigFile)) {
      '{}' | Set-Content -Path $paths.ConfigFile -Encoding UTF8
    }
    Start-Process notepad.exe $paths.ConfigFile
  } catch {
    & $showError $_.Exception.Message
  }
})

$openConfigDirButton.Add_Click({
  try {
    New-Item -ItemType Directory -Force -Path $paths.ConfigDir | Out-Null
    Start-Process explorer.exe $paths.ConfigDir
  } catch {
    & $showError $_.Exception.Message
  }
})

$openLogsButton.Add_Click({
  try {
    New-Item -ItemType Directory -Force -Path $paths.LogsDir | Out-Null
    Start-Process explorer.exe $paths.LogsDir
  } catch {
    & $showError $_.Exception.Message
  }
})

$showHelpButton.Add_Click({
  try {
    $helpText = Invoke-AgentCommand @("help")
    & $showMessage $helpText "CLI Help"
  } catch {
    & $showError $_.Exception.Message
  }
})

Refresh-Status -statusLabel $statusLabel -detailsBox $detailsBox
[void]$form.ShowDialog()

# Installs Oya Browser on this Windows computer when it is missing, opens it, and asks it to
# connect one coding agent.
# Usage: & ([scriptblock]::Create((irm https://oyabrowser.com/install-desktop.ps1))) claude-code
#
# It never connects anything by itself: Oya shows a dialog, and a person clicks Connect there.
# It never replaces an installed copy. The installer comes over HTTPS from oyabrowser.com, as the
# manual steps' does. It is not signature-checked here: today's Windows builds carry a self-signed
# certificate, which proves nothing, so a check would refuse every real installer. Pin the
# publisher here once builds are signed by Azure Trusted Signing.
# Errors stop the script without closing the person's PowerShell window, which `exit` would.

param([string]$Agent = '')

$ErrorActionPreference = 'Stop'
$Names = @{ 'claude-code' = 'Claude Code'; 'codex' = 'Codex'; 'cursor' = 'Cursor'; 'vscode' = 'VS Code' }
if (-not $Names.ContainsKey($Agent)) {
  Write-Error "Unknown agent `"$Agent`". Use one of: claude-code, codex, cursor, vscode."
}
$Name = $Names[$Agent]
$App = Join-Path $env:LOCALAPPDATA 'Programs\Oya Browser\Oya Browser.exe'

if (Test-Path $App) {
  Write-Output 'Oya Browser is already installed.'
} else {
  $NotInstalled = 'Could not download Oya Browser. Check the connection and run this line again, or download it from https://oyabrowser.com/#download. Nothing was installed.'
  Write-Output 'Downloading Oya Browser...'
  $Installer = Join-Path $env:TEMP "Oya-$([guid]::NewGuid()).exe"
  try {
    Invoke-WebRequest -Uri 'https://oyabrowser.com/download/windows' -OutFile $Installer -UseBasicParsing
  } catch {
    Write-Error $NotInstalled
  }
  Write-Output 'Installing...'
  Start-Process -Wait $Installer -ArgumentList '/S'
  Remove-Item $Installer -Force
}

Write-Output 'Opening Oya Browser...'
Write-Output "Asking Oya to connect $Name..."
Start-Process $App -ArgumentList "oya://agent-connect?agent=$Agent"
Write-Output "Oya Browser is showing a dialog. A person must click `"Connect $Name`" there; an agent cannot answer it."
Write-Output "If no dialog appears, this copy of Oya is older: click Connect next to $Name in the footer of the Oya window."
Write-Output 'On a first install, Oya opens on its welcome screen: the person picks how to start there before the tools answer.'
Write-Output "Then start a new $Name session so the oya-browser tools appear."

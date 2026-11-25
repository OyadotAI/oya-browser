<# Verify the harmless preflight executable on a real Windows runner without Azure credentials or signing calls. #>
$ErrorActionPreference = 'Stop'
$Directory = Join-Path $env:RUNNER_TEMP "oya-signing-probe-test-$([guid]::NewGuid())"
New-Item -ItemType Directory -Path $Directory | Out-Null
try {
  $Probe = & "$PSScriptRoot/create-windows-signing-probe.ps1" -Directory $Directory
  if ([System.Reflection.AssemblyName]::GetAssemblyName($Probe).Name -cne 'OyaSigningProbe') {
    throw 'The preflight probe is not the owned compiled assembly.'
  }
  if ((Get-AuthenticodeSignature -LiteralPath $Probe).Status -ne 'NotSigned') {
    throw 'The unsigned fixture unexpectedly contains a signature.'
  }
  Write-Host 'PASS owned Windows probe compilation and unsigned starting state; no Azure access used.'
} finally {
  Remove-Item -LiteralPath $Directory -Recurse -Force
}

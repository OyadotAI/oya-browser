<# Compile only Oya's harmless, owned executable so signing tests never alter or re-sign a system binary. #>
param([Parameter(Mandatory = $true)][string]$Directory)
$ErrorActionPreference = 'Stop'
$SuccessExitCode = 0
$Source = Join-Path $Directory 'OyaSigningProbe.cs'
$Probe = Join-Path $Directory 'OyaSigningProbe.exe'
'public static class OyaSigningProbe { public static void Main() {} }' | Set-Content -LiteralPath $Source -Encoding UTF8
$Compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
if (-not (Test-Path -LiteralPath $Compiler -PathType Leaf)) { throw 'The Windows probe compiler is unavailable.' }
$CompileOutput = & $Compiler /nologo /target:exe "/out:$Probe" $Source 2>&1
if ($LASTEXITCODE -ne $SuccessExitCode) { throw 'The owned Windows signing probe did not compile.' }
Write-Output $Probe

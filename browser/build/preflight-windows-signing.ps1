<# Prove the protected GitHub identity can sign and timestamp an owned harmless executable without publishing artifacts. #>
$ErrorActionPreference = 'Stop'
$SuccessExitCode = 0
$ConfigJson = & node -e "console.log(JSON.stringify(require(process.argv[1])().win.azureSignOptions))" "$PSScriptRoot/windows-signing.cjs"
if ($LASTEXITCODE -ne $SuccessExitCode) { throw 'Windows signing configuration is incomplete.' }
$Signing = $ConfigJson | ConvertFrom-Json
$ProbeDirectory = Join-Path $env:RUNNER_TEMP "oya-signing-probe-$([guid]::NewGuid())"
New-Item -ItemType Directory -Path $ProbeDirectory | Out-Null
try {
  $Probe = & "$PSScriptRoot/create-windows-signing-probe.ps1" -Directory $ProbeDirectory
  Install-PackageProvider -Name NuGet -MinimumVersion 2.8.5.201 -Force -Scope CurrentUser | Out-Null
  Install-Module -Name TrustedSigning -MinimumVersion 0.5.0 -Force -Repository PSGallery -Scope CurrentUser
  Import-Module TrustedSigning
  Invoke-TrustedSigning -Endpoint $Signing.endpoint -CodeSigningAccountName $Signing.codeSigningAccountName -CertificateProfileName $Signing.certificateProfileName -Files $Probe -FileDigest $Signing.fileDigest -TimestampDigest $Signing.timestampDigest -TimestampRfc3161 $Signing.timestampRfc3161
  & "$PSScriptRoot/verify-windows-signatures.ps1" -Files @($Probe) -ExpectedSubject $env:WINDOWS_SIGNER_SUBJECT
  Write-Host 'Windows signing preflight passed: approved signer and trusted timestamp; no release assets published.'
} finally {
  Remove-Item -LiteralPath $ProbeDirectory -Recurse -Force
}

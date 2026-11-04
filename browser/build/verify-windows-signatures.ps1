<# Verify trusted timestamped release signatures before any Windows artifact is uploaded. #>
param(
  [Parameter(Mandatory = $true)][string[]]$Files,
  [Parameter(Mandatory = $true)][string]$ExpectedSubject
)
$ErrorActionPreference = 'Stop'
foreach ($File in $Files) {
  if (-not (Test-Path -LiteralPath $File -PathType Leaf)) { throw 'A required signed artifact is missing.' }
  $Signature = Get-AuthenticodeSignature -LiteralPath $File
  if ($Signature.Status -ne 'Valid') { throw 'A Windows artifact has an invalid or missing signature.' }
  if ($null -eq $Signature.SignerCertificate -or $Signature.SignerCertificate.Subject -cne $ExpectedSubject) {
    throw 'A Windows artifact was signed by an unexpected identity.'
  }
  if ($null -eq $Signature.TimeStamperCertificate) { throw 'A Windows artifact has no trusted timestamp.' }
  Write-Host "Verified timestamped signature: $([System.IO.Path]::GetFileName($File))"
}

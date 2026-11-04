<# Exercise the release signature gate with hermetic unsigned, wrong-identity and missing-timestamp fixtures. #>
$ErrorActionPreference = 'Stop'
$Fixture = New-TemporaryFile
try {
  # This function shadows the OS reader only in this test process.
  function Get-AuthenticodeSignature { param([string]$LiteralPath) return $script:FixtureSignature }
  $Cases = @(
    @{ Status = 'NotSigned'; Subject = 'CN=Fixture'; Timestamp = $true; Reject = $true },
    @{ Status = 'Valid'; Subject = 'CN=Wrong'; Timestamp = $true; Reject = $true },
    @{ Status = 'Valid'; Subject = 'CN=Fixture'; Timestamp = $false; Reject = $true },
    @{ Status = 'Valid'; Subject = 'CN=Fixture'; Timestamp = $true; Reject = $false }
  )
  foreach ($Case in $Cases) {
    $script:FixtureSignature = [pscustomobject]@{
      Status = $Case.Status
      SignerCertificate = [pscustomobject]@{ Subject = $Case.Subject }
      TimeStamperCertificate = $(if ($Case.Timestamp) { [pscustomobject]@{ Subject = 'CN=Fixture timestamp' } } else { $null })
    }
    $Rejected = $false
    try { & "$PSScriptRoot/verify-windows-signatures.ps1" -Files @($Fixture.FullName) -ExpectedSubject 'CN=Fixture' }
    catch { $Rejected = $true }
    if ($Rejected -ne $Case.Reject) { throw 'Signature validation fixture produced the wrong verdict.' }
  }
  Write-Host 'PASS signature validation: unsigned, wrong identity, missing timestamp and valid fixture.'
} finally { Remove-Item -LiteralPath $Fixture.FullName -Force }
